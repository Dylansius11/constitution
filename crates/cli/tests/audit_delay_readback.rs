//! Audit must compare an actually observed delay, not the initialization JSON default.
use axum::{Json, Router, routing::post};
use serde_json::{Value, json};
use std::{path::Path, process::Command};

async fn rpc(Json(request): Json<Value>) -> Json<Value> {
    let response = if request["params"]["method_name"] == "get_config" {
        let config = json!({"chain_id":84532,"executor":"0x1111111111111111111111111111111111111111",
            "proxy":"0x2222222222222222222222222222222222222222","proxy_admin":"0x3333333333333333333333333333333333333333",
            "derived_eoa":"0x4444444444444444444444444444444444444444","mpc_account":"mpc.testnet",
            "mpc_path":"local","mpc_domain_id":0});
        json!({"jsonrpc":"2.0","id":1,"result":{"result":serde_json::to_vec(&config).unwrap()}})
    } else {
        json!({"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"unexpected later read"}})
    };
    Json(response)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn audit_rejects_missing_observed_governance_delay() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/", post(rpc)))
            .await
            .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let deployment = dir.path().join("deployment.json");
    let artifact = dir.path().join("executor.json");
    std::fs::write(
        &deployment,
        json!({"chain_id":84532,"governance_delay_days":3,
        "gate":"gate.testnet","gate_code_hash":"committed","mpc_account":"mpc.testnet",
        "executor":"0x1111111111111111111111111111111111111111",
        "proxy":"0x2222222222222222222222222222222222222222","verifiers":[]})
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        &artifact,
        json!({"deployedBytecode":{"object":"0x60","immutableReferences":{}}}).to_string(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_constitution"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args([
            "audit",
            "--deployment",
            deployment.to_str().unwrap(),
            "--executor-artifact",
            artifact.to_str().unwrap(),
            "--near-rpc",
            &url,
            "--evm-rpc",
            &url,
        ])
        .output()
        .unwrap();
    server.abort();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(text.contains("governance_delay_days"), "{text}");
    assert!(!text.contains("unexpected later read"), "{text}");
}
