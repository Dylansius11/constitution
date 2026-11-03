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
fn evidence(p: &LocalProposal, m: &RuleSetManifest, archive_provider: &str) -> EvidenceManifest {
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
        archive_provider: archive_provider.into(),
    }
}
#[allow(clippy::too_many_arguments)]
pub fn execute(
    repo: &Path,
    m: &RuleSetManifest,
    identity: &Identity,
    p: &LocalProposal,
    sealed: &[u8],
    attempt: u32,
    archive_provider: &str,
    archive_rpc: Option<&str>,
) -> Run {
    execute_observed(
        repo,
        m,
        identity,
        p,
        sealed,
        attempt,
        archive_provider,
        archive_rpc,
        &crate::ai::LocalSidecar,
        None,
    )
}
#[allow(clippy::too_many_arguments)]
pub fn execute_observed(
    repo: &Path,
    m: &RuleSetManifest,
    identity: &Identity,
    p: &LocalProposal,
    sealed: &[u8],
    attempt: u32,
    archive_provider: &str,
    archive_rpc: Option<&str>,
    ai_transport: &dyn crate::ai::Transport,
    observer: Option<Observer>,
) -> Run {
    let start = Instant::now();
    let mut run = Run::queued();
    run.observer = observer;
    let mut ev = evidence(p, m, archive_provider);
    run.record("queued", StageStatus::Pass, "run started", Instant::now());
    let mut ai_review = AiReview::NotRun;
    let rejection = run_stages(
        repo,
        m,
        identity,
        p,
        sealed,
        archive_rpc,
        ai_transport,
        &mut run,
        &mut ev,
        &mut ai_review,
    );
    if rejection.is_some() {
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
        if !run.stages.iter().any(|s| s.name == "ai_review") {
            run.record(
                "ai_review",
                StageStatus::NotRun,
                "deterministic stages did not all pass; provider near; candidates 0",
                Instant::now(),
            );
            run.record(
                "reproducing",
                StageStatus::NotRun,
                "AI not run",
                Instant::now(),
            );
        }
    }
    let verdict = rejection
        .map(|(rule_id, reason_code)| Verdict::Reject {
            rule_id,
            reason_code,
        })
        .or_else(|| {
            matches!(
                ai_review,
                AiReview::Clean {
                    signature_kind: 1,
                    ..
                }
            )
            .then_some(Verdict::Pass)
        });
    if let Some(verdict) = verdict {
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
            verifier_release_hash: identity.release_hash(),
            evidence_block_number: p.evidence_block_number,
            evidence_block_hash: p.evidence_block_hash.0,
            evidence_root: [0; 32],
            ai: ai_review,
            verdict,
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
        match identity.sign_certificate(&cert) {
            Ok((signature, bytes)) => {
                run.verdict = Some(cert.verdict.clone());
                run.certificate_b64 = Some(Base64Bytes(bytes));
                run.signature_b64 = Some(Base64(signature));
                run.evidence_root = Some(Hex32(root));
                run.outcome = match cert.verdict {
                    Verdict::Pass => "certified Pass (attested identity; independent appraisal required; not submitted)",
                    Verdict::Reject { .. } if identity.release_hash() == [0;32] => "rejected (local, not attested; not submitted)",
                    Verdict::Reject { .. } => "rejected (attested identity; not submitted)",
                }.into();
                run.record(
                    "signing",
                    StageStatus::Pass,
                    "ephemeral Ed25519 certificate",
                    Instant::now(),
                );
            }
            Err(e) => {
                run.outcome = format!("withheld: {e}");
                run.evidence_root = Some(Hex32(root));
                run.record("signing", StageStatus::Incomplete, e, Instant::now());
            }
        }
    } else {
        run.outcome = "withheld: AI review incomplete; no Pass without provider_tee Clean".into();
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
    archive_rpc: Option<&str>,
    ai_transport: &dyn crate::ai::Transport,
    run: &mut Run,
    ev: &mut EvidenceManifest,
    ai_review: &mut AiReview,
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
            "current implementation changed",
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
    let current = match sandbox.runtime(&m.reference.contract_name) {
        Ok(current) if keccak(&current) == p.current_impl_codehash.0 => current,
        _ => {
            run.record(
                "compiling",
                StageStatus::Fail,
                "current source runtime mismatch",
                start,
            );
            return Some((0, 4));
        }
    };
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
    run.begin("r5_replay");
    let start = Instant::now();
    let (evidence, detail) = replay_evidence(repo, m, p, archive_rpc, &current, &runtime);
    let code = evidence.detail_code;
    run.record("r5_replay", evidence.status, detail, start);
    ev.r5 = evidence;
    if code != 0 {
        return Some((5, code));
    }
    run.begin("ai_review");
    let start = Instant::now();
    let report = ai_evidence(
        repo,
        m,
        p,
        &b,
        archive_rpc,
        &current,
        &runtime,
        ai_transport,
    );
    run.record("ai_review", report.status, &report.detail, start);
    run.begin("reproducing");
    run.record(
        "reproducing",
        if report.evidence.candidates.is_empty() {
            StageStatus::NotRun
        } else {
            report.status
        },
        &report.detail,
        Instant::now(),
    );
    let rejection = report.verdict().and_then(|v| match v {
        Verdict::Reject {
            rule_id,
            reason_code,
        } => Some((rule_id, reason_code)),
        Verdict::Pass => None,
    });
    ev.ai = report.evidence;
    *ai_review = report.review;
    rejection
}

#[allow(clippy::too_many_arguments)]
fn ai_evidence(
    repo: &Path,
    m: &RuleSetManifest,
    p: &LocalProposal,
    bundle: &constitution_types::bundle::SourceBundle,
    archive_rpc: Option<&str>,
    current: &[u8],
    candidate: &[u8],
    ai_transport: &dyn crate::ai::Transport,
) -> crate::ai::Report {
    use crate::ai::{Report, ReviewRequest, SourceFile};
    if m.ai.model != crate::ai::MODEL
        || m.ai.signature_kind != "provider_tee"
        || !(1..=16).contains(&m.ai.max_candidates)
        || !(1..=300).contains(&m.ai.timeout_s)
    {
        return crate::ai::Report::incomplete("AI incomplete; unsupported committed policy");
    }
    use crypto_box::aead::rand_core::{OsRng, RngCore};
    let files = match bundle
        .files
        .iter()
        .map(|(path, bytes)| {
            String::from_utf8(bytes.clone()).map(|content| SourceFile {
                path: path.clone(),
                content,
            })
        })
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(files) => files,
        Err(_) => return Report::incomplete("AI incomplete; source is not UTF-8"),
    };
    let mut nonce = [0; 32];
    OsRng.fill_bytes(&mut nonce);
    let request = ReviewRequest {
        files,
        rules: vec![
            "R1: append-only storage layout".into(),
            "R2: totalDeposits() <= token.balanceOf(proxy); only deposit(uint256) and withdraw(uint256); replay every candidate locally".into(),
            "R3: no forbidden opcodes or proxy-slot constants".into(),
            "R4: committed initializer only".into(),
            "R5: committed differential replay corpus".into(),
        ],
        nonce: hex::encode(nonce),
        max_candidates: m.ai.max_candidates,
        timeout_s: m.ai.timeout_s,
    };
    let corpus_bytes = match std::fs::read(repo.join("rules/r5/corpus.json")) {
        Ok(bytes) => bytes,
        Err(_) => return Report::incomplete("AI incomplete; committed corpus unavailable"),
    };
    let corpus =
        match crate::replay::Corpus::from_json(&corpus_bytes, m.r5.max_txs, m.r5.confirmations) {
            Ok(corpus) if corpus.sha256 == m.r5.corpus_sha256.0 => corpus,
            _ => return Report::incomplete("AI incomplete; committed corpus invalid"),
        };
    crate::ai::review(ai_transport, &request, |sequence| {
        let calls =
            crate::ai::candidate_corpus(sequence, corpus.calls[0].from, m.r5.confirmations)?;
        let witness = crate::ai::fetch_candidate_witness(
            archive_rpc.ok_or("AI replay archive unavailable")?,
            crate::replay::EvidenceBlock {
                number: p.evidence_block_number,
                hash: p.evidence_block_hash.0.into(),
            },
            &calls,
            p.proxy.0.into(),
            p.new_impl.0.into(),
            p.new_impl_codehash.0.into(),
            &p.init_calldata.0,
            corpus.calls[0].from,
        )
        .map_err(|e| e.to_string())?;
        crate::ai::reproduce(
            &witness,
            &calls,
            current,
            candidate,
            &p.init_calldata.0,
            corpus.calls[0].from,
        )
    })
}

