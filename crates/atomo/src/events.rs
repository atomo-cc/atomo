//! Real-time event system for subscribing to model changes

use std::sync::Arc;
use tokio::sync::broadcast;
use serde::{Serialize, Deserialize};
use async_graphql::{SimpleObject, Enum};
use serde_json::Value;
use std::collections::HashMap;

use crate::client::AtomoClient;
use crate::query::WhereClause;

#[derive(Debug, Clone, Serialize, Deserialize, Enum, Copy, PartialEq, Eq)]
pub enum EventType {
    Created,
    Updated,
    Deleted,
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
    pub fn where_(mut self, field: &str, condition: impl Into<WhereClause>) -> Self {
        self.where_clauses.push(condition.into());
        self
    }
    
    /// Create the subscription stream
    pub async fn stream(self) -> ModelEventStream {
        let receiver = self.client.subscribe(
            &self.model_name,
            &self.event_types,
            &self.where_clauses
        ).await;
        
        ModelEventStream { receiver }
    }
}

/// Stream of model events
pub struct ModelEventStream {
    receiver: broadcast::Receiver<ModelEvent>,
}

impl ModelEventStream {
    /// Get the next event
    pub async fn next(&mut self) -> Option<ModelEvent> {
        match self.receiver.recv().await {
            Ok(event) => Some(event),
            Err(_) => None,
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
    pub timestamp: String, // Use String instead of chrono for now
    pub event_id: String,
}
