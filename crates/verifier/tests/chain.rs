use axum::{Json, Router, extract::State, routing::post};
use constitution_types::{
    AccountId, Hex20, Hex32, HexBytes,
    manifest::{RuleSetManifest, rule_set_hash},
};
use constitution_verifier::{
    chain::GateSource,
    proposals::{ProposalSource, SourceError},
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

fn manifest() -> RuleSetManifest {
    let mut m: RuleSetManifest =
        serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
    m.r5.confirmations = 5;
    m
}
#[derive(Clone)]
struct Stub {
    fault: &'static str,
    hash: Hex32,
    calls: Arc<Mutex<Vec<Value>>>,
}
async fn rpc(State(s): State<Stub>, Json(q): Json<Value>) -> Json<Value> {
    s.calls.lock().unwrap().push(q.clone());
    if s.fault == "rpc" || (s.fault == "base_rpc" && q["method"] != "query") {
        return Json(
            json!({"jsonrpc":"2.0","id":q["id"],"error":{"code":-1,"message":"unavailable"}}),
        );
    }
    let result = match q["method"].as_str().unwrap() {
        "query" => {
            assert_eq!(q["params"]["finality"], "final");
            assert_eq!(q["params"]["request_type"], "call_function");
            assert_eq!(q["params"]["account_id"], "gate.testnet");
            let v = match q["params"]["method_name"].as_str().unwrap() {
                "get_config" => {
                    json!({"chain_id":84532,"executor":Hex20([2;20]),"proxy":Hex20([1;20]),"proxy_admin":Hex20([8;20]),"derived_eoa":Hex20([9;20]),"mpc_account":"mpc.testnet","mpc_path":"constitution/base-sepolia/v1","mpc_domain_id":0,"governance_delay_days":3})
                }
                "get_proposal" => {
                    use base64::Engine;
                    let args = base64::engine::general_purpose::STANDARD
                        .decode(q["params"]["args_base64"].as_str().unwrap())
                        .unwrap();
                    assert_eq!(
                        serde_json::from_slice::<Value>(&args).unwrap(),
                        json!({"id":7})
                    );
                    if s.fault == "absent" {
                        Value::Null
                    } else {
                        let mut p = json!({"id":7,"proposer":"alice.testnet","created_at_ns":1000000000u64,"expires_at_ns":1234567890123456789u64,"rule_set_hash":if s.fault == "hash" {Hex32([0;32])} else {s.hash},"new_impl":Hex20([4;20]),"new_impl_codehash":Hex32([5;32]),"expected_current_impl":Hex20([3;20]),"init_calldata":"0x0102","source_commitment":Hex32([6;32]),"reserved":"100","state":if s.fault == "state" {"Rejected"} else {"Pending"},"certificate_hash":null,"rejection":null,"evm":null});
                        if s.fault == "missing" {
                            p.as_object_mut().unwrap().remove("source_commitment");
                        }
                        p
                    }
                }
                _ => panic!("unexpected view"),
            };
            json!({"result":serde_json::to_vec(&v).unwrap()})
        }
        "eth_blockNumber" => json!(if s.fault == "underflow" {
            "0x0"
        } else {
            "0x64"
        }),
        "eth_getBlockByNumber" => {
            if s.fault == "block_null" {
                Value::Null
            } else {
                let changed = s.fault == "reorg" && s.calls.lock().unwrap().len() > 6;
                json!({"number":if s.fault == "block_number" {json!("0x60")} else {q["params"][0].clone()},"hash":Hex32([if changed {8} else {7};32])})
            }
        }
        "eth_getStorageAt" => {
            assert_eq!(q["params"][0], json!(Hex20([1; 20])));
            assert_eq!(
                q["params"][1],
                "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"
            );
            let mut word = [0; 32];
            word[12..].fill(3);
            if s.fault == "slot" {
                word[0] = 1;
            }
            json!(Hex32(word))
        }
        "eth_getCode" => {
            assert_eq!(q["params"][0], json!(Hex20([3; 20])));
            json!(if s.fault == "empty_code" {
                "0x"
            } else {
                "0x60016000"
            })
        }
        other => panic!("unexpected method {other}"),
    };
    Json(json!({"jsonrpc":"2.0","id":q["id"],"result":result}))
}
async fn start_stub(
    fault: &'static str,
) -> (
    GateSource,
    Arc<Mutex<Vec<Value>>>,
    tokio::task::JoinHandle<()>,
) {
    let m = manifest();
    let s = Stub {
        fault,
        hash: Hex32(rule_set_hash(&m).unwrap()),
        calls: Default::default(),
    };
    let calls = s.calls.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/", post(rpc)).with_state(s))
            .await
            .unwrap()
    });
    let source = GateSource::new(&url, &url, &url, &m, "gate.testnet".parse().unwrap()).unwrap();
    (source, calls, server)
}
async fn observe(
    fault: &'static str,
) -> (
    Result<Option<constitution_verifier::proposals::LocalProposal>, SourceError>,
    Vec<Value>,
) {
    let (source, calls, server) = start_stub(fault).await;
    let result = tokio::task::spawn_blocking(move || {
        let gate = "gate.testnet".parse::<AccountId>().unwrap();
        match fault {
            "commitment" => source.get_for_bundle(&gate, 7, [0; 32], &mut || {
                panic!("admission must follow the commitment check")
            }),
            "wrong_gate" => source.get(&"other.testnet".parse().unwrap(), 7),
            _ => source.get(&gate, 7),
        }
    })
    .await
    .unwrap();
    server.abort();
    let calls = calls.lock().unwrap().clone();
    (result, calls)
}
#[tokio::test]
async fn gate_source_observes_every_field_at_confirmed_block() {
    let (result, calls) = observe("").await;
    let p = result.unwrap().unwrap();
    assert_eq!(p.gate.as_str(), "gate.testnet");
    assert_eq!(p.proposal_id, 7);
    assert_eq!(p.chain_id, 84532);
    assert_eq!(p.proxy, Hex20([1; 20]));
    assert_eq!(p.executor, Hex20([2; 20]));
    assert_eq!(p.expected_current_impl, Hex20([3; 20]));
    assert_eq!(p.observed_current_impl, Hex20([3; 20]));
    assert_eq!(
        p.current_impl_codehash,
        Hex32(constitution_verifier::keccak(&[0x60, 1, 0x60, 0]))
    );
    assert_eq!(p.new_impl, Hex20([4; 20]));
    assert_eq!(p.new_impl_codehash, Hex32([5; 32]));
    assert_eq!(p.init_calldata, HexBytes(vec![1, 2]));
    assert_eq!(p.source_commitment, Hex32([6; 32]));
    assert_eq!(p.rule_set_hash, Hex32(rule_set_hash(&manifest()).unwrap()));
    assert_eq!(p.evidence_block_number, 95);
    assert_eq!(p.evidence_block_hash, Hex32([7; 32]));
    assert_eq!(p.expires_at_s, 1234567890);
    let tag = "0x5f";
    assert_eq!(calls[3]["params"], json!([tag, false]));
    assert_eq!(calls[4]["params"][2], tag);
    assert_eq!(calls[5]["params"][1], tag);
}
#[tokio::test]
async fn gate_source_refuses_invalid_or_incomplete_observations() {
    for (fault, error) in [
        ("state", "Pending"),
        ("hash", "rule_set_hash"),
        ("slot", "high 12"),
        ("rpc", "RPC"),
        ("base_rpc", "RPC"),
        ("missing", "source_commitment"),
        ("underflow", "confirmations"),
        ("empty_code", "empty"),
        ("block_null", "RPC"),
        ("block_number", "number mismatch"),
        ("reorg", "changed"),
    ] {
        let (result, _) = observe(fault).await;
        assert!(result.unwrap_err().to_string().contains(error), "{fault}");
    }
    assert!(observe("absent").await.0.unwrap().is_none());
}
#[tokio::test]
async fn chain_state_errors_are_typed_not_rpc_faults() {
    for (fault, expected) in [
        ("state", SourceError::NotPending),
        (
            "hash",
            SourceError::Unsupported("unsupported rule_set_hash"),
        ),
        (
            "slot",
            SourceError::Unsupported("implementation slot high 12 bytes must be zero"),
        ),
        (
            "underflow",
            SourceError::NotReady("insufficient blocks for manifest confirmations"),
        ),
        (
            "empty_code",
            SourceError::NotReady("empty current implementation"),
        ),
    ] {
        assert_eq!(observe(fault).await.0.unwrap_err(), expected, "{fault}");
    }
    for fault in ["rpc", "base_rpc", "block_null", "block_number", "reorg"] {
        assert!(
            matches!(observe(fault).await.0.unwrap_err(), SourceError::Rpc(_)),
            "{fault}"
        );
    }
}
#[tokio::test]
async fn admission_runs_once_after_the_commitment_and_before_base_reads() {
    let (source, calls, server) = start_stub("").await;
    let seen = calls.clone();
    let admitted = tokio::task::spawn_blocking(move || {
        let gate = "gate.testnet".parse::<AccountId>().unwrap();
        let mut admissions = 0;
        let result = source.get_for_bundle(&gate, 7, [6; 32], &mut || {
            admissions += 1;
            // Only the unauthenticated get_proposal read has happened so far.
            assert_eq!(seen.lock().unwrap().len(), 1);
            Ok(())
        });
        (result, admissions)
    })
    .await
    .unwrap();
    assert!(admitted.0.unwrap().is_some());
    assert_eq!(admitted.1, 1);
    assert!(calls.lock().unwrap().len() > 1);
    server.abort();
    // A refusal stops the observation before any further read.
    let (source, calls, server) = start_stub("").await;
    let refused = tokio::task::spawn_blocking(move || {
        let gate = "gate.testnet".parse::<AccountId>().unwrap();
        source.get_for_bundle(&gate, 7, [6; 32], &mut || Err(SourceError::Refused))
    })
    .await
    .unwrap();
    assert_eq!(refused.unwrap_err(), SourceError::Refused);
    assert_eq!(calls.lock().unwrap().len(), 1);
    server.abort();
}

