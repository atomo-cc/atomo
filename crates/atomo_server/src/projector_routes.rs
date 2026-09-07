//! REST routes for CQRS projection management.

use crate::{
    auth::{AuthUser, HttpAuthService},
    platform_models::UserRole,
};
use atomo_projectors::ProjectorManager;
use axum::Extension;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

/// Production entry point: validate tokens before the handler's administrator check.
pub fn authenticated_projector_router(
    manager: Arc<ProjectorManager>,
    auth: HttpAuthService,
) -> Router {
    projector_router(manager).route_layer(axum::middleware::from_fn_with_state(
        auth,
        crate::auth::auth_middleware,
    ))
}

fn require_admin(user: Option<Extension<AuthUser>>) -> Result<(), (StatusCode, Json<Value>)> {
    let user = user.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code":"UNAUTHENTICATED"})),
        )
    })?;
    if user.role != UserRole::Admin {
        return Err((StatusCode::FORBIDDEN, Json(json!({"code":"FORBIDDEN"}))));
    }
    Ok(())
}

/// Router for trusted embedding. Both endpoints require an administrator AuthUser
/// extension and fail closed without one; use authenticated_projector_router for HTTP.
pub fn projector_router(manager: Arc<ProjectorManager>) -> Router {
    Router::new()
        .route("/projections", get(list_projections))
        .route("/projections/rebuild", post(rebuild_projections))
        .with_state(manager)
}

/// GET /projections - list registered projections (name + source model)
async fn list_projections(
    State(manager): State<Arc<ProjectorManager>>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    require_admin(user)?;
    let items: Vec<Value> = manager
        .projections()
        .iter()
        .map(|p| json!({ "name": p.name(), "source_model": p.source_model() }))
        .collect();
    Ok(Json(json!({ "projections": items })))
}

/// POST /projections/rebuild - truncate and rebuild all projections
async fn rebuild_projections(
    State(manager): State<Arc<ProjectorManager>>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    require_admin(user)?;
    match manager.rebuild_all().await {
        Ok(()) => Ok(Json(
            json!({ "status": "rebuilt", "count": manager.projections().len() }),
        )),
        Err(error) => {
            let message = error.to_string();
            if message.starts_with("HISTORY_REPLAY_UNAVAILABLE:") {
                Err((
                    StatusCode::CONFLICT,
                    Json(
                        json!({"code":"HISTORY_REPLAY_UNAVAILABLE","message":"Complete model history is unavailable. Projections were not rebuilt."}),
                    ),
                ))
            } else if message.starts_with("PROJECTION_REBUILD_UNSUPPORTED:") {
                Err((
                    StatusCode::CONFLICT,
                    Json(
                        json!({"code":"PROJECTION_REBUILD_UNSUPPORTED","message":"A projection does not support transactional rebuild. Projections were not rebuilt."}),
                    ),
                ))
            } else {
                tracing::error!(error=%error,"projection rebuild failed");
                Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(
                        json!({"code":"PROJECTION_REBUILD_FAILED","message":"Projection rebuild is temporarily unavailable."}),
                    ),
                ))
            }
        }
    }
}
