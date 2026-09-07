//! Read-only, administrator-only storage lifecycle diagnostics.
use atomo::Atomo;
use axum::{extract::State, http::StatusCode, response::Json, Extension};
use serde_json::{json, Value};

use crate::{audit::HttpAuditService, auth::AuthUser, platform_models::UserRole};

pub async fn diagnostics(
    State(audit): State<HttpAuditService>,
    Extension(atomo): Extension<Atomo>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    if user.role != UserRole::Admin {
        return Err(StatusCode::FORBIDDEN);
    }
    let history = atomo
        .client()
        .history_store()
        .diagnostics()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "history diagnostics unavailable");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    let audit = audit.diagnostics().await.map_err(|error| {
        tracing::warn!(%error, "audit diagnostics unavailable");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    Ok(Json(json!({
        "cache": atomo.client().cache_status().await,
        "history": history,
        "audit": audit,
        "capacity_semantics": "estimated payload budgets; database indexes, WAL and allocator overhead are separate"
    })))
}
