//! HTTP routes for direct action invocation: `POST /api/actions/:name`.
//!
//! A direct action is one the schema declares with a `returns` value (callable).
//! Lifecycle-only actions (no `returns`) can only fire from events, not from the API.
//!
//! In Phase 1, all direct actions are async: the endpoint enqueues a job and returns
//! the job id. The caller polls `GET /jobs/:id` for the result. Phase 2 adds a sync
//! path for actions that return a value.

use atomo::schema::Schema;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::post,
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::jobs::JobStore;

const ACTION_QUEUE: &str = "actions";

#[derive(Clone)]
pub struct ActionState {
    pub schema: Schema,
    pub job_store: Arc<JobStore>,
}

async fn invoke_action(
    State(state): State<ActionState>,
    Path(action_name): Path<String>,
    req: axum::extract::Request,
) -> Result<Json<Value>, StatusCode> {
    let action_def = state
        .schema
        .actions
        .get(&action_name)
        .ok_or(StatusCode::NOT_FOUND)?;

    if action_def.is_lifecycle_only() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let body = axum::body::to_bytes(req.into_body(), 1_000_000)
        .await
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;

    let input: Value = if body.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?
    };

    let payload = json!({
        "action": action_name,
        "event": "direct",
        "input": input,
    });

    let job_id = state
        .job_store
        .enqueue(ACTION_QUEUE, &action_name, payload, None, 5, 0, None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "jobId": job_id })))
}

pub fn action_router(schema: Schema, job_store: Arc<JobStore>) -> Router {
    let state = ActionState { schema, job_store };
    Router::new()
        .route("/api/actions/{name}", post(invoke_action))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomo::schema::{ActionDef, ActionInputDef, ActionReturn};
    use std::collections::HashMap;

    fn test_schema() -> Schema {
        let mut actions = HashMap::new();
        actions.insert(
            "scorePost".to_string(),
            ActionDef {
                name: "scorePost".to_string(),
                input: ActionInputDef::Object { fields: vec![] },
                returns: Some(ActionReturn::Model("Post".into())),
                source_model: None,
            },
        );
        actions.insert(
            "processPost".to_string(),
            ActionDef {
                name: "processPost".to_string(),
                input: ActionInputDef::PickFields {
                    model: "Post".into(),
                    fields: vec!["id".into()],
                },
                returns: None,
                source_model: Some("Post".into()),
            },
        );
        Schema {
            models: HashMap::new(),
            actions,
            builtins: Default::default(),
        }
    }

    #[test]
    fn callable_action_exists_in_schema() {
        let schema = test_schema();
        let action = schema.actions.get("scorePost").unwrap();
        assert!(action.is_callable());
    }

    #[test]
    fn lifecycle_action_is_not_callable() {
        let schema = test_schema();
        let action = schema.actions.get("processPost").unwrap();
        assert!(action.is_lifecycle_only());
    }
}
