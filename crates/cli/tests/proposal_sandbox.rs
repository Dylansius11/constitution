//! Local integration: real gate wasm + Base-chain-id anvil + real binary; HTTP verifier is a stub.
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use base64::Engine;
use constitution_types::{
    Base64, Config, Constitution, Params, Proposal, Roles, U128Str, VerifierRecord,
    bundle::SourceBundle,
};
use crypto_box::{SecretKey, aead::rand_core::OsRng};
use serde_json::{Value, json};
use std::{
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

struct Anvil {
    child: Child,
    url: String,
}
impl Drop for Anvil {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn contracts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts")
}
fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
fn forge(args: &[&str]) -> Output {
    let output = Command::new("forge")
        .current_dir(contracts())
        .env("NO_COLOR", "1")
        .args(args)
        .output()
        .expect("forge must be on PATH");
    assert!(output.status.success(), "{}", text(&output));
    output
}
fn deploy() -> (Anvil, Config, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let child = Command::new("anvil")
        .args([
            "--chain-id",
            "84532",
            "--port",
            &port.to_string(),
            "--silent",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("anvil must be on PATH");
    let anvil = Anvil {
        child,
        url: format!("http://127.0.0.1:{port}"),
    };
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "anvil did not start"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // Anvil's unlocked local account; no private key is read or printed.
    let sender = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
    let output = Command::new("forge")
        .current_dir(contracts())
        .env("NO_COLOR", "1")
        .env("SIGNER", sender)
        .args([
            "script",
            "script/Deploy.s.sol",
            "--rpc-url",
            &anvil.url,
            "--broadcast",
            "--unlocked",
            "--sender",
            sender,
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
    let logs = text(&output);
    let address = |name: &str| -> [u8; 20] {
        let line = logs
            .lines()
            .find(|line| line.trim_start().starts_with(&format!("{name} ")))
            .unwrap_or_else(|| panic!("missing {name}: {logs}"));
        hex::decode(
            line.split_whitespace()
                .nth(1)
                .unwrap()
                .trim_start_matches("0x"),
        )
        .unwrap()
        .try_into()
        .unwrap()
    };
    let config = Config {
        chain_id: 84532,
        executor: address("executor"),
        proxy: address("proxy"),
        proxy_admin: address("proxyAdmin"),
        derived_eoa: hex::decode(sender.trim_start_matches("0x"))
            .unwrap()
            .try_into()
            .unwrap(),
        mpc_account: "v1.signer-prod.testnet".parse().unwrap(),
        mpc_path: "constitution/base-sepolia/v1".into(),
        mpc_domain_id: 0,
    };
    let created = forge(&[
        "create",
        "src/demo/VaultV2Honest.sol:VaultV2Honest",
        "--rpc-url",
        &anvil.url,
        "--broadcast",
        "--unlocked",
        "--from",
        sender,
        "--json",
    ]);
    let result: Value = serde_json::from_slice(&created.stdout).unwrap();
    let implementation = result["deployedTo"].as_str().unwrap().to_ascii_lowercase();
    (anvil, config, implementation)
}

#[derive(Clone)]
struct Stub {
    key: Arc<SecretKey>,
    bundle: Arc<Mutex<Option<Vec<u8>>>>,
    gate: String,
    recovery: Arc<Mutex<Option<PathBuf>>>,
    refuse_start: Arc<AtomicBool>,
    identity_calls: Arc<std::sync::atomic::AtomicUsize>,
}
async fn identity(State(stub): State<Stub>) -> Json<Value> {
    stub.identity_calls.fetch_add(1, Ordering::SeqCst);
    Json(
        json!({"verifier_pubkey": format!("0x{}", hex::encode([7;32])), "transport_pubkey": format!("0x{}", hex::encode(stub.key.public_key().as_bytes())), "release_hash":format!("0x{}", hex::encode([6;32])), "report_data_preimage":"", "quote_hex":"", "event_log":[], "compose_hash":format!("0x{}", hex::encode([0;32])), "supported_rule_set_hashes":[]}),
    )
}
async fn submit(State(stub): State<Stub>, Json(request): Json<Value>) -> axum::response::Response {
    use axum::response::IntoResponse;
    assert_eq!(request["gate"], stub.gate);
    if let Some(path) = stub.recovery.lock().unwrap().as_ref() {
        let saved: Value =
            serde_json::from_slice(&std::fs::read(path.join("submission.json")).unwrap()).unwrap();
        assert_eq!(saved["proposal_id"], request["proposal_id"]);
        assert!(saved["near_propose_tx"].is_string());
    }
    if stub.refuse_start.load(Ordering::SeqCst) {
        return (axum::http::StatusCode::BAD_GATEWAY, "rpc_unavailable").into_response();
    }
    let sealed = base64::engine::general_purpose::STANDARD
        .decode(request["sealed_bundle_b64"].as_str().unwrap())
        .unwrap();
    *stub.bundle.lock().unwrap() = Some(stub.key.unseal(&sealed).unwrap());
    (axum::http::StatusCode::ACCEPTED, Json(json!({"run_id":7}))).into_response()
}
async fn run_status(State(stub): State<Stub>) -> Json<Value> {
    let stage = if stub.recovery.lock().unwrap().is_some() {
        "done"
    } else {
        "queued"
    };
    Json(json!({"stage":stage, "started_at":1, "stages":[]}))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn propose_seals_the_onchain_commitment_and_reads_pending_receipt()
-> Result<(), Box<dyn std::error::Error>> {
    let (anvil, config, implementation) = deploy();
    let worker = near_workspaces::sandbox().await?;
    let wasm = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/near/constitution_gate/constitution_gate.wasm");
    let gate = worker.dev_deploy(&std::fs::read(wasm)?).await?;
    let account = worker.dev_create_account().await?;
    let params = Params {
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
    };
    gate.call("new").args_json(json!({"config":config, "constitution":Constitution {rule_set_hash:[5;32], manifest_uri:"local".into(), ai_required:true, params}, "roles":Roles {governor:account.id().clone(), guardian:account.id().clone()}, "verifiers":[(Base64([7;32]), VerifierRecord {release_hash:[6;32], attestation_uri:"local".into(), added_at_ns:0})], "initial_evm_nonce":0})).transact().await?.into_result()?;
    // Existing credit covers 40 yocto of the reservation. The CLI must prepay the remaining
    // reservation plus the non-refundable proposal fee before its zero-deposit proposal.
    let entry_fee: U128Str = gate
        .view("get_credit_entry_fee")
        .args_json(json!({"account":account.id()}))
        .await?
        .json()?;
    account
        .call(gate.id(), "deposit_credit")
        .args_json(json!({"account":account.id()}))
        .deposit(near_workspaces::types::NearToken::from_yoctonear(
            40 + entry_fee.0,
        ))
        .transact()
        .await?
        .into_result()?;
    let temp = tempfile::tempdir()?;
    let key_path = temp.path().join("key.json");
    let key = account.secret_key();
    std::fs::write(&key_path, json!({"account_id":account.id(), "public_key":key.public_key().to_string(), "private_key":key.to_string()}).to_string())?;
    std::fs::create_dir(temp.path().join("src"))?;
    let source = std::fs::read(contracts().join("src/demo/VaultV2Honest.sol"))?;
    std::fs::write(temp.path().join("src/VaultV2Honest.sol"), &source)?;
    let stub = Stub {
        key: Arc::new(SecretKey::generate(&mut OsRng)),
        bundle: Arc::new(Mutex::new(None)),
        gate: gate.id().to_string(),
        recovery: Arc::new(Mutex::new(None)),
        refuse_start: Arc::new(AtomicBool::new(false)),
        identity_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let verifier = format!("http://{}", listener.local_addr()?);
    let router = Router::new()
        .route("/v1/identity", get(identity))
        .route("/v1/runs", post(submit))
        .route("/v1/runs/7", get(run_status))
        .with_state(stub.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let common = |command: &mut Command| {
        command.args([
            "--gate",
            gate.id().as_str(),
            "--near-rpc",
            &worker.rpc_addr(),
            "--evm-rpc",
            &anvil.url,
            "--network",
            "local",
        ]);
    };
    let propose = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_constitution"));
        command
            .args([
                "propose",
                "--contract",
                "src/VaultV2Honest.sol:VaultV2Honest",
                "--impl",
                &implementation,
                "--account",
                account.id().as_str(),
                "--verifier",
                &verifier,
            ])
            .arg("--bundle")
            .arg(temp.path())
            .arg("--key-file")
            .arg(&key_path);
        common(&mut command);
        command.output().unwrap()
    };
    let output = propose();
    assert!(output.status.success(), "{}", text(&output));
    let bytes = stub
        .bundle
        .lock()
        .unwrap()
        .clone()
        .expect("bundle delivered");
    let bundle: SourceBundle = borsh::from_slice(&bytes)?;
    assert_eq!(
        bundle,
        SourceBundle {
            contract_name: "src/VaultV2Honest.sol:VaultV2Honest".into(),
            files: vec![("src/VaultV2Honest.sol".into(), source)],
            foundry_profile: "default".into()
        }
    );
    let proposal: Proposal = gate
        .view("get_proposal")
        .args_json(json!({"id":0}))
        .await?
        .json()?;
    assert_eq!(
        proposal.source_commitment,
        constitution_types::certificate_hash(&bytes)
    );
    let credit: U128Str = gate
        .view("get_credit")
        .args_json(json!({"account":account.id()}))
        .await?
        .json()?;
    assert_eq!(credit.0, 0);
    let mut status = Command::new(env!("CARGO_BIN_EXE_constitution"));
    status.args(["status", "0", "--run", "7", "--verifier", &verifier]);
    common(&mut status);
    let status = status.output()?;
    assert!(status.status.success(), "{}", text(&status));
    let mut watch = Command::new(env!("CARGO_BIN_EXE_constitution"));
    watch.args(["status", "0", "--watch", "0"]);
    common(&mut watch);
    let watch = watch.output()?;
    assert_eq!(watch.status.code(), Some(1), "{}", text(&watch));
    assert!(text(&watch).contains("within 0s"), "{}", text(&watch));
    assert!(text(&watch).contains("next: "), "{}", text(&watch));
    let mut receipt = Command::new(env!("CARGO_BIN_EXE_constitution"));
    receipt.args([
        "receipt",
        "0",
        "--json",
        "--run",
        "7",
        "--verifier",
        &verifier,
    ]);
    common(&mut receipt);
    let receipt = receipt.output()?;
    assert!(receipt.status.success(), "{}", text(&receipt));
    let receipt: Value = serde_json::from_slice(&receipt.stdout)?;
    assert_eq!(receipt["schema"], "constitution.receipt/1");
    assert_eq!(receipt["proposal"]["id"], 0);
    assert_eq!(receipt["labels"]["network"], "local");
    assert_eq!(receipt["evm"]["status"], "not_signed");
    assert!(receipt["proxy"]["observed_at_block"].is_u64());
    assert_eq!(
        receipt["proxy"]["implementation_after"],
        format!("0x{}", hex::encode(proposal.expected_current_impl))
    );
    for key in ["verifier", "verdict", "near", "ai", "evidence_root"] {
        assert!(receipt.get(key).is_none(), "invented {key}");
    }
    let second = propose();
    assert_eq!(second.status.code(), Some(1), "{}", text(&second));
    assert!(
        text(&second).contains("E21_ACTIVE_PROPOSAL_EXISTS"),
        "{}",
        text(&second)
    );
    assert!(text(&second).contains("next: "), "{}", text(&second));
    // The real CLI must retain the accepted id before a failing verifier POST, and must not
    // turn a withheld completion into a relay. Only the verifier HTTP boundary is a double.
    for (name, refuse) in [
        ("post-failed", true),
        ("withheld", false),
        ("restricted-key", true),
    ] {
        let demo_account = worker.dev_create_account().await?;
        let mut demo_key = demo_account.secret_key().clone();
        if name == "restricted-key" {
            use near_workspaces::types::{AccessKey, KeyType, NearToken, SecretKey};
            demo_key = SecretKey::from_random(KeyType::ED25519);
            let entry: U128Str = gate
                .view("get_credit_entry_fee")
                .args_json(json!({"account":demo_account.id()}))
                .await?
                .json()?;
            let fee: U128Str = gate.view("get_proposal_fee").await?.json()?;
            demo_account
                .call(gate.id(), "deposit_credit")
                .args_json(json!({"account":null}))
                .deposit(NearToken::from_yoctonear(entry.0 + fee.0 + 100))
                .transact()
                .await?
                .into_result()?;
            demo_account
                .batch(demo_account.id())
                .add_key(
                    demo_key.public_key(),
                    AccessKey::function_call_access(
                        gate.id(),
                        &["propose", "schedule_change", "apply_change"],
                        Some(NearToken::from_near(1)),
                    ),
                )
                .transact()
                .await?
                .into_result()?;
            let limited = near_workspaces::Account::from_secret_key(
                demo_account.id().clone(),
                demo_key.clone(),
                &worker,
            );
            let forbidden = limited
                .call(gate.id(), "withdraw_credit")
                .args_json(json!({"amount":"1"}))
                .transact()
                .await;
            assert!(
                forbidden.is_err(),
                "method-limited key cannot withdraw prepaid credit"
            );
            let after: U128Str = gate
                .view("get_credit")
                .args_json(json!({"account":demo_account.id()}))
                .await?
                .json()?;
            assert_eq!(after.0, fee.0 + 100);
            let before = account.view_account().await?.balance;
            let transfer = limited
                .transfer_near(account.id(), NearToken::from_yoctonear(1))
                .await;
            assert!(transfer.is_err(), "function-call key cannot transfer funds");
            assert_eq!(account.view_account().await?.balance, before);
        }
        let demo_key_path = temp.path().join(format!("{name}-key.json"));
        std::fs::write(
            &demo_key_path,
            json!({
                "account_id":demo_account.id(), "public_key":demo_key.public_key().to_string(),
                "private_key":demo_key.to_string()
            })
            .to_string(),
        )?;
        let evidence = temp.path().join(name);
        *stub.recovery.lock().unwrap() = Some(evidence.clone());
        stub.refuse_start.store(refuse, Ordering::SeqCst);
        let mut demo = Command::new(env!("CARGO_BIN_EXE_constitution"));
        demo.args([
            "demo",
            "upgrade",
            "--contract",
            "src/VaultV2Honest.sol:VaultV2Honest",
            "--impl",
            &implementation,
            "--account",
            demo_account.id().as_str(),
            "--verifier",
            &verifier,
            "--watch",
            "0",
        ])
        .arg("--bundle")
        .arg(temp.path())
        .arg("--key-file")
        .arg(&demo_key_path)
        .arg("--output")
        .arg(&evidence);
        common(&mut demo);
        let output = demo.output()?;
        println!(
            "LOCAL DEMO {name} (HTTP verifier double):\n{}",
            text(&output)
        );
        assert!(!output.status.success(), "{}", text(&output));
        let saved: Value =
            serde_json::from_slice(&std::fs::read(evidence.join("submission.json"))?)?;
        let id = saved["proposal_id"].as_u64().unwrap();
        let p: Proposal = gate
            .view("get_proposal")
            .args_json(json!({"id":id}))
            .await?
            .json()?;
        assert_eq!(p.state, constitution_types::ProposalState::Pending);
        assert!(p.evm.is_none());
        assert_eq!(
            p.source_commitment,
            constitution_types::certificate_hash(&bytes)
        );
        let observed: Value =
            serde_json::from_slice(&std::fs::read(evidence.join("receipt.json"))?)?;
        assert_eq!(observed["proposal"]["id"], id);
        assert_eq!(observed["labels"]["network"], "local");
        assert_eq!(observed["evm"]["status"], "not_signed");
        assert_eq!(observed["near"]["propose_tx"], saved["near_propose_tx"]);
        assert_eq!(
            observed["proxy"]["implementation_after"],
            format!("0x{}", hex::encode(p.expected_current_impl))
        );
        assert!(observed.get("verdict").is_none());
        let recorded_run = evidence.join("run.json");
        assert_eq!(recorded_run.exists(), !refuse);
        assert!(evidence.join("failure.json").exists());
        let absent: Option<Proposal> = gate
            .view("get_proposal")
            .args_json(json!({"id":id+1}))
            .await?
            .json()?;
        assert!(
            absent.is_none(),
            "runner automatically created another proposal"
        );
        // Valid bundle/key/endpoints: output reuse must stop before even verifier preflight,
        // and before a repeated credit deposit (which would otherwise precede E21).
        let identity_calls = stub.identity_calls.load(Ordering::SeqCst);
        let credit_before: U128Str = gate
            .view("get_credit")
            .args_json(json!({"account":demo_account.id()}))
            .await?
            .json()?;
        let old_receipt = std::fs::read(evidence.join("receipt.json"))?;
        let duplicate = demo.output()?;
        assert!(!duplicate.status.success());
        assert_eq!(stub.identity_calls.load(Ordering::SeqCst), identity_calls);
        let credit_after: U128Str = gate
            .view("get_credit")
            .args_json(json!({"account":demo_account.id()}))
            .await?
            .json()?;
        assert_eq!(credit_after.0, credit_before.0);
        assert_eq!(std::fs::read(evidence.join("receipt.json"))?, old_receipt);
    }
    server.abort();
    Ok(())
}