#[test]
fn chain_provenance_exposes_hosts_not_rpc_credentials() {
    let source = GateSource::new(
        "https://alice:password@near.example/api/secret?key=hidden",
        "https://bob:password@base.example/secret?key=hidden",
        "https://carol:password@proof.example/proof-secret?key=hidden",
        &manifest(),
        "gate.testnet".parse().unwrap(),
    )
    .unwrap();
    let provenance = source.archive_provider();
    for host in ["near.example", "base.example", "proof.example"] {
        assert!(provenance.contains(host));
    }
    for secret in ["alice", "bob", "carol", "password", "secret", "hidden"] {
        assert!(!provenance.contains(secret));
    }
    assert_eq!(
        source.archive_rpc().as_deref(),
        Some("https://bob:password@base.example/secret?key=hidden"),
    );
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("registry.json");
    std::fs::write(&file, b"[]").unwrap();
    let local = constitution_verifier::proposals::LocalRegistry::load(file).unwrap();
    assert!(local.archive_rpc().is_none());
}

#[tokio::test]
async fn gate_and_source_commitment_prechecks_avoid_expensive_observations() {
    let (result, calls) = observe("wrong_gate").await;
    assert_eq!(result.unwrap_err(), SourceError::WrongGate);
    assert!(calls.is_empty());
    let (result, calls) = observe("commitment").await;
    assert_eq!(result.unwrap_err(), SourceError::BadBundle);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["params"]["method_name"], "get_proposal");
}

