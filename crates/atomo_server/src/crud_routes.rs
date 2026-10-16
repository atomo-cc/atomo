//! Internal worker CRUD API: `POST/GET/PATCH/DELETE /api/worker/crud/:model`.
//!
//! These endpoints let external workers (authenticated via `X-Worker-Token` with the
//! `crud:*` capability) call back into Atomo's CRUD layer with all invariants preserved:
//! validation, RBAC (via the job's actor snapshot), event sourcing, and RLS.
//!
//! Mutation requests (create/update/delete) accept an optional `origin` field naming the
//! action that triggered the call; the action dispatcher uses it to avoid re-enqueuing
//! the originating action (loop prevention).

use atomo::client::{with_action_origin, AtomoClient};
use atomo::graphql::parse_where;
use atomo::query::{WhereClause, WhereOperator};
use atomo::schema::Schema;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::job_routes::worker_auth_middleware;
use crate::jobs::{WorkerIdentity, WorkerTokenStore};
use atomo::security::{with_principal, Principal};

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct CrudState {
    client: Arc<AtomoClient>,
    schema: Schema,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActorSnapshot {
    job_id: Option<String>,
    lease_id: Option<String>,
    user_id: Option<String>,
    role: Option<String>,
    tenant_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateReq {
    data: std::collections::HashMap<String, Value>,
    #[serde(default)]
    actor: Option<ActorSnapshot>,
    #[serde(default)]
    origin: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateReq {
    data: std::collections::HashMap<String, Value>,
    #[serde(default)]
    actor: Option<ActorSnapshot>,
    #[serde(default)]
    origin: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteReq {
    #[serde(default)]
    actor: Option<ActorSnapshot>,
    #[serde(default)]
    origin: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindManyParams {
    #[serde(default, rename = "where")]
    where_json: Option<String>,
    #[serde(default)]
    limit: Option<i32>,
    #[serde(default)]
    offset: Option<i32>,
    #[serde(default)]
    actor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindByIdParams {
    #[serde(default)]
    actor: Option<String>,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn capability_check(worker: &WorkerIdentity, model: &str, op: &str) -> Option<Response> {
    if !worker.may_crud(model, op) {
        Some(
            (
                StatusCode::FORBIDDEN,
                Json(
                    json!({"error": format!("worker lacks crud capability for {}.{}", model, op)}),
                ),
            )
                .into_response(),
        )
    } else {
        None
    }
}

fn model_check(schema: &Schema, model: &str) -> Option<Response> {
    if !schema.models.contains_key(model) {
        Some(
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("model '{}' not found", model)})),
            )
                .into_response(),
        )
    } else {
        None
    }
}

fn error_response(e: anyhow::Error) -> Response {
    let msg = e.to_string();
    if msg.contains("validation failed") {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": msg})),
        )
            .into_response()
    } else if msg.contains("Access denied") || msg.contains("Authentication required") {
        (StatusCode::FORBIDDEN, Json(json!({"error": msg}))).into_response()
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": msg})),
        )
            .into_response()
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn verified_actor(
    state: &CrudState,
    worker: &WorkerIdentity,
    actor: Option<&ActorSnapshot>,
) -> Result<Principal, anyhow::Error> {
    let Some(actor) = actor else {
        return Ok(Principal::default());
    };
    let job_id = actor
        .job_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Access denied: verified job delegation required"))?;
    let lease_id = actor
        .lease_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Access denied: active job lease required"))?;
    let (id, tenant, queue): (Option<String>, Option<String>, String) = sqlx::query_as("SELECT delegated_user_id, tenant_id, queue FROM jobs WHERE id=$1 AND lease_id=$2 AND status='leased' AND visible_at > NOW()")
        .bind(job_id).bind(lease_id).fetch_optional(state.client.db_pool()).await?.ok_or_else(|| anyhow::anyhow!("Access denied: invalid or expired job delegation"))?;
    if !worker.may_lease(&queue) {
        anyhow::bail!("Access denied: worker queue scope");
    }
    let id = id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Access denied: job has no verified delegation"))?;
    if actor
        .user_id
        .as_deref()
        .is_some_and(|claimed| claimed != id)
    {
        anyhow::bail!("Access denied: forged actor identity");
    }
    if actor
        .tenant_id
        .as_deref()
        .is_some_and(|claimed| tenant.as_deref() != Some(claimed))
    {
        anyhow::bail!("Access denied: forged job tenant");
    }
    let (role, bound): (String, Option<String>) =
        sqlx::query_as("SELECT role, tenant_id FROM users WHERE id = $1 AND is_active = true")
            .bind(id)
            .fetch_optional(state.client.db_pool())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Access denied: actor not active"))?;
    if actor
        .role
        .as_deref()
        .is_some_and(|claimed| !claimed.eq_ignore_ascii_case(&role))
    {
        anyhow::bail!("Access denied: forged actor role");
    }
    let tenant_id = atomo::security::resolve_tenant(
        bound.as_deref(),
        tenant.as_deref(),
        role.eq_ignore_ascii_case("admin"),
    )?;
    Ok(Principal {
        user_id: Some(id.into()),
        role: Some(role),
        tenant_id,
    })
}

async fn create(
    State(state): State<CrudState>,
    Extension(worker): Extension<WorkerIdentity>,
    Path(model): Path<String>,
    Json(req): Json<CreateReq>,
) -> Response {
    if let Some(r) = capability_check(&worker, &model, "create") {
        return r;
    }
    if let Some(r) = model_check(&state.schema, &model) {
        return r;
    }
    let actor = req.actor.as_ref();
    let principal = match verified_actor(&state, &worker, actor).await {
        Ok(p) => p,
        Err(e) => return error_response(e),
    };
    let role = principal.role.as_deref();
    let user_id = principal.user_id.as_deref();

    let result = with_action_origin(
        req.origin,
        with_principal(
            principal.clone(),
            state
                .client
                .create_checked(role, &model, &req.data, &[], user_id),
        ),
    )
    .await;
    match result {
        Ok(record) => (StatusCode::CREATED, Json(json!(record))).into_response(),
        Err(e) => error_response(e),
    }
}

async fn find_by_id(
    State(state): State<CrudState>,
    Extension(worker): Extension<WorkerIdentity>,
    Path((model, id)): Path<(String, String)>,
    Query(params): Query<FindByIdParams>,
) -> Response {
    if let Some(r) = capability_check(&worker, &model, "read") {
        return r;
    }
    if let Some(r) = model_check(&state.schema, &model) {
        return r;
    }
    let actor: Option<ActorSnapshot> = params
        .actor
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());
    let principal = match verified_actor(&state, &worker, actor.as_ref()).await {
        Ok(p) => p,
        Err(e) => return error_response(e),
    };
    let role = principal.role.as_deref();

    if let Err(e) = state.client.enforce_access(&model, "read", role) {
        return error_response(e);
    }

    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: Value::String(id),
    }];

    let result = with_principal(
        principal.clone(),
        state.client.find_unique(&model, &where_clauses, &[]),
    )
    .await;
    match result {
        Ok(Some(record)) => Json(json!(record)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response(),
        Err(e) => error_response(e),
    }
}

async fn update(
    State(state): State<CrudState>,
    Extension(worker): Extension<WorkerIdentity>,
    Path((model, id)): Path<(String, String)>,
    Json(req): Json<UpdateReq>,
) -> Response {
    if let Some(r) = capability_check(&worker, &model, "update") {
        return r;
    }
    if let Some(r) = model_check(&state.schema, &model) {
        return r;
    }
    let actor = req.actor.as_ref();
    let principal = match verified_actor(&state, &worker, actor).await {
        Ok(p) => p,
        Err(e) => return error_response(e),
    };
    let role = principal.role.as_deref();
    let user_id = principal.user_id.as_deref();

    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: Value::String(id),
    }];

    let result = with_action_origin(
        req.origin,
        with_principal(
            principal.clone(),
            state
                .client
                .update_many_checked(role, &model, &where_clauses, &req.data, &[], user_id),
        ),
    )
    .await;
    match result {
        Ok(records) => match records.into_iter().next() {
            Some(record) => Json(json!(record)).into_response(),
            None => (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response(),
        },
        Err(e) => error_response(e),
    }
}