fn replay_evidence(
    repo: &Path,
    m: &RuleSetManifest,
    p: &LocalProposal,
    archive_rpc: Option<&str>,
    current: &[u8],
    candidate: &[u8],
) -> (RuleEvidence, String) {
    use crate::replay::{Corpus, EvidenceBlock, fetch_witness, replay};
    let result = (|| -> Result<(StageStatus, u16, [u8; 32]), String> {
        let rpc = archive_rpc.ok_or("no archive RPC configured")?;
        let bytes = std::fs::read(repo.join("rules/r5/corpus.json"))
            .map_err(|_| "cannot read committed R5 corpus")?;
        let corpus = Corpus::from_json(&bytes, m.r5.max_txs, m.r5.confirmations)
            .map_err(|e| e.to_string())?;
        if corpus.sha256 != m.r5.corpus_sha256.0 {
            return Err("R5 corpus commitment mismatch".into());
        }
        let witness = fetch_witness(
            rpc,
            EvidenceBlock {
                number: p.evidence_block_number,
                hash: p.evidence_block_hash.0.into(),
            },
            &corpus,
            p.proxy.0.into(),
            p.new_impl.0.into(),
            p.new_impl_codehash.0.into(),
            &p.init_calldata.0,
        )
        .map_err(|e| e.to_string())?;
        let (status, code) = replay(&witness, &corpus, current, candidate, &p.init_calldata.0);
        if status == StageStatus::Incomplete {
            return Err("differential replay incomplete".into());
        }
        let bytes = serde_json::to_vec(&witness).map_err(|_| "cannot encode R5 witness")?;
        Ok((status, code, sha256(&bytes)))
    })();
    match result {
        Ok((status, detail_code, hash)) => (
            RuleEvidence {
                status,
                detail_code,
                artifact_sha256: Hex32(hash),
            },
            format!("reason {detail_code}"),
        ),
        Err(error) => (
            RuleEvidence {
                status: StageStatus::Incomplete,
                detail_code: 51,
                artifact_sha256: Hex32(sha256(error.as_bytes())),
            },
            format!("reason 51; {error}"),
        ),
    }
}

