use crate::{
    identity::Identity,
    pipeline::{self, CanaryPolicy, Run, RunContext},
    proposals::{ProposalSource, SourceError},
    submit::{SubmittedRun, VerdictSubmitter},
    tools::{DockerBackend, ToolBackend},
};
use axum::{
    Json, Router,
    extract::{ConnectInfo, DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
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
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};
// Admission order: peer limiter and cool-down (no work), unauthenticated bounded
// `get_proposal`, then, only once the sealed box opened and its plaintext matches the on-chain
// source commitment, the per-proposal/global run limiters and the single run slot.
pub const PROPOSAL_INTERVAL: Duration = Duration::from_secs(10);
pub const GLOBAL_WINDOW: Duration = Duration::from_secs(1);
pub const GLOBAL_REQUESTS: usize = 10;
/// Unauthenticated requests per peer address in `PEER_WINDOW`.
pub const PEER_WINDOW: Duration = Duration::from_secs(10);
pub const PEER_REQUESTS: usize = 6;
/// Concurrent pre-authentication `get_proposal` RPCs.
pub const PREAUTH_RPC_SLOTS: usize = 4;
/// Per-proposal cool-down after a failed run that its own source can cause:
/// 10 min, doubling per failure, capped at 1 h; failures are forgotten after 2 h.
pub const COOLDOWN_BASE: Duration = Duration::from_secs(10 * 60);
pub const COOLDOWN_CAP: Duration = Duration::from_secs(60 * 60);
pub const COOLDOWN_MEMORY: Duration = Duration::from_secs(2 * 60 * 60);
pub const FINISHED_RUNS: usize = 64;
type ProposalKey = (AccountId, u64);
struct Cooldown {
    failures: u32,
    last_failure: Instant,
}
#[derive(Default)]
struct Admission {
    runs: BTreeMap<u64, SubmittedRun>,
    finished: VecDeque<u64>,
    next_id: u64,
    active: Option<ProposalKey>,
    attempts: BTreeMap<ProposalKey, u32>,
    proposals: BTreeMap<ProposalKey, Instant>,
    global: VecDeque<Instant>,
    peers: BTreeMap<IpAddr, VecDeque<Instant>>,
    peer_expirations: VecDeque<(Instant, IpAddr)>,
    read_peers: BTreeMap<IpAddr, VecDeque<Instant>>,
    read_expirations: VecDeque<(Instant, IpAddr)>,
    cooldowns: BTreeMap<ProposalKey, Cooldown>,
}
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
fn whole_seconds(wait: Duration) -> u64 {
    wait.as_secs()
        .saturating_add(u64::from(wait.subsec_nanos() != 0))
}
/// IPv6 peers share one budget per /64 so an address block cannot mint unlimited peers.
fn peer_bucket(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => {
                let mut octets = v6.octets();
                octets[8..].fill(0);
                IpAddr::V6(octets.into())
            }
        },
        v4 => v4,
    }
}
fn cooldown_for(failures: u32) -> Duration {
    COOLDOWN_BASE
        .saturating_mul(1u32 << failures.saturating_sub(1).min(8))
        .min(COOLDOWN_CAP)
}
fn limit_peer(
    peers: &mut BTreeMap<IpAddr, VecDeque<Instant>>,
    expirations: &mut VecDeque<(Instant, IpAddr)>,
    peer: IpAddr,
    now: Instant,
    limit: usize,
) -> Result<(), ApiError> {
    while expirations
        .front()
        .is_some_and(|(time, _)| now.duration_since(*time) >= PEER_WINDOW)
    {
        let (time, ip) = expirations.pop_front().expect("front exists");
        if let Some(times) = peers.get_mut(&ip) {
            if times.front() == Some(&time) {
                times.pop_front();
            }
            if times.is_empty() {
                peers.remove(&ip);
            }
        }
    }
    let peer = peer_bucket(peer);
    let times = peers.entry(peer).or_default();
    if times.len() >= limit {
        return Err(ApiError::limited(
            "rate_limited",
            whole_seconds(PEER_WINDOW - now.duration_since(times[0])),
        ));
    }
    times.push_back(now);
    expirations.push_back((now, peer));
    Ok(())
}
impl Admission {
    /// Unauthenticated requests are limited per peer address, never per proposal or globally.
    fn peer_limit(&mut self, peer: IpAddr, now: Instant) -> Result<(), ApiError> {
        limit_peer(
            &mut self.peers,
            &mut self.peer_expirations,
            peer,
            now,
            PEER_REQUESTS,
        )
    }
    fn read_limit(&mut self, peer: IpAddr, now: Instant) -> Result<(), ApiError> {
        limit_peer(
            &mut self.read_peers,
            &mut self.read_expirations,
            peer,
            now,
            60,
        )
    }
    /// Refuse a proposal that recently failed runs its own source can cause; no RPC or
    /// sandbox work happens before this check.
    fn cooling_down(&mut self, key: &ProposalKey, now: Instant) -> Result<(), ApiError> {
        self.cooldowns
            .retain(|_, c| now.duration_since(c.last_failure) < COOLDOWN_MEMORY);
        if let Some(c) = self.cooldowns.get(key) {
            let until = c.last_failure + cooldown_for(c.failures);
            if now < until {
                return Err(ApiError::limited(
                    "cooling_down",
                    whole_seconds(until - now),
                ));
            }
        }
        Ok(())
    }
    fn prior_failed_runs(&self, key: &ProposalKey) -> u32 {
        self.cooldowns.get(key).map_or(0, |c| c.failures)
    }
    /// Count failed and withheld outcomes except clearly verifier-owned faults.
    fn record_outcome(&mut self, key: &ProposalKey, run: &Run, now: Instant) {
        let verifier_fault = run.stage == "failed"
            && (run.canary_failed
                || run.error_code.as_deref().is_some_and(|code| {
                    matches!(
                        code,
                        "sandbox_unavailable" | "worker_panic" | "rpc_unavailable"
                    )
                })
                || (run.error_code.as_deref() == Some("infrastructure_timeout")
                    && run
                        .stages
                        .iter()
                        .rev()
                        .find(|stage| stage.name != "failed")
                        .is_some_and(|stage| {
                            matches!(
                                stage.name.as_str(),
                                "r5_replay" | "ai_review" | "reproducing"
                            )
                        })));
        if verifier_fault {
            return;
        }
        if run.certificate_b64.is_none() && matches!(run.stage.as_str(), "failed" | "done") {
            let c = self.cooldowns.entry(key.clone()).or_insert(Cooldown {
                failures: 0,
                last_failure: now,
            });
            c.failures = c.failures.saturating_add(1);
            c.last_failure = now;
        } else if run.certificate_b64.is_some() {
            self.cooldowns.remove(key);
        }
    }
    /// Authenticated admission: a refusal because the worker is busy charges nothing.
    fn admit(&mut self, key: &ProposalKey, now: Instant) -> Result<(), ApiError> {
        if let Some(active) = &self.active {
            return Err(if active == key {
                ApiError::new(StatusCode::CONFLICT, "run_in_progress")
            } else {
                ApiError::limited("queue_full", 1)
            });
        }
        self.rate_limit(key, now)?;
        self.active = Some(key.clone());
        Ok(())
    }
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
            return Err(ApiError::limited("rate_limited", whole_seconds(wait)));
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
impl From<SourceError> for ApiError {
    /// A proposal that is not Pending, not certifiable, or not ready is the client's state to
    /// fix (4xx); only real provider faults are 502 so clients never retry a terminal state.
    fn from(error: SourceError) -> Self {
        match error {
            SourceError::BadBundle => Self::new(StatusCode::BAD_REQUEST, "bad_bundle"),
            SourceError::WrongGate => Self::new(StatusCode::BAD_REQUEST, "wrong_gate"),
            SourceError::NotPending => Self::new(StatusCode::CONFLICT, "proposal_not_pending"),
            SourceError::Unsupported(_) => {
                Self::new(StatusCode::BAD_REQUEST, "unsupported_proposal")
            }
            SourceError::NotReady(_) => Self::new(StatusCode::CONFLICT, "not_ready"),
            SourceError::Rpc(_) | SourceError::Refused => {
                Self::new(StatusCode::BAD_GATEWAY, "rpc_unavailable")
            }
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
    /// Tool execution backend; the canary runs through the same backend as proposal stages.
    pub tools: Box<dyn ToolBackend>,
    pub canary: CanaryPolicy,
    pub canary_health: pipeline::CanaryHealth,
    submitter: Option<Box<dyn VerdictSubmitter>>,
    state: Mutex<Admission>,
    /// Bounds the unauthenticated `get_proposal` RPC.
    preauth: Arc<tokio::sync::Semaphore>,
    /// Header carrying the client address when a trusted reverse proxy fronts the listener.
    trusted_proxy_header: Option<HeaderName>,
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
            tools: Box::new(DockerBackend),
            canary: CanaryPolicy::default(),
            canary_health: pipeline::CanaryHealth::default(),
            submitter,
            state: Mutex::new(Admission::default()),
            preauth: Arc::new(tokio::sync::Semaphore::new(PREAUTH_RPC_SLOTS)),
            trusted_proxy_header: None,
            #[cfg(test)]
            panic_next_run: std::sync::atomic::AtomicBool::new(false),
        }))
    }
    /// Key the unauthenticated per-peer limiter on the last entry of `header` (the address
    /// appended by the reverse proxy in front of the loopback listener) instead of the socket
    /// peer. Off by default: without it every client behind a proxy shares the proxy's budget.
    /// Call only at startup, before the service is shared, and only for a header the proxy
    /// always overwrites.
    pub fn trust_proxy_header(service: &mut Arc<Self>, header: &str) -> Result<(), String> {
        let name = HeaderName::from_bytes(header.trim().as_bytes())
            .map_err(|_| "invalid trusted proxy header name")?;
        Arc::get_mut(service)
            .ok_or("service already shared")?
            .trusted_proxy_header = Some(name);
        Ok(())
    }
    fn peer(&self, socket: SocketAddr, headers: &HeaderMap) -> Result<IpAddr, ApiError> {
        let Some(name) = &self.trusted_proxy_header else {
            return Ok(socket.ip());
        };
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.rsplit(',').next())
            .and_then(|value| value.trim().parse().ok())
            .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "invalid_peer"))
    }
}
#[derive(serde::Deserialize, serde::Serialize)]
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
/// Serve with the peer address the admission limiter keys on.
pub async fn serve(listener: tokio::net::TcpListener, router: Router) -> std::io::Result<()> {
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}
/// Run limiters and the run slot, charged only for a caller that proved it holds the plaintext.
fn admit(service: &Arc<Service>, key: &ProposalKey) -> Result<ActiveRun, ApiError> {
    lock(&service.state).admit(key, Instant::now())?;
    Ok(ActiveRun {
        service: Arc::clone(service),
        id: None,
    })
}
async fn start(
    State(s): State<Arc<Service>>,
    ConnectInfo(socket): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    request: axum::extract::Request,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    lock(&s.state).peer_limit(s.peer(socket, &headers)?, Instant::now())?;
    let body = axum::body::to_bytes(request.into_body(), 3 * 1024 * 1024)
        .await
        .map_err(|_| ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "bad_request"))?;
    let request: StartRun = serde_json::from_slice(&body)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "bad_request"))?;
    if s.proposals
        .gate_account()
        .is_some_and(|gate| gate != &request.gate)
    {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "wrong_gate"));
    }
    let key = (request.gate.clone(), request.proposal_id);
    {
        let mut state = lock(&s.state);
        state.cooling_down(&key, Instant::now())?;
    }
    let permit = Arc::clone(&s.preauth)
        .try_acquire_owned()
        .map_err(|_| ApiError::limited("busy", 1))?;
    let (_, plaintext) = crate::bundle::open(&s.identity.transport, &request.sealed_bundle_b64.0)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "bad_bundle"))?;
    let commitment = crate::sha256(&plaintext);
    let source = Arc::clone(&s);
    let gate = request.gate.clone();
    let proposal_id = request.proposal_id;
    let admit_key = key.clone();
    let (observed, guard, refusal) = tokio::task::spawn_blocking(move || {
        let mut permit = Some(permit);
        let mut guard = None;
        let mut refusal = None;
        let observed = source
            .proposals
            .get_for_bundle(&gate, proposal_id, commitment, &mut || {
                // Authenticated from here on: stop occupying a pre-authentication RPC slot.
                permit.take();
                match admit(&source, &admit_key) {
                    Ok(held) => {
                        guard = Some(held);
                        Ok(())
                    }
                    Err(error) => {
                        refusal = Some(error);
                        Err(SourceError::Refused)
                    }
                }
            });
        (observed, guard, refusal)
    })
    .await
    .map_err(|_| ApiError::new(StatusCode::BAD_GATEWAY, "rpc_unavailable"))?;
    let p = match observed {
        Err(SourceError::Refused) => {
            return Err(refusal.unwrap_or_else(|| {
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "admission_missing")
            }));
        }
        Err(error) => return Err(error.into()),
        Ok(found) => {
            found.ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "proposal_not_found"))?
        }
    };
    let mut guard = guard
        .ok_or_else(|| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "admission_missing"))?;
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
    let (id, attempt, prior_failed_runs) = {
        let mut state = lock(&s.state);
        state.next_id += 1;
        let id = state.next_id;
        let prior_failed_runs = state.prior_failed_runs(&key);
        let attempt = state.attempts.entry(key.clone()).or_default();
        *attempt += 1;
        let attempt = *attempt;
        state.runs.insert(id, Run::queued().into());
        (id, attempt, prior_failed_runs)
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
        let archive_provider = s.proposals.archive_provider();
        let archive_rpc = s.proposals.archive_rpc();
        let result = pipeline::execute_observed(
            &s.repo,
            &s.manifest,
            &s.identity,
            &p,
            &request.sealed_bundle_b64.0,
            attempt,
            RunContext {
                archive_provider: &archive_provider,
                archive_rpc: archive_rpc.as_deref(),
                ai_transport: s.ai_transport.as_ref(),
                tools: s.tools.as_ref(),
                canary: &s.canary,
                canary_health: &s.canary_health,
                prior_failed_runs,
                deadline: Instant::now() + pipeline::RUN_DEADLINE,
                observer: Some(observer),
            },
        );
        lock(&s.state).record_outcome(&key, &result, Instant::now());
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
    ConnectInfo(socket): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<SubmittedRun>, ApiError> {
    let mut state = lock(&s.state);
    state.read_limit(s.peer(socket, &headers)?, Instant::now())?;
    state
        .runs
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "evicted"))
}
async fn evidence(
    State(s): State<Arc<Service>>,
    ConnectInfo(socket): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<u64>,
) -> Result<Json<constitution_types::EvidenceManifest>, ApiError> {
    let mut state = lock(&s.state);
    state.read_limit(s.peer(socket, &headers)?, Instant::now())?;
    state
        .runs
        .get(&id)
        .and_then(|r| r.run.evidence.clone())
        .map(Json)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "evicted"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proposals::LocalProposal;
    use constitution_types::{bundle::SourceBundle, *};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct Source {
        proposal: LocalProposal,
        reads: Arc<AtomicUsize>,
        /// Proposal id 5 parks inside `get` while this is set (a slow pre-auth RPC).
        park: Arc<AtomicBool>,
        fail: Arc<Mutex<Option<SourceError>>>,
    }
    impl ProposalSource for Source {
        fn gate_account(&self) -> Option<&AccountId> {
            Some(&self.proposal.gate)
        }
        fn get(&self, gate: &AccountId, id: u64) -> Result<Option<LocalProposal>, SourceError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            while id == 5 && self.park.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            if let Some(error) = lock(&self.fail).clone() {
                return Err(error);
            }
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
    struct Harness {
        s: Arc<Service>,
        bytes: Vec<u8>,
        reads: Arc<AtomicUsize>,
        park: Arc<AtomicBool>,
        fail: Arc<Mutex<Option<SourceError>>>,
    }
    fn harness() -> Harness {
        let manifest: RuleSetManifest =
            serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
        let bundle = SourceBundle {
            contract_name: "src/Test.sol:Test".into(),
            files: vec![("src/Test.sol".into(), b"contract Test {}".to_vec())],
            foundry_profile: "default".into(),
        };
        let bytes = borsh::to_vec(&bundle).unwrap();
        let reads = Arc::new(AtomicUsize::new(0));
        let park = Arc::new(AtomicBool::new(false));
        let fail = Arc::new(Mutex::new(None));
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
        let s = Service::new(
            Identity::new(),
            manifest,
            PathBuf::new(),
            Box::new(Source {
                proposal: p,
                reads: reads.clone(),
                park: park.clone(),
                fail: fail.clone(),
            }),
        )
        .unwrap();
        Harness {
            s,
            bytes,
            reads,
            park,
            fail,
        }
    }
    fn setup() -> (Arc<Service>, Vec<u8>, Arc<AtomicUsize>) {
        let h = harness();
        (h.s, h.bytes, h.reads)
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
    /// A valid sealed box around a different plaintext: opens, but does not match the commitment.
    fn wrong_plaintext(s: &Service, bytes: &[u8], id: u64) -> StartRun {
        let mut bad = bytes.to_vec();
        *bad.iter_mut().find(|b| **b == b'T').unwrap() = b'X';
        request(s, &bad, id)
    }
    fn peer(n: u8) -> ConnectInfo<SocketAddr> {
        ConnectInfo(SocketAddr::from(([10, 0, 0, n], 9000)))
    }
    async fn call(
        s: &Arc<Service>,
        n: u8,
        request: StartRun,
    ) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
        let body = serde_json::to_vec(&request).unwrap();
        start(
            State(s.clone()),
            peer(n),
            HeaderMap::new(),
            axum::http::Request::new(axum::body::Body::from(body)),
        )
        .await
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
    fn key(id: u64) -> ProposalKey {
        ("gate.testnet".parse().unwrap(), id)
    }
    #[tokio::test]
    async fn mismatch_and_unseal_errors_never_create_runs_or_attempts() {
        let (s, bytes, reads) = setup();
        let error = call(&s, 1, wrong_plaintext(&s, &bytes, 1))
            .await
            .unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::BAD_REQUEST, "bad_bundle")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        let mut invalid = request(&s, &bytes, 2);
        invalid.sealed_bundle_b64.0.clear();
        let error = call(&s, 1, invalid).await.unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::BAD_REQUEST, "bad_bundle")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        let state = lock(&s.state);
        assert!(state.runs.is_empty());
        assert!(state.attempts.is_empty());
        assert!(state.active.is_none());
        // Neither failed authentication charged a run limiter.
        assert!(state.proposals.is_empty() && state.global.is_empty());
    }
    #[tokio::test]
    async fn wrong_gate_is_refused_before_rpc_and_any_admission() {
        let (s, bytes, reads) = setup();
        let mut r = request(&s, &bytes, 1);
        r.gate = "other.testnet".parse().unwrap();
        let error = call(&s, 1, r).await.unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::BAD_REQUEST, "wrong_gate")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        let state = lock(&s.state);
        assert!(state.global.is_empty());
    }
    #[tokio::test]
    async fn bad_body_flood_cannot_lock_out_the_real_proposer() {
        let (s, bytes, _) = setup();
        let mut limited = 0;
        for round in 0..20 {
            let body = if round % 2 == 0 {
                wrong_plaintext(&s, &bytes, 1)
            } else {
                let mut garbage = request(&s, &bytes, 1);
                garbage.sealed_bundle_b64.0 = vec![7; 64];
                garbage
            };
            let error = call(&s, 66, body).await.unwrap_err();
            if error.status == StatusCode::TOO_MANY_REQUESTS {
                limited += 1;
                assert_eq!(error.code, "rate_limited");
            } else {
                assert_eq!(error.code, "bad_bundle");
            }
        }
        assert_eq!(
            limited,
            20 - PEER_REQUESTS,
            "the attacker's own budget ran out"
        );
        {
            let state = lock(&s.state);
            assert!(state.proposals.is_empty() && state.global.is_empty());
            assert!(state.active.is_none() && state.attempts.is_empty());
        }
        // The proposer, from another address, is admitted at once.
        let response = call(&s, 1, request(&s, &bytes, 1)).await.unwrap();
        assert_eq!(response.0, StatusCode::ACCEPTED);
        assert!(finished(&s, 1).await.run.certificate_b64.is_some());
    }
    #[tokio::test]
    async fn fake_id_flood_from_one_peer_does_not_stall_another_peer() {
        let (s, bytes, _) = setup();
        for id in 100..110 {
            let mut fake = request(&s, &bytes, id);
            fake.sealed_bundle_b64.0 = vec![9; 48];
            let _ = call(&s, 66, fake).await;
        }
        assert!(lock(&s.state).global.is_empty());
        let response = call(&s, 2, request(&s, &bytes, 1)).await.unwrap();
        assert_eq!(response.0, StatusCode::ACCEPTED);
        finished(&s, 1).await;
    }
    #[tokio::test]
    async fn wrong_plaintext_request_cannot_hold_the_run_slot() {
        let h = harness();
        h.park.store(true, Ordering::SeqCst);
        let attacker = {
            let s = h.s.clone();
            let body = wrong_plaintext(&h.s, &h.bytes, 5);
            tokio::spawn(async move { call(&s, 66, body).await.map(|_| ()) })
        };
        while h.reads.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        // The attacker is mid-RPC, unauthenticated: the run slot and limiters are untouched.
        {
            let state = lock(&h.s.state);
            assert!(state.active.is_none() && state.proposals.is_empty());
        }
        let response = call(&h.s, 1, request(&h.s, &h.bytes, 1)).await.unwrap();
        assert_eq!(response.0, StatusCode::ACCEPTED);
        finished(&h.s, 1).await;
        h.park.store(false, Ordering::SeqCst);
        let error = attacker.await.unwrap().unwrap_err();
        assert_eq!(error.code, "bad_bundle");
        assert!(lock(&h.s.state).active.is_none());
    }
    #[tokio::test]
    async fn pre_auth_rpc_slots_are_bounded() {
        let h = harness();
        h.park.store(true, Ordering::SeqCst);
        let parked: Vec<_> = (0..PREAUTH_RPC_SLOTS as u8)
            .map(|n| {
                let s = h.s.clone();
                let body = wrong_plaintext(&h.s, &h.bytes, 5);
                tokio::spawn(async move { call(&s, 20 + n, body).await.map(|_| ()) })
            })
            .collect();
        while h.reads.load(Ordering::SeqCst) < PREAUTH_RPC_SLOTS {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let error = call(&h.s, 90, wrong_plaintext(&h.s, &h.bytes, 5))
            .await
            .unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::TOO_MANY_REQUESTS, "busy")
        );
        h.park.store(false, Ordering::SeqCst);
        for task in parked {
            assert_eq!(task.await.unwrap().unwrap_err().code, "bad_bundle");
        }
        // Slots are returned: the next request reaches the source again.
        assert_eq!(
            call(&h.s, 91, wrong_plaintext(&h.s, &h.bytes, 5))
                .await
                .unwrap_err()
                .code,
            "bad_bundle"
        );
    }
    #[tokio::test]
    async fn busy_refusals_do_not_burn_the_proposers_admission_slot() {
        let (s, bytes, _) = setup();
        lock(&s.state).active = Some(key(99));
        let error = call(&s, 1, request(&s, &bytes, 1)).await.unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::TOO_MANY_REQUESTS, "queue_full")
        );
        lock(&s.state).active = Some(key(1));
        let error = call(&s, 1, request(&s, &bytes, 1)).await.unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::CONFLICT, "run_in_progress")
        );
        {
            let mut state = lock(&s.state);
            assert!(state.proposals.is_empty() && state.global.is_empty());
            state.active = None;
        }
        // The immediate retry is admitted, not rate_limited.
        let response = call(&s, 1, request(&s, &bytes, 1)).await.unwrap();
        assert_eq!(response.0, StatusCode::ACCEPTED);
        finished(&s, 1).await;
    }
    #[tokio::test]
    async fn chain_state_errors_are_client_statuses_and_only_rpc_faults_are_502() {
        let h = harness();
        for (n, (error, status, code)) in [
            (
                SourceError::NotPending,
                StatusCode::CONFLICT,
                "proposal_not_pending",
            ),
            (
                SourceError::Unsupported("unsupported rule_set_hash"),
                StatusCode::BAD_REQUEST,
                "unsupported_proposal",
            ),
            (
                SourceError::NotReady("insufficient blocks"),
                StatusCode::CONFLICT,
                "not_ready",
            ),
            (
                SourceError::Rpc("eth_blockNumber RPC transport".into()),
                StatusCode::BAD_GATEWAY,
                "rpc_unavailable",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            *lock(&h.fail) = Some(error);
            let failure = call(&h.s, 30 + n as u8, request(&h.s, &h.bytes, 1))
                .await
                .unwrap_err();
            assert_eq!((failure.status, failure.code), (status, code));
        }
        let state = lock(&h.s.state);
        assert!(state.active.is_none() && state.attempts.is_empty() && state.runs.is_empty());
    }
    #[test]
    fn peer_limit_is_per_peer_block_and_recovers() {
        let now = Instant::now();
        let mut state = Admission::default();
        let a: IpAddr = "203.0.113.7".parse().unwrap();
        for _ in 0..PEER_REQUESTS {
            state.peer_limit(a, now).unwrap();
        }
        let error = state.peer_limit(a, now).unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::TOO_MANY_REQUESTS, "rate_limited")
        );
        state
            .peer_limit("203.0.113.8".parse().unwrap(), now)
            .unwrap();
        state.peer_limit(a, now + PEER_WINDOW).unwrap();
        // Addresses inside one IPv6 /64 share a budget; the next /64 does not.
        let mut state = Admission::default();
        for n in 0..PEER_REQUESTS as u16 {
            state
                .peer_limit(format!("2001:db8:0:1::{n:x}").parse().unwrap(), now)
                .unwrap();
        }
        assert!(
            state
                .peer_limit("2001:db8:0:1:ffff::1".parse().unwrap(), now)
                .is_err()
        );
        state
            .peer_limit("2001:db8:0:2::1".parse().unwrap(), now)
            .unwrap();
    }
    #[test]
    fn forwarded_header_is_ignored_unless_a_trusted_proxy_header_is_configured() {
        let (mut s, _, _) = setup();
        let socket: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.9, 192.0.2.4".parse().unwrap(),
        );
        assert_eq!(s.peer(socket, &headers).unwrap(), socket.ip());
        Service::trust_proxy_header(&mut s, "X-Forwarded-For").unwrap();
        // The last hop is the address the trusted proxy appended.
        assert_eq!(
            s.peer(socket, &headers).unwrap(),
            "192.0.2.4".parse::<IpAddr>().unwrap()
        );
        assert!(s.peer(socket, &HeaderMap::new()).is_err());
        headers.insert("x-forwarded-for", "not an address".parse().unwrap());
        assert!(s.peer(socket, &headers).is_err());
        assert!(Service::trust_proxy_header(&mut s, "bad header").is_err());
    }
    #[tokio::test]
    async fn peer_limit_precedes_body_parsing_and_bounds_reads() {
        let (s, _, _) = setup();
        for _ in 0..PEER_REQUESTS {
            let request = axum::http::Request::new(axum::body::Body::from("not json"));
            assert_eq!(
                start(State(s.clone()), peer(9), HeaderMap::new(), request)
                    .await
                    .unwrap_err()
                    .code,
                "bad_request"
            );
        }
        let request = axum::http::Request::new(axum::body::Body::from("not json"));
        assert_eq!(
            start(State(s.clone()), peer(9), HeaderMap::new(), request)
                .await
                .unwrap_err()
                .code,
            "rate_limited"
        );
        for _ in 0..60 {
            assert_eq!(
                status(State(s.clone()), peer(10), HeaderMap::new(), Path(1))
                    .await
                    .err()
                    .unwrap()
                    .code,
                "evicted"
            );
        }
        assert_eq!(
            evidence(State(s), peer(10), HeaderMap::new(), Path(1))
                .await
                .err()
                .unwrap()
                .code,
            "rate_limited"
        );
    }
    #[test]
    fn withheld_and_storage_faults_count_but_rpc_timeouts_do_not() {
        let mut state = Admission::default();
        let now = Instant::now();
        let k = key(1);
        let mut withheld = Run::queued();
        withheld.stage = "done".into();
        state.record_outcome(&k, &withheld, now);
        assert_eq!(state.prior_failed_runs(&k), 1);
        let mut storage = Run::queued();
        storage.fail("storage_error");
        state.record_outcome(&k, &storage, now);
        assert_eq!(state.prior_failed_runs(&k), 2);
        let mut rpc = Run::queued();
        rpc.stages.push(pipeline::Stage {
            name: "r5_replay".into(),
            status: constitution_types::manifest::StageStatus::Incomplete,
            detail: String::new(),
            elapsed_ms: 0,
        });
        rpc.fail("infrastructure_timeout");
        state.record_outcome(&k, &rpc, now);
        assert_eq!(state.prior_failed_runs(&k), 2);
    }
    #[test]
    fn cooldown_grows_to_the_cap_and_forgives_success_and_environment_blame() {
        let now = Instant::now();
        let minutes = |m: u64| Duration::from_secs(m * 60);
        let seconds: Vec<_> = (1..=5).map(|n| cooldown_for(n).as_secs() / 60).collect();
        assert_eq!(seconds, [10, 20, 40, 60, 60]);
        let k = key(1);
        let mut state = Admission::default();
        let failed = |code: &str| {
            let mut run = Run::queued();
            run.fail(code);
            run
        };
        assert_eq!(state.prior_failed_runs(&k), 0);
        state.record_outcome(&k, &failed("infrastructure_timeout"), now);
        let error = state.cooling_down(&k, now + minutes(5)).unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::TOO_MANY_REQUESTS, "cooling_down")
        );
        assert_eq!(error.retry_after, Some(5 * 60));
        state.cooling_down(&k, now + minutes(10)).unwrap();
        state.record_outcome(&k, &failed("tool_output_invalid"), now + minutes(11));
        assert_eq!(state.prior_failed_runs(&k), 2);
        assert!(state.cooling_down(&k, now + minutes(30)).is_err());
        state.cooling_down(&k, now + minutes(31)).unwrap();
        // Failures the proposal cannot cause never count.
        for code in ["rpc_unavailable", "sandbox_unavailable", "worker_panic"] {
            state.record_outcome(&k, &failed(code), now + minutes(32));
        }
        assert_eq!(state.prior_failed_runs(&k), 2);
        // The canary also failed: environment, the proposer is not penalised.
        let mut environment = failed("infrastructure_timeout");
        environment.canary_failed = true;
        state.record_outcome(&k, &environment, now + minutes(33));
        assert_eq!(state.prior_failed_runs(&k), 2);
        state.record_outcome(&k, &failed("infrastructure_timeout"), now + minutes(34));
        let mut done = Run::queued();
        done.stage = "done".into();
        state.record_outcome(&k, &done, now + minutes(35));
        assert_eq!(state.prior_failed_runs(&k), 4);
        // Forgotten after the memory window.
        state.record_outcome(&k, &failed("infrastructure_timeout"), now);
        state.cooling_down(&k, now + COOLDOWN_MEMORY).unwrap();
        assert_eq!(state.prior_failed_runs(&k), 0);
    }
    #[tokio::test]
    async fn cooling_down_proposal_is_refused_before_any_rpc() {
        let (s, bytes, reads) = setup();
        lock(&s.state).cooldowns.insert(
            key(1),
            Cooldown {
                failures: 1,
                last_failure: Instant::now(),
            },
        );
        let error = call(&s, 1, request(&s, &bytes, 1)).await.unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::TOO_MANY_REQUESTS, "cooling_down")
        );
        assert!(error.retry_after.is_some_and(|s| (590..=600).contains(&s)));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        // Other proposals are unaffected.
        let response = call(&s, 1, request(&s, &bytes, 2)).await.unwrap();
        assert_eq!(response.0, StatusCode::ACCEPTED);
        finished(&s, 1).await;
    }
    /// Solver times out for the proposal's contract, never for the honest canary fixture.
    struct ProposalTimesOut;
    struct Tools {
        solver_times_out: bool,
    }
    impl crate::tools::Toolchain for Tools {
        fn compile(&self) -> Result<Vec<u8>, String> {
            Ok(vec![])
        }
        fn runtime(&self, _: &str) -> Result<Vec<u8>, String> {
            Ok(vec![0x60, 0x01])
        }
        fn layout(
            &self,
            _: &str,
            _: &str,
            _: &[String],
        ) -> Result<(constitution_types::manifest::StageStatus, Vec<u8>), String> {
            Ok((constitution_types::manifest::StageStatus::Pass, vec![]))
        }
        fn solvency(
            &self,
            _: &RuleSetManifest,
            _: &constitution_types::bundle::SourceBundle,
        ) -> Result<(constitution_types::manifest::StageStatus, u16, Vec<u8>), String> {
            if self.solver_times_out {
                Err("infrastructure:infrastructure_timeout".into())
            } else {
                Ok((constitution_types::manifest::StageStatus::Pass, 0, vec![]))
            }
        }
    }
    impl ToolBackend for ProposalTimesOut {
        fn open(
            &self,
            _: &std::path::Path,
            bundle: &SourceBundle,
            manifest: &RuleSetManifest,
        ) -> Result<Box<dyn crate::tools::Toolchain>, String> {
            Ok(Box::new(Tools {
                solver_times_out: bundle.contract_name == manifest.reference.contract_name,
            }))
        }
    }
    #[tokio::test]
    async fn failed_runs_feed_the_cooldown_and_the_second_one_the_canary() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest: RuleSetManifest =
            serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
        let files = manifest
            .reference
            .source_sha256s
            .keys()
            .map(|name| {
                (
                    name.clone(),
                    std::fs::read(repo.join("contracts").join(name)).unwrap(),
                )
            })
            .collect();
        let bytes = borsh::to_vec(&SourceBundle {
            contract_name: manifest.reference.contract_name.clone(),
            files,
            foundry_profile: "default".into(),
        })
        .unwrap();
        let code_hash = Hex32(crate::keccak(&[0x60, 0x01]));
        let proposal = LocalProposal {
            gate: "gate.testnet".parse().unwrap(),
            proposal_id: 1,
            chain_id: 84532,
            proxy: Hex20([1; 20]),
            executor: Hex20([2; 20]),
            expected_current_impl: Hex20([3; 20]),
            observed_current_impl: Hex20([3; 20]),
            current_impl_codehash: code_hash,
            new_impl: Hex20([6; 20]),
            new_impl_codehash: code_hash,
            init_calldata: HexBytes(vec![]),
            source_commitment: Hex32(crate::sha256(&bytes)),
            rule_set_hash: Hex32(rule_set_hash(&manifest).unwrap()),
            evidence_block_number: 1,
            evidence_block_hash: Hex32([8; 32]),
            expires_at_s: pipeline::now() + 300,
        };
        let mut s = Service::new(
            Identity::new(),
            manifest,
            repo,
            Box::new(Source {
                proposal,
                reads: Default::default(),
                park: Default::default(),
                fail: Default::default(),
            }),
        )
        .unwrap();
        Arc::get_mut(&mut s).unwrap().tools = Box::new(ProposalTimesOut);
        // First failed run: retryable, no canary, cool-down starts.
        let first = call(&s, 1, request(&s, &bytes, 1)).await.unwrap();
        let first = finished(&s, first.1.0["run_id"].as_u64().unwrap()).await;
        assert_eq!(first.run.stage, "failed");
        assert_eq!(
            first.run.error_code.as_deref(),
            Some("infrastructure_timeout")
        );
        assert!(first.run.stages.iter().any(|stage| stage.name == "canary"));
        assert_eq!(lock(&s.state).prior_failed_runs(&key(1)), 1);
        let error = call(&s, 1, request(&s, &bytes, 1)).await.unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::TOO_MANY_REQUESTS, "cooling_down")
        );
        // Cool-down elapsed: the second failure is checked against the honest canary.
        {
            let mut state = lock(&s.state);
            let c = state.cooldowns.get_mut(&key(1)).unwrap();
            c.last_failure = Instant::now()
                .checked_sub(COOLDOWN_BASE + Duration::from_secs(1))
                .expect("monotonic clock older than the cool-down");
            state.proposals.clear();
        }
        let second = call(&s, 1, request(&s, &bytes, 1)).await.unwrap();
        let second = finished(&s, second.1.0["run_id"].as_u64().unwrap()).await;
        assert_eq!(second.run.stage, "done");
        assert_eq!(
            second.run.verdict,
            Some(constitution_types::Verdict::Reject {
                rule_id: 2,
                reason_code: 23
            })
        );
        assert!(second.run.stages.iter().any(|stage| stage.name == "canary"
            && stage.status == constitution_types::manifest::StageStatus::Pass));
        assert_eq!(lock(&s.state).prior_failed_runs(&key(1)), 0);
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
    async fn authenticated_repeat_within_ten_seconds_is_rate_limited_after_the_commitment_check() {
        let (s, bytes, reads) = setup();
        let first = call(&s, 1, request(&s, &bytes, 1)).await.unwrap();
        finished(&s, first.1.0["run_id"].as_u64().unwrap()).await;
        let error = call(&s, 1, request(&s, &bytes, 1)).await.unwrap_err();
        assert_eq!(
            (error.status, error.code),
            (StatusCode::TOO_MANY_REQUESTS, "rate_limited")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }
    #[tokio::test]
    async fn sixty_fifth_request_works_after_previous_runs_finish() {
        let (s, bytes, _) = setup();
        for id in 1..=65 {
            {
                let mut state = lock(&s.state);
                state.global.clear();
                state.peers.clear();
            }
            let response = call(&s, 1, request(&s, &bytes, id)).await.unwrap();
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
        let response = call(&s, 1, request(&s, &bytes, 1)).await.unwrap();
        let run = finished(&s, response.1.0["run_id"].as_u64().unwrap()).await;
        assert_eq!(run.run.error_code.as_deref(), Some("worker_panic"));
        assert_eq!(run.run.stage, "failed");
        assert!(run.run.certificate_b64.is_none());
        let response = call(&s, 1, request(&s, &bytes, 2)).await.unwrap();
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
        let response = call(&s, 1, request(&s, &bytes, 1)).await.unwrap();
        let result = finished(&s, response.1.0["run_id"].as_u64().unwrap()).await;
        assert_eq!(result.run.stage, "done");
        assert!(result.run.certificate_b64.is_some());
        assert!(result.near_tx_hash.is_none());
        assert!(result.run.stages.iter().any(|s| s.name == "submit"
            && s.status == constitution_types::manifest::StageStatus::Incomplete));
        assert!(lock(&s.state).active.is_none());
    }
}
