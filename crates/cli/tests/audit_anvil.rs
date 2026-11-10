//! Local real hardened gate wasm + near-sandbox + real Deploy.s.sol/anvil, not testnet evidence.
//! Only MPC `derived_public_key` is an offline double seeded by vectors/mpc-derivation.json.
//! Build first: `(cd crates/gate && cargo near build non-reproducible-wasm)`.
#[path = "fixtures/deployment_schema.rs"]
mod deployment_schema;
use axum::{Json, Router, extract::State, routing::post};
use base64::Engine;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

struct Anvil(Child);
impl Drop for Anvil {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}
fn forge(args: &[&str]) -> Output {
    let o = Command::new("forge")
        .current_dir(root().join("contracts"))
        .args(args)
        .arg("--offline")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", text(&o));
    o
}
fn rpc(url: &str, method: &str, params: Value) -> Value {
    tokio::task::block_in_place(|| {
        let v: Value = reqwest::blocking::Client::new()
            .post(url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .unwrap()
            .json()
            .unwrap();
        assert!(v.get("error").is_none(), "{v}");
        v["result"].clone()
    })
}
const DERIVED_KEY: &str = "secp256k1:2LxkyX7Nuw1MbtVBAHWubggYkZ91pGE2yxuEo7bNuVakGYsBrmnHvAuDQtiVExSFF2RPLeepXsYszq8ynFToa8KZ";

struct OfflineMpc {
    sandbox_rpc: String,
    gates: [String; 2],
    mpc: String,
    client: reqwest::Client,
}

async fn near(State(s): State<Arc<OfflineMpc>>, Json(q): Json<Value>) -> Json<Value> {
    let p = &q["params"];
    if p["account_id"] == s.mpc {
        assert_eq!(q["method"], "query");
        assert_eq!(p["request_type"], "call_function");
        assert_eq!(p["method_name"], "derived_public_key");
        let args: Value = serde_json::from_slice(
            &base64::engine::general_purpose::STANDARD
                .decode(p["args_base64"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        let predecessor = args["predecessor"].as_str().unwrap();
        assert!(s.gates.iter().any(|gate| gate == predecessor));
        assert_eq!(
            args,
            json!({"predecessor":predecessor,"path":"constitution/base-sepolia/v1","domain_id":0})
        );
        return Json(json!({"jsonrpc":"2.0","id":q["id"],
            "result":{"result":serde_json::to_vec(DERIVED_KEY).unwrap()}}));
    }
    // No gate/account view is synthesized, including pause, access keys and code hash.
    assert!(s.gates.iter().any(|gate| p["account_id"] == *gate));
    Json(
        s.client
            .post(&s.sandbox_rpc)
            .json(&q)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap(),
    )
}

const PRECONDITIONS: [&str; 12] = [
    "gate_has_no_access_keys",
    "gate_operating_surplus",
    "gate_code_is_committed_build",
    "executor_code_is_committed_build",
    "gate_binds_this_deployment",
    "derived_eoa_is_executor_signer",
    "executor_controls_this_proxy",
    "executor_owns_proxy_admin",
    "proxy_admin_is_erc1967_admin",
    "implementation_tracker_matches_slot",
    "active_rule_set_is_committed_manifest",
    "verifier_allowlist_is_committed",
];

fn assert_checks(out: &Output, failed: &[&str], warnings: &[&str]) {
    let output = text(out);
    assert_eq!(
        out.status.code(),
        Some(i32::from(!failed.is_empty())),
        "{output}"
    );
    for name in PRECONDITIONS {
        let status = if failed.contains(&name) {
            "FAIL"
        } else {
            "PASS"
        };
        assert!(output.contains(&format!("{status} {name}:")), "{output}");
    }
    let names = |status: &str| {
        output
            .lines()
            .filter_map(|line| {
                line.strip_prefix(status)?
                    .split_once(':')
                    .map(|(name, _)| name)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(names("FAIL "), failed, "{output}");
    assert_eq!(names("WARN "), warnings, "{output}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_gate_audit_ceremony_and_runtime_tampering() -> Result<(), Box<dyn std::error::Error>>
{
    let total = Instant::now();
    let worker = near_workspaces::sandbox().await?;
    let wasm = std::fs::read(root().join("target/near/constitution_gate/constitution_gate.wasm"))?;
    let gate = worker.dev_deploy(&wasm).await?;
    // Same hardened gate, but an extra valid custom section gives a genuinely different code hash.
    let mut different_wasm = wasm.clone();
    different_wasm.extend_from_slice(&[0, 2, 1, b'x']);
    let different_gate = worker.dev_deploy(&different_wasm).await?;
    let governor = worker.dev_create_account().await?;
    let guardian = worker.dev_create_account().await?;
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let _anvil = Anvil(
        Command::new("anvil")
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
            .unwrap(),
    );
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(30));
    }
    let evm = format!("http://127.0.0.1:{port}");
    let sender = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
    let deployed = Command::new("forge")
        .current_dir(root().join("contracts"))
        .env("SIGNER", "0x3b20f46860d27d81917a45221c594fc051af7e94")
        .args([
            "script",
            "script/Deploy.s.sol",
            "--rpc-url",
            &evm,
            "--broadcast",
            "--unlocked",
            "--sender",
            sender,
        ])
        .output()
        .unwrap();
    assert!(deployed.status.success(), "{}", text(&deployed));
    let logs = text(&deployed);
    let address = |name: &str| {
        logs.lines()
            .find(|l| l.trim_start().starts_with(&format!("{name} ")))
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .to_ascii_lowercase()
    };
    let executor = address("executor");
    let proxy = address("proxy");
    let admin = address("proxyAdmin");
    let manifest: constitution_types::RuleSetManifest =
        serde_json::from_slice(&std::fs::read(root().join("rules/manifest.json")).unwrap())
            .unwrap();
    let hash = constitution_types::manifest::rule_set_hash(&manifest).unwrap();
    // Gate pins this account; the loopback relay intercepts its sole derivation view.
    let mpc = "v1.signer-prod.testnet";
    let config = json!({"chain_id":84532,"executor":executor,"proxy":proxy,"proxy_admin":admin,"derived_eoa":"0x3b20f46860d27d81917a45221c594fc051af7e94","mpc_account":mpc,"mpc_path":"constitution/base-sepolia/v1","mpc_domain_id":0,"governance_delay_days":3});
    let constitution = json!({"rule_set_hash":format!("0x{}",hex::encode(hash)),"manifest_uri":"local:rules/manifest.json","ai_required":true,"params":{"proposal_deposit":"100","proposal_ttl_s":7200,"max_cert_ttl_s":1800,"sign_validity_s":3600,"gas_limit":300000,"max_fee_per_gas":"50000000","max_priority_fee_per_gas":"1000000","fee_cap":"1000000000","sign_gas_tgas":15,"callback_gas_tgas":20}});
    let pubkey = base64::engine::general_purpose::STANDARD.encode([7; 32]);
    let release_hash = format!("0x{}", "06".repeat(32));
    for contract in [&gate, &different_gate] {
        contract.call("new").args_json(json!({
            "config":config,"constitution":constitution,
            "roles":{"governor":governor.id(),"guardian":guardian.id()},
            "verifiers":[[pubkey,{"release_hash":release_hash,"attestation_uri":"local:offline-fixture-not-attested","added_at_ns":0}]],
            "initial_evm_nonce":0
        })).transact().await?.into_result()?;
    }
    different_gate
        .batch()
        .delete_key(different_gate.as_account().secret_key().public_key())
        .transact()
        .await?
        .into_result()?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let near_url = format!("http://{}", listener.local_addr()?);
    let state = Arc::new(OfflineMpc {
        sandbox_rpc: worker.rpc_addr(),
        gates: [gate.id().to_string(), different_gate.id().to_string()],
        mpc: mpc.into(),
        client: reqwest::Client::new(),
    });
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/", post(near)).with_state(state),
        )
        .await
        .unwrap();
    });
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("deployment.json");
    // Every audit must read the exact manifest used at initialization, even during sibling edits.
    let manifest_path = temp.path().join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
    let expected = Command::new(env!("CARGO_BIN_EXE_constitution"))
        .current_dir(root())
        .arg("executor-runtime-hash")
        .output()
        .unwrap();
    assert!(expected.status.success(), "{}", text(&expected));
    let expected_hash = String::from_utf8(expected.stdout).unwrap();
    let mut deployment = json!({"chain_id":84532,"governance_delay_days":3,"gate":gate.id(),"gate_code_hash":bs58::encode(Sha256::digest(&wasm)).into_string(),"mpc_account":mpc,"executor":executor,"proxy":proxy,"executor_runtime_hash":expected_hash.trim(),"verifiers":[{"pubkey":pubkey,"release_hash":release_hash}],"roles":{"governor":governor.id(),"guardian":guardian.id()}});
    deployment_schema::validate(&deployment);
    let audit = |label: &str, d: &Value| {
        std::fs::write(&path, d.to_string()).unwrap();
        let start = Instant::now();
        let out = Command::new(env!("CARGO_BIN_EXE_constitution"))
            .current_dir(root())
            .args([
                "audit",
                "--deployment",
                path.to_str().unwrap(),
                "--manifest",
                manifest_path.to_str().unwrap(),
                "--near-rpc",
                &near_url,
                "--evm-rpc",
                &evm,
            ])
            .output()
            .unwrap();
        eprintln!(
            "{label}: {:.2}s\n{}",
            start.elapsed().as_secs_f64(),
            text(&out)
        );
        out
    };
    let keyed = audit("before key deletion", &deployment);
    assert_checks(&keyed, &["gate_has_no_access_keys"], &[]);

    let mut different_deployment = deployment.clone();
    different_deployment["gate"] = json!(different_gate.id());
    let changed = audit(
        "different deployed keyless gate wasm",
        &different_deployment,
    );
    assert_checks(&changed, &["gate_code_is_committed_build"], &[]);
    gate.batch()
        .delete_key(gate.as_account().secret_key().public_key())
        .transact()
        .await?
        .into_result()?;
    assert!(gate.view_access_keys().await?.is_empty());
    let honest = audit("honest keyless deployment", &deployment);
    assert_checks(&honest, &[], &[]);
    assert!(text(&honest).contains("PASS pending_governance_changes: 0 pending change(s)"));
    assert!(!text(&honest).contains("pending_authority_change"));
    assert!(text(&honest).contains("audit passed: 15 named checks/information lines"));
    deployment
        .as_object_mut()
        .unwrap()
        .remove("executor_runtime_hash");
    let missing = audit("missing executor commitment", &deployment);
    assert_eq!(missing.status.code(), Some(1));
    assert!(text(&missing).contains("FAIL executor_code_is_committed_build"));
    deployment["executor_runtime_hash"] = json!(expected_hash.trim());
    // Test-only Solidity fixture: preserve all honest getters through inheritance, add one function.
    let fixture = temp.path().join("ExtraExecutor.sol");
    std::fs::write(&fixture,"pragma solidity 0.8.38; import {ConstitutionExecutor} from \"src/ConstitutionExecutor.sol\"; contract ExtraExecutor is ConstitutionExecutor { constructor(address s,address i) ConstitutionExecutor(s,i,\"\") {} function testFixtureExtra() external pure returns(uint) { return 7; } }").unwrap();
    let build = forge(&[
        "build",
        fixture.to_str().unwrap(),
        "--out",
        temp.path().join("out").to_str().unwrap(),
        "--cache-path",
        temp.path().join("cache").to_str().unwrap(),
    ]);
    assert!(build.status.success());
    let artifact: Value = serde_json::from_slice(
        &std::fs::read(temp.path().join("out/ExtraExecutor.sol/ExtraExecutor.json")).unwrap(),
    )
    .unwrap();
    let original = rpc(&evm, "eth_getCode", json!([executor, "latest"]));
    let committed_artifact: Value = serde_json::from_slice(
        &std::fs::read(
            root().join("contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let regions = |a: &Value| {
        let mut entries: Vec<_> = a["deployedBytecode"]["immutableReferences"]
            .as_object()
            .unwrap()
            .iter()
            .collect();
        // Solidity declaration AST ids order the two inherited declarations: proxy, proxyAdmin.
        entries.sort_by_key(|(id, _)| id.parse::<u64>().unwrap());
        entries
            .into_iter()
            .map(|(_, v)| v.as_array().unwrap().clone())
            .collect::<Vec<_>>()
    };
    let old_regions = regions(&committed_artifact);
    let new_regions = regions(&artifact);
    assert_eq!(old_regions.len(), 2);
    assert_eq!(new_regions.len(), 2);
    let original_bytes = hex::decode(original.as_str().unwrap().trim_start_matches("0x")).unwrap();
    let mut modified_bytes = hex::decode(
        artifact["deployedBytecode"]["object"]
            .as_str()
            .unwrap()
            .trim_start_matches("0x"),
    )
    .unwrap();
    for (old, new) in old_regions.iter().zip(new_regions) {
        let start = old[0]["start"].as_u64().unwrap() as usize;
        let value = &original_bytes[start..start + 32];
        for r in new {
            let at = r["start"].as_u64().unwrap() as usize;
            assert_eq!(r["length"], 32);
            modified_bytes[at..at + 32].copy_from_slice(value);
        }
    }
    rpc(
        &evm,
        "anvil_setCode",
        json!([executor, format!("0x{}", hex::encode(modified_bytes))]),
    );
    let modified = audit("modified executor runtime", &deployment);
    assert_eq!(modified.status.code(), Some(1));
    assert!(
        text(&modified).contains("FAIL executor_code_is_committed_build"),
        "{}",
        text(&modified)
    );
    assert_eq!(
        text(&modified)
            .lines()
            .filter(|l| l.starts_with("FAIL "))
            .count(),
        1,
        "{}",
        text(&modified)
    );
    rpc(&evm, "anvil_setCode", json!([executor, original]));
    let mut changes = Vec::new();
    for kind in [
        json!({"MigrateAuthority":{"new_signer":"0x3333333333333333333333333333333333333333"}}),
        json!({"SetGuardian":{"account":"other.testnet"}}),
        json!({"SetGovernor":{"account":"other.testnet"}}),
        json!({"AddVerifier":{"pubkey":base64::engine::general_purpose::STANDARD.encode([8;32]),"release_hash":release_hash,"attestation_uri":"local"}}),
    ] {
        let id: u64 = governor
            .call(gate.id(), "schedule_change")
            .args_json(json!({"kind":kind}))
            .transact()
            .await?
            .into_result()?
            .json()?;
        let change: Value = gate
            .view("get_change")
            .args_json(json!({"id":id}))
            .await?
            .json()?;
        changes.push(change);
        if id == 0 {
            let warning = audit("governor schedules migration", &deployment);
            assert_checks(&warning, &[], &["pending_authority_change"]);
            assert!(text(&warning).contains(&format!("eta_ns={}", changes[0]["eta_ns"])));
            assert!(text(&warning).contains("audit WARNING: pending authority change"));
        }
    }
    deployment["planned_changes"] = json!(
        changes
            .iter()
            .map(|c| json!({"id":c["id"],"kind":c["kind"],"eta_ns":c["eta_ns"]}))
            .collect::<Vec<_>>()
    );
    deployment_schema::validate(&deployment);
    let planned = audit("exact planned authority changes", &deployment);
    assert_checks(&planned, &[], &[]);
    // The hardened gate permits only one pending change per authority kind.
    // Cancel through the guardian before scheduling the later SetGovernor.
    guardian
        .call(gate.id(), "cancel_changes")
        .args_json(json!({"ids":[0,1,2,3]}))
        .transact()
        .await?
        .into_result()?;
    // Real cancelled changes must not hide an authority change on the next page.
    for _ in 4..50 {
        let id: u64 = governor
            .call(gate.id(), "schedule_change")
            .args_json(json!({"kind":"Unpause"}))
            .transact()
            .await?
            .into_result()?
            .json()?;
        guardian
            .call(gate.id(), "cancel_change")
            .args_json(json!({"change_id":id}))
            .transact()
            .await?
            .into_result()?;
    }
    for kind in [
        json!({"SetGovernor":{"account":"late.testnet"}}),
        json!({"SetParams":{"params":constitution["params"]}}),
        json!({"SetRuleSet":{"rule_set_hash":format!("0x{}","07".repeat(32)),"manifest_uri":"local","ai_required":true}}),
    ] {
        governor
            .call(gate.id(), "schedule_change")
            .args_json(json!({"kind":kind}))
            .transact()
            .await?
            .into_result()?;
    }
    let paged = audit("authority change after 50 real changes", &deployment);
    // The SetRuleSet change changes what a Pass means, so an unplanned one warns too.
    assert_checks(
        &paged,
        &[],
        &[
            "pending_authority_change",
            "pending_authority_change",
            "pending_authority_change",
        ],
    );
    assert!(text(&paged).contains("WARN pending_authority_change: id=50"));
    assert!(text(&paged).contains("SetParams") && text(&paged).contains("SetRuleSet"));
    assert!(text(&paged).contains("3 pending change(s)"));
    guardian
        .call(gate.id(), "pause_signing")
        .args_json(json!({}))
        .transact()
        .await?
        .into_result()?;
    let paused = audit("guardian pauses real gate", &deployment);
    assert_checks(
        &paused,
        &[],
        &[
            "signing_paused",
            "pending_authority_change",
            "pending_authority_change",
            "pending_authority_change",
        ],
    );
    assert!(text(&paused).contains("WARN signing_paused: signing_paused=true"));
    // A warning cannot conceal a failed precondition.
    deployment["gate_code_hash"] = json!(bs58::encode([0; 32]).into_string());
    let warning_and_fail = audit(
        "pause and authority warnings cannot mask failure",
        &deployment,
    );
    assert_checks(
        &warning_and_fail,
        &["gate_code_is_committed_build"],
        &[
            "signing_paused",
            "pending_authority_change",
            "pending_authority_change",
            "pending_authority_change",
        ],
    );
    server.abort();
    eprintln!(
        "real gate audit total: {:.2}s",
        total.elapsed().as_secs_f64()
    );
    Ok(())
}
