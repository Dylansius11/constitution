use crate::{
    identity::Identity,
    pipeline::{self, Run},
    proposals::ProposalSource,
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
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
pub struct Service {
    pub identity: Identity,
    pub manifest: RuleSetManifest,
    pub repo: PathBuf,
    pub proposals: Box<dyn ProposalSource>,
    runs: Mutex<BTreeMap<u64, Run>>,
    active: Mutex<Option<(AccountId, u64)>>,
    attempts: Mutex<BTreeMap<(AccountId, u64), u32>>,
}
impl Service {
    pub fn new(
        identity: Identity,
        manifest: RuleSetManifest,
        repo: PathBuf,
        proposals: Box<dyn ProposalSource>,
    ) -> Result<Arc<Self>, String> {
        constitution_types::manifest::validate_rule_set(&manifest)?;
        Ok(Arc::new(Self {
            identity,
            manifest,
            repo,
            proposals,
            runs: Mutex::new(BTreeMap::new()),
            active: Mutex::new(None),
            attempts: Mutex::new(BTreeMap::new()),
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
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, String)> {
    let bad = |e: String| (StatusCode::BAD_REQUEST, e);
    let p = s
        .proposals
        .get(&request.gate, request.proposal_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or((StatusCode::NOT_FOUND, "proposal_not_found".into()))?;
    if p.rule_set_hash.0 != rule_set_hash(&s.manifest).map_err(|e| bad(e.to_string()))?
        || p.expires_at_s <= pipeline::now()
    {
        return Err(bad("proposal expired or unsupported rule set".into()));
    }
    crate::bundle::open(&s.identity.transport, &request.sealed_bundle_b64.0).map_err(bad)?;
    let mut active = s.active.lock().expect("active mutex");
    if let Some(key) = active.as_ref() {
        return Err(if key == &(request.gate.clone(), request.proposal_id) {
            (StatusCode::CONFLICT, "run_in_progress".into())
        } else {
            (StatusCode::TOO_MANY_REQUESTS, "queue_full".into())
        });
    }
    let mut runs = s.runs.lock().expect("runs mutex");
    // A bounded local process retains evidence in memory; restart explicitly to clear it.
    if runs.len() >= 64 {
        return Err((StatusCode::TOO_MANY_REQUESTS, "queue_full".into()));
    }
    let id = runs.len() as u64 + 1;
    runs.insert(id, Run::queued());
    *active = Some((request.gate.clone(), request.proposal_id));
    let mut attempts = s.attempts.lock().expect("attempt mutex");
    let attempt = attempts
        .entry((request.gate, request.proposal_id))
        .or_default();
    *attempt += 1;
    let attempt = *attempt;
    drop(attempts);
    drop(runs);
    drop(active);
    tokio::task::spawn_blocking(move || {
        let weak = Arc::downgrade(&s);
        let observer = Arc::new(move |run: &Run| {
            if let Some(service) = weak.upgrade() {
                service
                    .runs
                    .lock()
                    .expect("runs mutex")
                    .insert(id, run.snapshot());
            }
        });
        let result = pipeline::execute_observed(
            &s.repo,
            &s.manifest,
            &s.identity,
            &p,
            &request.sealed_bundle_b64.0,
            attempt,
            Some(observer),
        );
        s.runs.lock().expect("runs mutex").insert(id, result);
        *s.active.lock().expect("active mutex") = None;
    });
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"run_id":id,"label":"local, not attested"})),
    ))
}
async fn status(
    State(s): State<Arc<Service>>,
    Path(id): Path<u64>,
) -> Result<Json<Run>, StatusCode> {
    s.runs
        .lock()
        .expect("runs mutex")
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
async fn evidence(
    State(s): State<Arc<Service>>,
    Path(id): Path<u64>,
) -> Result<Json<constitution_types::EvidenceManifest>, StatusCode> {
    s.runs
        .lock()
        .expect("runs mutex")
        .get(&id)
        .and_then(|r| r.evidence.clone())
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
