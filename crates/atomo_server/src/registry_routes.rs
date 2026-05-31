//! REST routes for the plugin marketplace registry (read-only, milestone 1).

use crate::registry::RegistryStore;
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// Router for registry reads, scoped to a shared RegistryStore.
pub fn registry_router(store: Arc<RegistryStore>) -> Router {
    Router::new()
        .route("/registry/plugins", get(search_plugins))
        .route("/registry/plugins/{name}", get(get_plugin))
        .route(
            "/registry/plugins/{name}/{version}/download",
            get(download_artifact),
        )
        .with_state(store)
}

/// GET /registry/plugins?q= — search (empty q lists all).
async fn search_plugins(
    State(store): State<Arc<RegistryStore>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let q = params.get("q").map(|s| s.as_str()).unwrap_or("");
    match store.search(q).await {
        Ok(items) => Ok(Json(json!({ "plugins": items }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// GET /registry/plugins/{name} — metadata + versions (404 if unknown).
async fn get_plugin(
    State(store): State<Arc<RegistryStore>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    match store.get_plugin(&name).await {
        Ok(Some(plugin)) => Ok(Json(plugin)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// GET /registry/plugins/{name}/{version}/download — artifact bytes (404 if missing).
async fn download_artifact(
    State(store): State<Arc<RegistryStore>>,
    Path((name, version)): Path<(String, String)>,
) -> impl IntoResponse {
    match store.get_artifact(&name, &version).await {
        Ok(Some(bytes)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/wasm")],
            bytes,
        )
            .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
