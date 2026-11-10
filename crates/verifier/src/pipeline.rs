use crate::{
    bundle,
    identity::Identity,
    keccak,
    proposals::LocalProposal,
    sha256,
    tools::{DockerBackend, ToolBackend},
};
use constitution_types::{manifest::*, *};
use std::{
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
/// Whole-run wall clock covering sandbox stages, replay RPCs and AI candidates. It stays
/// well below the gate's proposal TTL and the 2 h default so a stalled provider cannot pin
/// the single worker; exceeding it ends the run `failed` with `infrastructure_timeout`.
pub const RUN_DEADLINE: Duration = Duration::from_secs(30 * 60);
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
    pub error_code: Option<String>,
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
    /// Set when the honest canary also failed: the failure belongs to the verifier
    /// environment, not to the proposal.
    #[serde(skip)]
    pub(crate) canary_failed: bool,
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
            error_code: None,
            verdict: None,
            certificate_b64: None,
            signature_b64: None,
            evidence_root: None,
            elapsed_ms: 0,
            evidence: None,
            canary_failed: false,
            observer: None,
        }
    }
    pub(crate) fn fail(&mut self, code: &str) {
        self.error_code = Some(code.into());
        self.outcome = "failed; no certificate or on-chain verdict; retry permitted".into();
        self.verdict = None;
        self.certificate_b64 = None;
        self.signature_b64 = None;
        self.record("failed", StageStatus::Incomplete, code, Instant::now());
        self.observer = None;
    }
    pub(crate) fn interrupt_submission(&mut self) {
        // A sender panic cannot prove a transaction was not broadcast.
        self.outcome = "submission incomplete; certificate retained".into();
        self.record(
            "submit",
            StageStatus::Incomplete,
            "worker_panic; inspect relay account transactions before starting another run",
            Instant::now(),
        );
        self.record(
            "done",
            StageStatus::Incomplete,
            "submission outcome unknown",
            Instant::now(),
        );
        self.observer = None;
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
/// Known-honest fixture that tells a verifier-environment failure from a proposal-caused one.
/// It is read from the repository (the same files the sandbox tests use) and run through the
/// same tool stage that just failed.
#[derive(Clone, Debug)]
pub struct CanaryPolicy {
    /// `path/File.sol:Contract` of the honest candidate inside `files`.
    pub contract: String,
    /// Repository paths under `contracts/` forming the canary bundle (reference included).
    pub files: Vec<String>,
    /// The canary runs once a proposal has had this many failed runs including the current one.
    pub after_failed_runs: u32,
}
impl Default for CanaryPolicy {
    fn default() -> Self {
        Self {
            contract: "src/demo/VaultV2Honest.sol:VaultV2Honest".into(),
            files: [
                "src/demo/VaultV1.sol",
                "src/demo/VaultV2Honest.sol",
                "src/demo/MockToken.sol",
            ]
            .map(String::from)
            .into(),
            after_failed_runs: 2,
        }
    }
}
impl CanaryPolicy {
    fn bundle(&self, repo: &Path) -> Result<constitution_types::bundle::SourceBundle, String> {
        let files = self
            .files
            .iter()
            .map(|name| {
                std::fs::read(repo.join("contracts").join(name))
                    .map(|bytes| (name.clone(), bytes))
                    .map_err(|_| "canary fixture unreadable".to_owned())
            })
            .collect::<Result<_, _>>()?;
        Ok(constitution_types::bundle::SourceBundle {
            contract_name: self.contract.clone(),
            files,
            foundry_profile: "default".into(),
        })
    }
}
/// Failure codes the proposal's own source can cause (wall timeout or OOM, solver exception,
/// unparseable tool output) and therefore the only ones that count toward a cool-down or
/// trigger the canary.
pub fn proposal_attributable(code: &str) -> bool {
    matches!(code, "infrastructure_timeout" | "tool_output_invalid")
}
#[derive(Default)]
pub struct CanaryHealth(std::sync::Mutex<[Option<Instant>; 3]>);
impl CanaryHealth {
    fn fresh(&self, stage: SandboxStage) -> bool {
        self.0.lock().unwrap_or_else(|e| e.into_inner())[stage as usize]
            .is_some_and(|at| at.elapsed() <= Duration::from_secs(30 * 60))
    }
    fn record(&self, stage: SandboxStage, healthy: bool) {
        self.0.lock().unwrap_or_else(|e| e.into_inner())[stage as usize] =
            healthy.then(Instant::now);
    }
}
/// Everything one run needs besides the proposal and the sealed bundle.
pub struct RunContext<'a> {
    pub archive_provider: &'a str,
    pub archive_rpc: Option<&'a str>,
    pub proof_rpc: Option<&'a str>,
    pub ai_transport: &'a dyn crate::ai::Transport,
    pub tools: &'a dyn ToolBackend,
    pub canary: &'a CanaryPolicy,
    pub canary_health: &'a CanaryHealth,
    /// Earlier failed runs of this proposal that count toward the canary rule.
    pub prior_failed_runs: u32,
    pub deadline: Instant,
    pub observer: Option<Observer>,
}
impl RunContext<'_> {
    fn expired(&self) -> bool {
        Instant::now() >= self.deadline
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
    proof_rpc: Option<&str>,
) -> Run {
    execute_observed(
        repo,
        m,
        identity,
        p,
        sealed,
        attempt,
        RunContext {
            archive_provider,
            archive_rpc,
            proof_rpc,
            ai_transport: &crate::ai::LocalSidecar,
            tools: &DockerBackend,
            canary: &CanaryPolicy::default(),
            canary_health: &CanaryHealth::default(),
            prior_failed_runs: 0,
            deadline: Instant::now() + RUN_DEADLINE,
            observer: None,
        },
    )
}
pub fn execute_observed(
    repo: &Path,
    m: &RuleSetManifest,
    identity: &Identity,
    p: &LocalProposal,
    sealed: &[u8],
    attempt: u32,
    ctx: RunContext<'_>,
) -> Run {
    let start = Instant::now();
    let mut run = Run::queued();
    run.observer = ctx.observer.clone();
    let mut ev = evidence(p, m, ctx.archive_provider);
    run.record("queued", StageStatus::Pass, "run started", Instant::now());
    let mut ai_review = AiReview::NotRun;
    let result = run_stages(
        repo,
        m,
        identity,
        p,
        sealed,
        &ctx,
        &mut run,
        &mut ev,
        &mut ai_review,
    );
    if ctx.expired() {
        run.elapsed_ms = start.elapsed().as_millis() as u64;
        run.evidence = Some(ev);
        run.fail("infrastructure_timeout");
        return run;
    }
    let rejection = match result {
        Ok(rejection) => rejection,
        Err(code) => {
            run.elapsed_ms = start.elapsed().as_millis() as u64;
            run.evidence = Some(ev);
            run.fail(code);
            return run;
        }
    };
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
        if ctx.expired() {
            run.elapsed_ms = start.elapsed().as_millis() as u64;
            run.evidence = Some(ev);
            run.fail("infrastructure_timeout");
            return run;
        }
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
#[derive(Clone, Copy, Debug)]
enum SandboxStage {
    Compile,
    Layout,
    Solvency,
}
impl SandboxStage {
    fn name(self) -> &'static str {
        match self {
            Self::Compile => "compiling",
            Self::Layout => "r1_layout",
            Self::Solvency => "r2_solvency",
        }
    }
    /// The signed Reject a proposal-attributable failure of this stage becomes.
    fn rejection(self) -> (u8, u16) {
        match self {
            Self::Compile => (0, 2),
            Self::Layout => (1, 11),
            Self::Solvency => (2, 23),
        }
    }
}
fn infra(error: &str, fallback: &'static str) -> &'static str {
    crate::tools::infrastructure_error(error).unwrap_or(fallback)
}
fn deadline_check(ctx: &RunContext, run: &mut Run, stage: &str) -> Result<(), &'static str> {
    if ctx.expired() {
        run.record(
            stage,
            StageStatus::Incomplete,
            "infrastructure_timeout; run deadline exceeded",
            Instant::now(),
        );
        return Err("infrastructure_timeout");
    }
    Ok(())
}
/// Run the honest fixture through the stage that just failed. Completing within bounds
/// shows that the verifier environment works, so the failure belongs to the proposal.
fn canary_completes(
    ctx: &RunContext,
    repo: &Path,
    m: &RuleSetManifest,
    stage: SandboxStage,
    run: &mut Run,
) -> bool {
    run.begin("canary");
    let start = Instant::now();
    let completed = !ctx.expired()
        && (|| {
            let bundle = ctx.canary.bundle(repo).ok()?;
            let tools = ctx.tools.open(repo, &bundle, m).ok()?;
            tools.compile().ok()?;
            if ctx.expired() {
                return None;
            }
            Some(match stage {
                SandboxStage::Compile => tools.runtime(&bundle.contract_name).is_ok(),
                SandboxStage::Layout => matches!(
                    tools.layout(
                        &bundle.contract_name,
                        &m.reference.contract_name,
                        &m.r1.unsafe_allow,
                    ),
                    Ok((StageStatus::Pass, _))
                ),
                SandboxStage::Solvency => {
                    matches!(tools.solvency(m, &bundle), Ok((StageStatus::Pass, 0, _)))
                }
            })
        })()
        .unwrap_or(false);
    ctx.canary_health.record(stage, completed);
    run.record(
        "canary",
        if completed {
            StageStatus::Pass
        } else {
            StageStatus::Fail
        },
        format!(
            "honest fixture through {} {}",
            stage.name(),
            if completed {
                "completed within bounds"
            } else {
                "did not complete"
            }
        ),
        start,
    );
    completed
}
/// A sandbox stage failed with infrastructure code `code`. After enough failed runs the
/// canary decides who owns the failure: it completing makes the failure the proposal's
/// (a signed Reject); otherwise, or before the canary is due, the run stays `failed`.
#[allow(clippy::too_many_arguments)]
fn attribute_failure(
    ctx: &RunContext,
    repo: &Path,
    m: &RuleSetManifest,
    stage: SandboxStage,
    code: &'static str,
    run: &mut Run,
    start: Instant,
    before: bool,
) -> Result<(u8, u16), &'static str> {
    let due = proposal_attributable(code)
        && ctx.prior_failed_runs.saturating_add(1) >= ctx.canary.after_failed_runs;
    let after = proposal_attributable(code) && canary_completes(ctx, repo, m, stage, run);
    if due && before && after {
        run.record(
            stage.name(),
            StageStatus::Incomplete,
            format!("{code}; attributed to the proposal: the honest canary completed"),
            start,
        );
        return Ok(stage.rejection());
    }
    run.canary_failed |= proposal_attributable(code) && !after;
    run.record(stage.name(), StageStatus::Incomplete, code, start);
    Err(code)
}
#[allow(clippy::too_many_arguments)]
fn run_stages(
    repo: &Path,
    m: &RuleSetManifest,
    identity: &Identity,
    p: &LocalProposal,
    sealed: &[u8],
    ctx: &RunContext,
    run: &mut Run,
    ev: &mut EvidenceManifest,
    ai_review: &mut AiReview,
) -> Result<Option<(u8, u16)>, &'static str> {
    run.begin("unsealing");
    let start = Instant::now();
    let (b, bytes) = match bundle::open(&identity.transport, sealed) {
        Ok(b) => b,
        Err(_) => {
            run.record("unsealing", StageStatus::Incomplete, "bad_bundle", start);
            return Err("bad_bundle");
        }
    };
    if sha256(&bytes) != p.source_commitment.0 {
        run.record(
            "unsealing",
            StageStatus::Fail,
            "source commitment mismatch",
            start,
        );
        return Err("bad_bundle");
    }
    run.record(
        "unsealing",
        StageStatus::Pass,
        "source commitment verified",
        start,
    );
    run.begin("compiling");
    let start = Instant::now();
    deadline_check(ctx, run, "compiling")?;
    if p.observed_current_impl != p.expected_current_impl {
        run.record(
            "compiling",
            StageStatus::Fail,
            "current implementation changed",
            start,
        );
        return Ok(Some((0, 3)));
    }
    if let Err(error) = bundle::validate_sources(&b, m) {
        run.record("compiling", StageStatus::Fail, error, start);
        return Ok(Some((0, 4)));
    }
    let compile_before = ctx.canary_health.fresh(SandboxStage::Compile);
    let tools = match ctx.tools.open(repo, &b, m) {
        Ok(s) => s,
        Err(e) => {
            let code = infra(&e, "storage_error");
            run.record("compiling", StageStatus::Incomplete, code, start);
            return Err(code);
        }
    };
    if let Err(e) = tools.compile() {
        if let Some(code) = crate::tools::infrastructure_error(&e) {
            return attribute_failure(
                ctx,
                repo,
                m,
                SandboxStage::Compile,
                code,
                run,
                start,
                compile_before,
            )
            .map(Some);
        }
        if !canary_completes(ctx, repo, m, SandboxStage::Compile, run) {
            run.canary_failed = true;
            return Err("sandbox_unavailable");
        }
        run.record(
            "compiling",
            StageStatus::Incomplete,
            format!(
                "compiler failed; diagnostic digest {:?}",
                Hex32(sha256(e.as_bytes()))
            ),
            start,
        );
        return Ok(Some((0, 2)));
    }
    let runtime = match tools.runtime(&b.contract_name) {
        Ok(v) => v,
        Err(e) => {
            if let Some(code) = crate::tools::infrastructure_error(&e) {
                return attribute_failure(
                    ctx,
                    repo,
                    m,
                    SandboxStage::Compile,
                    code,
                    run,
                    start,
                    compile_before,
                )
                .map(Some);
            }
            run.record(
                "compiling",
                StageStatus::Fail,
                "candidate runtime unavailable",
                start,
            );
            return Ok(Some((0, 2)));
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
        return Ok(Some((0, 2)));
    }
    let current = match tools.runtime(&m.reference.contract_name) {
        Ok(current) if keccak(&current) == p.current_impl_codehash.0 => current,
        Err(e) if crate::tools::infrastructure_error(&e).is_some() => {
            let code = infra(&e, "tool_unavailable");
            return attribute_failure(
                ctx,
                repo,
                m,
                SandboxStage::Compile,
                code,
                run,
                start,
                compile_before,
            )
            .map(Some);
        }
        _ => {
            run.record(
                "compiling",
                StageStatus::Fail,
                "current source runtime mismatch",
                start,
            );
            return Ok(Some((0, 4)));
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
    deadline_check(ctx, run, "r1_layout")?;
    let layout_before = ctx.canary_health.fresh(SandboxStage::Layout);
    let (status, log) = match tools.layout(
        &b.contract_name,
        &m.reference.contract_name,
        &m.r1.unsafe_allow,
    ) {
        Ok(result) => result,
        Err(e) => {
            let code = infra(&e, "tool_unavailable");
            let reject = attribute_failure(
                ctx,
                repo,
                m,
                SandboxStage::Layout,
                code,
                run,
                start,
                layout_before,
            )?;
            ev.r1 = RuleEvidence {
                status: StageStatus::Incomplete,
                artifact_sha256: Hex32(sha256(code.as_bytes())),
                detail_code: reject.1,
            };
            return Ok(Some(reject));
        }
    };
    if status == StageStatus::Incomplete {
        return Err("tool_unavailable");
    }
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
        return Ok(Some((1, code)));
    }
    run.begin("r2_solvency");
    let start = Instant::now();
    deadline_check(ctx, run, "r2_solvency")?;
    let solvency_before = ctx.canary_health.fresh(SandboxStage::Solvency);
    let (status, code, log) = match tools.solvency(m, &b) {
        Ok(result) => result,
        Err(e) => {
            let code = infra(&e, "tool_unavailable");
            let reject = attribute_failure(
                ctx,
                repo,
                m,
                SandboxStage::Solvency,
                code,
                run,
                start,
                solvency_before,
            )?;
            ev.r2 = RuleEvidence {
                status: StageStatus::Incomplete,
                artifact_sha256: Hex32(sha256(code.as_bytes())),
                detail_code: reject.1,
            };
            return Ok(Some(reject));
        }
    };
    // A solver timeout or unknown depends on host load as much as on the proposal: it may
    // only become a terminal Reject if the honest fixture still completes the same stage.
    if status == StageStatus::Incomplete {
        let after = canary_completes(ctx, repo, m, SandboxStage::Solvency, run);
        if !solvency_before || !after {
            run.canary_failed = !after;
            run.record("r2_solvency", StageStatus::Incomplete,
                "infrastructure_timeout; solver incomplete without healthy canaries before and after",
                start);
            return Err("infrastructure_timeout");
        }
    }
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
        return Ok(Some((2, code)));
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
        return Ok(Some((3, code)));
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
        return Ok(Some((4, code)));
    }
    run.begin("r5_replay");
    let start = Instant::now();
    deadline_check(ctx, run, "r5_replay")?;
    match replay_stage(repo, m, p, ctx, &current, &runtime) {
        ReplayOutcome::Fault(code) => {
            run.record("r5_replay", StageStatus::Incomplete, code, start);
            return Err(code);
        }
        ReplayOutcome::DetailedFault(code, detail) => {
            run.record("r5_replay", StageStatus::Incomplete, detail, start);
            return Err(code);
        }
        ReplayOutcome::Evidence(evidence, detail) => {
            let code = evidence.detail_code;
            run.record("r5_replay", evidence.status, detail, start);
            ev.r5 = evidence;
            if code != 0 {
                return Ok(Some((5, code)));
            }
        }
    }
    run.begin("ai_review");
    let start = Instant::now();
    let report = ai_evidence(repo, m, p, &b, ctx, &current, &runtime);
    run.record("ai_review", report.status, &report.detail, start);
    deadline_check(ctx, run, "ai_review")?;
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
    Ok(rejection)
}

