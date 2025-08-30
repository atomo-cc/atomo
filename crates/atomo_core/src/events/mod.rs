//! Event Sourcing Core - "事件的河流"
//! 
//! This module implements the heart of Atomo's event sourcing architecture.
//! All state changes flow through this immutable event stream.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod event_store;
pub mod stream;

pub use event_store::*;
pub use stream::*;

/// Core event structure - the atomic unit of change in Atomo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event identifier (ULID for time-ordered sorting)
    pub event_id: String,
    
    /// Stream identifier - groups related events (e.g., all events for a specific Contact)
    pub stream_id: Uuid,
    
    /// Event type identifier (e.g., "ContactCreated", "ContactEmailUpdated")
    pub event_type: String,
    
    /// Event payload containing the actual data
    pub payload: serde_json::Value,
    
    /// Event metadata (user info, request context, etc.)
    pub metadata: EventMetadata,
    
    /// When this event occurred
    pub timestamp: DateTime<Utc>,
    
    /// Event version within the stream (for optimistic concurrency)
    pub version: i64,
}

/// Event metadata for audit and context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// User who triggered this event
    pub user_id: Option<String>,
    
    /// IP address of the request
    pub ip_address: Option<String>,
    
    /// User agent string
    pub user_agent: Option<String>,
    
    /// Request correlation ID
    pub correlation_id: Option<String>,
    
    /// Additional context
    pub context: serde_json::Value,
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self {
            user_id: None,
            ip_address: None,
            user_agent: None,
            correlation_id: None,
            context: serde_json::Value::Null,
        }
    }
}

/// Event stream identifier and current position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub stream_id: Uuid,
    pub current_version: i64,
    pub event_count: i64,
    pub created_at: DateTime<Utc>,
    pub last_event_at: DateTime<Utc>,
}

/// Marker trait for domain events
pub trait DomainEvent: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static {
    /// Event type name
    fn event_type() -> &'static str;
    
    /// Convert to generic Event
    fn to_event(&self, stream_id: Uuid, metadata: EventMetadata) -> Event;
    
    /// Create from generic Event
    fn from_event(event: &Event) -> Result<Self, serde_json::Error>;
}

/// Event sourcing error types
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("Stream not found: {stream_id}")]
    StreamNotFound { stream_id: Uuid },
    
    #[error("Optimistic concurrency error: expected version {expected}, got {actual}")]
    ConcurrencyError { expected: i64, actual: i64 },
    
    #[error("Event serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Invalid event data: {0}")]
    InvalidEventData(String),
}

pub type EventResult<T> = Result<T, EventError>;
