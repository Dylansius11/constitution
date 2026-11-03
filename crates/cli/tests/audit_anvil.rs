//! Local anvil EVM + explicitly stubbed NEAR views. No testnet evidence.
use axum::{Json, Router, extract::State, routing::post};
use base64::Engine;
use serde_json::{Value, json};
use std::{
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::{Arc, Mutex},
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
#[derive(Clone)]
struct NearState {
    config: Value,
    constitution: Value,
    changes: Arc<Mutex<Vec<Value>>>,
    pause: Arc<Mutex<Option<bool>>>,
}
async fn near(State(s): State<NearState>, Json(q): Json<Value>) -> Json<Value> {
    let p = &q["params"];
    let result = match p["request_type"].as_str().unwrap() {
        "view_access_key_list" => json!({"keys":[]}),
        "view_account" => json!({"code_hash":"committed-gate"}),
        "call_function" => {
            let value = match p["method_name"].as_str().unwrap() {
                "get_config" => s.config,
                "get_constitution" => s.constitution,
                "get_roles" => json!({"governor":"governor.testnet","guardian":"guardian.testnet"}),
                "is_signing_paused" => match *s.pause.lock().unwrap() {
                    Some(value) => json!(value),
                    None => {
                        return Json(
                            json!({"jsonrpc":"2.0","id":q["id"],"error":{"code":-32000,"message":"MethodNotFound: is_signing_paused"}}),
                        );
                    }
                },
                "list_changes" => {
                    let args: Value = serde_json::from_slice(
                        &base64::engine::general_purpose::STANDARD
                            .decode(p["args_base64"].as_str().unwrap())
                            .unwrap(),
                    )
                    .unwrap();
                    let changes = s.changes.lock().unwrap();
                    json!(
                        changes
                            .iter()
                            .filter(
                                |c| c["id"].as_u64().unwrap() >= args["from_id"].as_u64().unwrap()
                            )
                            .take(50)
                            .collect::<Vec<_>>()
                    )
                }
                "list_verifiers" => json!([]),
                "derived_public_key" => json!(
                    "secp256k1:2LxkyX7Nuw1MbtVBAHWubggYkZ91pGE2yxuEo7bNuVakGYsBrmnHvAuDQtiVExSFF2RPLeepXsYszq8ynFToa8KZ"
                ),
                other => panic!("unexpected view {other}"),
            };
            json!({"result":serde_json::to_vec(&value).unwrap()})
        }
        other => panic!("unexpected request {other}"),
    };
    Json(json!({"jsonrpc":"2.0","id":q["id"],"result":result}))
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn committed_executor_passes_and_modified_runtime_fails_by_name() {
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
    let changes = Arc::new(Mutex::new(vec![]));
    let pause = Arc::new(Mutex::new(Some(false)));
    let state = NearState {
        config: json!({"chain_id":84532,"executor":executor,"proxy":proxy,"proxy_admin":admin,"derived_eoa":"0x3b20f46860d27d81917a45221c594fc051af7e94","mpc_account":"mpc.testnet","mpc_path":"constitution/base-sepolia/v1","mpc_domain_id":0}),
        constitution: json!({"rule_set_hash":format!("0x{}",hex::encode(hash)),"manifest_uri":"rules/manifest.json","ai_required":true,"params":{"proposal_deposit":"1","proposal_ttl_s":7200,"max_cert_ttl_s":1800,"sign_validity_s":3600,"gas_limit":300000,"max_fee_per_gas":"50000000","max_priority_fee_per_gas":"1000000","fee_cap":"1000000000","sign_gas_tgas":15,"callback_gas_tgas":20}}),
        changes: changes.clone(),
        pause: pause.clone(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let near_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/", post(near)).with_state(state),
        )
        .await
        .unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("deployment.json");
    let expected = Command::new(env!("CARGO_BIN_EXE_constitution"))
        .current_dir(root())
        .arg("executor-runtime-hash")
        .output()
        .unwrap();
    assert!(expected.status.success(), "{}", text(&expected));
    let expected_hash = String::from_utf8(expected.stdout).unwrap();
    let mut deployment = json!({"chain_id":84532,"gate":"gate.testnet","gate_code_hash":"committed-gate","mpc_account":"mpc.testnet","executor":executor,"proxy":proxy,"executor_runtime_hash":expected_hash.trim(),"verifiers":[]});
    let audit = |d: &Value| {
        std::fs::write(&path, d.to_string()).unwrap();
        Command::new(env!("CARGO_BIN_EXE_constitution"))
            .current_dir(root())
            .args([
                "audit",
                "--deployment",
                path.to_str().unwrap(),
                "--near-rpc",
                &near_url,
                "--evm-rpc",
                &evm,
            ])
            .output()
            .unwrap()
    };
    let honest = audit(&deployment);
    assert!(honest.status.success(), "{}", text(&honest));
    assert!(text(&honest).contains("PASS executor_code_is_committed_build"));
    deployment
        .as_object_mut()
        .unwrap()
        .remove("executor_runtime_hash");
    let missing = audit(&deployment);
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
    let modified = audit(&deployment);
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
    for (id, kind) in [
        (
            0,
            json!({"MigrateAuthority":{"new_signer":"0x3333333333333333333333333333333333333333"}}),
        ),
        (1, json!({"SetGuardian":{"account":"other.testnet"}})),
        (2, json!({"SetGovernor":{"account":"other.testnet"}})),
        (
            3,
            json!({"AddVerifier":{"pubkey":base64::engine::general_purpose::STANDARD.encode([7;32]),"release_hash":format!("0x{}","06".repeat(32)),"attestation_uri":"local"}}),
        ),
    ] {
        changes.lock().unwrap().push(json!({"id":id,"kind":kind,"scheduled_by":"governor.testnet","scheduled_at_ns":1,"eta_ns":604800000000001u64,"status":"Pending","evm":null}));
    }
    let warning = audit(&deployment);
    assert!(warning.status.success(), "{}", text(&warning));
    assert!(text(&warning).contains("WARN pending_authority_change"));
    assert!(text(&warning).contains("pending authority change"));
    assert!(text(&warning).contains("eta_ns=604800000000001"));
    deployment["planned_changes"] = json!(
        changes
            .lock()
            .unwrap()
            .iter()
            .map(|c| json!({"id":c["id"],"kind":c["kind"],"eta_ns":c["eta_ns"]}))
            .collect::<Vec<_>>()
    );
    let planned = audit(&deployment);
    assert!(planned.status.success(), "{}", text(&planned));
    assert!(!text(&planned).contains("WARN pending_authority_change"));
    // Historical cancelled changes must not hide an authority change on the next page.
    for id in 4..50 {
        changes.lock().unwrap().push(json!({"id":id,"kind":"Unpause","scheduled_by":"governor.testnet","scheduled_at_ns":1,"eta_ns":604800000000001u64,"status":"Cancelled","evm":null}));
    }
    changes.lock().unwrap().push(json!({"id":50,"kind":{"SetGovernor":{"account":"late.testnet"}},"scheduled_by":"governor.testnet","scheduled_at_ns":1,"eta_ns":604800000000001u64,"status":"Pending","evm":null}));
    changes.lock().unwrap().push(json!({"id":51,"kind":{"SetParams":{"params":{
        "proposal_deposit":"1","proposal_ttl_s":7200,"max_cert_ttl_s":1800,"sign_validity_s":3600,"gas_limit":300000,
        "max_fee_per_gas":"50000000","max_priority_fee_per_gas":"1000000","fee_cap":"1000000000","sign_gas_tgas":15,"callback_gas_tgas":20
    }}},"scheduled_by":"governor.testnet","scheduled_at_ns":1,"eta_ns":604800000000001u64,"status":"Pending","evm":null}));
    changes.lock().unwrap().push(json!({"id":52,"kind":{"SetRuleSet":{"rule_set_hash":format!("0x{}","07".repeat(32)),"manifest_uri":"local","ai_required":true}},"scheduled_by":"governor.testnet","scheduled_at_ns":1,"eta_ns":604800000000001u64,"status":"Pending","evm":null}));
    let paged = audit(&deployment);
    assert!(paged.status.success(), "{}", text(&paged));
    assert!(text(&paged).contains("WARN pending_authority_change: id=50"));
    assert!(text(&paged).contains("SetParams") && text(&paged).contains("SetRuleSet"));
    assert!(text(&paged).contains("7 pending change(s)"));
    *pause.lock().unwrap() = Some(true);
    let paused = audit(&deployment);
    assert!(paused.status.success(), "{}", text(&paused));
    assert!(text(&paused).contains("WARN signing_paused: signing_paused=true"));
    *pause.lock().unwrap() = None;
    let unavailable = audit(&deployment);
    assert_eq!(unavailable.status.code(), Some(1));
    assert!(
        text(&unavailable).contains("FAIL read_failed")
            && text(&unavailable).contains("is_signing_paused")
    );
    server.abort();
}
