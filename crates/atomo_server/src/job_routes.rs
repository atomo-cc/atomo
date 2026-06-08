//! HTTP lease API for the durable job queue + worker-token auth.
//!
//! Two trust planes share the `/jobs` prefix:
//! - **Worker plane** (`X-Worker-Token`): `lease` / `heartbeat` / `complete` / `fail` — the pull
//!   side an external worker drives. The token is capability-scoped to specific queues.
//! - **Admin plane** (user JWT, Admin role): `POST /jobs/workers` mints worker tokens.
//!
//! Enqueueing from the app side (GraphQL mutation / workflow step / plugin effect) is a separate
//! slice; here jobs are put on the queue via `JobStore::enqueue` directly.

use crate::auth::{optional_auth_middleware, AuthUser, HttpAuthService};
use crate::jobs::{FailOutcome, JobStore, LeasedJob, WorkerIdentity, WorkerTokenStore};
use crate::platform_models::UserRole;
use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::post,
    Extension, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

fn default_capacity() -> i64 {
    1
}
fn default_visibility() -> i64 {
    30
}

/// Authenticate a worker by its `X-Worker-Token` header, injecting the verified `WorkerIdentity`
/// (with its allowed queues) into request extensions. 401 when the header is missing or invalid.
pub async fn worker_auth_middleware(
    State(store): State<Arc<WorkerTokenStore>>,
    mut req: Request,
    next: Next,
) -> Response {
    let token = match req
        .headers()
        .get("x-worker-token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
    {
        Some(t) => t,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    match store.verify(&token).await {
        Ok(Some(identity)) => {
            req.extensions_mut().insert(identity);
            next.run(req).await
        }
        Ok(None) => StatusCode::UNAUTHORIZED.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn leased_json(j: &LeasedJob) -> Value {
    json!({
        "id": j.id,
        "queue": j.queue,
        "kind": j.kind,
        "payload": j.payload,
        "attempts": j.attempts,
        "maxAttempts": j.max_attempts,
        "leaseId": j.lease_id,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeaseReq {
    queues: Vec<String>,
    #[serde(default = "default_capacity")]
    capacity: i64,
    #[serde(default = "default_visibility")]
    visibility_secs: i64,
}

async fn lease(
    State(jobs): State<Arc<JobStore>>,
    Extension(worker): Extension<WorkerIdentity>,
    Json(req): Json<LeaseReq>,
) -> Response {
    if req.queues.is_empty() {
        return (StatusCode::BAD_REQUEST, "queues must not be empty").into_response();
    }
    // Capability scoping: a worker may only lease from queues its token allows.
    if let Some(bad) = req.queues.iter().find(|q| !worker.may_lease(q)) {
        return (
            StatusCode::FORBIDDEN,
            format!("worker not authorized for queue '{bad}'"),
        )
            .into_response();
    }
    let cap = req.capacity.clamp(1, 100);
    let vis = req.visibility_secs.clamp(1, 86_400);
    match jobs.lease(&req.queues, cap, vis).await {
        Ok(leased) => {
            let arr: Vec<Value> = leased.iter().map(leased_json).collect();
            Json(json!({ "jobs": arr })).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeartbeatReq {
    lease_id: String,
    #[serde(default = "default_visibility")]
    visibility_secs: i64,
}

async fn heartbeat(
    State(jobs): State<Arc<JobStore>>,
    Extension(_worker): Extension<WorkerIdentity>,
    Path(id): Path<String>,
    Json(req): Json<HeartbeatReq>,
) -> Response {
    let vis = req.visibility_secs.clamp(1, 86_400);
    match jobs.heartbeat(&id, &req.lease_id, vis).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        // Lease lost (expired/reassigned) — tell the worker to stop.
        Ok(false) => StatusCode::CONFLICT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompleteReq {
    lease_id: String,
    #[serde(default)]
    result: Value,
}

async fn complete(
    State(jobs): State<Arc<JobStore>>,
    Extension(_worker): Extension<WorkerIdentity>,
    Path(id): Path<String>,
    Json(req): Json<CompleteReq>,
) -> Response {
    match jobs.complete(&id, &req.lease_id, req.result).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::CONFLICT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FailReq {
    lease_id: String,
    #[serde(default)]
    error: String,
    #[serde(default)]
    retryable: bool,
}

async fn fail(
    State(jobs): State<Arc<JobStore>>,
    Extension(_worker): Extension<WorkerIdentity>,
    Path(id): Path<String>,
    Json(req): Json<FailReq>,
) -> Response {
    match jobs
        .fail(&id, &req.lease_id, &req.error, req.retryable)
        .await
    {
        Ok(Some(FailOutcome::Retry { delay_secs })) => {
            Json(json!({ "outcome": "retry", "delaySecs": delay_secs })).into_response()
        }
        Ok(Some(FailOutcome::DeadLetter)) => Json(json!({ "outcome": "dead" })).into_response(),
        // Stale lease — no-op.
        Ok(None) => StatusCode::CONFLICT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct MintReq {
    name: String,
    #[serde(default)]
    queues: Vec<String>,
}

async fn mint_worker(
    State(workers): State<Arc<WorkerTokenStore>>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<MintReq>,
) -> Response {
    let user = match user {
        Some(Extension(u)) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    if !matches!(user.role, UserRole::Admin) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match workers.mint(&req.name, &req.queues).await {
        // The plaintext token is returned ONCE — it is not recoverable later.
        Ok((id, token)) => (
            StatusCode::CREATED,
            Json(json!({ "id": id, "token": token, "queues": req.queues })),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `/jobs/*`: worker-token-authed pull routes + an admin-authed token-mint route.
pub fn jobs_router(
    jobs: Arc<JobStore>,
    workers: Arc<WorkerTokenStore>,
    auth: HttpAuthService,
) -> Router {
    let worker_routes = Router::new()
        .route("/jobs/lease", post(lease))
        .route("/jobs/{id}/heartbeat", post(heartbeat))
        .route("/jobs/{id}/complete", post(complete))
        .route("/jobs/{id}/fail", post(fail))
        .route_layer(middleware::from_fn_with_state(
            workers.clone(),
            worker_auth_middleware,
        ))
        .with_state(jobs);

    let admin_routes = Router::new()
        .route("/jobs/workers", post(mint_worker))
        .route_layer(middleware::from_fn_with_state(
            auth,
            optional_auth_middleware,
        ))
        .with_state(workers);

    worker_routes.merge(admin_routes)
}