fn ai_evidence(
    repo: &Path,
    m: &RuleSetManifest,
    p: &LocalProposal,
    bundle: &constitution_types::bundle::SourceBundle,
    ctx: &RunContext,
    current: &[u8],
    candidate: &[u8],
) -> crate::ai::Report {
    use crate::ai::{Report, ReviewRequest, SourceFile};
    if m.ai.model != crate::ai::MODEL
        || m.ai.signature_kind != "provider_tee"
        || !(1..=16).contains(&m.ai.max_candidates)
        || !(1..=600).contains(&m.ai.timeout_s)
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
    crate::ai::review(ctx.ai_transport, &request, |sequence| {
        if ctx.expired() {
            return Err("run deadline exceeded".into());
        }
        let calls =
            crate::ai::candidate_corpus(sequence, corpus.calls[0].from, m.r5.confirmations)?;
        let witness = crate::ai::fetch_candidate_witness(
            ctx.archive_rpc.ok_or("AI replay archive unavailable")?,
            ctx.proof_rpc
                .ok_or("AI replay proof provider unavailable")?,
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
            ctx.deadline,
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

/// R5 outcome: a replay verdict, or a fault of the verifier or its provider (no certificate).
enum ReplayOutcome {
    Evidence(RuleEvidence, String),
    Fault(&'static str),
    DetailedFault(&'static str, String),
}
/// One witness fetch plus pure replay.
enum ReplayAttempt {
    Done(StageStatus, u16, [u8; 32]),
    Incomplete(crate::replay::Incomplete),
    Unencodable,
}
/// Provider lag, null results, unverifiable provider data and transport faults are faults.
/// Proposal-attributable execution failures over proof-checked state are rechecked before
/// they may reject; both observations must report the same contradiction.
fn replay_stage(
    repo: &Path,
    m: &RuleSetManifest,
    p: &LocalProposal,
    ctx: &RunContext,
    current: &[u8],
    candidate: &[u8],
) -> ReplayOutcome {
    use crate::replay::{Corpus, EvidenceBlock, IncompleteKind, fetch_witness, replay_detailed};
    let Some(rpc) = ctx.archive_rpc else {
        return ReplayOutcome::DetailedFault("rpc_unavailable", "archive_rpc_missing".into());
    };
    let Some(proof_rpc) = ctx.proof_rpc else {
        return ReplayOutcome::DetailedFault("rpc_unavailable", "proof_rpc_missing".into());
    };
    let Ok(bytes) = std::fs::read(repo.join("rules/r5/corpus.json")) else {
        return ReplayOutcome::DetailedFault(
            "committed_data_unavailable",
            "corpus_read_failed".into(),
        );
    };
    let corpus = match Corpus::from_json(&bytes, m.r5.max_txs, m.r5.confirmations) {
        Ok(corpus) if corpus.sha256 == m.r5.corpus_sha256.0 => corpus,
        Ok(_) => {
            return ReplayOutcome::DetailedFault(
                "committed_data_unavailable",
                "corpus_mismatch".into(),
            );
        }
        Err(_) => {
            return ReplayOutcome::DetailedFault(
                "committed_data_unavailable",
                "corpus_invalid".into(),
            );
        }
    };
    let attempt = || {
        let witness = match fetch_witness(
            rpc,
            proof_rpc,
            EvidenceBlock {
                number: p.evidence_block_number,
                hash: p.evidence_block_hash.0.into(),
            },
            &corpus,
            p.proxy.0.into(),
            p.new_impl.0.into(),
            p.new_impl_codehash.0.into(),
            &p.init_calldata.0,
            ctx.deadline,
        ) {
            Ok(witness) => witness,
            Err(error) => return ReplayAttempt::Incomplete(error),
        };
        let (status, code) =
            match replay_detailed(&witness, &corpus, current, candidate, &p.init_calldata.0) {
                Ok(true) => (StageStatus::Pass, 0),
                Ok(false) => (StageStatus::Fail, 50),
                Err(error) => return ReplayAttempt::Incomplete(error),
            };
        match serde_json::to_vec(&witness) {
            Ok(bytes) => ReplayAttempt::Done(status, code, sha256(&bytes)),
            Err(_) => ReplayAttempt::Unencodable,
        }
    };
    let mut confirmed = None;
    for _ in 0..2 {
        match attempt() {
            ReplayAttempt::Done(status, detail_code, hash) => {
                return ReplayOutcome::Evidence(
                    RuleEvidence {
                        status,
                        detail_code,
                        artifact_sha256: Hex32(hash),
                    },
                    format!("reason {detail_code}"),
                );
            }
            ReplayAttempt::Unencodable => {
                return ReplayOutcome::DetailedFault(
                    "committed_data_unavailable",
                    "witness_serialization_failed".into(),
                );
            }
            ReplayAttempt::Incomplete(error) => {
                if ctx.expired() {
                    return ReplayOutcome::Fault("infrastructure_timeout");
                }
                if error.kind == IncompleteKind::Environment {
                    let code = if error.rpc_transport {
                        "rpc_unavailable"
                    } else {
                        "replay_unavailable"
                    };
                    return ReplayOutcome::DetailedFault(
                        code,
                        format!("{}; {}; {}", code, error.reason.code(), error.message),
                    );
                }
                if confirmed
                    .as_ref()
                    .is_some_and(|previous: &crate::replay::Incomplete| {
                        previous.message != error.message || previous.reason != error.reason
                    })
                {
                    return ReplayOutcome::DetailedFault(
                        "rpc_unavailable",
                        "contradiction_changed_between_fetches".into(),
                    );
                }
                confirmed = Some(error);
            }
        }
    }
    let error = confirmed.expect("two incomplete replay attempts");
    ReplayOutcome::Evidence(
        RuleEvidence {
            status: StageStatus::Incomplete,
            detail_code: 51,
            artifact_sha256: Hex32(sha256(error.message.as_bytes())),
        },
        format!("reason 51; {}; {}", error.reason.code(), error.message),
    )
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
    static CANARY: std::sync::LazyLock<CanaryPolicy> =
        std::sync::LazyLock::new(CanaryPolicy::default);
    static HEALTH: std::sync::LazyLock<CanaryHealth> =
        std::sync::LazyLock::new(CanaryHealth::default);
    fn context<'a>(
        rpc: Option<&'a str>,
        tools: &'a dyn ToolBackend,
        prior: u32,
        deadline: Instant,
    ) -> RunContext<'a> {
        RunContext {
            archive_provider: "local",
            archive_rpc: rpc,
            proof_rpc: rpc,
            ai_transport: &crate::ai::LocalSidecar,
            tools,
            canary: &CANARY,
            canary_health: &HEALTH,
            prior_failed_runs: prior,
            deadline,
            observer: None,
        }
    }
    fn later() -> Instant {
        Instant::now() + Duration::from_secs(600)
    }
    fn repo_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }
    fn committed_manifest() -> RuleSetManifest {
        let mut m = manifest();
        m.r5.max_txs = 7;
        m.r5.corpus_sha256 = Hex32(sha256(include_bytes!("../../../rules/r5/corpus.json")));
        m
    }
    fn outcome(
        repo: &Path,
        m: &RuleSetManifest,
        p: &LocalProposal,
        rpc: Option<&str>,
        deadline: Instant,
    ) -> ReplayOutcome {
        replay_stage(
            repo,
            m,
            p,
            &context(rpc, &DockerBackend, 0, deadline),
            &[],
            &[],
        )
    }
    fn fault(repo: &Path, m: &RuleSetManifest, rpc: Option<&str>) -> &'static str {
        match outcome(repo, m, &proposal(), rpc, later()) {
            ReplayOutcome::Fault(code) => code,
            ReplayOutcome::DetailedFault(code, _) => code,
            ReplayOutcome::Evidence(_, detail) => panic!("expected a fault, got {detail}"),
        }
    }
    #[test]
    fn replay_stage_requires_rpc_and_readable_committed_corpus() {
        let repo = tempfile::tempdir().unwrap();
        let m = manifest();
        assert_eq!(fault(repo.path(), &m, None), "rpc_unavailable");
        assert_eq!(
            fault(repo.path(), &m, Some("http://unused.invalid")),
            "committed_data_unavailable"
        );
        std::fs::create_dir_all(repo.path().join("rules/r5")).unwrap();
        std::fs::write(repo.path().join("rules/r5/corpus.json"), b"not JSON").unwrap();
        assert_eq!(
            fault(repo.path(), &m, Some("http://unused.invalid")),
            "committed_data_unavailable"
        );
    }
    #[test]
    fn replay_stage_refuses_wrong_commitment_before_any_rpc() {
        let mut m = manifest();
        m.r5.max_txs = 7;
        m.r5.corpus_sha256 = Hex32([0; 32]);
        assert_eq!(
            fault(&repo_root(), &m, Some("http://unused.invalid")),
            "committed_data_unavailable"
        );
    }
    #[test]
    fn replay_stage_rpc_failure_is_a_fault_without_endpoint_credentials() {
        let m = committed_manifest();
        assert_eq!(
            fault(
                &repo_root(),
                &m,
                Some("http://relay:password@127.0.0.1:0/secret-key")
            ),
            "rpc_unavailable"
        );
    }

    /// Minimal JSON-RPC provider on loopback; one thread per connection.
    enum Reply {
        Result(serde_json::Value),
        Null,
        Slow(Duration),
    }
    struct Provider {
        url: String,
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }
    impl Provider {
        fn count(&self, method: &str) -> usize {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|m| *m == method)
                .count()
        }
    }
    fn provider(handler: impl Fn(&str, usize) -> Reply + Send + Sync + 'static) -> Provider {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let handler = std::sync::Arc::new(handler);
        let seen = calls.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let (handler, seen) = (handler.clone(), seen.clone());
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream);
                    let mut length = 0;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                            break;
                        }
                        if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                            length = value.trim().parse().unwrap_or(0);
                        }
                    }
                    let mut body = vec![0; length];
                    if reader.read_exact(&mut body).is_err() {
                        return;
                    }
                    let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
                    let method = request["method"].as_str().unwrap().to_owned();
                    let nth = {
                        let mut seen = seen.lock().unwrap();
                        seen.push(method.clone());
                        seen.iter().filter(|m| **m == method).count()
                    };
                    let result = match handler(&method, nth) {
                        Reply::Result(value) => value,
                        Reply::Null => serde_json::Value::Null,
                        Reply::Slow(pause) => {
                            std::thread::sleep(pause);
                            serde_json::Value::Null
                        }
                    };
                    let payload =
                        serde_json::json!({"jsonrpc":"2.0","id":1,"result":result}).to_string();
                    let _ = write!(
                        reader.get_mut(),
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                        payload.len()
                    );
                });
            }
        });
        Provider { url, calls }
    }
    /// A header whose RLP hash is its own `hash`, deep enough to pass the gas bound.
    fn header(number: u64) -> (serde_json::Value, Hex32) {
        let word = |byte: u8, len: usize| format!("0x{}", hex::encode(vec![byte; len]));
        let mut header = serde_json::json!({
            "parentHash": word(1, 32), "sha3Uncles": word(2, 32), "miner": word(3, 20),
            "stateRoot": word(4, 32), "transactionsRoot": word(5, 32),
            "receiptsRoot": word(6, 32), "logsBloom": word(0, 256),
            "difficulty": "0x0", "number": format!("0x{number:x}"),
            "gasLimit": "0x1c9c380", "gasUsed": "0x0", "timestamp": "0x1",
            "extraData": "0x", "mixHash": word(7, 32), "nonce": "0x0000000000000000",
            "baseFeePerGas": "0x7", "withdrawalsRoot": word(8, 32),
            "blobGasUsed": "0x0", "excessBlobGas": "0x0",
        });
        let hash = crate::replay::header_hash(&header).unwrap();
        header["hash"] = serde_json::json!(hash);
        (header, Hex32(hash.0))
    }
    /// Provider that serves a valid header and a tip `depth` blocks above it.
    fn chain_stub(
        number: u64,
        depth: impl Fn(usize) -> u64 + Send + Sync + 'static,
        code: impl Fn(usize) -> &'static str + Send + Sync + 'static,
    ) -> (Provider, LocalProposal) {
        let (header, hash) = header(number);
        let mut p = proposal();
        p.evidence_block_number = number;
        p.evidence_block_hash = hash;
        p.new_impl_codehash = Hex32(keccak(&[0x60, 0x01]));
        let stub = provider(move |method, nth| match method {
            "eth_chainId" => Reply::Result(serde_json::json!("0x14a34")),
            "eth_getBlockByNumber" => Reply::Result(header.clone()),
            "eth_blockNumber" => {
                Reply::Result(serde_json::json!(format!("0x{:x}", number + depth(nth))))
            }
            "eth_getCode" => Reply::Result(serde_json::json!(code(nth))),
            _ => Reply::Null,
        });
        (stub, p)
    }
    #[test]
    fn lagging_provider_null_block_is_an_environment_fault_after_one_retry() {
        let stub = provider(|method, _| match method {
            "eth_chainId" => Reply::Result(serde_json::json!("0x14a34")),
            _ => Reply::Null,
        });
        let m = committed_manifest();
        assert_eq!(
            fault(&repo_root(), &m, Some(&stub.url)),
            "replay_unavailable"
        );
        assert_eq!(stub.count("eth_getBlockByNumber"), 2);
    }
    #[test]
    fn lagging_confirmations_are_environmental_not_a_witness_reject() {
        let m = committed_manifest();
        let (stub, p) = chain_stub(100, |_| 1, |_| "0x");
        let result = outcome(&repo_root(), &m, &p, Some(&stub.url), later());
        assert!(
            matches!(result, ReplayOutcome::DetailedFault("replay_unavailable", detail) if detail.contains("insufficient_confirmations"))
        );
        assert_eq!(stub.count("eth_blockNumber"), 2);
        assert_eq!(stub.count("eth_getCode"), 0);
        // A node that catches up on the second read lets the run continue to the code read.
        let (stub, p) = chain_stub(100, |nth| if nth == 1 { 1 } else { 5 }, |_| "0x");
        outcome(&repo_root(), &m, &p, Some(&stub.url), later());
        assert!(stub.count("eth_getCode") >= 1);
    }
    #[test]
    fn unauthenticated_missing_candidate_code_is_environmental() {
        let m = committed_manifest();
        let (stub, p) = chain_stub(100, |_| 5, |_| "0x");
        let result = outcome(&repo_root(), &m, &p, Some(&stub.url), later());
        assert!(
            matches!(result, ReplayOutcome::DetailedFault("replay_unavailable", detail) if detail.contains("code_mismatch"))
        );
        // A later response changing cannot turn the first unauthenticated read into a verdict.
        let (stub, p) = chain_stub(100, |_| 5, |nth| if nth == 1 { "0x" } else { "0x6001" });
        let result = outcome(&repo_root(), &m, &p, Some(&stub.url), later());
        assert!(matches!(
            result,
            ReplayOutcome::DetailedFault("replay_unavailable", _)
        ));
    }
    #[test]
    fn slow_provider_hits_the_run_deadline_not_a_witness_reject() {
        let stub = provider(|_, _| Reply::Slow(Duration::from_secs(5)));
        let m = committed_manifest();
        let started = Instant::now();
        let result = outcome(
            &repo_root(),
            &m,
            &proposal(),
            Some(&stub.url),
            Instant::now() + Duration::from_millis(400),
        );
        assert!(matches!(
            result,
            ReplayOutcome::Fault("infrastructure_timeout")
        ));
        assert!(started.elapsed() < Duration::from_secs(3));
    }
    fn sealed_fixture(
        identity: &Identity,
        tamper: bool,
    ) -> (RuleSetManifest, LocalProposal, Vec<u8>) {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let m = manifest();
        let files = m.reference.source_sha256s.keys().map(|name| {
            let bytes = if tamper && name.ends_with("/MockToken.sol") {
                b"contract MockToken { function balanceOf(address) external pure returns(uint256) { return type(uint256).max; } }".to_vec()
            } else {
                std::fs::read(repo.join("contracts").join(name)).unwrap()
            };
            (name.clone(), bytes)
        }).collect();
        let bundle = constitution_types::bundle::SourceBundle {
            contract_name: m.reference.contract_name.clone(),
            files,
            foundry_profile: "default".into(),
        };
        let bytes = borsh::to_vec(&bundle).unwrap();
        let mut p = proposal();
        p.source_commitment = Hex32(sha256(&bytes));
        p.rule_set_hash = Hex32(rule_set_hash(&m).unwrap());
        p.expires_at_s = now() + 300;
        let sealed = identity
            .transport
            .public_key()
            .seal(&mut crypto_box::aead::rand_core::OsRng, &bytes)
            .unwrap();
        (m, p, sealed)
    }
    #[test]
    fn forged_trusted_token_is_rejected_before_sandbox() {
        let identity = Identity::new();
        let (m, p, sealed) = sealed_fixture(&identity, true);
        let run = execute(
            Path::new("/absent-verifier-repository"),
            &m,
            &identity,
            &p,
            &sealed,
            1,
            "local",
            None,
            None,
        );
        assert_eq!(
            run.verdict,
            Some(Verdict::Reject {
                rule_id: 0,
                reason_code: 4
            })
        );
        assert!(run.certificate_b64.is_some());
        assert!(run.error_code.is_none());
    }
    #[test]
    fn verifier_storage_fault_never_signs_or_submits() {
        struct NoSubmit;
        impl crate::submit::VerdictSubmitter for NoSubmit {
            fn submit(
                &self,
                _: &AccountId,
                _: &Base64Bytes,
                _: &Base64<64>,
            ) -> Result<String, String> {
                panic!("fault must not submit a verdict")
            }
        }
        let identity = Identity::new();
        let (m, p, sealed) = sealed_fixture(&identity, false);
        let absent = tempfile::tempdir().unwrap();
        let run = execute(
            &absent.path().join("absent"),
            &m,
            &identity,
            &p,
            &sealed,
            1,
            "local",
            None,
            None,
        );
        assert_eq!(run.stage, "failed");
        assert_eq!(run.error_code.as_deref(), Some("storage_error"));
        assert!(run.verdict.is_none());
        assert!(run.certificate_b64.is_none());
        assert!(run.signature_b64.is_none());
        let submitted = crate::submit::SubmittedRun::submit(run, &p.gate, &NoSubmit);
        assert!(submitted.near_tx_hash.is_none());
        assert_eq!(submitted.run.stage, "failed");
    }
    #[test]
    fn pipeline_unseal_and_commitment_failures_never_sign() {
        let identity = Identity::new();
        let (m, mut p, sealed) = sealed_fixture(&identity, false);
        p.source_commitment = Hex32([0; 32]);
        for ciphertext in [&sealed[..], &[][..]] {
            let run = execute(
                Path::new("."),
                &m,
                &identity,
                &p,
                ciphertext,
                1,
                "local",
                None,
                None,
            );
            assert_eq!(run.stage, "failed");
            assert_eq!(run.error_code.as_deref(), Some("bad_bundle"));
            assert!(run.verdict.is_none());
            assert!(run.certificate_b64.is_none());
            assert!(run.signature_b64.is_none());
        }
    }
    #[derive(Clone)]
    struct Behavior {
        compile: Result<(), &'static str>,
        layout: Result<StageStatus, &'static str>,
        solvency: Result<(StageStatus, u16), &'static str>,
    }
    impl Behavior {
        fn healthy() -> Self {
            Self {
                compile: Ok(()),
                layout: Ok(StageStatus::Pass),
                solvency: Ok((StageStatus::Pass, 0)),
            }
        }
    }
    const RUNTIME: &[u8] = &[0x60, 0x01];
    struct FakeTools(Behavior);
    fn infrastructure(code: &str) -> String {
        format!("infrastructure:{code}")
    }
    impl crate::tools::Toolchain for FakeTools {
        fn compile(&self) -> Result<Vec<u8>, String> {
            self.0.compile.map(|()| b"ok".to_vec()).map_err(|code| {
                if code == "diagnostic" {
                    "ParserError: no space left on device".to_owned()
                } else {
                    infrastructure(code)
                }
            })
        }
        fn runtime(&self, _: &str) -> Result<Vec<u8>, String> {
            Ok(RUNTIME.to_vec())
        }
        fn layout(&self, _: &str, _: &str, _: &[String]) -> Result<(StageStatus, Vec<u8>), String> {
            self.0
                .layout
                .map(|status| (status, b"layout".to_vec()))
                .map_err(infrastructure)
        }
        fn solvency(
            &self,
            _: &RuleSetManifest,
            _: &constitution_types::bundle::SourceBundle,
        ) -> Result<(StageStatus, u16, Vec<u8>), String> {
            self.0
                .solvency
                .map(|(status, code)| (status, code, b"solver".to_vec()))
                .map_err(infrastructure)
        }
    }
    struct FakeBackend {
        proposal: Behavior,
        canary: Behavior,
        opened: std::sync::Mutex<Vec<String>>,
    }
    impl ToolBackend for FakeBackend {
        fn open(
            &self,
            _: &Path,
            bundle: &constitution_types::bundle::SourceBundle,
            _: &RuleSetManifest,
        ) -> Result<Box<dyn crate::tools::Toolchain>, String> {
            self.opened
                .lock()
                .unwrap()
                .push(bundle.contract_name.clone());
            let behavior = if bundle.contract_name == CANARY.contract {
                self.canary.clone()
            } else {
                self.proposal.clone()
            };
            Ok(Box::new(FakeTools(behavior)))
        }
    }
    /// Runs the real pipeline over an injected sandbox; returns the run and the bundles opened.
    fn fake_run(
        proposal: Behavior,
        canary: Behavior,
        prior_failed_runs: u32,
        deadline: Instant,
    ) -> (Run, Vec<String>) {
        let identity = Identity::new();
        let (m, mut p, sealed) = sealed_fixture(&identity, false);
        p.current_impl_codehash = Hex32(keccak(RUNTIME));
        p.new_impl_codehash = Hex32(keccak(RUNTIME));
        *HEALTH.0.lock().unwrap() = if prior_failed_runs > 0 {
            [Some(Instant::now()); 3]
        } else {
            [None; 3]
        };
        let backend = FakeBackend {
            proposal,
            canary,
            opened: Default::default(),
        };
        let run = execute_observed(
            &repo_root(),
            &m,
            &identity,
            &p,
            &sealed,
            1,
            context(None, &backend, prior_failed_runs, deadline),
        );
        (run, backend.opened.into_inner().unwrap())
    }
    fn solver_timeout() -> Behavior {
        Behavior {
            solvency: Err("infrastructure_timeout"),
            ..Behavior::healthy()
        }
    }
    fn rejected(run: &Run, rule_id: u8, reason_code: u16) {
        assert_eq!(run.stage, "done");
        assert_eq!(
            run.verdict,
            Some(Verdict::Reject {
                rule_id,
                reason_code
            })
        );
        assert!(run.certificate_b64.is_some() && run.error_code.is_none());
    }
    fn canary_stage(run: &Run) -> Option<StageStatus> {
        run.stages
            .iter()
            .rev()
            .find(|stage| stage.name == "canary")
            .map(|stage| stage.status)
    }
    #[test]
    fn cached_health_expires_and_success_path_does_not_run_canary() {
        let health = CanaryHealth::default();
        health.record(SandboxStage::Solvency, true);
        assert!(health.fresh(SandboxStage::Solvency));
        health.0.lock().unwrap()[SandboxStage::Solvency as usize] =
            Some(Instant::now() - Duration::from_secs(30 * 60 + 1));
        assert!(!health.fresh(SandboxStage::Solvency));
        let (_, opened) = fake_run(Behavior::healthy(), Behavior::healthy(), 0, later());
        assert_eq!(
            opened.len(),
            1,
            "successful tool stages must not run a canary"
        );
    }
    #[test]
    fn compiler_diagnostic_requires_a_healthy_canary() {
        let diagnostic = Behavior {
            compile: Err("diagnostic"),
            ..Behavior::healthy()
        };
        let (run, _) = fake_run(diagnostic.clone(), Behavior::healthy(), 0, later());
        rejected(&run, 0, 2);
        let broken = Behavior {
            compile: Err("tool_unavailable"),
            ..Behavior::healthy()
        };
        let (run, _) = fake_run(diagnostic, broken, 0, later());
        assert_eq!(run.stage, "failed");
        assert_eq!(run.error_code.as_deref(), Some("sandbox_unavailable"));
        assert!(run.certificate_b64.is_none());
    }
    #[test]
    fn first_proposal_timeout_records_health_for_later_runs() {
        let (run, opened) = fake_run(solver_timeout(), Behavior::healthy(), 0, later());
        assert_eq!(run.stage, "failed");
        assert_eq!(run.error_code.as_deref(), Some("infrastructure_timeout"));
        assert!(run.certificate_b64.is_none() && !run.canary_failed);
        assert_eq!(opened.len(), 2);
    }
    #[test]
    fn second_timeout_with_a_completing_canary_is_a_signed_solver_reject() {
        let (run, opened) = fake_run(solver_timeout(), Behavior::healthy(), 1, later());
        rejected(&run, 2, 23);
        assert_eq!(canary_stage(&run), Some(StageStatus::Pass));
        assert_eq!(
            opened
                .iter()
                .filter(|name| *name == &CANARY.contract)
                .count(),
            1
        );
        let evidence = run.evidence.as_ref().unwrap();
        assert_eq!(
            (evidence.r2.status, evidence.r2.detail_code),
            (StageStatus::Incomplete, 23)
        );
    }
    #[test]
    fn second_timeout_with_a_failing_canary_is_an_environment_failure() {
        let (run, _) = fake_run(solver_timeout(), solver_timeout(), 1, later());
        assert_eq!(run.stage, "failed");
        assert_eq!(run.error_code.as_deref(), Some("infrastructure_timeout"));
        assert!(run.canary_failed && run.certificate_b64.is_none());
        assert_eq!(canary_stage(&run), Some(StageStatus::Fail));
    }
    #[test]
    fn tool_output_invalid_follows_the_same_canary_rule() {
        let invalid = Behavior {
            solvency: Err("tool_output_invalid"),
            ..Behavior::healthy()
        };
        let (run, _) = fake_run(invalid.clone(), Behavior::healthy(), 0, later());
        assert_eq!(run.error_code.as_deref(), Some("tool_output_invalid"));
        let (run, _) = fake_run(invalid.clone(), Behavior::healthy(), 1, later());
        rejected(&run, 2, 23);
        let (run, _) = fake_run(invalid.clone(), invalid, 1, later());
        assert!(run.canary_failed && run.error_code.as_deref() == Some("tool_output_invalid"));
    }
    #[test]
    fn solver_incomplete_may_reject_only_if_the_honest_canary_completes() {
        let incomplete = Behavior {
            solvency: Ok((StageStatus::Incomplete, 23)),
            ..Behavior::healthy()
        };
        let (cold, _) = fake_run(incomplete.clone(), Behavior::healthy(), 0, later());
        assert_eq!(cold.stage, "failed");
        assert!(cold.certificate_b64.is_none());
        let (run, _) = fake_run(incomplete.clone(), Behavior::healthy(), 1, later());
        rejected(&run, 2, 23);
        assert_eq!(canary_stage(&run), Some(StageStatus::Pass));
        // Load made the honest fixture time out too: the solver result is environmental.
        let (run, _) = fake_run(incomplete.clone(), incomplete, 0, later());
        assert_eq!(run.stage, "failed");
        assert_eq!(run.error_code.as_deref(), Some("infrastructure_timeout"));
        assert!(run.canary_failed && run.certificate_b64.is_none());
    }
    #[test]
    fn compile_and_layout_failures_become_their_stage_rejections() {
        let compile = Behavior {
            compile: Err("infrastructure_timeout"),
            ..Behavior::healthy()
        };
        let (run, _) = fake_run(compile, Behavior::healthy(), 1, later());
        rejected(&run, 0, 2);
        let layout = Behavior {
            layout: Err("tool_output_invalid"),
            ..Behavior::healthy()
        };
        let (run, _) = fake_run(layout, Behavior::healthy(), 1, later());
        rejected(&run, 1, 11);
    }
    #[test]
    fn environment_only_failures_never_run_a_canary() {
        let broken = Behavior {
            compile: Err("sandbox_unavailable"),
            ..Behavior::healthy()
        };
        let (run, opened) = fake_run(broken, Behavior::healthy(), 9, later());
        assert_eq!(run.error_code.as_deref(), Some("sandbox_unavailable"));
        assert!(run.certificate_b64.is_none() && !run.canary_failed);
        assert_eq!(opened.len(), 1);
    }
    #[test]
    fn deadline_expiring_before_signing_never_issues_a_certificate() {
        let identity = Identity::new();
        let (m, p, sealed) = sealed_fixture(&identity, true);
        let mut ctx = context(
            None,
            &DockerBackend,
            0,
            Instant::now() + Duration::from_millis(100),
        );
        ctx.observer = Some(std::sync::Arc::new(|run| {
            if run.stage == "r1_layout" {
                std::thread::sleep(Duration::from_millis(200));
            }
        }));
        let run = execute_observed(&repo_root(), &m, &identity, &p, &sealed, 1, ctx);
        assert_eq!(run.stage, "failed");
        assert_eq!(run.error_code.as_deref(), Some("infrastructure_timeout"));
        assert!(run.certificate_b64.is_none());
        assert!(run.signature_b64.is_none());
    }
    #[test]
    fn an_expired_run_deadline_fails_before_any_tool_work() {
        let (run, opened) = fake_run(
            Behavior::healthy(),
            Behavior::healthy(),
            0,
            Instant::now() - Duration::from_secs(1),
        );
        assert_eq!(run.stage, "failed");
        assert_eq!(run.error_code.as_deref(), Some("infrastructure_timeout"));
        assert!(opened.is_empty() && run.certificate_b64.is_none());
    }
    /// Real Docker run of the default canary fixture through the solver stage (minutes).
    /// `cargo test -p constitution-verifier --lib real_docker_canary -- --ignored`
    #[test]
    #[ignore = "runs the committed depth-4 R2 harness in Docker"]
    fn real_docker_canary_completes_the_honest_fixture() {
        let m = manifest();
        let mut run = Run::queued();
        let ctx = context(None, &DockerBackend, 0, Instant::now() + RUN_DEADLINE);
        let started = Instant::now();
        for stage in [
            SandboxStage::Compile,
            SandboxStage::Layout,
            SandboxStage::Solvency,
        ] {
            assert!(
                canary_completes(&ctx, &repo_root(), &m, stage, &mut run),
                "{stage:?}"
            );
        }
        eprintln!("real canary, three stages: {:?}", started.elapsed());
    }
    #[test]
    fn deadline_expiring_between_stages_ends_failed() {
        // The deadline passes while the (slow) solver runs; the next stage must not start.
        struct Slow;
        impl ToolBackend for Slow {
            fn open(
                &self,
                _: &Path,
                _: &constitution_types::bundle::SourceBundle,
                _: &RuleSetManifest,
            ) -> Result<Box<dyn crate::tools::Toolchain>, String> {
                struct SlowTools;
                impl crate::tools::Toolchain for SlowTools {
                    fn compile(&self) -> Result<Vec<u8>, String> {
                        Ok(vec![])
                    }
                    fn runtime(&self, _: &str) -> Result<Vec<u8>, String> {
                        Ok(RUNTIME.to_vec())
                    }
                    fn layout(
                        &self,
                        _: &str,
                        _: &str,
                        _: &[String],
                    ) -> Result<(StageStatus, Vec<u8>), String> {
                        std::thread::sleep(Duration::from_millis(300));
                        Ok((StageStatus::Pass, vec![]))
                    }
                    fn solvency(
                        &self,
                        _: &RuleSetManifest,
                        _: &constitution_types::bundle::SourceBundle,
                    ) -> Result<(StageStatus, u16, Vec<u8>), String> {
                        panic!("the solver stage must not start after the deadline")
                    }
                }
                Ok(Box::new(SlowTools))
            }
        }
        let identity = Identity::new();
        let (m, mut p, sealed) = sealed_fixture(&identity, false);
        p.current_impl_codehash = Hex32(keccak(RUNTIME));
        p.new_impl_codehash = Hex32(keccak(RUNTIME));
        let backend = Slow;
        let run = execute_observed(
            &repo_root(),
            &m,
            &identity,
            &p,
            &sealed,
            1,
            context(
                None,
                &backend,
                0,
                Instant::now() + Duration::from_millis(150),
            ),
        );
        assert_eq!(run.stage, "failed");
        assert_eq!(run.error_code.as_deref(), Some("infrastructure_timeout"));
    }
    #[test]
    fn proposal_compilation_error_remains_a_signed_reject() {
        let identity = Identity::new();
        let (m, mut p, sealed) = sealed_fixture(&identity, false);
        let (mut bundle, _) = bundle::open(&identity.transport, &sealed).unwrap();
        bundle
            .files
            .push(("src/Invalid.sol".into(), b"not Solidity".to_vec()));
        let bytes = borsh::to_vec(&bundle).unwrap();
        p.source_commitment = Hex32(sha256(&bytes));
        let sealed = identity
            .transport
            .public_key()
            .seal(&mut crypto_box::aead::rand_core::OsRng, &bytes)
            .unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let run = execute(&repo, &m, &identity, &p, &sealed, 1, "local", None, None);
        assert_eq!(run.stage, "done");
        assert_eq!(
            run.verdict,
            Some(Verdict::Reject {
                rule_id: 0,
                reason_code: 2
            })
        );
        assert!(run.error_code.is_none());
        assert!(run.certificate_b64.is_some());
        assert!(run.signature_b64.is_some());
    }
}
