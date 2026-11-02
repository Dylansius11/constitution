use crate::{bundle, identity::Identity, keccak, proposals::LocalProposal, sha256, tools::Sandbox};
use constitution_types::{manifest::*, *};
use std::{
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
#[derive(Debug, Clone, serde::Serialize)]
pub struct Stage {
    pub name: String,
    pub status: StageStatus,
    pub detail: String,
    pub elapsed_ms: u64,
}
pub type Observer = std::sync::Arc<dyn Fn(&Run) + Send + Sync>;
#[derive(Clone, serde::Serialize)]
pub struct Run {
    pub stage: String,
    pub started_at: u64,
    pub stages: Vec<Stage>,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<Verdict>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate_b64: Option<Base64Bytes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_b64: Option<Base64<64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_root: Option<Hex32>,
    pub elapsed_ms: u64,
    #[serde(skip)]
    pub evidence: Option<EvidenceManifest>,
    #[serde(skip)]
    observer: Option<Observer>,
}
impl Run {
    pub fn queued() -> Self {
        Self {
            stage: "queued".into(),
            started_at: now(),
            stages: vec![],
            outcome: "pending".into(),
            verdict: None,
            certificate_b64: None,
            signature_b64: None,
            evidence_root: None,
            elapsed_ms: 0,
            evidence: None,
            observer: None,
        }
    }
    pub fn snapshot(&self) -> Self {
        let mut copy = self.clone();
        copy.observer = None;
        copy
    }
    fn begin(&mut self, name: &str) {
        self.stage = name.into();
        if let Some(observer) = &self.observer {
            observer(self);
        }
    }
    fn record(
        &mut self,
        name: &str,
        status: StageStatus,
        detail: impl Into<String>,
        start: Instant,
    ) {
        self.stage = name.into();
        self.stages.push(Stage {
            name: name.into(),
            status,
            detail: detail.into(),
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
        if let Some(observer) = &self.observer {
            observer(self);
        }
    }
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn empty_rule() -> RuleEvidence {
    RuleEvidence {
        status: StageStatus::NotRun,
        artifact_sha256: Hex32([0; 32]),
        detail_code: 0,
    }
}
fn evidence(p: &LocalProposal, m: &RuleSetManifest) -> EvidenceManifest {
    EvidenceManifest {
        certificate_fields_digest: Hex32([0; 32]),
        compile: CompileEvidence {
            solc: m.solc.clone(),
            settings_hash: Hex32(sha256(
                &borsh::to_vec(&(m.solc.as_str(), m.evm_version.as_str(), m.optimizer_runs))
                    .expect("settings serialize"),
            )),
            runtime_hash: Hex32([0; 32]),
        },
        current: CurrentEvidence {
            address: p.observed_current_impl,
            runtime_hash: p.current_impl_codehash,
            reproduced: false,
        },
        r1: empty_rule(),
        r2: empty_rule(),
        r3: empty_rule(),
        r4: empty_rule(),
        r5: empty_rule(),
        ai: AiEvidence {
            model: m.ai.model.clone(),
            completion_id: String::new(),
            request_sha256: Hex32([0; 32]),
            response_sha256: Hex32([0; 32]),
            signature_kind: String::new(),
            signature: HexBytes(vec![]),
            preflight_digest: Hex32([0; 32]),
            candidates: vec![],
        },
        evidence_block: EvidenceBlock {
            number: p.evidence_block_number,
            hash: p.evidence_block_hash,
        },
        archive_provider: "local registry, not chain observed".into(),
    }
}
pub fn execute(
    repo: &Path,
    m: &RuleSetManifest,
    identity: &Identity,
    p: &LocalProposal,
    sealed: &[u8],
    attempt: u32,
) -> Run {
    execute_observed(repo, m, identity, p, sealed, attempt, None)
}
pub fn execute_observed(
    repo: &Path,
    m: &RuleSetManifest,
    identity: &Identity,
    p: &LocalProposal,
    sealed: &[u8],
    attempt: u32,
    observer: Option<Observer>,
) -> Run {
    let start = Instant::now();
    let mut run = Run::queued();
    run.observer = observer;
    let mut ev = evidence(p, m);
    run.record("queued", StageStatus::Pass, "run started", Instant::now());
    let rejection = run_stages(repo, m, identity, p, sealed, &mut run, &mut ev);
    if let Some((rule, reason)) = rejection {
        for (name, evidence) in [
            ("r1_layout", &mut ev.r1),
            ("r2_solvency", &mut ev.r2),
            ("r3_filter", &mut ev.r3),
            ("r4_initializer", &mut ev.r4),
            ("r5_replay", &mut ev.r5),
        ] {
            if !run.stages.iter().any(|s| s.name == name) {
                evidence.status = StageStatus::Skipped;
                run.record(
                    name,
                    StageStatus::Skipped,
                    "earlier stage stopped run",
                    Instant::now(),
                );
            }
        }
        run.record(
            "ai_review",
            StageStatus::NotRun,
            "deterministic stages did not all pass",
            Instant::now(),
        );
        run.record(
            "reproducing",
            StageStatus::NotRun,
            "AI not run",
            Instant::now(),
        );
        let issued = now();
        let mut cert = Certificate {
            domain: DOMAIN,
            gate: p.gate.clone(),
            proposal_id: p.proposal_id,
            attempt,
            chain_id: p.chain_id,
            proxy: p.proxy.0,
            executor: p.executor.0,
            current_impl: p.expected_current_impl.0,
            current_impl_codehash: p.current_impl_codehash.0,
            new_impl: p.new_impl.0,
            new_impl_codehash: p.new_impl_codehash.0,
            init_calldata_hash: keccak(&p.init_calldata.0),
            source_commitment: p.source_commitment.0,
            rule_set_hash: p.rule_set_hash.0,
            verifier_pubkey: identity.pubkey(),
            verifier_release_hash: [0; 32],
            evidence_block_number: p.evidence_block_number,
            evidence_block_hash: p.evidence_block_hash.0,
            evidence_root: [0; 32],
            ai: AiReview::NotRun,
            verdict: Verdict::Reject {
                rule_id: rule,
                reason_code: reason,
            },
            issued_at_s: issued,
            expires_at_s: p.expires_at_s.min(issued.saturating_add(m.max_cert_ttl_s)),
        };
        // [I] Avoid a digest cycle: certificate_fields_digest commits to the certificate
        // with evidence_root zeroed, then the signed certificate binds the resulting root.
        ev.certificate_fields_digest = Hex32(sha256(
            &borsh::to_vec(&cert).expect("certificate serialize"),
        ));
        let root = evidence_root(&ev).expect("evidence serialize");
        cert.evidence_root = root;
        match identity.sign_reject(&cert) {
            Ok((signature, bytes)) => {
                run.verdict = Some(cert.verdict);
                run.certificate_b64 = Some(Base64Bytes(bytes));
                run.signature_b64 = Some(Base64(signature));
                run.evidence_root = Some(Hex32(root));
                run.outcome = "rejected (local, not attested; not submitted)".into();
                run.record(
                    "signing",
                    StageStatus::Pass,
                    "ephemeral Ed25519 Reject",
                    Instant::now(),
                );
            }
            Err(e) => {
                run.outcome = format!("withheld: {e}");
            }
        }
    } else {
        run.record(
            "r5_replay",
            StageStatus::NotRun,
            "required stage not implemented",
            Instant::now(),
        );
        run.record(
            "ai_review",
            StageStatus::NotRun,
            "required stage not implemented",
            Instant::now(),
        );
        run.record(
            "reproducing",
            StageStatus::NotRun,
            "AI not run",
            Instant::now(),
        );
        run.outcome = "withheld: required stages not implemented".into();
        run.evidence_root = Some(Hex32(evidence_root(&ev).expect("evidence serialize")));
        run.record(
            "signing",
            StageStatus::NotRun,
            "no Pass certificate",
            Instant::now(),
        );
    }
    run.record(
        "submitting",
        StageStatus::NotRun,
        "local only",
        Instant::now(),
    );
    run.elapsed_ms = start.elapsed().as_millis() as u64;
    run.evidence = Some(ev);
    run.record(
        "done",
        StageStatus::Pass,
        "run complete; outcome states certificate policy",
        Instant::now(),
    );
    run.observer = None;
    run
}
#[allow(clippy::too_many_arguments)]
fn run_stages(
    repo: &Path,
    m: &RuleSetManifest,
    identity: &Identity,
    p: &LocalProposal,
    sealed: &[u8],
    run: &mut Run,
    ev: &mut EvidenceManifest,
) -> Option<(u8, u16)> {
    run.begin("unsealing");
    let start = Instant::now();
    let (b, bytes) = match bundle::open(&identity.transport, sealed) {
        Ok(b) => b,
        Err(e) => {
            run.record("unsealing", StageStatus::Incomplete, e, start);
            return Some((0, 1));
        }
    };
    if sha256(&bytes) != p.source_commitment.0 {
        run.record(
            "unsealing",
            StageStatus::Fail,
            "source commitment mismatch",
            start,
        );
        return Some((0, 1));
    }
    run.record(
        "unsealing",
        StageStatus::Pass,
        "source commitment verified",
        start,
    );
    run.begin("compiling");
    let start = Instant::now();
    if p.observed_current_impl != p.expected_current_impl {
        run.record(
            "compiling",
            StageStatus::Fail,
            "current implementation changed (local registry)",
            start,
        );
        return Some((0, 3));
    }
    for (name, hash) in &m.reference.source_sha256s {
        if !b
            .files
            .iter()
            .any(|(n, v)| n == name && sha256(v) == hash.0)
        {
            run.record(
                "compiling",
                StageStatus::Fail,
                "reference source missing or modified",
                start,
            );
            return Some((0, 4));
        }
    }
    let sandbox = match Sandbox::new(repo, &b, m) {
        Ok(s) => s,
        Err(e) => {
            run.record("compiling", StageStatus::Incomplete, e, start);
            return Some((0, 2));
        }
    };
    if let Err(e) = sandbox.compile() {
        run.record(
            "compiling",
            StageStatus::Incomplete,
            format!(
                "compiler failed; diagnostic digest {:?}",
                Hex32(sha256(e.as_bytes()))
            ),
            start,
        );
        return Some((0, 2));
    }
    let runtime = match sandbox.runtime(&b.contract_name) {
        Ok(v) => v,
        Err(e) => {
            run.record("compiling", StageStatus::Incomplete, e, start);
            return Some((0, 2));
        }
    };
    ev.compile.runtime_hash = Hex32(keccak(&runtime));
    if ev.compile.runtime_hash != p.new_impl_codehash {
        run.record(
            "compiling",
            StageStatus::Fail,
            "compiled runtime mismatch",
            start,
        );
        return Some((0, 2));
    }
    let current = sandbox.runtime(&m.reference.contract_name).ok();
    if current
        .as_ref()
        .is_none_or(|v| keccak(v) != p.current_impl_codehash.0)
    {
        run.record(
            "compiling",
            StageStatus::Fail,
            "current source runtime mismatch",
            start,
        );
        return Some((0, 4));
    }
    ev.current.reproduced = true;
    run.record(
        "compiling",
        StageStatus::Pass,
        "pinned runtime hashes reproduced (local)",
        start,
    );
    run.begin("r1_layout");
    let start = Instant::now();
    let (status, log) = sandbox
        .layout(
            &b.contract_name,
            &m.reference.contract_name,
            &m.r1.unsafe_allow,
        )
        .unwrap_or_else(|e| (StageStatus::Incomplete, e.into_bytes()));
    let code = if status == StageStatus::Pass {
        0
    } else if status == StageStatus::Fail {
        10
    } else {
        11
    };
    ev.r1 = RuleEvidence {
        status,
        artifact_sha256: Hex32(sha256(&log)),
        detail_code: code,
    };
    run.record(
        "r1_layout",
        status,
        format!(
            "reason {code}; diagnostic digest {:?}",
            ev.r1.artifact_sha256
        ),
        start,
    );
    if code != 0 {
        return Some((1, code));
    }
    run.begin("r2_solvency");
    let start = Instant::now();
    let (status, code, log) = sandbox
        .solvency(m, &b)
        .unwrap_or_else(|e| (StageStatus::Incomplete, 23, e.into_bytes()));
    ev.r2 = RuleEvidence {
        status,
        artifact_sha256: Hex32(sha256(&log)),
        detail_code: code,
    };
    run.record(
        "r2_solvency",
        status,
        format!(
            "reason {code}; diagnostic digest {:?}",
            ev.r2.artifact_sha256
        ),
        start,
    );
    if code != 0 {
        return Some((2, code));
    }
    run.begin("r3_filter");
    let start = Instant::now();
    let result = crate::rules::filter(&runtime, &m.r3.forbidden_opcodes, &m.r3.forbidden_constants);
    let code = result.err().unwrap_or(0);
    let status = if code == 0 {
        StageStatus::Pass
    } else {
        StageStatus::Fail
    };
    ev.r3 = RuleEvidence {
        status,
        artifact_sha256: Hex32(sha256(&runtime)),
        detail_code: code,
    };
    run.record("r3_filter", status, format!("reason {code}"), start);
    if code != 0 {
        return Some((3, code));
    }
    run.begin("r4_initializer");
    let start = Instant::now();
    let code = crate::rules::initializer(&p.init_calldata.0, &m.r4)
        .err()
        .unwrap_or(0);
    let status = if code == 0 {
        StageStatus::Pass
    } else {
        StageStatus::Fail
    };
    ev.r4 = RuleEvidence {
        status,
        artifact_sha256: Hex32(sha256(&p.init_calldata.0)),
        detail_code: code,
    };
    run.record("r4_initializer", status, format!("reason {code}"), start);
    if code != 0 {
        return Some((4, code));
    }
    None
}
