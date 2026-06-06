//! Control-plane HTTP API over the registry + provisioner. (Phase 2.)
//!
//! CRUD projects and trigger lifecycle actions. Operator-authenticated; this credential
//! never grants access to project data.
//!
//! Endpoints:
//! - `GET    /healthz`                — control-plane liveness
//! - `GET    /projects`              — list the fleet
//! - `POST   /projects`             — register + provision a project
//! - `GET    /projects/{id}`        — fetch one project
//! - `POST   /projects/{id}/start`  — converge to running
//! - `POST   /projects/{id}/stop`   — converge to stopped
//! - `POST   /projects/{id}/schema` — bump schema to a new commit SHA
//! - `GET    /projects/{id}/health` — last recorded health probe
//! - `DELETE /projects/{id}?drop_database=bool` — deprovision (DB drop guarded)

use crate::error::ControlPlaneError;
use crate::provisioner::Provisioner;
use crate::registry::{DesiredState, Project, ProjectStatus, SchemaRef};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Build the control-plane axum router over a shared [`Provisioner`].
pub fn router(provisioner: Arc<Provisioner>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/projects", get(list_projects).post(create_project))
        .route("/projects/{id}", get(get_project).delete(delete_project))
        .route("/projects/{id}/start", post(start_project))
        .route("/projects/{id}/stop", post(stop_project))
        .route("/projects/{id}/schema", post(schema_update))
        .route("/projects/{id}/health", get(project_health))
        .with_state(provisioner)
}

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

/// Operator-facing error envelope. Maps [`ControlPlaneError`] → an HTTP status.
struct ApiError(ControlPlaneError);

impl From<ControlPlaneError> for ApiError {
    fn from(e: ControlPlaneError) -> Self {
        ApiError(e)
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            ControlPlaneError::NotFound(_) => StatusCode::NOT_FOUND,
            ControlPlaneError::InvalidState(_) => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = Json(ErrorBody {
            error: self.0.to_string(),
        });
        (status, body).into_response()
    }
}

type ApiResult<T> = std::result::Result<T, ApiError>;

// ---------------------------------------------------------------------------
// Request / response bodies
// ---------------------------------------------------------------------------

/// Body for `POST /projects`. Mirrors [`Project`] minus server-managed fields
/// (status, timestamps, health), which the control plane fills in.
#[derive(Debug, Deserialize)]
struct CreateProject {
    id: String,
    display_name: String,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    database_url_ref: String,
    schema_ref: SchemaRef,
    #[serde(default)]
    schema_version: Option<String>,
    #[serde(default)]
    upstream: Option<String>,
    #[serde(default = "default_env")]
    env: serde_json::Value,
    #[serde(default = "default_desired_state")]
    desired_state: DesiredState,
}

fn default_env() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn default_desired_state() -> DesiredState {
    DesiredState::Running
}

impl CreateProject {
    fn into_project(self) -> Project {
        let now = chrono::Utc::now();
        Project {
            id: self.id,
            display_name: self.display_name,
            hostname: self.hostname,
            aliases: self.aliases,
            database_url_ref: self.database_url_ref,
            schema_ref: self.schema_ref,
            schema_version: self.schema_version,
            upstream: self.upstream,
            env: self.env,
            status: ProjectStatus::Provisioning,
            desired_state: self.desired_state,
            last_health: None,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Body for `POST /projects/{id}/schema` — `{ "ref": "<commit-sha>" }`.
#[derive(Debug, Deserialize)]
struct SchemaUpdate {
    #[serde(rename = "ref")]
    git_ref: String,
}

/// Query for `DELETE /projects/{id}?drop_database=bool`.
#[derive(Debug, Deserialize)]
struct DeleteQuery {
    #[serde(default)]
    drop_database: bool,
}

#[derive(Serialize)]
struct Accepted {
    id: String,
    action: &'static str,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Control-plane liveness — does not touch the registry or any project.
async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

async fn list_projects(State(p): State<Arc<Provisioner>>) -> ApiResult<Json<Vec<Project>>> {
    let projects = p.registry.list().await?;
    Ok(Json(projects))
}

async fn create_project(
    State(p): State<Arc<Provisioner>>,
    Json(body): Json<CreateProject>,
) -> ApiResult<(StatusCode, Json<Project>)> {
    let project = body.into_project();
    p.create(project.clone()).await?;
    // Return the registry's view (canonical status/timestamps) if available.
    let created = p.registry.get(&project.id).await.unwrap_or(project);
    Ok((StatusCode::CREATED, Json(created)))
}

async fn get_project(
    State(p): State<Arc<Provisioner>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Project>> {
    let project = p.registry.get(&id).await?;
    Ok(Json(project))
}

async fn start_project(
    State(p): State<Arc<Provisioner>>,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Accepted>)> {
    p.start(&id).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(Accepted {
            id,
            action: "start",
        }),
    ))
}

async fn stop_project(
    State(p): State<Arc<Provisioner>>,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Accepted>)> {
    p.stop(&id).await?;
    Ok((StatusCode::ACCEPTED, Json(Accepted { id, action: "stop" })))
}

async fn schema_update(
    State(p): State<Arc<Provisioner>>,
    Path(id): Path<String>,
    Json(body): Json<SchemaUpdate>,
) -> ApiResult<(StatusCode, Json<Accepted>)> {
    p.schema_update(&id, &body.git_ref).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(Accepted {
            id,
            action: "schema_update",
        }),
    ))
}

async fn delete_project(
    State(p): State<Arc<Provisioner>>,
    Path(id): Path<String>,
    Query(q): Query<DeleteQuery>,
) -> ApiResult<(StatusCode, Json<Accepted>)> {
    p.delete(&id, q.drop_database).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(Accepted {
            id,
            action: "delete",
        }),
    ))
}

async fn project_health(
    State(p): State<Arc<Provisioner>>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let project = p.registry.get(&id).await?;
    Ok(Json(project.last_health.unwrap_or(serde_json::Value::Null)))
}
