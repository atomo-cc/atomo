//! Real-time event system for subscribing to model changes

use async_graphql::{Enum, SimpleObject};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::client::AtomoClient;
use crate::query::WhereClause;

#[derive(Debug, Clone, Serialize, Deserialize, Enum, Copy, PartialEq, Eq)]
pub enum EventType {
    Created,
    Updated,
    Deleted,
    /// Emitted by a plugin (not a CRUD mutation).
    Custom,
}

/// Builder for creating subscriptions
pub struct SubscriptionBuilder {
    client: Arc<AtomoClient>,
    model_name: String,
    event_types: Vec<EventType>,
    where_clauses: Vec<WhereClause>,
}

impl SubscriptionBuilder {
    pub fn new(client: Arc<AtomoClient>, model_name: &str) -> Self {
        Self {
            client,
            model_name: model_name.to_string(),
            event_types: Vec::new(),
            where_clauses: Vec::new(),
        }
    }

    /// Subscribe to create events
    pub fn on_create(mut self) -> Self {
        self.event_types.push(EventType::Created);
        self
    }

    /// Subscribe to update events
    pub fn on_update(mut self) -> Self {
        self.event_types.push(EventType::Updated);
        self
    }

    /// Subscribe to delete events
    pub fn on_delete(mut self) -> Self {
        self.event_types.push(EventType::Deleted);
        self
    }

    /// Subscribe to all events
    pub fn on_all(mut self) -> Self {
        self.event_types = vec![EventType::Created, EventType::Updated, EventType::Deleted];
        self
    }

    /// Add a where filter
    pub fn where_(mut self, _field: &str, condition: impl Into<WhereClause>) -> Self {
        self.where_clauses.push(condition.into());
        self
    }

    /// Create the subscription stream
    pub async fn stream(self) -> ModelEventStream {
        let receiver = self
            .client
            .subscribe(&self.model_name, &self.event_types, &self.where_clauses)
            .await;

        ModelEventStream {
            receiver,
            model_name: self.model_name,
            event_types: self.event_types,
        }
    }
}

/// Stream of model events, filtered by the builder's model + event-type criteria.
pub struct ModelEventStream {
    receiver: broadcast::Receiver<ModelEvent>,
    model_name: String,
    event_types: Vec<EventType>,
}

impl ModelEventStream {
    /// Get the next event matching the subscription filter (skips non-matching events).
    /// Empty `event_types` means "any type". This is where the builder's filtering is applied
    /// (the broadcast channel itself carries every model's events).
    pub async fn next(&mut self) -> Option<ModelEvent> {
        loop {
            let ev = self.receiver.recv().await.ok()?;
            let model_ok = ev.model_name == self.model_name;
            let type_ok = self.event_types.is_empty() || self.event_types.contains(&ev.event_type);
            if model_ok && type_ok {
                return Some(ev);
            }
        }
    }
}

/// Model event containing the change data
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct ModelEvent {
    pub event_type: EventType,
    pub model_name: String,
    pub data: HashMap<String, Value>,
    pub previous_data: Option<HashMap<String, Value>>, // For updates
    pub timestamp: String,                             // Use String instead of chrono for now
    pub event_id: String,
    #[graphql(default)]
    pub actor: Option<String>, // user_id of the actor that caused the event, if known
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(model: &str, et: EventType) -> ModelEvent {
        ModelEvent {
            event_type: et,
            model_name: model.into(),
            data: HashMap::new(),
            previous_data: None,
            timestamp: String::new(),
            event_id: String::new(),
            actor: None,
        }
    }

    // The SDK SubscriptionBuilder filter args were dead code; ModelEventStream::next now applies
    // them. Subscribing to Deal/Created must skip Contact events and Deal/Updated events.
    #[tokio::test]
    async fn stream_filters_by_model_and_event_type() {
        let (tx, rx) = broadcast::channel(16);
        let mut stream = ModelEventStream {
            receiver: rx,
            model_name: "Deal".into(),
            event_types: vec![EventType::Created],
        };
        tx.send(ev("Contact", EventType::Created)).unwrap(); // wrong model — skipped
        tx.send(ev("Deal", EventType::Updated)).unwrap(); // wrong type — skipped
        tx.send(ev("Deal", EventType::Created)).unwrap(); // match
        let got = stream.next().await.expect("a matching event");
        assert_eq!(got.model_name, "Deal");
        assert!(matches!(got.event_type, EventType::Created));
    }
}
