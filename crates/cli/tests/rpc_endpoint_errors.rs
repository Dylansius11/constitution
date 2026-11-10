//! Exercise RPC/provider error echoes and the relay send path, not only failed NEAR connects.
use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, Uri},
    routing::post,
};
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Output},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone)]
struct StateData {
    proposal: Value,
    sent: Arc<AtomicBool>,
}
async fn handler(
    State(s): State<StateData>,
    uri: Uri,
    Json(q): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let result = match q["method"].as_str().unwrap() {
        "query" => {
            let value = match q["params"]["method_name"].as_str().unwrap() {
                "get_config" => {
                    json!({"chain_id":84532,"executor":"0xa44d05793c89be410c93647fd08f3523444681ae","proxy":"0x1111111111111111111111111111111111111111","proxy_admin":"0x2222222222222222222222222222222222222222","derived_eoa":"0x8221c9477886cc456d7e1cccc0866e0401b81070","mpc_account":"mpc.testnet","mpc_path":"local","mpc_domain_id":0,"governance_delay_days":3})
                }
                "get_proposal" => s.proposal,
                other => panic!("unexpected view {other}"),
            };
            json!({"result":serde_json::to_vec(&value).unwrap()})
        }
        "eth_chainId" => json!("0x14a34"),
        "eth_getTransactionByHash" => Value::Null,
        "eth_sendRawTransaction" => {
            s.sent.store(true, Ordering::SeqCst);
            return (
                StatusCode::OK,
                Json(
                    json!({"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":format!("provider rejected request {uri}; FAKE_SEND_KEY credential invalid")}}),
                ),
            );
        }
        other => panic!("unexpected RPC {other}"),
    };
    (
        StatusCode::OK,
        Json(json!({"jsonrpc":"2.0","id":1,"result":result})),
    )
}
fn output(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_constitution"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args(args)
        .output()
        .unwrap()
}
fn assert_private(out: &Output) {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(1), "{text}");
    for secret in [
        "FAKE_SEND_KEY",
        "FAKE_EVM_KEY",
        "FAKE_QUERY_KEY",
        "http://",
        "password",
    ] {
        assert!(!text.contains(secret), "leaked {secret}: {text}");
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relay_broadcast_and_evm_read_failures_hide_keyed_endpoints() {
    let vector: Value =
        serde_json::from_str(include_str!("../../../vectors/evm-tx-spike.json")).unwrap();
    let signed = &vector["cases"][0]["signed"];
    let proposal = json!({"id":0,"proposer":"proposer.testnet","created_at_ns":1,"expires_at_ns":2,
        "rule_set_hash":format!("0x{}","00".repeat(32)),"new_impl":"0x1111111111111111111111111111111111111111","new_impl_codehash":format!("0x{}","00".repeat(32)),"expected_current_impl":"0x2222222222222222222222222222222222222222","init_calldata":"0x","source_commitment":format!("0x{}","00".repeat(32)),"reserved":"1","state":"Signed","certificate_hash":null,"rejection":null,
        "evm":{"nonce":0,"deadline_s":0,"gas_limit":0,"max_fee_per_gas":"0","max_priority_fee_per_gas":"0","signing_hash":signed["signing_hash"],"signed_tx":signed["raw_tx"],"attempts":1}});
    let sent = Arc::new(AtomicBool::new(false));
    let state = StateData {
        proposal,
        sent: sent.clone(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/", post(handler))
                .route("/FAKE_SEND_KEY", post(handler))
                .with_state(state),
        )
        .await
        .unwrap();
    });
    let send = format!("{base}/FAKE_SEND_KEY?token=FAKE_QUERY_KEY");
    let out = output(&[
        "relay",
        "0",
        "--gate",
        "gate.testnet",
        "--near-rpc",
        &base,
        "--evm-rpc",
        &base,
        "--send-rpc",
        &send,
    ]);
    assert!(
        sent.load(Ordering::SeqCst),
        "relay never reached the failing broadcast: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_private(&out);
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let evm = format!("http://user:password@127.0.0.1:{port}/FAKE_EVM_KEY?token=FAKE_QUERY_KEY");
    for command in ["relay", "status", "receipt"] {
        let mut args = vec![
            command,
            "0",
            "--gate",
            "gate.testnet",
            "--near-rpc",
            &base,
            "--evm-rpc",
            &evm,
        ];
        if command == "receipt" {
            args.push("--json");
        }
        assert_private(&output(&args));
    }
    server.abort();
}
