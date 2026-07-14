//! HTTP API for the durable job queue + worker-token auth.
//!
//! Three trust planes share the `/jobs` prefix:
//! - **Worker plane** (`X-Worker-Token`): `lease` / `{id}/heartbeat` / `{id}/complete` / `{id}/fail`
//!   / `{id}/progress` — the pull side an external worker drives. The token is capability-scoped to
//!   specific queues. `progress` publishes an ephemeral update to the job's realtime channel.
//! - **App plane** (user JWT): `POST /jobs` enqueues work (stamped with the caller's tenant);
//!   `GET /jobs/{id}` polls a job's status/result (tenant-scoped). (Richer enqueue seams — GraphQL
//!   mutation, plugin effect — can follow; the workflow `Job` step already enqueues.)
//! - **Admin plane** (user JWT, Admin role): `POST /jobs/workers` mints worker tokens.

use crate::auth::{optional_auth_middleware, AuthUser, HttpAuthService};
use crate::jobs::{FailOutcome, JobStore, LeasedJob, WorkerIdentity, WorkerTokenStore};
use crate::platform_models::UserRole;
use atomo_realtime::{ClientHandle, ClientMsg};
use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
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
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "missing worker token"})),
            )
                .into_response()
        }
    };
    match store.verify(&token).await {
        Ok(Some(identity)) => {
            req.extensions_mut().insert(identity);
            next.run(req).await
        }
        Ok(None) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid worker token"})),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal server error"})),
        )
            .into_response(),
    }
}

