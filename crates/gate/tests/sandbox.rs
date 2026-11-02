//! Local sandbox only. No testnet or MPC writes.
use constitution_types::{Base64, Config, Constitution, Params, Roles, VerifierRecord};
use near_workspaces::types::NearToken;
use serde_json::{Value, json};
use std::path::PathBuf;

fn artifact(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/near/constitution_gate")
        .join(name)
}

#[test]
fn exported_abi_is_exactly_the_allowed_surface() {
    let abi: Value = serde_json::from_slice(
        &std::fs::read(artifact("constitution_gate_abi.json"))
            .expect("build the gate first with cargo near build non-reproducible-wasm"),
    )
    .unwrap();
    let mut methods: Vec<_> = abi["body"]["functions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    methods.sort_unstable();
    let mut expected = vec![
        "contract_source_metadata",
        "new",
        "deposit_credit",
        "withdraw_credit",
        "propose",
        "expire",
        "schedule_change",
        "apply_change",
        "cancel_change",
        "pause_signing",
        "remove_verifier_now",
        "get_config",
        "get_constitution",
        "get_roles",
        "list_verifiers",
        "get_proposal",
        "list_proposals",
        "get_change",
        "list_changes",
        "get_credit",
        "get_active",
        "submit_verdict",
        "on_signed",
        "retry_sign",
        "bump_fee",
        "signing_preimage",
    ];
    expected.sort_unstable();
    assert_eq!(
        methods, expected,
        "unexpected ABI method could bypass the constitution"
    );
}

#[tokio::test]
async fn sandbox_credit_proposal_and_early_governance_refusal()
-> Result<(), Box<dyn std::error::Error>> {
    let worker = near_workspaces::sandbox().await?;
    let contract = worker
        .dev_deploy(&std::fs::read(artifact("constitution_gate.wasm"))?)
        .await?;
    let governor = worker.dev_create_account().await?;
    let guardian = worker.dev_create_account().await?;
    let proposer = worker.dev_create_account().await?;
    let config = Config {
        chain_id: 84532,
        executor: [1; 20],
        proxy: [2; 20],
        proxy_admin: [3; 20],
        derived_eoa: [4; 20],
        mpc_account: "v1.signer-prod.testnet".parse()?,
        mpc_path: "constitution/base-sepolia/v1".into(),
        mpc_domain_id: 0,
    };
    let constitution = Constitution {
        rule_set_hash: [5; 32],
        manifest_uri: "ipfs://rules".into(),
        ai_required: true,
        params: Params {
            proposal_deposit: 100,
            proposal_ttl_s: 7200,
            max_cert_ttl_s: 1800,
            sign_validity_s: 3600,
            gas_limit: 500000,
            max_fee_per_gas: 10,
            max_priority_fee_per_gas: 1,
            fee_cap: 100,
            sign_gas_tgas: 15,
            callback_gas_tgas: 20,
        },
    };
    contract.call("new").args_json(json!({"config":config,"constitution":constitution,
        "roles":Roles {governor:governor.id().clone(),guardian:guardian.id().clone()},
        "verifiers":[(Base64([7;32]),VerifierRecord {release_hash:[6;32],attestation_uri:"ipfs://quote".into(),added_at_ns:0})],
        "initial_evm_nonce":42})).transact().await?.into_result()?;
    proposer
        .call(contract.id(), "deposit_credit")
        .args_json(json!({"account":null}))
        .deposit(NearToken::from_yoctonear(150))
        .transact()
        .await?
        .into_result()?;
    let result = proposer.call(contract.id(),"propose").args_json(json!({"args":{
        "new_impl":format!("0x{}","08".repeat(20)),"new_impl_codehash":format!("0x{}","09".repeat(32)),
        "expected_current_impl":format!("0x{}","0a".repeat(20)),"init_calldata":"0x","source_commitment":format!("0x{}","0b".repeat(32))
    }})).transact().await?.into_result()?;
    assert_eq!(result.json::<u64>()?, 0);
    let proposal: Value = contract
        .view("get_proposal")
        .args_json(json!({"id":0}))
        .await?
        .json()?;
    assert_eq!(proposal["state"], "Pending");
    assert_eq!(proposal["reserved"], "100");
    let credit: String = contract
        .view("get_credit")
        .args_json(json!({"account":proposer.id()}))
        .await?
        .json()?;
    assert_eq!(credit, "50");
    let active: Option<u64> = contract
        .view("get_active")
        .args_json(json!({"account":proposer.id()}))
        .await?
        .json()?;
    assert_eq!(active, Some(0));
    let id: u64 = governor
        .call(contract.id(), "schedule_change")
        .args_json(json!({"kind":"Unpause"}))
        .transact()
        .await?
        .into_result()?
        .json()?;
    let early = proposer
        .call(contract.id(), "apply_change")
        .args_json(json!({"change_id":id}))
        .transact()
        .await?;
    assert!(early.is_failure());
    assert!(format!("{:?}", early.failures()).contains("E53_TIMELOCK_NOT_ELAPSED"));
    let change: Value = contract
        .view("get_change")
        .args_json(json!({"id":id}))
        .await?
        .json()?;
    assert_eq!(change["status"], "Pending");
    let proposals: Vec<Value> = contract
        .view("list_proposals")
        .args_json(json!({"from_id":0,"limit":50}))
        .await?
        .json()?;
    assert_eq!(proposals.len(), 1);
    let cfg: Config = contract.view("get_config").await?.json()?;
    assert_eq!(cfg, config);
    Ok(())
}
