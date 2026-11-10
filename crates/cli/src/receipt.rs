//! Receipts contain observations, never invented transaction hashes or AI results.
use crate::{
    failure::Failure,
    rpc::{Evm, Near, Verifier},
    signed_tx,
};
use base64::Engine;
use constitution_types::{AiReview, Certificate, Config, Proposal, ProposalState};
use serde_json::{Value, json};
use sha3::{Digest, Keccak256};

pub fn read_proposal(near: &Near, gate: &str, id: u64) -> Result<Proposal, Failure> {
    let proposal: Option<Proposal> = near
        .view(gate, "get_proposal", json!({"id": id}))
        .map_err(read_error)?;
    proposal.ok_or_else(|| {
        Failure::new(
            format!("gate {gate} has no proposal {id}"),
            "check --gate and the proposal id printed by propose",
        )
    })
}

fn read_error(cause: String) -> Failure {
    Failure::new(
        cause,
        "check --gate, --near-rpc and --evm-rpc; retry the read command",
    )
}

fn hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

/// Only facts available from the gate. Additional public observations are added by `observe`.
pub fn assemble(proposal: &Proposal, network: &str) -> Value {
    let mut value = json!({
        "schema": "constitution.receipt/1",
        "proposal": proposal,
        "rule_set_hash": hex(&proposal.rule_set_hash),
        "evm": {"status": "not_signed"},
        "labels": {"network": network},
    });
    if let Some(rejection) = &proposal.rejection {
        value["verdict"] = json!({"kind": "Reject", "rule_id": rejection.rule_id, "reason_code": rejection.reason_code});
        value["evidence_root"] = json!(hex(&rejection.evidence_root));
    } else if proposal.certificate_hash.is_some()
        && matches!(
            proposal.state,
            ProposalState::SignRequested | ProposalState::SignFailed | ProposalState::Signed
        )
    {
        value["verdict"] = json!({"kind": "Pass"});
    }
    if let Some(evm) = &proposal.evm {
        value["evm"]["nonce"] = json!(evm.nonce);
        if let Some(raw) = &evm.signed_tx {
            value["evm"]["status"] = json!("signed");
            value["evm"]["tx_hash"] = json!(hex(&Keccak256::digest(raw)));
        }
    }
    // ProposalView has nullable storage fields; receipt-only fields above remain absent.
    value
}

fn add_run(value: &mut Value, proposal: &Proposal, run: &Value) -> Result<(), Failure> {
    let Some(encoded) = run.get("certificate_b64").and_then(Value::as_str) else {
        return Ok(());
    };
    if proposal.certificate_hash.is_none() {
        return Ok(());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| read_error(format!("run certificate base64: {e}")))?;
    let cert = Certificate::from_canonical_bytes(&bytes)
        .map_err(|e| read_error(format!("run certificate Borsh: {e}")))?;
    // A run is not an authority: use certificate facts only after the gate accepted these bytes.
    if proposal.certificate_hash != Some(constitution_types::certificate_hash(&bytes)) {
        return Err(Failure::new(
            "verifier run certificate is not the certificate accepted by this proposal",
            "wait for verdict submission or pass the run id belonging to this proposal",
        ));
    }
    value["verifier"] = json!({"pubkey": hex(&cert.verifier_pubkey), "release_hash": hex(&cert.verifier_release_hash)});
    value["evidence_root"] = json!(hex(&cert.evidence_root));
    if let AiReview::Clean { signature_kind, .. } = cert.ai {
        let signature_kind = match signature_kind {
            1 => "provider_tee",
            2 => "gateway",
            _ => return Err(read_error("unknown certificate AI signature kind".into())),
        };
        value["ai"] = json!({"signature_kind": signature_kind});
    }
    if let Some(tx) = run.get("near_tx_hash").filter(|v| v.is_string()) {
        value["near"] = json!({"verdict_tx": tx});
    }
    Ok(())
}

