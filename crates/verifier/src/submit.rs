//! A single submission attempt after certificate issuance; no background retry loop.
use crate::pipeline::{Run, Stage};
use constitution_types::{AccountId, Base64, Base64Bytes, manifest::StageStatus};
use near_api::{Contract, NearGas, NetworkConfig, Signer};
use std::{path::Path, sync::Arc, time::Instant};

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
    rpc_url: String,
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
            rpc_url: url.as_str().to_owned(),
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
            .map_err(|e| e.to_string())?;
        let sent=runtime.block_on(Contract(gate.clone()).call_function("submit_verdict",serde_json::json!({"certificate":certificate,"signature":signature})).transaction().gas(NearGas::from_tgas(300)).with_signer(self.account.clone(),self.signer.clone()).send_to(&self.network))
            .map_err(|e|format!("submit_verdict was not confirmed: {}; inspect relay account transactions before starting another run",e.to_string().replace(&self.rpc_url,"<NEAR_RPC_URL>")))?;
        let full = sent
            .into_full()
            .ok_or("submit_verdict still pending; inspect relay account transactions")?;
        let tx_hash = full.outcome().transaction_hash.to_string();
        full.into_result()
            .map_err(|e| format!("submit_verdict failed in tx {tx_hash}: {e}"))?;
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
                run.outcome = "verdict submitted (not attested)".into();
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
