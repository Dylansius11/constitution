use crate::{
    identity::Identity,
    pipeline::{self, Run},
    proposals::ProposalSource,
    submit::{SubmittedRun, VerdictSubmitter},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use constitution_types::{
    AccountId, Base64Bytes,
    manifest::{RuleSetManifest, rule_set_hash},
};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};
// Admission applies even to bad bundles, before decryption or RPC.
pub const PROPOSAL_INTERVAL: Duration = Duration::from_secs(10);
pub const GLOBAL_WINDOW: Duration = Duration::from_secs(1);
pub const GLOBAL_REQUESTS: usize = 10;
pub const FINISHED_RUNS: usize = 64;
type ProposalKey = (AccountId, u64);
#[derive(Default)]
struct Admission {
    runs: BTreeMap<u64, SubmittedRun>,
    finished: VecDeque<u64>,
    next_id: u64,
    active: Option<ProposalKey>,
    attempts: BTreeMap<ProposalKey, u32>,
    proposals: BTreeMap<ProposalKey, Instant>,
    global: VecDeque<Instant>,
}
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
impl Admission {
    fn rate_limit(&mut self, key: &ProposalKey, now: Instant) -> Result<(), ApiError> {
        self.global
            .retain(|time| now.duration_since(*time) < GLOBAL_WINDOW);
        self.proposals
            .retain(|_, time| now.duration_since(*time) < PROPOSAL_INTERVAL);
        let wait = self
            .proposals
            .get(key)
            .map(|time| PROPOSAL_INTERVAL - now.duration_since(*time))
            .or_else(|| {
                (self.global.len() >= GLOBAL_REQUESTS)
                    .then(|| GLOBAL_WINDOW - now.duration_since(self.global[0]))
            });
        if let Some(wait) = wait {
            return Err(ApiError::limited(
                "rate_limited",
                wait.as_secs()
                    .saturating_add(u64::from(wait.subsec_nanos() != 0)),
            ));
        }
        self.global.push_back(now);
        self.proposals.insert(key.clone(), now);
        Ok(())
    }
    fn finish(&mut self, id: u64) {
        if !self
            .runs
            .get(&id)
            .is_some_and(|r| matches!(r.run.stage.as_str(), "done" | "failed"))
        {
            return;
        }
        self.finished.push_back(id);
        while self.finished.len() > FINISHED_RUNS {
            if let Some(oldest) = self.finished.pop_front() {
                self.runs.remove(&oldest);
            }
        }
    }
}
struct ActiveRun {
    service: Arc<Service>,
    id: Option<u64>,
}
impl Drop for ActiveRun {
    fn drop(&mut self) {
        let mut state = lock(&self.service.state);
        if let Some(id) = self.id {
            if std::thread::panicking() {
                let mut run = state
                    .runs
                    .get(&id)
                    .map(|r| r.run.snapshot())
                    .unwrap_or_else(Run::queued);
                if run.stage == "submitting" && run.certificate_b64.is_some() {
                    run.interrupt_submission();
                } else {
                    run.fail("worker_panic");
                }
                state.runs.insert(id, run.into());
            }
            state.finish(id);
        }
        state.active = None;
    }
}
#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    retry_after: Option<u64>,
}
impl ApiError {
    fn new(status: StatusCode, code: &'static str) -> Self {
        Self {
            status,
            code,
            retry_after: None,
        }
    }
    fn limited(code: &'static str, seconds: u64) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            code,
            retry_after: Some(seconds.max(1)),
        }
    }
}
impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, self.code).into_response();
        if let Some(seconds) = self.retry_after {
            response.headers_mut().insert(
                header::RETRY_AFTER,
                HeaderValue::from_str(&seconds.to_string()).expect("integer header"),
            );
        }
        response
    }
}
pub struct Service {
    pub identity: Identity,
    pub manifest: RuleSetManifest,
    pub repo: PathBuf,
    pub proposals: Box<dyn ProposalSource>,
    pub ai_transport: Box<dyn crate::ai::Transport>,
    submitter: Option<Box<dyn VerdictSubmitter>>,
    state: Mutex<Admission>,
    #[cfg(test)]
    panic_next_run: std::sync::atomic::AtomicBool,
}
impl Service {
    pub fn new(
        identity: Identity,
        manifest: RuleSetManifest,
        repo: PathBuf,
        proposals: Box<dyn ProposalSource>,
    ) -> Result<Arc<Self>, String> {
        Self::build(identity, manifest, repo, proposals, None)
    }
    pub fn with_submitter(
        identity: Identity,
        manifest: RuleSetManifest,
        repo: PathBuf,
        proposals: Box<dyn ProposalSource>,
        submitter: Box<dyn VerdictSubmitter>,
    ) -> Result<Arc<Self>, String> {
        Self::build(identity, manifest, repo, proposals, Some(submitter))
    }
    fn build(
        identity: Identity,
        manifest: RuleSetManifest,
        repo: PathBuf,
        proposals: Box<dyn ProposalSource>,
        submitter: Option<Box<dyn VerdictSubmitter>>,
    ) -> Result<Arc<Self>, String> {
        constitution_types::manifest::validate_rule_set(&manifest)?;
        Ok(Arc::new(Self {
            identity,
            manifest,
            repo,
            proposals,
            ai_transport: Box::new(crate::ai::LocalSidecar),
            submitter,
            state: Mutex::new(Admission::default()),
            #[cfg(test)]
            panic_next_run: std::sync::atomic::AtomicBool::new(false),
        }))
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartRun {
    pub gate: AccountId,
    pub proposal_id: u64,
    pub sealed_bundle_b64: Base64Bytes,
}
pub fn router(service: Arc<Service>, origin: &str) -> Result<Router, String> {
    let origin: HeaderValue = origin.parse().map_err(|_| "invalid PAGE_ORIGIN")?;
    let cors = middleware::from_fn(move |request: axum::extract::Request, next: Next| {
        let origin = origin.clone();
        async move {
            let allowed = request.headers().get(header::ORIGIN) == Some(&origin);
            let mut response = if request.method() == Method::OPTIONS {
                Response::new(axum::body::Body::empty())
            } else {
                next.run(request).await
            };
            if allowed {
                let h = response.headers_mut();
                h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
                h.insert(
                    header::ACCESS_CONTROL_ALLOW_METHODS,
                    HeaderValue::from_static("GET, POST, OPTIONS"),
                );
                h.insert(
                    header::ACCESS_CONTROL_ALLOW_HEADERS,
                    HeaderValue::from_static("content-type"),
                );
                h.insert(
                    header::ACCESS_CONTROL_EXPOSE_HEADERS,
                    HeaderValue::from_static("Retry-After"),
                );
                h.insert(header::VARY, HeaderValue::from_static("Origin"));
            }
            response
        }
    });
    Ok(Router::new()
        .route("/v1/identity", get(identity))
        .route("/v1/runs", post(start))
        .route("/v1/runs/{id}", get(status))
        .route("/v1/runs/{id}/evidence", get(evidence))
        .layer(DefaultBodyLimit::max(3 * 1024 * 1024))
        .layer(cors)
        .with_state(service))
}
async fn identity(State(s): State<Arc<Service>>) -> Json<crate::identity::IdentityView> {
    Json(
        s.identity
            .view(rule_set_hash(&s.manifest).expect("manifest serialize")),
    )
}
async fn start(
    State(s): State<Arc<Service>>,
    Json(request): Json<StartRun>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    if s.proposals
        .gate_account()
        .is_some_and(|gate| gate != &request.gate)
    {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "wrong_gate"));
    }
    let key = (request.gate.clone(), request.proposal_id);
    {
        let mut state = lock(&s.state);
        state.rate_limit(&key, Instant::now())?;
        if let Some(active) = &state.active {
            return Err(if active == &key {
                ApiError::new(StatusCode::CONFLICT, "run_in_progress")
            } else {
                ApiError::limited("queue_full", 1)
            });
        }
        state.active = Some(key.clone());
    }
    let mut guard = ActiveRun {
        service: Arc::clone(&s),
        id: None,
    };
    let (_, plaintext) = crate::bundle::open(&s.identity.transport, &request.sealed_bundle_b64.0)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "bad_bundle"))?;
    let commitment = crate::sha256(&plaintext);
    let source = Arc::clone(&s);
    let gate = request.gate.clone();
    let proposal_id = request.proposal_id;
    let p = tokio::task::spawn_blocking(move || {
        source
            .proposals
            .get_for_bundle(&gate, proposal_id, commitment)
    })
    .await
    .map_err(|_| ApiError::new(StatusCode::BAD_GATEWAY, "rpc_unavailable"))?
    .map_err(|e| {
        if e == "bad_bundle" {
            ApiError::new(StatusCode::BAD_REQUEST, "bad_bundle")
        } else {
            ApiError::new(StatusCode::BAD_GATEWAY, "rpc_unavailable")
        }
    })?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "proposal_not_found"))?;
    if p.source_commitment.0 != commitment {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "bad_bundle"));
    }
    if p.rule_set_hash.0
        != rule_set_hash(&s.manifest)
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "invalid_manifest"))?
        || p.expires_at_s <= pipeline::now()
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "unsupported_proposal",
        ));
    }
    let (id, attempt) = {
        let mut state = lock(&s.state);
        state.next_id += 1;
        let id = state.next_id;
        let attempt = state.attempts.entry(key).or_default();
        *attempt += 1;
        let attempt = *attempt;
        state.runs.insert(id, Run::queued().into());
        (id, attempt)
    };
    guard.id = Some(id);
    tokio::task::spawn_blocking(move || {
        let _guard = guard;
        #[cfg(test)]
        if s.panic_next_run
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            panic!("injected worker failure");
        }
        let weak = Arc::downgrade(&s);
        let observer = Arc::new(move |run: &Run| {
            if let Some(service) = weak.upgrade() {
                let mut snapshot = run.snapshot();
                if snapshot.stage == "done"
                    && service.submitter.is_some()
                    && snapshot.certificate_b64.is_some()
                {
                    snapshot.stage = "submitting".into();
                }
                lock(&service.state).runs.insert(id, snapshot.into());
            }
        });
        let result = pipeline::execute_observed(
            &s.repo,
            &s.manifest,
            &s.identity,
            &p,
            &request.sealed_bundle_b64.0,
            attempt,
            &s.proposals.archive_provider(),
            s.proposals.archive_rpc().as_deref(),
            s.ai_transport.as_ref(),
            Some(observer),
        );
        let result = if let Some(submitter) = &s.submitter {
            if result.certificate_b64.is_some() {
                let mut submitting = result.snapshot();
                submitting.stage = "submitting".into();
                lock(&s.state).runs.insert(id, submitting.into());
            }
            SubmittedRun::submit(result, &p.gate, submitter.as_ref())
        } else {
            result.into()
        };
        lock(&s.state).runs.insert(id, result);
    });
    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({"run_id":id}))))
}
async fn status(
    State(s): State<Arc<Service>>,
    Path(id): Path<u64>,
) -> Result<Json<SubmittedRun>, StatusCode> {
    lock(&s.state)
        .runs
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
async fn evidence(
    State(s): State<Arc<Service>>,
    Path(id): Path<u64>,
) -> Result<Json<constitution_types::EvidenceManifest>, StatusCode> {
    lock(&s.state)
        .runs
        .get(&id)
        .and_then(|r| r.run.evidence.clone())
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proposals::LocalProposal;
    use constitution_types::{bundle::SourceBundle, *};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Source {
        proposal: LocalProposal,
        reads: Arc<AtomicUsize>,
    }
    impl ProposalSource for Source {
        fn gate_account(&self) -> Option<&AccountId> {
            Some(&self.proposal.gate)
        }
        fn get(&self, gate: &AccountId, id: u64) -> Result<Option<LocalProposal>, String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let mut p = self.proposal.clone();
            p.gate = gate.clone();
            p.proposal_id = id;
            Ok(Some(p))
        }
        fn archive_provider(&self) -> String {
            "local test".into()
        }
        fn archive_rpc(&self) -> Option<String> {
            None
        }
    }
    fn setup() -> (Arc<Service>, Vec<u8>, Arc<AtomicUsize>) {
        let manifest: RuleSetManifest =
            serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
        let bundle = SourceBundle {
            contract_name: "src/Test.sol:Test".into(),
            files: vec![("src/Test.sol".into(), b"contract Test {}".to_vec())],
            foundry_profile: "default".into(),
        };
        let bytes = borsh::to_vec(&bundle).unwrap();
        let reads = Arc::new(AtomicUsize::new(0));
        let p = LocalProposal {
            gate: "gate.testnet".parse().unwrap(),
            proposal_id: 1,
            chain_id: 84532,
            proxy: Hex20([1; 20]),
            executor: Hex20([2; 20]),
            expected_current_impl: Hex20([3; 20]),
            observed_current_impl: Hex20([4; 20]),
            current_impl_codehash: Hex32([5; 32]),
            new_impl: Hex20([6; 20]),
            new_impl_codehash: Hex32([7; 32]),
            init_calldata: HexBytes(vec![]),
            source_commitment: Hex32(crate::sha256(&bytes)),
            rule_set_hash: Hex32(rule_set_hash(&manifest).unwrap()),
            evidence_block_number: 1,
            evidence_block_hash: Hex32([8; 32]),
            expires_at_s: pipeline::now() + 300,
        };
        let service = Service::new(
            Identity::new(),
            manifest,
            PathBuf::new(),
            Box::new(Source {
                proposal: p,
                reads: reads.clone(),
            }),
        )
        .unwrap();
        (service, bytes, reads)
    }
    fn request(s: &Service, bytes: &[u8], id: u64) -> StartRun {
        StartRun {
            gate: "gate.testnet".parse().unwrap(),
            proposal_id: id,
            sealed_bundle_b64: Base64Bytes(
                s.identity
                    .transport
                    .public_key()
                    .seal(&mut crypto_box::aead::rand_core::OsRng, bytes)
                    .unwrap(),
            ),
        }
    }
    async fn finished(s: &Service, id: u64) -> SubmittedRun {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                {
                    let state = lock(&s.state);
                    if state.active.is_none() {
                        return state.runs.get(&id).unwrap().clone();
                    }
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn mismatch_and_unseal_errors_never_create_runs_or_attempts() {
        let (s, bytes, reads) = setup();
        let mut bad = bytes.clone();
        *bad.iter_mut().find(|b| **b == b'T').unwrap() = b'X';
        let error = start(State(s.clone()), Json(request(&s, &bad, 1)))
            .await
            .unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::BAD_REQUEST, "bad_bundle")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        let mut invalid = request(&s, &bytes, 2);
        invalid.sealed_bundle_b64.0.clear();
        let error = start(State(s.clone()), Json(invalid)).await.unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::BAD_REQUEST, "bad_bundle")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        let state = lock(&s.state);
        assert!(state.runs.is_empty());
        assert!(state.attempts.is_empty());
        assert!(state.active.is_none());
    }
    #[tokio::test]
    async fn wrong_gate_is_refused_before_rpc_and_rate_admission() {
        let (s, bytes, reads) = setup();
        let mut r = request(&s, &bytes, 1);
        r.gate = "other.testnet".parse().unwrap();
        let error = start(State(s.clone()), Json(r)).await.unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::BAD_REQUEST, "wrong_gate")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert!(lock(&s.state).global.is_empty());
    }
    #[test]
    fn rate_limits_recover_at_boundaries_and_allow_ten_second_retry() {
        use axum::response::IntoResponse;
        let now = Instant::now();
        let key: ProposalKey = ("gate.testnet".parse().unwrap(), 1);
        let mut state = Admission::default();
        state.rate_limit(&key, now).unwrap();
        let error = state
            .rate_limit(&key, now + Duration::from_secs(9))
            .unwrap_err();
        assert_eq!(error.code, "rate_limited");
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "1");
        state.rate_limit(&key, now + PROPOSAL_INTERVAL).unwrap();
        let mut state = Admission::default();
        for id in 0..GLOBAL_REQUESTS as u64 {
            state.rate_limit(&(key.0.clone(), id), now).unwrap();
        }
        assert!(state.rate_limit(&(key.0.clone(), 100), now).is_err());
        state
            .rate_limit(&(key.0, 100), now + GLOBAL_WINDOW)
            .unwrap();
    }
    #[tokio::test]
    async fn rate_limit_precedes_rpc() {
        let (s, bytes, reads) = setup();
        let first = start(State(s.clone()), Json(request(&s, &bytes, 1)))
            .await
            .unwrap();
        finished(&s, first.1.0["run_id"].as_u64().unwrap()).await;
        let error = start(State(s.clone()), Json(request(&s, &bytes, 1)))
            .await
            .unwrap_err();
        assert_eq!(error.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn sixty_fifth_request_works_after_previous_runs_finish() {
        let (s, bytes, _) = setup();
        for id in 1..=65 {
            lock(&s.state).global.clear();
            let response = start(State(s.clone()), Json(request(&s, &bytes, id)))
                .await
                .unwrap();
            assert_eq!(response.1.0["run_id"], id);
            assert!(finished(&s, id).await.run.certificate_b64.is_some());
        }
        let state = lock(&s.state);
        assert_eq!(state.runs.len(), FINISHED_RUNS);
        assert!(!state.runs.contains_key(&1));
        assert!(state.runs.contains_key(&65));
    }
    #[tokio::test]
    async fn panic_and_poisoned_mutex_do_not_wedge_worker() {
        let (s, bytes, _) = setup();
        let poison = s.clone();
        assert!(
            std::thread::spawn(move || {
                let _state = poison.state.lock().unwrap();
                panic!("injected mutex poison");
            })
            .join()
            .is_err()
        );
        s.panic_next_run.store(true, Ordering::SeqCst);
        let response = start(State(s.clone()), Json(request(&s, &bytes, 1)))
            .await
            .unwrap();
        let run = finished(&s, response.1.0["run_id"].as_u64().unwrap()).await;
        assert_eq!(run.run.error_code.as_deref(), Some("worker_panic"));
        assert_eq!(run.run.stage, "failed");
        assert!(run.run.certificate_b64.is_none());
        let response = start(State(s.clone()), Json(request(&s, &bytes, 2)))
            .await
            .unwrap();
        assert!(
            finished(&s, response.1.0["run_id"].as_u64().unwrap())
                .await
                .run
                .certificate_b64
                .is_some()
        );
    }
    #[test]
    fn history_eviction_never_removes_unfinished_runs() {
        let mut state = Admission::default();
        state.runs.insert(1, Run::queued().into());
        state.finish(1);
        assert!(state.finished.is_empty());
        for id in 2..=66 {
            let mut run = Run::queued();
            run.stage = "done".into();
            state.runs.insert(id, run.into());
            state.finish(id);
        }
        assert!(state.runs.contains_key(&1));
        assert!(!state.runs.contains_key(&2));
        assert_eq!(state.runs.len(), FINISHED_RUNS + 1);
    }
    #[tokio::test]
    async fn submitter_panic_retains_signed_certificate_and_submission_uncertainty() {
        struct PanicSubmitter;
        impl VerdictSubmitter for PanicSubmitter {
            fn submit(
                &self,
                _: &AccountId,
                _: &Base64Bytes,
                _: &Base64<64>,
            ) -> Result<String, String> {
                panic!("injected submission uncertainty");
            }
        }
        let (mut s, bytes, _) = setup();
        Arc::get_mut(&mut s).unwrap().submitter = Some(Box::new(PanicSubmitter));
        let response = start(State(s.clone()), Json(request(&s, &bytes, 1)))
            .await
            .unwrap();
        let result = finished(&s, response.1.0["run_id"].as_u64().unwrap()).await;
        assert_eq!(result.run.stage, "done");
        assert!(result.run.certificate_b64.is_some());
        assert!(result.near_tx_hash.is_none());
        assert!(result.run.stages.iter().any(|s| s.name == "submit"
            && s.status == constitution_types::manifest::StageStatus::Incomplete));
        assert!(lock(&s.state).active.is_none());
    }
}
