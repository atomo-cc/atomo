//! Action dispatcher: listens to the model event broadcast and enqueues lifecycle
//! action jobs for matching `ModelEvents` bindings defined in the schema.
//!
//! This is the bridge between "a record was created/updated/deleted" and "run the
//! worker action the schema says should fire." It runs as a background task, just
//! like the audit and projector listeners.

use atomo::events::{EventType, ModelEvent};
use atomo::schema::{EventActionBinding, Schema};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::jobs::JobStore;

const ACTION_QUEUE: &str = "actions";

pub fn spawn_action_dispatcher(
    schema: Schema,
    job_store: Arc<JobStore>,
    mut rx: broadcast::Receiver<ModelEvent>,
) {
    tokio::spawn(async move {
        while let Ok(ev) = rx.recv().await {
            let bindings = match matching_bindings(&schema, &ev) {
                Some(b) if !b.is_empty() => b,
                _ => continue,
            };

            for binding in bindings {
                let action_def = schema.actions.get(&binding.action);
                let input = match action_def {
                    Some(def) => def.build_input(&ev.data),
                    None => ev.data.clone(),
                };

                let payload = build_job_payload(&ev, &binding.action, input);
                let idempotency_key =
                    format!("{}:{}", ev.event_id, binding.action);
                let tenant_id = ev
                    .data
                    .get("tenant_id")
                    .and_then(|v| v.as_str());

                if let Err(e) = job_store
                    .enqueue(
                        ACTION_QUEUE,
                        &binding.action,
                        payload,
                        Some(&idempotency_key),
                        5,
                        0,
                        tenant_id,
                    )
                    .await
                {
                    tracing::warn!(
                        action = %binding.action,
                        model = %ev.model_name,
                        event_id = %ev.event_id,
                        error = %e,
                        "failed to enqueue lifecycle action"
                    );
                }
            }
        }
    });
}

fn matching_bindings<'a>(
    schema: &'a Schema,
    ev: &ModelEvent,
) -> Option<Vec<&'a EventActionBinding>> {
    let model = schema.models.get(&ev.model_name)?;
    let events = &model.events;

    let candidates: &[EventActionBinding] = match ev.event_type {
        EventType::Created => &events.created,
        EventType::Updated => &events.updated,
        EventType::Deleted | EventType::HardDeleted => &events.deleted,
        _ => return None,
    };

    let matched: Vec<&EventActionBinding> = candidates
        .iter()
        .filter(|b| {
            // Loop prevention: if the event was caused by this action, skip it.
            if let Some(ref origin) = ev.origin {
                if origin == &b.action {
                    return false;
                }
            }
            b.condition.as_ref().is_none_or(|cond| {
                cond.matches(&ev.data, ev.previous_data.as_ref())
            })
        })
        .collect();

    Some(matched)
}