async fn delete_one(
    State(state): State<CrudState>,
    Extension(worker): Extension<WorkerIdentity>,
    Path((model, id)): Path<(String, String)>,
    body: Option<Json<DeleteReq>>,
) -> Response {
    if let Some(r) = capability_check(&worker, &model, "delete") {
        return r;
    }
    if let Some(r) = model_check(&state.schema, &model) {
        return r;
    }
    let actor = body.as_ref().and_then(|b| b.actor.as_ref());
    let principal = match verified_actor(&state, &worker, actor).await {
        Ok(p) => p,
        Err(e) => return error_response(e),
    };
    let role = principal.role.as_deref();
    let user_id = principal.user_id.as_deref();

    let where_clauses = vec![WhereClause {
        field: "id".to_string(),
        operator: WhereOperator::Equals,
        value: Value::String(id),
    }];

    let origin = body.as_ref().and_then(|b| b.origin.clone());
    let result = with_action_origin(
        origin,
        with_principal(
            principal.clone(),
            state
                .client
                .delete_many_checked(role, &model, &where_clauses, user_id),
        ),
    )
    .await;
    match result {
        Ok(count) => Json(json!({"deleted": count})).into_response(),
        Err(e) => error_response(e),
    }
}

async fn find_many(
    State(state): State<CrudState>,
    Extension(worker): Extension<WorkerIdentity>,
    Path(model): Path<String>,
    Query(params): Query<FindManyParams>,
) -> Response {
    if let Some(r) = capability_check(&worker, &model, "read") {
        return r;
    }
    if let Some(r) = model_check(&state.schema, &model) {
        return r;
    }
    let actor: Option<ActorSnapshot> = params
        .actor
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());
    let principal = match verified_actor(&state, &worker, actor.as_ref()).await {
        Ok(p) => p,
        Err(e) => return error_response(e),
    };
    let role = principal.role.as_deref();

    if let Err(e) = state.client.enforce_access(&model, "read", role) {
        return error_response(e);
    }

    let where_clauses: Vec<WhereClause> = params
        .where_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .map(|v| parse_where(&v))
        .unwrap_or_default();

    let limit = params.limit.unwrap_or(50).min(200) as usize;
    let offset = params.offset.unwrap_or(0).max(0) as usize;

    let result = with_principal(
        principal.clone(),
        state
            .client
            .find_many(&model, &where_clauses, &[], Some(limit), Some(offset), &[]),
    )
    .await;
    match result {
        Ok(records) => Json(json!(records)).into_response(),
        Err(e) => error_response(e),
    }
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn crud_router(
    client: Arc<AtomoClient>,
    schema: Schema,
    worker_tokens: Arc<WorkerTokenStore>,
) -> Router {
    let state = CrudState { client, schema };
    Router::new()
        .route("/api/worker/crud/{model}", post(create).get(find_many))
        .route(
            "/api/worker/crud/{model}/{id}",
            get(find_by_id).patch(update).delete(delete_one),
        )
        .route_layer(middleware::from_fn_with_state(
            worker_tokens,
            worker_auth_middleware,
        ))
        .with_state(state)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_gate_crud_star_allows_all() {
        let w = WorkerIdentity {
            id: "1".into(),
            name: "w".into(),
            queues: vec![],
            capabilities: vec!["crud:*".into()],
        };
        assert!(w.may_crud("Post", "create"));
        assert!(w.may_crud("User", "delete"));
        assert!(w.may_crud("Article", "read"));
    }

    #[test]
    fn capability_gate_empty_denies() {
        let w = WorkerIdentity {
            id: "1".into(),
            name: "w".into(),
            queues: vec!["actions".into()],
            capabilities: vec![],
        };
        assert!(!w.may_crud("Post", "create"));
    }

    #[test]
    fn capability_gate_unrelated_cap_denies() {
        let w = WorkerIdentity {
            id: "1".into(),
            name: "w".into(),
            queues: vec![],
            capabilities: vec!["audit:read".into()],
        };
        assert!(!w.may_crud("Post", "create"));
    }

    #[test]
    fn capability_gate_model_scoped() {
        let w = WorkerIdentity {
            id: "1".into(),
            name: "w".into(),
            queues: vec![],
            capabilities: vec!["crud:Post:read,update".into()],
        };
        assert!(w.may_crud("Post", "read"));
        assert!(w.may_crud("Post", "update"));
        assert!(!w.may_crud("Post", "create"), "unlisted op");
        assert!(!w.may_crud("Post", "delete"), "unlisted op");
        assert!(!w.may_crud("User", "read"), "different model");
    }

    #[test]
    fn model_check_rejects_unknown() {
        let schema = Schema {
            models: std::collections::HashMap::new(),
            actions: std::collections::HashMap::new(),
            builtins: Default::default(),
            warnings: Vec::new(),
        };
        assert!(model_check(&schema, "Ghost").is_some());
    }

    #[test]
    fn model_check_passes_known() {
        let mut models = std::collections::HashMap::new();
        models.insert(
            "Post".to_string(),
            atomo::schema::Model {
                name: "Post".to_string(),
                table_name: Some("post".to_string()),
                fields: std::collections::HashMap::new(),
                relationships: std::collections::HashMap::new(),
                validation: std::collections::HashMap::new(),
                access: None,
                hooks: None,
                constraints: vec![],
                events: Default::default(),
                ui: None,
                tenant_policy: Default::default(),
            },
        );
        let schema = Schema {
            models,
            actions: std::collections::HashMap::new(),
            builtins: Default::default(),
            warnings: Vec::new(),
        };
        assert!(model_check(&schema, "Post").is_none());
    }
}
