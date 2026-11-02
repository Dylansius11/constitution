//! Permissionless relay: broadcast the raw transaction the gate recorded, after recovering its
//! sender off-chain. Rebroadcasting is idempotent; a known transaction is not sent twice.
use crate::failure::Failure;
use crate::rpc::{Evm, Near};
use crate::signed_tx::{self, SignedTx};
use constitution_types::{Config, EvmTx, PendingChange, Proposal};
use serde_json::json;
use std::time::{Duration, Instant};

pub enum Target {
    Proposal(u64),
    Change(u64),
}

fn read_failed(cause: String) -> Failure {
    Failure::new(
        cause,
        "check NEAR_RPC_URL, BASE_SEPOLIA_RPC_URL and BASE_SEPOLIA_SEND_URL and retry",
    )
}

fn recorded(near: &Near, gate: &str, target: &Target) -> Result<(String, Option<EvmTx>), Failure> {
    let missing = |what: &str, id: u64| {
        Failure::new(
            format!("gate {gate} has no {what} {id}"),
            "check the id with `constitution status <id>`",
        )
    };
    match *target {
        Target::Proposal(id) => {
            let p: Option<Proposal> = near
                .view(gate, "get_proposal", json!({ "id": id }))
                .map_err(read_failed)?;
            let p = p.ok_or_else(|| missing("proposal", id))?;
            Ok((format!("proposal {id} ({:?})", p.state), p.evm))
        }
        Target::Change(id) => {
            let c: Option<PendingChange> = near
                .view(gate, "get_change", json!({ "id": id }))
                .map_err(read_failed)?;
            let c = c.ok_or_else(|| missing("change", id))?;
            Ok((format!("change {id} ({:?})", c.status), c.evm))
        }
    }
}

pub fn prepare(near: &Near, gate: &str, target: &Target) -> Result<(Vec<u8>, SignedTx), Failure> {
    let config: Config = near
        .view(gate, "get_config", json!({}))
        .map_err(read_failed)?;
    let (label, evm) = recorded(near, gate, target)?;
    let evm = evm.ok_or_else(|| {
        Failure::new(
            format!("{label} has no EVM transaction"),
            "only a certified proposal or an applied MigrateAuthority change is relayable",
        )
    })?;
    let raw = evm.signed_tx.clone().ok_or_else(|| {
        Failure::new(
            format!("{label} is not signed yet (attempt {})", evm.attempts),
            "wait for the MPC callback; after SignFailed anyone can call retry_sign",
        )
    })?;
    let tx = signed_tx::verify(&raw, &config, &evm).map_err(|e| {
        Failure::new(
            format!("refusing to relay {label}: {e}"),
            "do not broadcast; report the gate account and id, the executor would revert NotSigner anyway",
        )
    })?;
    Ok((raw, tx))
}

pub fn relay(
    near: &Near,
    gate: &str,
    target: Target,
    send: &Evm,
    read: &Evm,
    watch: Option<Duration>,
) -> Result<(), Failure> {
    let (raw, tx) = prepare(near, gate, &target)?;
    let chain = read.chain_id().map_err(read_failed)?;
    if chain != tx.chain_id {
        return Err(Failure::new(
            format!(
                "BASE_SEPOLIA_RPC_URL serves chain {chain}, the transaction is for chain {}",
                tx.chain_id
            ),
            "point the RPC URLs at the chain the gate signs for",
        ));
    }
    let hash = format!("0x{}", hex::encode(tx.tx_hash));
    let sender = format!("0x{}", hex::encode(tx.sender));
    if read.knows(&tx.tx_hash).map_err(read_failed)? {
        println!(
            "already broadcast {hash} (sender {sender}, nonce {})",
            tx.nonce
        );
    } else {
        match send.send_raw(&raw) {
            Ok(_) => println!("broadcast {hash} (sender {sender}, nonce {})", tx.nonce),
            // A competing relay may have won the race; only a transaction the chain knows is fine.
            Err(e) if read.knows(&tx.tx_hash).map_err(read_failed)? => {
                println!("already broadcast {hash} ({e})")
            }
            Err(e) => {
                return Err(Failure::new(
                    format!("broadcast of {hash} failed: {e}"),
                    "if the nonce is used by another transaction, read the gate state; otherwise retry or bump_fee",
                ));
            }
        }
    }
    let Some(timeout) = watch else { return Ok(()) };
    let start = Instant::now();
    loop {
        if let Some(receipt) = read.receipt(&tx.tx_hash).map_err(read_failed)? {
            let quantity = |key: &str| {
                receipt[key]
                    .as_str()
                    .and_then(|q| u64::from_str_radix(q.trim_start_matches("0x"), 16).ok())
                    .map_or_else(|| "unknown".to_owned(), |q| q.to_string())
            };
            let (block, gas) = (quantity("blockNumber"), quantity("gasUsed"));
            return match receipt["status"].as_str() {
                Some("0x1") => {
                    println!("included {hash} in block {block}, status success, gas {gas}");
                    Ok(())
                }
                _ => Err(Failure::new(
                    format!("{hash} was included in block {block} but reverted (gas {gas})"),
                    "a DeadlinePassed revert consumed the nonce safely; propose again",
                )),
            };
        }
        if start.elapsed() >= timeout {
            return Err(Failure::new(
                format!("{hash} not included after {}s", timeout.as_secs()),
                "run relay again later; if fees are too low anyone can call bump_fee",
            ));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}
