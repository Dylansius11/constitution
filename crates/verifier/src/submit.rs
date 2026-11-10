//! A single submission attempt after certificate issuance; no background retry loop.
use crate::pipeline::{Run, Stage};
use constitution_types::{AccountId, Base64, Base64Bytes, manifest::StageStatus};
use near_api::{Contract, NearGas, NetworkConfig, Signer};
use std::{path::Path, sync::Arc, time::Instant};

// Never return dependency error text: it can contain RPC URLs, keys or request bodies.
fn diagnostic(error: &str) -> String {
    let status = ["status: ", "status: StatusCode("]
        .iter()
        .find_map(|marker| {
            let suffix = error.split_once(marker)?.1;
            let digits: String = suffix
                .chars()
                .take_while(char::is_ascii_digit)
                .take(3)
                .collect();
            let status = digits.parse::<u16>().ok()?;
            (400..=599).contains(&status).then_some(status)
        });
    let classify = |code: &str| match status {
        Some(status) => format!("{code}; http_{status}"),
        None => code.to_owned(),
    };
    for (needle, code) in [
        ("NotEnoughAllowance", "access_key_allowance"),
        ("MethodNameMismatch", "access_key_permission"),
        ("ReceiverMismatch", "access_key_permission"),
        ("InvalidNonce", "invalid_nonce"),
        ("FetchNonceError", "nonce_query_failed"),
        ("status: 429", "rpc_rate_limited"),
        ("timed out", "rpc_timeout"),
        ("TransportError", "rpc_transport"),
        ("ServerError", "rpc_server_error"),
        ("RequestValidationError", "rpc_request_invalid"),
    ] {
        if error.contains(needle) {
            return classify(code);
        }
    }
    for token in error.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        let bytes = token.as_bytes();
        if bytes.len() >= 4
            && bytes[0] == b'E'
            && bytes[1].is_ascii_digit()
            && bytes[2].is_ascii_digit()
            && bytes[3] == b'_'
        {
            return format!("gate_{}", &token[..3]);
        }
    }
    classify("submission_error")
}

pub trait VerdictSubmitter: Send + Sync {
    /// Runs on the service's blocking worker, never inside the HTTP async runtime.
    fn submit(
        &self,
        gate: &AccountId,
        certificate: &Base64Bytes,
        signature: &Base64<64>,
    ) -> Result<String, String>;
}
pub struct NearSubmitter {
    network: NetworkConfig,
    account: near_api::AccountId,
    signer: Arc<Signer>,
}
impl NearSubmitter {
    pub fn new(rpc_url: &str, account: &str, key_file: &Path) -> Result<Self, String> {
        let url: reqwest::Url = rpc_url.parse().map_err(|_| "invalid NEAR_RPC_URL")?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err("NEAR_RPC_URL must use http(s)".into());
        }
        let account = account
            .parse()
            .map_err(|e| format!("invalid VERIFIER_RELAY_ACCOUNT_ID: {e}"))?;
        let signer = Signer::from_access_keyfile(key_file.to_path_buf())
            .map_err(|_| format!("cannot load VERIFIER_RELAY_KEY_FILE {}", key_file.display()))?;
        Ok(Self {
            network: NetworkConfig::from_rpc_url("constitution-verifier", url),
            account,
            signer,
        })
    }
}
impl VerdictSubmitter for NearSubmitter {
    fn submit(
        &self,
        gate: &AccountId,
        certificate: &Base64Bytes,
        signature: &Base64<64>,
    ) -> Result<String, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "submission_runtime_unavailable")?;
        let sent = runtime
            .block_on(
                Contract(gate.clone())
                    .call_function(
                        "submit_verdict",
                        serde_json::json!({"certificate":certificate,"signature":signature}),
                    )
                    .transaction()
                    .gas(NearGas::from_tgas(300))
                    .with_signer(self.account.clone(), self.signer.clone())
                    .send_to(&self.network),
            )
            .map_err(|error| {
                format!(
                    "submit_verdict {}; inspect relay account transactions before another run",
                    diagnostic(&format!("{error:?}"))
                )
            })?;
        let full = sent
            .into_full()
            .ok_or("submit_verdict still pending; inspect relay account transactions")?;
        let tx_hash = full.outcome().transaction_hash.to_string();
        full.into_result().map_err(|error| {
            format!(
                "submit_verdict {} in observed tx {tx_hash}; inspect its public execution outcome",
                diagnostic(&format!("{error:?}"))
            )
        })?;
        Ok(tx_hash)
    }
}
#[derive(Clone, serde::Serialize)]
pub struct SubmittedRun {
    #[serde(flatten)]
    pub run: Run,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_tx_hash: Option<String>,
}
impl From<Run> for SubmittedRun {
    fn from(run: Run) -> Self {
        Self {
            run,
            near_tx_hash: None,
        }
    }
}
impl SubmittedRun {
    pub fn submit(mut run: Run, gate: &AccountId, submitter: &dyn VerdictSubmitter) -> Self {
        let (Some(certificate), Some(signature)) = (&run.certificate_b64, &run.signature_b64)
        else {
            return run.into();
        };
        let start = Instant::now();
        // Replace the pipeline's local-only terminal markers, retaining the signed evidence.
        run.stages
            .retain(|s| s.name != "submitting" && s.name != "done");
        let (near_tx_hash, status, detail) = match submitter.submit(gate, certificate, signature) {
            Ok(hash) => {
                run.outcome = "verdict submitted; independent identity appraisal required".into();
                (
                    Some(hash),
                    StageStatus::Pass,
                    "submit_verdict landed".into(),
                )
            }
            Err(error) => {
                run.outcome = "submission incomplete; certificate retained".into();
                (None, StageStatus::Incomplete, error)
            }
        };
        let elapsed = start.elapsed().as_millis() as u64;
        run.elapsed_ms += elapsed;
        run.stages.push(Stage {
            name: "submit".into(),
            status,
            detail,
            elapsed_ms: elapsed,
        });
        run.stages.push(Stage {
            name: "done".into(),
            status,
            detail: "run complete; inspect submit stage".into(),
            elapsed_ms: 0,
        });
        run.stage = "done".into();
        Self { run, near_tx_hash }
    }
}