async fn oversized_response_stops_admission(chunked: bool) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const LIMIT: usize = 16 * 1024 * 1024;
    let m = manifest();
    let state = Stub {
        fault: "",
        hash: Hex32(rule_set_hash(&m).unwrap()),
        calls: Default::default(),
    };
    let calls = state.calls.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://alice:url-password@{}/rpc-path-secret?key=query-secret",
        listener.local_addr().unwrap()
    );
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let header_end = loop {
            let byte = stream.read_u8().await.unwrap();
            request.push(byte);
            if request.ends_with(b"\r\n\r\n") {
                break request.len();
            }
        };
        let headers = std::str::from_utf8(&request).unwrap();
        let length: usize = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse().unwrap())
            })
            .unwrap();
        request.resize(header_end + length, 0);
        stream.read_exact(&mut request[header_end..]).await.unwrap();
        let query = serde_json::from_slice(&request[header_end..]).unwrap();
        let Json(mut value) = rpc(State(state.clone()), Json(query)).await;
        value["provider_private"] = json!("body-secret");
        let json = serde_json::to_vec(&value).unwrap();
        // Stream valid JSON plus harmless trailing whitespace; never allocate a huge fixture.
        let padding = LIMIT + 1 - json.len();
        let framing = if chunked {
            "Transfer-Encoding: chunked".to_owned()
        } else {
            format!("Content-Length: {}", LIMIT + 1)
        };
        stream
            .write_all(
                format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{framing}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        if chunked {
            stream
                .write_all(format!("{:x}\r\n", json.len()).as_bytes())
                .await
                .unwrap();
        }
        stream.write_all(&json).await.unwrap();
        if chunked {
            stream.write_all(b"\r\n").await.unwrap();
        }
        let whitespace = [b' '; 8192];
        let mut remaining = padding;
        while remaining > 0 {
            let length = remaining.min(whitespace.len());
            if chunked
                && stream
                    .write_all(format!("{length:x}\r\n").as_bytes())
                    .await
                    .is_err()
            {
                break;
            }
            if stream.write_all(&whitespace[..length]).await.is_err() {
                break;
            }
            if chunked && stream.write_all(b"\r\n").await.is_err() {
                break;
            }
            remaining -= length;
        }
        if chunked {
            // The reader may close immediately after its bounded read.
            let _ = stream.write_all(b"0\r\n\r\n").await;
        }
        drop(stream);
        axum::serve(
            listener,
            Router::new().fallback(post(rpc)).with_state(state),
        )
        .await
        .unwrap();
    });
    let source = GateSource::new(&url, &url, &url, &m, "gate.testnet".parse().unwrap()).unwrap();
    let result = tokio::task::spawn_blocking(move || {
        let gate = "gate.testnet".parse::<AccountId>().unwrap();
        let mut admissions = 0;
        let oversized = source.get_for_bundle(&gate, 7, [6; 32], &mut || {
            admissions += 1;
            Ok(())
        });
        let oversized_admissions = admissions;
        let recovery = source.get_for_bundle(&gate, 7, [6; 32], &mut || {
            admissions += 1;
            Ok(())
        });
        (oversized, oversized_admissions, recovery, admissions)
    })
    .await
    .unwrap();
    server.abort();
    let (oversized, oversized_admissions, recovery, admissions) = result;
    assert!(
        matches!(&oversized, Err(SourceError::Rpc(_))),
        "oversized valid JSON must be rejected as an RPC fault, got {oversized:?}"
    );
    let error = oversized.unwrap_err().to_string();
    for secret in [
        "body-secret",
        "alice",
        "url-password",
        "rpc-path-secret",
        "query-secret",
    ] {
        assert!(
            !error.contains(secret),
            "provider secrets must not be echoed"
        );
    }
    assert_eq!(oversized_admissions, 0);
    let proposal = recovery.unwrap().unwrap();
    assert_eq!(proposal.proposal_id, 7);
    assert_eq!(proposal.evidence_block_number, 95);
    assert_eq!(admissions, 1);
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 8);
    assert_eq!(calls[0]["params"]["method_name"], "get_proposal");
    assert_eq!(calls[1]["params"]["method_name"], "get_proposal");
}

#[tokio::test]
async fn oversized_chunked_rpc_response_is_rejected_before_admission_and_recovers() {
    oversized_response_stops_admission(true).await;
}

#[tokio::test]
async fn oversized_content_length_rpc_response_is_rejected_before_admission_and_recovers() {
    oversized_response_stops_admission(false).await;
}
