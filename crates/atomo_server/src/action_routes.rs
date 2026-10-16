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
    pub auth: crate::auth::HttpAuthService,
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

    let user = req
        .extensions()
        .get::<crate::auth::AuthUser>()
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let principal = crate::auth::request_principal(
        Some(user),
        req.headers()
            .get("x-tenant-id")
            .map(|v| v.to_str().map_err(|_| StatusCode::BAD_REQUEST))
            .transpose()?,
    )?;
    let idempotency = req
        .headers()
        .get("idempotency-key")
        .map(|v| {
            v.to_str()
                .map(String::from)
                .map_err(|_| StatusCode::BAD_REQUEST)
        })
        .transpose()?;
    if idempotency
        .as_ref()
        .is_some_and(|k| k.is_empty() || k.len() > 200)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let model = match &action_def.returns {
        Some(atomo::schema::ActionReturn::Model(name)) => {
            state.schema.models.get(name).ok_or(StatusCode::FORBIDDEN)?
        }
        _ => return Err(StatusCode::FORBIDDEN),
    };
    if model.tenant_policy == atomo::schema::TenantPolicy::Required && principal.tenant_id.is_none()
    {
        return Err(StatusCode::FORBIDDEN);
    }
    atomo::security::with_principal(principal.clone(), async {
        atomo::security::check_row(model, "create", &Default::default())
    })
    .await
    .map_err(|_| StatusCode::FORBIDDEN)?;
    let body = axum::body::to_bytes(req.into_body(), 1_000_000)
        .await
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
    let input: Value = if body.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?
    };
    validate_input(action_def, &state.schema, &input)?;
    let payload = json!({ "action": action_name, "event": "direct", "input": input,
        "actor": { "userId": principal.user_id, "role": principal.role, "tenantId": principal.tenant_id } });
    let key = idempotency.map(|key| {
        use sha2::{Digest, Sha256};
        format!(
            "direct:{:x}",
            Sha256::digest(
                serde_json::to_vec(&json!([
                    principal.user_id,
                    principal.tenant_id,
                    action_name,
                    key
                ]))
                .unwrap()
            )
        )
    });
    let job_id = state
        .job_store
        .enqueue_as(
            ACTION_QUEUE,
            &action_name,
            payload.clone(),
            key.as_deref(),
            5,
            0,
            principal.tenant_id.as_deref(),
            principal.user_id.as_deref(),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // enqueue's unique key serializes concurrent retries. Compare the persisted
    // payload after it returns so a changed request cannot silently reuse a job.
    let saved: Value = sqlx::query_scalar("SELECT payload FROM jobs WHERE id = $1")
        .bind(&job_id)
        .fetch_one(state.auth.db_pool())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if saved != payload {
        return Err(StatusCode::CONFLICT);
    }
    Ok(Json(json!({ "jobId": job_id, "mode": "async" })))
}

fn validate_input(
    action: &atomo::schema::ActionDef,
    schema: &Schema,
    input: &Value,
) -> Result<(), StatusCode> {
    use atomo::schema::{ActionInputDef, FieldType};
    let object = input.as_object().ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
    let fields: Vec<(String, String, bool)> = match &action.input {
        ActionInputDef::Object { fields } => fields
            .iter()
            .map(|f| (f.name.clone(), f.field_type.clone(), f.required))
            .collect(),
        ActionInputDef::PickFields { model, fields } => {
            let model = schema.models.get(model).ok_or(StatusCode::FORBIDDEN)?;
            fields
                .iter()
                .map(|name| {
                    let field = model.fields.get(name).ok_or(StatusCode::FORBIDDEN)?;
                    if field
                        .attributes
                        .iter()
                        .any(|a| matches!(a, atomo::schema::FieldAttribute::FromAuth(_)))
                    {
                        return Err(StatusCode::FORBIDDEN);
                    }
                    let kind = match field.field_type {
                        FieldType::Number => "number",
                        FieldType::Boolean => "boolean",
                        FieldType::Json => "json",
                        _ => "string",
                    };
                    Ok((name.clone(), kind.into(), !field.optional))
                })
                .collect::<Result<_, StatusCode>>()?
        }
    };
    if object.keys().any(|k| !fields.iter().any(|f| &f.0 == k)) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    for (name, kind, required) in fields {
        let value = object.get(&name);
        if required && value.is_none_or(Value::is_null) {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        if let Some(value) = value.filter(|v| !v.is_null()) {
            let valid = match kind.as_str() {
                "string" | "text" => value.is_string(),
                "number" => value
                    .as_i64()
                    .is_some_and(|n| (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&n)),
                "boolean" => value.is_boolean(),
                "json" => true,
                _ => false,
            };
            if !valid {
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
        }
    }
    let rules = match &action.input {
        ActionInputDef::Object { fields } => fields
            .iter()
            .filter(|f| !f.validation.is_empty())
            .map(|f| (f.name.clone(), f.validation.clone()))
            .collect(),
        ActionInputDef::PickFields { model, fields } => schema.models[model]
            .validation
            .iter()
            .filter(|(f, _)| fields.contains(f))
            .map(|(f, r)| (f.clone(), r.clone()))
            .collect(),
    };
    let data = object.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    if !atomo::validation::validate(&data, &rules).is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    Ok(())
}

pub fn action_router(
    schema: Schema,
    job_store: Arc<JobStore>,
    auth: crate::auth::HttpAuthService,
) -> Router {
    let state = ActionState {
        schema,
        job_store,
        auth: auth.clone(),
    };
    Router::new()
        .route("/api/actions/{name}", post(invoke_action))
        .route_layer(axum::middleware::from_fn_with_state(
            auth,
            crate::auth::auth_middleware,
        ))
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
            warnings: Vec::new(),
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