#[cfg(test)]
mod diagnostics_tests {
    use super::*;

    #[test]
    fn unavailable_nonce_rpc_leaves_submission_incomplete_without_endpoint_secrets() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/secret-path?token=secret",
            listener.local_addr().unwrap()
        );
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 8192];
            let _ = stream.read(&mut request);
            let body = "private response";
            write!(
                stream,
                "HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let mut network = NetworkConfig::from_rpc_url("offline", url.parse().unwrap());
        network.rpc_endpoints[0].retries = 1;
        let key = near_api::types::SecretKey::ED25519(
            near_api::types::crypto::secret_key::ED25519SecretKey::from_secret_key([7; 32]),
        );
        let submitter = NearSubmitter {
            network,
            account: "relay.testnet".parse().unwrap(),
            signer: Signer::from_secret_key(key).unwrap(),
        };
        let mut run = Run::queued();
        run.certificate_b64 = Some(Base64Bytes(vec![1, 2]));
        run.signature_b64 = Some(Base64([3; 64]));
        let result = SubmittedRun::submit(run, &"gate.testnet".parse().unwrap(), &submitter);
        let stage = result
            .run
            .stages
            .iter()
            .find(|s| s.name == "submit")
            .unwrap();
        assert_eq!(stage.status, StageStatus::Incomplete);
        assert!(
            stage.detail.contains("nonce_query_failed"),
            "{}",
            stage.detail
        );
        assert!(!stage.detail.contains("secret"));
        assert!(!stage.detail.contains("private response"));
        assert!(result.near_tx_hash.is_none());
        server.join().unwrap();
    }

    #[test]
    fn submission_diagnostics_do_not_disclose_provider_text() {
        for (input, expected) in [
            ("FetchNonceError secret-url", "nonce_query_failed"),
            ("InvalidNonce secret-url", "invalid_nonce"),
            ("NotEnoughAllowance secret-url", "access_key_allowance"),
            ("MethodNameMismatch secret-url", "access_key_permission"),
            ("status: 429 secret-url", "rpc_rate_limited; http_429"),
            ("E42_INVALID_CERTIFICATE secret-url", "gate_E42"),
            ("unrecognized secret-url", "submission_error"),
        ] {
            let result = diagnostic(input);
            assert_eq!(result, expected);
            assert!(!result.contains("secret"));
        }
    }
}
