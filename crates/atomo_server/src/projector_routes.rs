//! REST routes for CQRS projection management.

use atomo_projectors::ProjectorManager;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

/// Router for projection management, scoped to a shared ProjectorManager.
pub fn projector_router(manager: Arc<ProjectorManager>) -> Router {
    Router::new()
        .route("/projections", get(list_projections))
        .route("/projections/rebuild", post(rebuild_projections))
        .with_state(manager)
}

/// GET /projections - list registered projections (name + source model)
async fn list_projections(State(manager): State<Arc<ProjectorManager>>) -> Json<Value> {
    let items: Vec<Value> = manager
        .projections()
        .iter()
        .map(|p| json!({ "name": p.name(), "source_model": p.source_model() }))
        .collect();
    Json(json!({ "projections": items }))
}

/// POST /projections/rebuild - truncate and rebuild all projections
async fn rebuild_projections(
    State(manager): State<Arc<ProjectorManager>>,
) -> Result<Json<Value>, StatusCode> {
    match manager.rebuild_all().await {
        Ok(()) => Ok(Json(
            json!({ "status": "rebuilt", "count": manager.projections().len() }),
        )),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
