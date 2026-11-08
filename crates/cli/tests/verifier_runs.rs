//! Local fake verifier HTTP responses copied from crates/verifier/src/LOCAL.md.
//! These tests exercise CLI behavior, not a live verifier or testnet evidence.
#[allow(dead_code)]
#[path = "../src/failure.rs"]
mod failure;
#[allow(dead_code)]
#[path = "../src/rpc.rs"]
mod rpc;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::{process::Command, sync::Arc};

#[derive(Clone)]
struct Fake {
    status: StatusCode,
    body: String,
    retry_after: Option<&'static str>,
}
async fn response(State(fake): State<Arc<Fake>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    if let Some(seconds) = fake.retry_after {
        headers.insert("Retry-After", seconds.parse().unwrap());
    }
    (fake.status, headers, fake.body.clone())
}
async fn serve(fake: Fake) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route("/FAKE_PATH_KEY/v1/runs", post(response))
        .route("/FAKE_PATH_KEY/v1/runs/7", get(response))
        .with_state(Arc::new(fake));
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (base, server)
}
fn private(text: &str) {
    for secret in [
        "FAKE_PATH_KEY",
        "FAKE_QUERY_KEY",
        "FAKE_PASSWORD",
        "FAKE_USER",
        "http://",
    ] {
        assert!(!text.contains(secret), "leaked {secret}: {text}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn admission_refusals_explain_each_cause_and_recovery_without_credentials() {
    // LOCAL.md: stable plain-text admission codes; only 429 has Retry-After seconds.
    for (status, code, retry_after, cause, next) in [
        (400, "bad_bundle", None, "source commitment", "seal"),
        (400, "wrong_gate", None, "different gate", "--gate"),
        (
            400,
            "unsupported_proposal",
            None,
            "proposal state or rule set",
            "status",
        ),
        (502, "rpc_unavailable", None, "RPC", "retry"),
        (
            429,
            "rate_limited",
            Some("17"),
            "admission rate",
            "after 17 seconds",
        ),
        (
            429,
            "queue_full",
            Some("3"),
            "worker is busy",
            "after 3 seconds",
        ),
        (
            429,
            "cooling_down",
            Some("600"),
            "retries are spaced out",
            "after 600 seconds",
        ),
        (
            429,
            "busy",
            Some("1"),
            "unauthenticated submissions",
            "after 1 seconds",
        ),
        (
            409,
            "proposal_not_pending",
            None,
            "no longer Pending",
            "status",
        ),
        (409, "not_ready", None, "committed data", "retry"),
        (409, "run_in_progress", None, "already running", "status"),
    ] {
        let (base, server) = serve(Fake {
            status: StatusCode::from_u16(status).unwrap(),
            body: code.into(),
            retry_after,
        })
        .await;
        let url = base.replace("http://", "http://FAKE_USER:FAKE_PASSWORD@")
            + "/FAKE_PATH_KEY?token=FAKE_QUERY_KEY";
        let failure = tokio::task::spawn_blocking(move || {
            let verifier = rpc::Verifier::new(&url).unwrap();
            verifier
                .start("gate.testnet", 0, b"fake sealed bytes")
                .unwrap_err()
        })
        .await
        .unwrap();
        let text = failure.to_string();
        assert!(failure.cause.contains(code), "{text}");
        assert!(failure.cause.contains(cause), "{text}");
        assert!(failure.next.contains(next), "{text}");
        private(&text);
        server.abort();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_numeric_run_id_and_evicted_run_follow_the_documented_contract() {
    let (base, server) = serve(Fake {
        status: StatusCode::ACCEPTED,
        body: r#"{"run_id":7}"#.into(),
        retry_after: None,
    })
    .await;
    let id = tokio::task::spawn_blocking(move || {
        rpc::Verifier::new(&(base + "/FAKE_PATH_KEY"))
            .unwrap()
            .start("gate.testnet", 0, b"fake sealed bytes")
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(id, "7");
    server.abort();
    let (base, server) = serve(Fake {
        status: StatusCode::NOT_FOUND,
        body: "not_found".into(),
        retry_after: None,
    })
    .await;
    let failure = tokio::task::spawn_blocking(move || {
        rpc::Verifier::new(&(base + "/FAKE_PATH_KEY"))
            .unwrap()
            .run("7")
            .unwrap_err()
    })
    .await
    .unwrap();
    assert!(failure.cause.contains("evicted"), "{failure}");
    assert!(
        failure.next.contains("constitution verifier run"),
        "{failure}"
    );
    server.abort();
}

async fn chain(Json(request): Json<Value>) -> Json<Value> {
    let result = match request["method"].as_str().unwrap() {
        "query" => {
            let value = match request["params"]["method_name"].as_str().unwrap() {
                "get_proposal" => {
                    json!({"id":0,"proposer":"proposer.testnet","created_at_ns":1,"expires_at_ns":2,
                    "rule_set_hash":format!("0x{}","00".repeat(32)),"new_impl":"0x1111111111111111111111111111111111111111",
                    "new_impl_codehash":format!("0x{}","00".repeat(32)),"expected_current_impl":"0x2222222222222222222222222222222222222222",
                    "init_calldata":"0x","source_commitment":format!("0x{}","00".repeat(32)),"reserved":"1","state":"Pending",
                    "certificate_hash":null,"rejection":null,"evm":null})
                }
                "get_config" => {
                    json!({"chain_id":84532,"executor":"0x1111111111111111111111111111111111111111",
                    "proxy":"0x1111111111111111111111111111111111111111","proxy_admin":"0x2222222222222222222222222222222222222222",
                    "derived_eoa":"0x2222222222222222222222222222222222222222","mpc_account":"mpc.testnet","mpc_path":"fake","mpc_domain_id":0})
                }
                other => panic!("unexpected fake view {other}"),
            };
            json!({"result":serde_json::to_vec(&value).unwrap()})
        }
        "eth_chainId" => json!("0x14a34"),
        "eth_blockNumber" => json!("0x1"),
        "eth_getStorageAt" => json!(format!("0x{}", "00".repeat(32))),
        other => panic!("unexpected fake RPC {other}"),
    };
    Json(json!({"jsonrpc":"2.0","id":"constitution","result":result}))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_watch_stops_on_failed_and_distinguishes_withheld_from_submission_uncertainty() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let chain_url = format!("http://{}", listener.local_addr().unwrap());
    let chain_server = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/", post(chain)))
            .await
            .unwrap();
    });
    let codes = [
        ("worker_panic", "worker stopped unexpectedly"),
        ("sandbox_unavailable", "sandbox is unavailable"),
        ("storage_error", "storage failed"),
        ("tool_unavailable", "required tool is unavailable"),
        ("tool_output_invalid", "tool output is invalid"),
        ("infrastructure_timeout", "infrastructure time limit"),
        ("rpc_unavailable", "RPC is unavailable"),
        (
            "committed_data_unavailable",
            "committed verifier data is unavailable",
        ),
        ("bad_bundle", "source commitment"),
    ];
    let mut cases: Vec<_> = codes.iter().map(|(code, explanation)| {
        (json!({"stage":"failed","started_at":1,"stages":[],"outcome":"failed","elapsed_ms":1,"error_code":code}),
         Some(*code), *explanation, "the proposal is still Pending", 1)
    }).collect();
    cases.push((
        json!({"stage":"done","started_at":1,"stages":[],"outcome":"withheld","elapsed_ms":1}),
        None,
        "AI review incomplete",
        "no certificate",
        1,
    ));
    cases.push((json!({"stage":"done","started_at":1,"elapsed_ms":1,"outcome":"submission incomplete",
        "stages":[{"name":"submit","status":"Incomplete","detail":"fake lost response","elapsed_ms":1}],
        "certificate_b64":"ZmFrZQ==","signature_b64":"ZmFrZQ==","verdict":"Pass"}),
        None, "certificate retained", "inspect relay transactions before retrying", 1));
    cases.push((
        json!({"stage":"done","started_at":1,"elapsed_ms":1,"outcome":"certified",
        "stages":[],"certificate_b64":"ZmFrZQ==","signature_b64":"ZmFrZQ==","verdict":"Pass"}),
        None,
        "certificate issued",
        "reported separately",
        0,
    ));
    for (run, code, explanation, recovery, exit) in cases {
        let (base, server) = serve(Fake {
            status: StatusCode::OK,
            body: run.to_string(),
            retry_after: None,
        })
        .await;
        let out = Command::new(env!("CARGO_BIN_EXE_constitution"))
            .args([
                "status",
                "0",
                "--gate",
                "gate.testnet",
                "--near-rpc",
                &chain_url,
                "--evm-rpc",
                &chain_url,
                "--verifier",
                &(base + "/FAKE_PATH_KEY"),
                "--run",
                "7",
                "--watch",
                "0",
            ])
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.status.code(), Some(exit), "{text}");
        if let Some(code) = code {
            assert!(text.contains(code), "{text}");
            assert!(text.contains("after 10 seconds"), "{text}");
            assert!(
                text.contains("constitution verifier run --proposal"),
                "{text}"
            );
        }
        assert!(text.contains(explanation), "{text}");
        assert!(text.contains(recovery), "{text}");
        assert!(
            !text.contains("within 0s"),
            "terminal run was polled: {text}"
        );
        private(&text);
        server.abort();
    }
    chain_server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retry_command_reseals_only_the_committed_pending_bundle() {
    use base64::Engine;
    use constitution_types::bundle::SourceBundle;
    use crypto_box::{SecretKey, aead::rand_core::OsRng};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let bundle = SourceBundle {
        contract_name: "src/V.sol:V".into(),
        files: vec![("src/V.sol".into(), b"contract V {}".to_vec())],
        foundry_profile: "default".into(),
    };
    let bytes = borsh::to_vec(&bundle).unwrap();
    let commitment = format!(
        "0x{}",
        hex::encode(constitution_types::certificate_hash(&bytes))
    );
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("bundle.borsh");
    std::fs::write(&file, &bytes).unwrap();
    std::fs::create_dir(temp.path().join("src")).unwrap();
    std::fs::write(temp.path().join("src/V.sol"), b"contract V {}").unwrap();
    for (state, hash, directory, succeeds) in [
        ("Pending", commitment.clone(), false, true),
        ("Pending", commitment.clone(), true, true),
        ("Pending", format!("0x{}", "00".repeat(32)), false, false),
        ("Signed", commitment.clone(), false, false),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let key = Arc::new(SecretKey::generate(&mut OsRng));
        let identity_calls = calls.clone();
        let post_calls = calls.clone();
        let public_key = format!("0x{}", hex::encode(key.public_key().as_bytes()));
        let expected = bytes.clone();
        let router = Router::new()
            .route(
                "/",
                post(move |request: Json<Value>| async move {
                    let mut reply = chain(request).await.0;
                    let raw: Vec<u8> =
                        serde_json::from_value(reply["result"]["result"].clone()).unwrap();
                    let mut proposal: Value = serde_json::from_slice(&raw).unwrap();
                    proposal["source_commitment"] = json!(hash);
                    proposal["state"] = json!(state);
                    reply["result"]["result"] = json!(serde_json::to_vec(&proposal).unwrap());
                    Json(reply)
                }),
            )
            .route(
                "/v1/identity",
                get(move || async move {
                    identity_calls.fetch_add(1, Ordering::SeqCst);
                    Json(json!({"transport_pubkey":public_key}))
                }),
            )
            .route(
                "/v1/runs",
                post(move |Json(request): Json<Value>| async move {
                    post_calls.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(request["gate"], "gate.testnet");
                    assert_eq!(request["proposal_id"], 0);
                    let sealed = base64::engine::general_purpose::STANDARD
                        .decode(request["sealed_bundle_b64"].as_str().unwrap())
                        .unwrap();
                    assert_eq!(key.unseal(&sealed).unwrap(), expected);
                    (StatusCode::ACCEPTED, Json(json!({"run_id":7})))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let mut command = Command::new(env!("CARGO_BIN_EXE_constitution"));
        command.args([
            "verifier",
            "run",
            "--proposal",
            "0",
            "--gate",
            "gate.testnet",
            "--near-rpc",
            &url,
            "--verifier-url",
            &url,
            "--bundle",
        ]);
        if directory {
            command.arg(temp.path()).args(["--contract", "src/V.sol:V"]);
        } else {
            command.arg(&file);
        }
        let out = command.output().unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.status.success(), succeeds, "{text}");
        if succeeds {
            assert_eq!(calls.load(Ordering::SeqCst), 2);
            assert!(text.contains("verifier run: 7"), "{text}");
            assert!(text.contains("status 0 --run 7 --watch"), "{text}");
        } else {
            assert_eq!(out.status.code(), Some(1), "{text}");
            assert_eq!(
                calls.load(Ordering::SeqCst),
                0,
                "refusal contacted verifier"
            );
            assert!(
                text.contains(if state == "Pending" {
                    "source commitment"
                } else {
                    "not Pending"
                }),
                "{text}"
            );
        }
        server.abort();
    }
}