fn build_job_payload(
    ev: &ModelEvent,
    action_name: &str,
    input: HashMap<String, Value>,
) -> Value {
    json!({
        "action": action_name,
        "model": ev.model_name,
        "event": format!("{:?}", ev.event_type).to_lowercase(),
        "eventId": ev.event_id,
        "input": input,
        "actor": ev.actor,
        "timestamp": ev.timestamp,
        "origin": ev.origin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomo::schema::{
        ActionCondition, ActionDef, ActionInputDef, EventActionBinding, Model, ModelEvents, Schema,
    };
    use serde_json::json;

    fn test_schema() -> Schema {
        let mut actions = HashMap::new();
        actions.insert(
            "processPost".to_string(),
            ActionDef {
                name: "processPost".to_string(),
                input: ActionInputDef::PickFields {
                    model: "Post".into(),
                    fields: vec!["id".into(), "title".into()],
                },
                returns: None,
                source_model: Some("Post".into()),
            },
        );
        actions.insert(
            "onStatusChange".to_string(),
            ActionDef {
                name: "onStatusChange".to_string(),
                input: ActionInputDef::PickFields {
                    model: "Post".into(),
                    fields: vec!["id".into(), "status".into()],
                },
                returns: None,
                source_model: Some("Post".into()),
            },
        );

        let mut models = HashMap::new();
        models.insert(
            "Post".to_string(),
            Model {
                name: "Post".to_string(),
                fields: HashMap::new(),
                access: None,
                hooks: None,
                validation: HashMap::new(),
                table_name: None,
                relationships: HashMap::new(),
                constraints: Vec::new(),
                events: ModelEvents {
                    created: vec![EventActionBinding {
                        action: "processPost".to_string(),
                        condition: None,
                    }],
                    updated: vec![EventActionBinding {
                        action: "onStatusChange".to_string(),
                        condition: Some(ActionCondition::ChangedAny(vec!["status".into()])),
                    }],
                    deleted: vec![],
                },
                ui: None,
            },
        );
        Schema { models, actions }
    }

    #[test]
    fn created_event_matches_unconditional_binding() {
        let schema = test_schema();
        let ev = ModelEvent {
            event_type: EventType::Created,
            model_name: "Post".into(),
            data: {
                let mut d = HashMap::new();
                d.insert("id".into(), json!("p1"));
                d.insert("title".into(), json!("Hello"));
                d
            },
            previous_data: None,
            timestamp: "2026-01-01T00:00:00Z".into(),
            event_id: "evt1".into(),
            actor: None,
            origin: None,
        };
        let bindings = matching_bindings(&schema, &ev).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].action, "processPost");
    }

    #[test]
    fn updated_event_matches_when_condition_met() {
        let schema = test_schema();
        let ev = ModelEvent {
            event_type: EventType::Updated,
            model_name: "Post".into(),
            data: {
                let mut d = HashMap::new();
                d.insert("id".into(), json!("p1"));
                d.insert("status".into(), json!("published"));
                d
            },
            previous_data: Some({
                let mut d = HashMap::new();
                d.insert("id".into(), json!("p1"));
                d.insert("status".into(), json!("draft"));
                d
            }),
            timestamp: "2026-01-01T00:00:00Z".into(),
            event_id: "evt2".into(),
            actor: None,
            origin: None,
        };
        let bindings = matching_bindings(&schema, &ev).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].action, "onStatusChange");
    }

    #[test]
    fn updated_event_skips_when_condition_not_met() {
        let schema = test_schema();
        let ev = ModelEvent {
            event_type: EventType::Updated,
            model_name: "Post".into(),
            data: {
                let mut d = HashMap::new();
                d.insert("id".into(), json!("p1"));
                d.insert("title".into(), json!("new title"));
                d.insert("status".into(), json!("draft"));
                d
            },
            previous_data: Some({
                let mut d = HashMap::new();
                d.insert("id".into(), json!("p1"));
                d.insert("title".into(), json!("old title"));
                d.insert("status".into(), json!("draft"));
                d
            }),
            timestamp: "2026-01-01T00:00:00Z".into(),
            event_id: "evt3".into(),
            actor: None,
            origin: None,
        };
        let bindings = matching_bindings(&schema, &ev).unwrap();
        assert_eq!(bindings.len(), 0);
    }

    #[test]
    fn unknown_model_returns_none() {
        let schema = test_schema();
        let ev = ModelEvent {
            event_type: EventType::Created,
            model_name: "Unknown".into(),
            data: HashMap::new(),
            previous_data: None,
            timestamp: "2026-01-01T00:00:00Z".into(),
            event_id: "evt4".into(),
            actor: None,
            origin: None,
        };
        assert!(matching_bindings(&schema, &ev).is_none());
    }

    #[test]
    fn build_job_payload_structure() {
        let ev = ModelEvent {
            event_type: EventType::Created,
            model_name: "Post".into(),
            data: {
                let mut d = HashMap::new();
                d.insert("id".into(), json!("p1"));
                d
            },
            previous_data: None,
            timestamp: "2026-01-01T00:00:00Z".into(),
            event_id: "evt5".into(),
            actor: Some("user1".into()),
            origin: None,
        };
        let mut input = HashMap::new();
        input.insert("id".into(), json!("p1"));

        let payload = build_job_payload(&ev, "processPost", input);
        assert_eq!(payload["action"], "processPost");
        assert_eq!(payload["model"], "Post");
        assert_eq!(payload["event"], "created");
        assert_eq!(payload["eventId"], "evt5");
        assert_eq!(payload["input"]["id"], "p1");
        assert_eq!(payload["actor"], "user1");
    }

    #[test]
    fn origin_suppresses_same_action() {
        let schema = test_schema();
        let ev = ModelEvent {
            event_type: EventType::Created,
            model_name: "Post".into(),
            data: {
                let mut d = HashMap::new();
                d.insert("id".into(), json!("p1"));
                d.insert("title".into(), json!("Hello"));
                d
            },
            previous_data: None,
            timestamp: "2026-01-01T00:00:00Z".into(),
            event_id: "evt-origin".into(),
            actor: None,
            origin: Some("processPost".into()),
        };
        let bindings = matching_bindings(&schema, &ev).unwrap();
        assert_eq!(bindings.len(), 0, "processPost should be suppressed by origin");
    }

    #[test]
    fn origin_none_does_not_suppress() {
        let schema = test_schema();
        let ev = ModelEvent {
            event_type: EventType::Created,
            model_name: "Post".into(),
            data: {
                let mut d = HashMap::new();
                d.insert("id".into(), json!("p1"));
                d.insert("title".into(), json!("Hello"));
                d
            },
            previous_data: None,
            timestamp: "2026-01-01T00:00:00Z".into(),
            event_id: "evt-no-origin".into(),
            actor: None,
            origin: None,
        };
        let bindings = matching_bindings(&schema, &ev).unwrap();
        assert_eq!(bindings.len(), 1, "origin=None should not suppress anything");
        assert_eq!(bindings[0].action, "processPost");
    }

    #[test]
    fn origin_allows_different_action() {
        let schema = test_schema();
        let ev = ModelEvent {
            event_type: EventType::Updated,
            model_name: "Post".into(),
            data: {
                let mut d = HashMap::new();
                d.insert("id".into(), json!("p1"));
                d.insert("status".into(), json!("published"));
                d
            },
            previous_data: Some({
                let mut d = HashMap::new();
                d.insert("id".into(), json!("p1"));
                d.insert("status".into(), json!("draft"));
                d
            }),
            timestamp: "2026-01-01T00:00:00Z".into(),
            event_id: "evt-origin2".into(),
            actor: None,
            origin: Some("processPost".into()),
        };
        let bindings = matching_bindings(&schema, &ev).unwrap();
        assert_eq!(bindings.len(), 1, "onStatusChange should still fire");
        assert_eq!(bindings[0].action, "onStatusChange");
    }
}
