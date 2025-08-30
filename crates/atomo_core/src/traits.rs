use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::{EventId, StreamId, Timestamp, Result};

/// Base trait for all domain events
/// 
/// This is the core platform trait that all business domain events
/// must implement. Business applications (like CRM) will define
/// their own event types that implement this trait.
pub trait DomainEvent: Send + Sync + Clone {
    fn event_type(&self) -> &'static str;
    fn stream_id(&self) -> StreamId;
}

/// Event envelope that wraps domain events with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: EventId,
    pub stream_id: StreamId,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub metadata: EventMetadata,
    pub timestamp: Timestamp,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub user_id: Option<String>,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Event store trait for persisting and retrieving events
/// 
/// This is a generic trait that business applications can implement
/// for their specific event storage needs.
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append_events(
        &self,
        stream_id: StreamId,
        expected_version: Option<i64>,
        events: Vec<EventEnvelope>,
        metadata: EventMetadata,
    ) -> Result<Vec<EventEnvelope>>;
    
    async fn read_events(
        &self,
        stream_id: StreamId,
        from_version: Option<i64>,
    ) -> Result<Vec<EventEnvelope>>;
    
    async fn read_all_events(
        &self,
        from_position: Option<i64>,
        max_count: Option<usize>,
    ) -> Result<Vec<EventEnvelope>>;
}

/// Command handler trait
/// 
/// Generic trait for handling commands in CQRS applications.
/// The type parameter E represents the event type returned.
#[async_trait]
pub trait CommandHandler<C, E>: Send + Sync 
where 
    E: DomainEvent,
{
    async fn handle(&self, command: C) -> Result<Vec<E>>;
}

/// Event handler trait for projections
/// 
/// Used by projectors to handle domain events and update read models.
#[async_trait]
pub trait EventHandler<E>: Send + Sync {
    async fn handle(&self, event: &E) -> Result<()>;
}

/// Repository trait for read models
#[async_trait]
pub trait Repository<T>: Send + Sync {
    async fn find_by_id(&self, id: &str) -> Result<Option<T>>;
    async fn find_all(&self) -> Result<Vec<T>>;
    async fn save(&self, entity: &T) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}
