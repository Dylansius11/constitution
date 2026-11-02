//! T-ENC-1: the gate's EIP-1559 preimages, pinned in `vectors/evm-tx.json` and recomputed
//! independently by viem in `tools/vectors-viem`. Regenerate with
//! `CONSTITUTION_WRITE_VECTORS=1 cargo test -p constitution-gate --lib enc_vectors`.
use super::{proposal_transaction, rotate_transaction};
use constitution_types::{Config, EvmTx, Proposal, ProposalState};
use near_sdk::env;
use serde_json::{Value, json};

const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../vectors/evm-tx.json");

fn config() -> Config {
    Config {
        chain_id: 84532,
        executor: [0x11; 20],
        proxy: [0x22; 20],
        proxy_admin: [0x33; 20],
        derived_eoa: [0x44; 20],
        mpc_account: "v1.signer-prod.testnet".parse().unwrap(),
        mpc_path: "constitution/base-sepolia/v1".into(),
        mpc_domain_id: 0,
    }
}

fn proposal(init_calldata: Vec<u8>) -> Proposal {
    Proposal {
        id: 0,
        proposer: "alice.testnet".parse().unwrap(),
        created_at_ns: 0,
        expires_at_ns: 0,
        rule_set_hash: [0x55; 32],
        new_impl: [0x66; 20],
        new_impl_codehash: [0x77; 32],
        expected_current_impl: [0x88; 20],
        init_calldata,
        source_commitment: [0x99; 32],
        reserved: 0,
        state: ProposalState::SignRequested,
        certificate_hash: Some([0xab; 32]),
        rejection: None,
        evm: None,
    }
}

fn evm(nonce: u64, max_fee: u128, priority: u128) -> EvmTx {
    EvmTx {
        nonce,
        deadline_s: 1_793_700_000,
        gas_limit: 400_000,
        max_fee_per_gas: max_fee,
        max_priority_fee_per_gas: priority,
        signing_hash: [0; 32],
        signed_tx: None,
        attempts: 1,
    }
}

fn bumped(fee: u128) -> u128 {
    fee + fee.div_ceil(4)
}

fn hex0(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn vector(name: &str, p: Option<&Proposal>, e: &EvmTx, rotate: Option<([u8; 20], [u8; 32])>) -> Value {
    let c = config();
    let tx = match (p, rotate) {
        (Some(p), None) => proposal_transaction(&c, p, e).unwrap(),
        (None, Some((signer, change_id))) => rotate_transaction(&c, e, signer, change_id),
        _ => unreachable!(),
    };
    let preimage = tx.build_for_signing();
    let call = match (p, rotate) {
        (Some(p), None) => json!({
            "function": "execute",
            "certId": hex0(&p.certificate_hash.unwrap()),
            "newImpl": hex0(&p.new_impl),
            "newImplCodehash": hex0(&p.new_impl_codehash),
            "expectedCurrentImpl": hex0(&p.expected_current_impl),
            "deadline": e.deadline_s.to_string(),
            "initCalldata": hex0(&p.init_calldata),
        }),
        (None, Some((signer, change_id))) => json!({
            "function": "rotateSigner",
            "newSigner": hex0(&signer),
            "changeId": hex0(&change_id),
            "deadline": e.deadline_s.to_string(),
        }),
        _ => unreachable!(),
    };
    json!({
        "name": name,
        "chain_id": c.chain_id.to_string(),
        "to": hex0(&c.executor),
        "nonce": e.nonce.to_string(),
        "gas_limit": e.gas_limit.to_string(),
        "max_fee_per_gas": e.max_fee_per_gas.to_string(),
        "max_priority_fee_per_gas": e.max_priority_fee_per_gas.to_string(),
        "call": call,
        "preimage": hex0(&preimage),
        "signing_hash": hex0(&env::keccak256_array(&preimage)),
    })
}

fn vectors() -> Value {
    let empty = proposal(vec![]);
    // initializeV2(1000e18): selector 0x5b4e128c, the demo's real initializer.
    let mut init = hex::decode("5b4e128c").unwrap();
    let mut amount = [0u8; 32];
    amount[16..].copy_from_slice(&1_000_000_000_000_000_000_000u128.to_be_bytes());
    init.extend_from_slice(&amount);
    let max_init = proposal((0..1024u32).map(|i| (i * 31 % 251) as u8).collect());
    let base = evm(0, 1_009_731_712, 1_000_000);
    let bump = evm(0, bumped(base.max_fee_per_gas), bumped(base.max_priority_fee_per_gas));
    json!({
        "label": "T-ENC-1 vectors produced by the gate (omni-transaction 0.5.0 + alloy-sol-types 1.7.3); recomputed by tools/vectors-viem",
        "vectors": [
            vector("execute_empty_init_nonce_0", Some(&empty), &base, None),
            vector("execute_initializeV2_init", Some(&proposal(init)), &evm(1, 1_009_731_712, 1_000_000), None),
            vector("execute_max_length_init_1024", Some(&max_init), &evm(2, 1_009_731_712, 1_000_000), None),
            vector("execute_fee_bumped_once", Some(&empty), &bump, None),
            // viem types nonce as a JS number, so the largest nonce both sides can express is 2^53 - 1.
            vector("execute_large_nonce_and_fees", Some(&empty), &evm(9_007_199_254_740_991, u128::MAX / 3, u128::MAX / 7), None),
            vector("rotate_signer", None, &evm(5, 1_009_731_712, 1_000_000), Some(([0xcd; 20], [0xef; 32]))),
        ],
    })
}

#[test]
fn t_enc_1_gate_preimages_match_committed_vectors() {
    let produced = vectors();
    if std::env::var("CONSTITUTION_WRITE_VECTORS").as_deref() == Ok("1") {
        let text = serde_json::to_string_pretty(&produced).unwrap() + "\n";
        std::fs::write(FILE, text).unwrap();
    }
    let committed: Value = serde_json::from_str(&std::fs::read_to_string(FILE).unwrap()).unwrap();
    assert_eq!(produced, committed, "gate encoding drifted from vectors/evm-tx.json");
}