#[cfg(test)]
mod replay_stage_tests {
    use super::*;

    fn manifest() -> RuleSetManifest {
        serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap()
    }
    #[test]
    fn committed_replay_policy_matches_corpus() {
        let m = manifest();
        constitution_types::manifest::validate_rule_set(&m).unwrap();
        let corpus = crate::replay::Corpus::from_json(
            include_bytes!("../../../rules/r5/corpus.json"),
            m.r5.max_txs,
            m.r5.confirmations,
        )
        .unwrap();
        assert_eq!(corpus.sha256, m.r5.corpus_sha256.0);
    }
    fn proposal() -> LocalProposal {
        LocalProposal {
            gate: "local.testnet".parse().unwrap(),
            proposal_id: 0,
            chain_id: 84532,
            proxy: Hex20([1; 20]),
            executor: Hex20([2; 20]),
            expected_current_impl: Hex20([3; 20]),
            observed_current_impl: Hex20([3; 20]),
            current_impl_codehash: Hex32([4; 32]),
            new_impl: Hex20([5; 20]),
            new_impl_codehash: Hex32([6; 32]),
            init_calldata: HexBytes(vec![]),
            source_commitment: Hex32([7; 32]),
            rule_set_hash: Hex32([8; 32]),
            evidence_block_number: 1,
            evidence_block_hash: Hex32([9; 32]),
            expires_at_s: 1,
        }
    }
    fn assert_incomplete(repo: &Path, m: &RuleSetManifest, rpc: Option<&str>, expected: &str) {
        let (evidence, detail) = replay_evidence(repo, m, &proposal(), rpc, &[], &[]);
        assert_eq!(evidence.status, StageStatus::Incomplete);
        assert_eq!(evidence.detail_code, 51);
        assert_eq!(detail, format!("reason 51; {expected}"));
        assert_eq!(evidence.artifact_sha256, Hex32(sha256(expected.as_bytes())));
    }
    #[test]
    fn replay_stage_requires_rpc_and_readable_committed_corpus() {
        let repo = tempfile::tempdir().unwrap();
        let m = manifest();
        assert_incomplete(repo.path(), &m, None, "no archive RPC configured");
        assert_incomplete(
            repo.path(),
            &m,
            Some("http://unused.invalid"),
            "cannot read committed R5 corpus",
        );
        std::fs::create_dir_all(repo.path().join("rules/r5")).unwrap();
        std::fs::write(repo.path().join("rules/r5/corpus.json"), b"not JSON").unwrap();
        assert_incomplete(
            repo.path(),
            &m,
            Some("http://unused.invalid"),
            "invalid corpus JSON",
        );
    }
    #[test]
    fn replay_stage_refuses_wrong_commitment_before_any_rpc() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut m = manifest();
        m.r5.max_txs = 7;
        m.r5.corpus_sha256 = Hex32([0; 32]);
        assert_incomplete(
            &repo,
            &m,
            Some("http://unused.invalid"),
            "R5 corpus commitment mismatch",
        );
    }
    #[test]
    fn replay_stage_rpc_failure_never_exposes_endpoint_credentials() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut m = manifest();
        m.r5.max_txs = 7;
        m.r5.corpus_sha256 = Hex32(sha256(include_bytes!("../../../rules/r5/corpus.json")));
        assert_incomplete(
            &repo,
            &m,
            Some("http://relay:password@127.0.0.1:0/secret-key"),
            "eth_chainId: transport failure",
        );
    }
}
