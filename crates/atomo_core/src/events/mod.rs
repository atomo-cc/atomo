//! Event Sourcing Core Abstractions - "事件的河流"
//! 
//! This module defines the fundamental abstractions for Atomo's event sourcing architecture.
//! According to the whitepaper, this should contain only traits and basic types,
//! with all concrete implementations in the server layer.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::types::{EntityId, StreamId};

pub mod event_store;
pub mod stream;
pub mod envelope;

pub use event_store::*;
pub use stream::*;
pub use envelope::*;

/// Core event metadata for audit and context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// User who triggered this event
    pub user_id: Option<EntityId>,
    
    /// IP address of the request
    pub ip_address: Option<String>,
    
    /// User agent string
    pub user_agent: Option<String>,
    
    /// Request correlation ID for tracing
    pub correlation_id: Option<String>,
    
    /// Causation ID (which event caused this event)
    pub causation_id: Option<String>,
    
    /// Additional context data
    pub context: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self {
            user_id: None,
            ip_address: None,
            user_agent: None,
            correlation_id: None,
            causation_id: None,
            context: std::collections::HashMap::new(),
        }
    }
}

impl EventMetadata {
    /// Create new metadata with user context
    pub fn with_user(user_id: EntityId) -> Self {
        Self {
            user_id: Some(user_id),
            ..Default::default()
        }
    }
    
    /// Add correlation ID for request tracing
    pub fn with_correlation(mut self, correlation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
    
    /// Add causation ID for event causality
    pub fn with_causation(mut self, causation_id: String) -> Self {
        self.causation_id = Some(causation_id);
        self
    }
    
    /// Add custom context data
    pub fn with_context(mut self, key: String, value: serde_json::Value) -> Self {
        self.context.insert(key, value);
        self
    }
}

/// Event type enumeration for domain events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventType {
    /// Entity creation events
    Created,
    /// Entity update events
    Updated,
    /// Entity deletion events
    Deleted,
    /// Entity state transition events
    StateChanged,
    /// Integration events from external systems
    Integration,
    /// Custom domain-specific events
    Custom(String),
}

impl EventType {
    /// Check if this is a lifecycle event
    pub fn is_lifecycle_event(&self) -> bool {
        matches!(self, EventType::Created | EventType::Updated | EventType::Deleted)
    }
    
    /// Check if this is a state transition event
    pub fn is_state_transition(&self) -> bool {
        matches!(self, EventType::StateChanged)
    }
    
    /// Check if this is an integration event
    pub fn is_integration_event(&self) -> bool {
        matches!(self, EventType::Integration)
    }
}

/// Core domain event trait
/// 
/// All domain events must implement this trait to be processable
/// by the event sourcing infrastructure.
pub trait DomainEvent: Send + Sync + Clone {
    /// Get the stream ID this event belongs to
    fn stream_id(&self) -> StreamId;
    
    /// Get the event type
    fn event_type(&self) -> EventType;
    
    /// Get the event metadata
    fn metadata(&self) -> &EventMetadata;
    
    /// Serialize the event payload
    fn payload(&self) -> serde_json::Value;
    
    /// Get the aggregate ID this event affects
    fn aggregate_id(&self) -> EntityId;
    
    /// Get the version of the aggregate after this event
    fn aggregate_version(&self) -> i64;
}

/// Event sourcing snapshot interface
/// 
/// Snapshots are periodic captures of aggregate state used
/// to optimize event replay performance.
#[async_trait]
pub trait Snapshot: Send + Sync + Clone {
    /// Get the aggregate ID this snapshot represents
    fn aggregate_id(&self) -> EntityId;
    
    /// Get the version this snapshot represents
    fn version(&self) -> i64;
    
    /// Get when this snapshot was taken
    fn taken_at(&self) -> crate::types::Timestamp;
    
    /// Serialize the snapshot data
    fn data(&self) -> serde_json::Value;
}
