//! The `constitution` binary against a local near-sandbox running the real gate wasm.
//! Local only; no testnet writes. Build the gate first: `cd crates/gate && cargo near build non-reproducible-wasm`.
use constitution_types::{Base64, Config, Constitution, Params, Roles, VerifierRecord};
use near_workspaces::{Account, Contract, Worker, network::Sandbox};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Output;

fn gate_wasm() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/near/constitution_gate/constitution_gate.wasm")
}

struct Local {
    worker: Worker<Sandbox>,
    gate: Contract,
    governor: Account,
    guardian: Account,
    keys: tempfile::TempDir,
}

impl Local {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let worker = near_workspaces::sandbox().await?;
        let gate = worker
            .dev_deploy(&std::fs::read(gate_wasm()).expect("build the gate wasm first"))
            .await?;
        let governor = worker.dev_create_account().await?;
        let guardian = worker.dev_create_account().await?;
        let config = Config {
            chain_id: 84532,
            executor: [1; 20],
            proxy: [2; 20],
            proxy_admin: [3; 20],
            derived_eoa: [4; 20],
            mpc_account: "v1.signer-prod.testnet".parse()?,
            mpc_path: "constitution/base-sepolia/v1".into(),
            mpc_domain_id: 0,
            governance_delay_days: constitution_types::DEFAULT_GOVERNANCE_DELAY_DAYS,
        };
        let constitution = Constitution {
            rule_set_hash: [5; 32],
            manifest_uri: "rules/manifest.json".into(),
            ai_required: true,
            params: Params {
                proposal_deposit: 100,
                proposal_ttl_s: 7200,
                max_cert_ttl_s: 1800,
                sign_validity_s: 3600,
                gas_limit: 500_000,
                max_fee_per_gas: 10,
                max_priority_fee_per_gas: 1,
                fee_cap: 100,
                sign_gas_tgas: 15,
                callback_gas_tgas: 20,
            },
        };
        gate.call("new")
            .args_json(json!({
                "config": config, "constitution": constitution,
                "roles": Roles { governor: governor.id().clone(), guardian: guardian.id().clone() },
                "verifiers": [(Base64([7; 32]), VerifierRecord { release_hash: [6; 32], attestation_uri: "local".into(), added_at_ns: 0 })],
                "initial_evm_nonce": 0,
            }))
            .transact()
            .await?
            .into_result()?;
        Ok(Self {
            worker,
            gate,
            governor,
            guardian,
            keys: tempfile::tempdir()?,
        })
    }

    fn key_file(&self, account: &Account) -> PathBuf {
        let path = self.keys.path().join(format!("{}.json", account.id()));
        let key = account.secret_key();
        std::fs::write(
            &path,
            json!({ "account_id": account.id(), "public_key": key.public_key().to_string(), "private_key": key.to_string() })
                .to_string(),
        )
        .unwrap();
        path
    }

    fn run(&self, signer: &Account, args: &[&str]) -> Output {
        std::process::Command::new(env!("CARGO_BIN_EXE_constitution"))
            .args(args)
            .args([
                "--account",
                signer.id().as_str(),
                "--gate",
                self.gate.id().as_str(),
                "--near-rpc",
                &self.worker.rpc_addr(),
            ])
            .arg("--key-file")
            .arg(self.key_file(signer))
            .output()
            .unwrap()
    }
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[tokio::test]
async fn governance_waits_the_timelock_and_only_the_guardian_cancels()
-> Result<(), Box<dyn std::error::Error>> {
    let local = Local::new().await?;
    let scheduled = local.run(
        &local.governor,
        &["govern", "schedule", "--kind", "\"Unpause\""],
    );
    assert!(scheduled.status.success(), "{}", text(&scheduled));
    assert!(
        text(&scheduled).contains("scheduled change 0"),
        "{}",
        text(&scheduled)
    );

    // T-E2E-3 locally: the governor applies immediately and the gate refuses.
    let early = local.run(&local.governor, &["govern", "apply", "0"]);
    assert_eq!(early.status.code(), Some(1));
    let early = text(&early);
    assert!(
        early.contains("E53_TIMELOCK_NOT_ELAPSED") && early.contains("next: "),
        "{early}"
    );

    let governor_cancel = local.run(&local.governor, &["govern", "cancel", "0"]);
    assert_eq!(governor_cancel.status.code(), Some(1));
    assert!(
        text(&governor_cancel).contains("E56_NOT_GUARDIAN"),
        "{}",
        text(&governor_cancel)
    );

    let cancelled = local.run(&local.guardian, &["govern", "cancel", "0"]);
    assert!(cancelled.status.success(), "{}", text(&cancelled));
    // A cancelled change's body is deleted (storage metering), so the gate no longer returns it.
    let change: serde_json::Value = local
        .gate
        .view("get_change")
        .args_json(json!({ "id": 0 }))
        .await?
        .json()?;
    assert!(change.is_null(), "{change}");

    let bad_kind = local.run(
        &local.governor,
        &["govern", "schedule", "--kind", "\"Upgrade\""],
    );
    assert_eq!(bad_kind.status.code(), Some(1));
    assert!(
        text(&bad_kind).contains("--kind is not a ChangeKind"),
        "{}",
        text(&bad_kind)
    );
    Ok(())
}