pub fn observe(
    near: &Near,
    gate: &str,
    proposal: &Proposal,
    evm: &Evm,
    network: &str,
    run: Option<&Value>,
) -> Result<Value, Failure> {
    if !matches!(network, "testnet" | "local") {
        return Err(Failure::new(
            format!("receipt network {network} is unsupported"),
            "use --network local or --network testnet",
        ));
    }
    let config: Config = near
        .view(gate, "get_config", json!({}))
        .map_err(read_error)?;
    if evm.chain_id().map_err(read_error)? != config.chain_id {
        return Err(Failure::new(
            "EVM RPC chain differs from the gate configuration",
            "point --evm-rpc at the gate's configured chain",
        ));
    }
    let mut value = assemble(proposal, network);
    if let Some(run) = run {
        add_run(&mut value, proposal, run)?;
    }
    if let Some(tx) = proposal.evm.as_ref().filter(|tx| tx.signed_tx.is_some()) {
        let signed = signed_tx::verify(
            tx.signed_tx
                .as_deref()
                .expect("filtered signed transaction"),
            &config,
            tx,
        )
        .map_err(read_error)?;
        if let Some(receipt) = evm.receipt(&signed.tx_hash).map_err(read_error)? {
            let block = crate::rpc::quantity(&receipt["blockNumber"], "receipt blockNumber")
                .map_err(read_error)?;
            let status =
                crate::rpc::quantity(&receipt["status"], "receipt status").map_err(read_error)?;
            value["evm"]["status"] = match status {
                0 => json!("reverted"),
                1 => json!("included"),
                _ => return Err(read_error("EVM receipt status is neither 0 nor 1".into())),
            };
            value["evm"]["block"] = json!(block);
            if network == "testnet" {
                value["evm"]["explorer"] = json!(format!(
                    "https://sepolia.basescan.org/tx/{}",
                    hex(&signed.tx_hash)
                ));
            }
        } else if evm.knows(&signed.tx_hash).map_err(read_error)? {
            value["evm"]["status"] = json!("pending");
        }
    }
    let block = evm.block_number().map_err(read_error)?;
    let implementation = evm
        .storage_address_at(config.proxy, crate::audit::IMPLEMENTATION_SLOT, block)
        .map_err(read_error)?;
    value["proxy"] =
        json!({"implementation_after": hex(&implementation), "observed_at_block": block});
    Ok(value)
}

fn add_evidence(value: &mut Value, raw: Option<Value>) -> Result<(), Failure> {
    let raw = raw.ok_or_else(|| Failure::new(
        "accepted certificate evidence is unavailable; receipt is incomplete",
        "retain the gate-only receipt and restore the matching evidence; do not claim complete verification",
    ))?;
    let evidence: constitution_types::EvidenceManifest =
        serde_json::from_value(raw).map_err(|e| read_error(format!("evidence manifest: {e}")))?;
    let bytes = borsh::to_vec(&evidence).map_err(|e| read_error(e.to_string()))?;
    if value["evidence_root"].as_str()
        != Some(hex(&constitution_types::certificate_hash(&bytes)).as_str())
    {
        return Err(Failure::new(
            "evidence manifest does not match the accepted evidence_root",
            "do not trust this evidence; check the verifier and run id",
        ));
    }
    value["proxy"]["implementation_before"] = json!(hex(&evidence.current.address.0));
    if !evidence.ai.signature_kind.is_empty() {
        value["ai"] = json!({
            "signature_kind": evidence.ai.signature_kind,
            "reproduced_candidates": evidence.ai.candidates.iter().filter(|candidate| candidate.reproduced).count(),
        });
    }
    Ok(())
}

pub fn collect(
    near: &Near,
    gate: &str,
    id: u64,
    evm: &Evm,
    network: &str,
    run: Option<(&Verifier, &str)>,
) -> Result<Value, Failure> {
    let proposal = read_proposal(near, gate, id)?;
    let run_value = run
        .map(|(client, run_id)| client.run(run_id, gate, id))
        .transpose()?;
    let mut value = observe(near, gate, &proposal, evm, network, run_value.as_ref())?;
    if let Some((client, id)) = run.filter(|_| value.get("evidence_root").is_some()) {
        add_evidence(&mut value, client.evidence(id)?)?;
    }
    Ok(value)
}