/// Non-fatal variant of [`worker_auth_middleware`]: injects the verified `WorkerIdentity` when a
/// valid `X-Worker-Token` is present, and otherwise passes the request through unchanged (no 401).
/// Lets a route accept *either* a user JWT or a worker token — e.g. media upload, where an external
/// worker stores generated artifacts with its worker token rather than a user session.
pub async fn optional_worker_auth_middleware(
    State(store): State<Arc<WorkerTokenStore>>,
    mut req: Request,
    next: Next,
) -> Response {
    if let Some(token) = req
        .headers()
        .get("x-worker-token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
    {
        if let Ok(Some(identity)) = store.verify(&token).await {
            req.extensions_mut().insert(identity);
        }
    }
    next.run(req).await
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
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
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
        Ok(false) => (StatusCode::CONFLICT, Json(json!({"error": "lease lost"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
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
        Ok(false) => (StatusCode::CONFLICT, Json(json!({"error": "lease lost"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
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
        Ok(None) => (StatusCode::CONFLICT, Json(json!({"error": "lease lost"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Realtime channel a job's live progress is published to. A client that enqueued job `id`
/// subscribes to `job:{id}` over `/realtime/ws` to watch it.
pub fn progress_channel(id: &str) -> String {
    format!("job:{id}")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgressReq {
    lease_id: String,
    #[serde(default = "default_visibility")]
    visibility_secs: i64,
    percent: Option<f64>,
    message: Option<String>,
    #[serde(default)]
    data: Value,
}

/// Report live progress for a leased job. Validates+extends the lease (progress is a sign of life,
/// like a heartbeat), then publishes an ephemeral message to the job's realtime channel — it is
/// **not** written to the durable event log. `409` if the lease was lost.
async fn progress(
    State((jobs, publisher)): State<(Arc<JobStore>, Option<ClientHandle>)>,
    Extension(_worker): Extension<WorkerIdentity>,
    Path(id): Path<String>,
    Json(req): Json<ProgressReq>,
) -> Response {
    let vis = req.visibility_secs.clamp(1, 86_400);
    match jobs.heartbeat(&id, &req.lease_id, vis).await {
        Ok(true) => {
            if let Some(pubr) = &publisher {
                let body = json!({
                    "jobId": id,
                    "percent": req.percent,
                    "message": req.message,
                    "data": req.data,
                });
                if let Ok(raw) = serde_json::value::to_raw_value(&body) {
                    pubr.dispatch(ClientMsg::Publish {
                        channel: progress_channel(&id),
                        payload: raw,
                    })
                    .await;
                }
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (StatusCode::CONFLICT, Json(json!({"error": "lease lost"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

fn default_max_attempts() -> i32 {
    5
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnqueueReq {
    queue: String,
    kind: String,
    #[serde(default)]
    payload: Value,
    #[serde(default)]
    idempotency_key: Option<String>,
    #[serde(default = "default_max_attempts")]
    max_attempts: i32,
    #[serde(default)]
    priority: i32,
}

/// App-side enqueue. Any authenticated user may submit work; the job is stamped with the caller's
/// tenant (so tenant-scoped queues/RLS apply). Idempotent when `idempotencyKey` is given.
async fn enqueue(
    State(jobs): State<Arc<JobStore>>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<EnqueueReq>,
) -> Response {
    let user = match user {
        Some(Extension(u)) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "authentication required"})),
            )
                .into_response()
        }
    };
    if req.queue.is_empty() || req.kind.is_empty() {
        return (StatusCode::BAD_REQUEST, "queue and kind are required").into_response();
    }
    let payload = if req.payload.is_null() {
        json!({})
    } else {
        req.payload
    };
    let max_attempts = req.max_attempts.clamp(1, 1000);
    match jobs
        .enqueue(
            &req.queue,
            &req.kind,
            payload,
            req.idempotency_key.as_deref(),
            max_attempts,
            req.priority,
            user.tenant_id.as_deref(),
        )
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(json!({ "id": id }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// App-side status poll. Returns the job's current state + result. A tenant-bound user cannot read
/// another tenant's job (treated as not found).
async fn get_job(
    State(jobs): State<Arc<JobStore>>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<String>,
) -> Response {
    let user = match user {
        Some(Extension(u)) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "authentication required"})),
            )
                .into_response()
        }
    };
    match jobs.get(&id).await {
        Ok(Some(v)) => {
            if let Some(t) = &v.tenant_id {
                if user.tenant_id.as_deref() != Some(t.as_str()) {
                    return (StatusCode::NOT_FOUND, Json(json!({"error": "not found"})))
                        .into_response();
                }
            }
            Json(json!({
                "id": v.id,
                "queue": v.queue,
                "kind": v.kind,
                "status": v.status,
                "attempts": v.attempts,
                "maxAttempts": v.max_attempts,
                "result": v.result,
                "error": v.error,
            }))
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /jobs/stats — queue-health aggregates for the admin observability panel.
async fn job_stats(
    State(jobs): State<Arc<JobStore>>,
    user: Option<Extension<AuthUser>>,
) -> Response {
    if let Some(resp) = require_admin(user) {
        return resp;
    }
    match jobs.stats().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /jobs/recent?queue=&status=&limit=&offset= — newest jobs first, no payload bodies.
async fn recent_jobs(
    State(jobs): State<Arc<JobStore>>,
    user: Option<Extension<AuthUser>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    if let Some(resp) = require_admin(user) {
        return resp;
    }
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(25);
    let offset = params
        .get("offset")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    match jobs
        .list_recent(
            params.get("queue").map(String::as_str),
            params.get("status").map(String::as_str),
            limit,
            offset,
        )
        .await
    {
        Ok(items) => Json(json!({ "jobs": items })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct MintReq {
    name: String,
    #[serde(default)]
    queues: Vec<String>,
    #[serde(default)]
    capabilities: Vec<String>,
}

async fn mint_worker(
    State(workers): State<Arc<WorkerTokenStore>>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<MintReq>,
) -> Response {
    if let Some(resp) = require_admin(user) {
        return resp;
    }
    match workers
        .mint(&req.name, &req.queues, &req.capabilities)
        .await
    {
        Ok((id, token)) => (
            StatusCode::CREATED,
            Json(json!({
                "id": id, "token": token,
                "queues": req.queues, "capabilities": req.capabilities,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// `Some(error_response)` if the caller is not an authenticated Admin, else `None`.
fn require_admin(user: Option<Extension<AuthUser>>) -> Option<Response> {
    match user {
        Some(u) if matches!(u.role, UserRole::Admin) => None,
        Some(_) => Some(
            (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "admin access required"})),
            )
                .into_response(),
        ),
        None => Some(
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "authentication required"})),
            )
                .into_response(),
        ),
    }
}

async fn list_workers(
    State(workers): State<Arc<WorkerTokenStore>>,
    user: Option<Extension<AuthUser>>,
) -> Response {
    if let Some(resp) = require_admin(user) {
        return resp;
    }
    match workers.list().await {
        Ok(tokens) => {
            // Metadata only — never the token/hash.
            let arr: Vec<Value> = tokens
                .iter()
                .map(|t| {
                    json!({
                        "id": t.id,
                        "name": t.name,
                        "queues": t.queues,
                        "capabilities": t.capabilities,
                        "isRevoked": t.is_revoked,
                        "createdAt": t.created_at.to_rfc3339(),
                    })
                })
                .collect();
            Json(json!({ "workers": arr })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn revoke_worker(
    State(workers): State<Arc<WorkerTokenStore>>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<String>,
) -> Response {
    if let Some(resp) = require_admin(user) {
        return resp;
    }
    match workers.revoke(&id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "worker not found or already revoked"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// `/jobs/*`: three trust planes merged —
/// - worker-token pull routes (`lease`/`heartbeat`/`complete`/`fail`),
/// - user-authed app-side `enqueue` (`POST /jobs`) + status poll (`GET /jobs/{id}`),
/// - admin-authed worker-token lifecycle (`POST`/`GET /jobs/workers`, `DELETE /jobs/workers/{id}`).
pub fn jobs_router(
    jobs: Arc<JobStore>,
    workers: Arc<WorkerTokenStore>,
    auth: HttpAuthService,
    progress_publisher: Option<ClientHandle>,
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
        .with_state(jobs.clone());

    // Progress reporting carries the realtime publisher alongside the store, so it lives in its own
    // worker-token-authed sub-router (distinct state type from the routes above).
    let progress_route = Router::new()
        .route("/jobs/{id}/progress", post(progress))
        .route_layer(middleware::from_fn_with_state(
            workers.clone(),
            worker_auth_middleware,
        ))
        .with_state((jobs.clone(), progress_publisher));

    // App-side enqueue + status poll: any authenticated user.
    let jobs_for_observability = jobs.clone();
    let app_routes = Router::new()
        .route("/jobs", post(enqueue))
        .route("/jobs/{id}", get(get_job))
        .route_layer(middleware::from_fn_with_state(
            auth.clone(),
            optional_auth_middleware,
        ))
        .with_state(jobs);

    // Worker-token lifecycle: Admin only (checked in the handlers).
    let admin_routes = Router::new()
        .route("/jobs/workers", post(mint_worker).get(list_workers))
        .route("/jobs/workers/{id}", delete(revoke_worker))
        .route_layer(middleware::from_fn_with_state(
            auth.clone(),
            optional_auth_middleware,
        ))
        .with_state(workers);

    // Queue observability (admin only, checked in the handlers): stats + recent
    // jobs for the admin panel. Static segments, so they coexist with the
    // `/jobs/{id}` param route (axum resolves static first).
    let observability_routes = Router::new()
        .route("/jobs/stats", get(job_stats))
        .route("/jobs/recent", get(recent_jobs))
        .route_layer(middleware::from_fn_with_state(
            auth,
            optional_auth_middleware,
        ))
        .with_state(jobs_for_observability);

    worker_routes
        .merge(progress_route)
        .merge(app_routes)
        .merge(admin_routes)
        .merge(observability_routes)
}
