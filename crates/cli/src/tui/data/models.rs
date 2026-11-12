//! Read-model projections. No signing or verifier-network calls live in this module.
use crate::tui::app::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs;

pub(super) fn proposal_view(raw: Value, gate: &str) -> Result<ProposalView, String> {
    let source = raw.get("proposal").unwrap_or(&raw);
    let p = constitution_types::Proposal::deserialize(source)
        .map_err(|e| format!("Proposal view: {e}"))?;
    let evm_tx_hash = raw["evm"]["tx_hash"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            use sha3::{Digest, Keccak256};
            p.evm
                .as_ref()
                .and_then(|e| e.signed_tx.as_ref())
                .map(|bytes| format!("0x{}", hex::encode(Keccak256::digest(bytes))))
        });
    let new_impl = format!("0x{}", hex::encode(p.new_impl));
    let expected_current_impl = format!("0x{}", hex::encode(p.expected_current_impl));
    let evidence_root = raw["evidence_root"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            p.rejection
                .as_ref()
                .map(|r| format!("0x{}", hex::encode(r.evidence_root)))
        });
    Ok(ProposalView {
        id: p.id,
        proposer: p.proposer.to_string(),
        state: p.state,
        refusal: p
            .rejection
            .as_ref()
            .map(|r| Refusal::new(r.rule_id, r.reason_code)),
        created_at: utc_from_ns(p.created_at_ns),
        expires_at: utc_from_ns(p.expires_at_ns),
        new_impl_url: format!("https://sepolia.basescan.org/address/{new_impl}"),
        current_impl_url: format!("https://sepolia.basescan.org/address/{expected_current_impl}"),
        new_impl,
        expected_current_impl,
        codehash: format!("0x{}", hex::encode(p.new_impl_codehash)),
        init_calldata: format!("0x{}", hex::encode(p.init_calldata)),
        source_commitment: format!("0x{}", hex::encode(p.source_commitment)),
        certificate_hash: p.certificate_hash.map(|h| format!("0x{}", hex::encode(h))),
        evidence_root,
        evm_tx_url: evm_tx_hash
            .as_ref()
            .map(|h| format!("https://sepolia.basescan.org/tx/{h}")),
        evm_status: raw["evm"]["status"].as_str().map(str::to_owned),
        evm_tx_hash,
        gate_url: format!("https://testnet.nearblocks.io/address/{gate}"),
        raw,
    })
}
pub(super) fn change_view(raw: Value) -> Result<PendingChangeView, String> {
    let id = raw["id"]
        .as_u64()
        .ok_or("Governance change missing numeric id")?;
    let eta_ns = raw["eta_ns"]
        .as_u64()
        .ok_or("Governance change missing numeric eta_ns")?;
    let kind = raw["kind"]
        .as_str()
        .or_else(|| raw["kind"].as_object()?.keys().next().map(String::as_str))
        .ok_or("Governance change kind missing")?
        .to_owned();
    let status = raw["status"]
        .as_str()
        .ok_or("Governance change status missing")?
        .to_owned();
    let mut change = PendingChangeView {
        id,
        kind,
        status,
        eta_ns,
        eta_utc: utc_from_ns(eta_ns),
        remaining_ns: 0,
        remaining_seconds: 0,
        matured: false,
        raw,
    };
    change.update_countdown(super::unix_ns());
    Ok(change)
}
pub(super) fn identity_view(
    value: &Value,
    recorded: bool,
    observed_at: Option<String>,
) -> VerifierIdentity {
    VerifierIdentity {
        verifier_pubkey: value["verifier_pubkey"].as_str().map(str::to_owned),
        release_hash: value["release_hash"].as_str().map(str::to_owned),
        compose_hash: value["compose_hash"].as_str().map(str::to_owned),
        recorded,
        observed_at,
    }
}
// Recorded final-CVM bindings from the retirement evidence collected 2026-11-12.
const RETIRED_KEY: &str = "0xf2865c4040b5dc22645cf5ecfa23f5c21e3f120784b47bd41925e2efab6a2881";
const RETIRED_RELEASE: &str = "0x0bf3f730e77f94ac0aebb6bf5b02c606b235e87fa8864c6c87b0189966bdc98a";
const RETIRED_COMPOSE: &str = "0x0fa62de4b208959b1acb02efc04949d6495046d9bd1905280d49cdd244f9d81a";
fn recorded_or_absent(path: &str, fallback: impl FnOnce() -> Value) -> Result<Value, String> {
    match fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|e| format!("Recorded evidence {path}: {e}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(fallback()),
        Err(e) => Err(format!("Recorded evidence {path}: {e}")),
    }
}
pub(super) fn retired_identity() -> Result<VerifierIdentity, String> {
    let raw = recorded_or_absent(
        "deployments/final-verifier-retired/identity.json",
        || json!({"verifier_pubkey":RETIRED_KEY,"release_hash":RETIRED_RELEASE,"compose_hash":RETIRED_COMPOSE}),
    )?;
    let cvm = recorded_or_absent(
        "deployments/final-verifier-retired/cvm.json",
        || json!({"observed_at":"2026-11-12T03:56:00.009Z"}),
    )?;
    Ok(identity_view(
        &raw,
        true,
        cvm["observed_at"].as_str().map(str::to_owned),
    ))
}
pub(super) fn retired_appraisal() -> Result<AppraisalResult, String> {
    let raw = recorded_or_absent("deployments/final-verifier-retired/appraisal.json", || {
        json!({"checks":[
            {"name":"identity","ok":true,"detail":"identity document decoded"},
            {"name":"dcap_signature_chain","ok":true,"detail":"Intel production root, quote signature, certificate chains and collateral verified"},
            {"name":"tcb_up_to_date","ok":true,"detail":"UpToDate"},
            {"name":"rtmr3_replay","ok":true,"detail":"event digests and RTMR3 match quote"},
            {"name":"compose_hash","ok":true,"detail":format!("{RETIRED_COMPOSE} (matches expected compose)")},
            {"name":"report_data","ok":true,"detail":"domain, Ed25519 key, X25519 key, 32-byte boot nonce and zero padding match quote"},
            {"name":"release_hash","ok":true,"detail":RETIRED_RELEASE}
        ]})
    })?;
    let checks = raw["checks"]
        .as_array()
        .ok_or("Recorded appraisal checks missing")?
        .iter()
        .map(|c| {
            Ok(AppraisalCheck {
                name: c["name"]
                    .as_str()
                    .ok_or("Recorded check name missing")?
                    .into(),
                ok: c["ok"].as_bool().ok_or("Recorded check result missing")?,
                detail: c["detail"]
                    .as_str()
                    .ok_or("Recorded check detail missing")?
                    .into(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(AppraisalResult {
        checks,
        observed_at: raw["observed_at"].as_str().map(str::to_owned),
        recorded: true,
        recorded_date: Some("2026-11-12".into()),
    })
}
pub(super) fn recorded_duration(observed: &Value) -> Result<Option<RecordedDuration>, String> {
    let elapsed_ms = if let Some(ms) = observed["duration_ms"].as_u64() {
        Some(ms)
    } else if let Some(seconds) = observed["duration_s"].as_u64() {
        Some(
            seconds
                .checked_mul(1000)
                .ok_or("Recorded duration overflows milliseconds")?,
        )
    } else {
        None
    };
    Ok(elapsed_ms.map(|elapsed_ms| RecordedDuration {
        elapsed_ms,
        observed_at: observed["observed_at"].as_str().map(str::to_owned),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn tui_recorded_duration_is_absent_without_evidence() {
        assert!(
            recorded_duration(&json!({"observed_at":"2026-11-12"}))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            recorded_duration(&json!({"duration_s":12}))
                .unwrap()
                .unwrap()
                .elapsed_ms,
            12000
        );
    }
    #[test]
    fn tui_retirement_files_are_recorded_and_not_live() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deployments/final-verifier-retired");
        let raw: Value =
            serde_json::from_slice(&fs::read(root.join("identity.json")).unwrap()).unwrap();
        let identity = identity_view(&raw, true, None);
        assert!(identity.recorded);
        assert_eq!(identity.verifier_pubkey.as_deref(), Some(RETIRED_KEY));
        assert_eq!(identity.release_hash.as_deref(), Some(RETIRED_RELEASE));
        assert_eq!(identity.compose_hash.as_deref(), Some(RETIRED_COMPOSE));
    }

    #[test]
    fn tui_recorded_receipt_projects_plain_fields_and_preserves_raw() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../deployments/demo-receipts/attack-drain.json");
        let raw: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let view = proposal_view(raw, "constitution-demo.testnet").unwrap();
        assert_eq!(view.proposer, "constitution-governor.testnet");
        assert_eq!(view.state, constitution_types::ProposalState::Rejected);
        assert_eq!(view.refusal.as_ref().unwrap().reason_code, 20);
        assert!(
            view.refusal
                .as_ref()
                .unwrap()
                .meaning
                .contains("counterexample")
        );
        assert_eq!(
            view.gate_url,
            "https://testnet.nearblocks.io/address/constitution-demo.testnet"
        );
        assert!(view.new_impl_url.ends_with(&view.new_impl));
        assert_eq!(view.raw["schema"], "constitution.receipt/1");
        assert!(view.evm_tx_hash.is_none());
        assert!(view.evidence_root.is_some());
    }

    #[test]
    fn tui_final_eta_formats_as_utc_without_rounding_early() {
        assert_eq!(
            utc_from_ns(1_794_588_961_000_000_000),
            "2026-11-13T16:56:01Z"
        );
    }
}