pub fn receipt(
    near: &Near,
    gate: &str,
    id: u64,
    evm: &Evm,
    network: &str,
    run: Option<(&Verifier, &str)>,
    as_json: bool,
) -> Result<(), Failure> {
    let value = collect(near, gate, id, evm, network, run)?;
    if !as_json {
        println!("Observed receipt ({}): proposal {id}", network);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|e| read_error(e.to_string()))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use constitution_types::{EvmTx, Rejection};
    fn proposal(state: ProposalState) -> Proposal {
        Proposal {
            id: 0,
            proposer: "alice.testnet".parse().unwrap(),
            created_at_ns: 1,
            expires_at_ns: 2,
            rule_set_hash: [1; 32],
            new_impl: [2; 20],
            new_impl_codehash: [3; 32],
            expected_current_impl: [4; 20],
            init_calldata: vec![],
            source_commitment: [5; 32],
            reserved: 100,
            state,
            certificate_hash: None,
            rejection: None,
            evm: None,
        }
    }
    #[test]
    fn receipt_maps_every_gate_state_without_inventing_observations() {
        for state in [
            ProposalState::Pending,
            ProposalState::Rejected,
            ProposalState::Expired,
            ProposalState::SignRequested,
            ProposalState::Signed,
            ProposalState::SignFailed,
        ] {
            let mut p = proposal(state.clone());
            match state {
                ProposalState::Rejected => {
                    p.certificate_hash = Some([8; 32]);
                    p.rejection = Some(Rejection {
                        rule_id: 2,
                        reason_code: 20,
                        evidence_root: [9; 32],
                    });
                }
                ProposalState::SignRequested
                | ProposalState::SignFailed
                | ProposalState::Signed => {
                    p.certificate_hash = Some([8; 32]);
                    p.evm = Some(EvmTx {
                        nonce: 0,
                        deadline_s: 10,
                        gas_limit: 1,
                        max_fee_per_gas: 2,
                        max_priority_fee_per_gas: 1,
                        signing_hash: [9; 32],
                        signed_tx: (state == ProposalState::Signed).then(|| vec![2, 1]),
                        attempts: 1,
                    });
                }
                _ => {}
            }
            let r = assemble(&p, "local");
            assert_eq!(r["schema"], "constitution.receipt/1");
            assert_eq!(
                r["proposal"]["state"],
                serde_json::to_value(&state).unwrap()
            );
            assert_eq!(r["labels"]["network"], "local");
            for key in ["near", "verifier", "ai", "proxy"] {
                assert!(r.get(key).is_none(), "fabricated {key}");
            }
            assert!(r["evm"].get("block").is_none());
            assert!(r["evm"].get("explorer").is_none());
            if state == ProposalState::Signed {
                assert_eq!(r["evm"]["status"], "signed");
                assert!(r["evm"]["tx_hash"].is_string());
            } else {
                assert_eq!(r["evm"]["status"], "not_signed");
                assert!(r["evm"].get("tx_hash").is_none());
            }
            match state {
                ProposalState::Pending | ProposalState::Expired => {
                    assert!(r.get("verdict").is_none())
                }
                ProposalState::Rejected => {
                    assert_eq!(
                        r["verdict"],
                        json!({"kind":"Reject", "rule_id":2, "reason_code":20})
                    );
                    assert_eq!(r["evidence_root"], hex(&[9; 32]));
                }
                _ => assert_eq!(r["verdict"]["kind"], "Pass"),
            }
        }
    }
    #[test]
    fn run_certificate_must_be_accepted_before_receipt_claims_verifier_or_ai() {
        let mut p = proposal(ProposalState::Pending);
        let cert = Certificate {
            domain: constitution_types::DOMAIN,
            gate: "gate.testnet".parse().unwrap(),
            proposal_id: 0,
            attempt: 0,
            chain_id: 84532,
            proxy: [6; 20],
            executor: [7; 20],
            current_impl: p.expected_current_impl,
            current_impl_codehash: [8; 32],
            new_impl: p.new_impl,
            new_impl_codehash: p.new_impl_codehash,
            init_calldata_hash: Keccak256::digest([]).into(),
            source_commitment: p.source_commitment,
            rule_set_hash: p.rule_set_hash,
            verifier_pubkey: [9; 32],
            verifier_release_hash: [10; 32],
            evidence_block_number: 10,
            evidence_block_hash: [11; 32],
            evidence_root: [12; 32],
            ai: AiReview::Clean {
                evidence_hash: [13; 32],
                signature_kind: 1,
            },
            verdict: constitution_types::Verdict::Pass,
            issued_at_s: 1,
            expires_at_s: 2,
        };
        let bytes = cert.to_canonical_bytes().unwrap();
        let run =
            json!({"certificate_b64":base64::engine::general_purpose::STANDARD.encode(&bytes)});
        let mut r = assemble(&p, "local");
        add_run(&mut r, &p, &run).unwrap();
        assert!(r.get("verifier").is_none());
        assert!(r.get("ai").is_none());
        p.certificate_hash = Some(constitution_types::certificate_hash(&bytes));
        p.state = ProposalState::SignRequested;
        let mut r = assemble(&p, "local");
        add_run(&mut r, &p, &run).unwrap();
        assert_eq!(r["verifier"]["pubkey"], hex(&[9; 32]));
        assert_eq!(r["verifier"]["release_hash"], hex(&[10; 32]));
        assert_eq!(r["ai"]["signature_kind"], "provider_tee");
        assert!(r["ai"].get("reproduced_candidates").is_none());
        assert!(r.get("near").is_none());
        p.certificate_hash = Some([99; 32]);
        assert!(add_run(&mut r, &p, &run).is_err());
    }

    #[test]
    fn evidence_is_hashed_before_adding_proxy_and_ai_observations() {
        use constitution_types::{EvidenceManifest, Hex20, Hex32, HexBytes, manifest::*};
        let rule = RuleEvidence {
            status: StageStatus::Pass,
            artifact_sha256: Hex32([1; 32]),
            detail_code: 0,
        };
        let evidence = EvidenceManifest {
            certificate_fields_digest: Hex32([2; 32]),
            compile: CompileEvidence {
                solc: "0.8.38".into(),
                settings_hash: Hex32([3; 32]),
                runtime_hash: Hex32([4; 32]),
            },
            current: CurrentEvidence {
                address: Hex20([5; 20]),
                runtime_hash: Hex32([6; 32]),
                reproduced: true,
            },
            r1: rule.clone(),
            r2: rule.clone(),
            r3: rule.clone(),
            r4: rule.clone(),
            r5: rule,
            ai: AiEvidence {
                model: "local-test".into(),
                completion_id: "observed".into(),
                request_sha256: Hex32([7; 32]),
                response_sha256: Hex32([8; 32]),
                signature_kind: "provider_tee".into(),
                signature: HexBytes(vec![1]),
                preflight_digest: Hex32([9; 32]),
                candidates: vec![
                    AiCandidate {
                        sha256: Hex32([10; 32]),
                        reproduced: true,
                    },
                    AiCandidate {
                        sha256: Hex32([11; 32]),
                        reproduced: false,
                    },
                ],
            },
            evidence_block: EvidenceBlock {
                number: 12,
                hash: Hex32([13; 32]),
            },
            archive_provider: "local".into(),
        };
        let bytes = borsh::to_vec(&evidence).unwrap();
        let mut r =
            json!({"evidence_root":hex(&constitution_types::certificate_hash(&bytes)), "proxy":{}});
        add_evidence(&mut r, Some(serde_json::to_value(&evidence).unwrap())).unwrap();
        assert_eq!(r["proxy"]["implementation_before"], hex(&[5; 20]));
        assert_eq!(r["ai"]["reproduced_candidates"], 1);
        let mut changed = evidence;
        changed.current.address = Hex20([99; 20]);
        assert!(add_evidence(&mut r, Some(serde_json::to_value(changed).unwrap())).is_err());
    }

    #[test]
    fn accepted_certificate_with_unavailable_evidence_is_not_complete() {
        let mut receipt = json!({"evidence_root":hex(&[8;32]),"proxy":{}});
        assert!(add_evidence(&mut receipt, None).is_err());
        assert!(receipt["proxy"].get("implementation_before").is_none());
        assert!(receipt.get("ai").is_none());
    }
}
