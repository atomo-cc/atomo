//! Event Envelope - Event包装器
//! 
//! 事件信封包含事件的元数据和有效载荷，是事件存储的基本单元。

use serde::{Deserialize, Serialize};
use crate::types::{EntityId, StreamId, Timestamp};
use super::{DomainEvent, EventMetadata, EventType};

/// Event envelope that wraps domain events with metadata
/// 
/// This is the unit of storage in the event store. It contains
/// both the domain event payload and essential metadata for
/// audit, causation tracking, and replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Unique identifier for this event
    pub event_id: EntityId,
    
    /// Stream this event belongs to
    pub stream_id: StreamId,
    
    /// Version within the stream
    pub stream_version: i64,
    
    /// Global sequence number across all events
    pub global_sequence: i64,
    
    /// Type of the event
    pub event_type: EventType,
    
    /// Event metadata for audit and context
    pub metadata: EventMetadata,
    
    /// The actual event payload as JSON
    pub payload: serde_json::Value,
    
    /// When this event was recorded
    pub recorded_at: Timestamp,
}

impl EventEnvelope {
    /// Create a new event envelope from a domain event
    pub fn from_domain_event<E>(event: &E, stream_version: i64, global_sequence: i64) -> Self 
    where
        E: DomainEvent,
    {
        Self {
            event_id: EntityId::new(),
            stream_id: event.stream_id(),
            stream_version,
            global_sequence,
            event_type: event.event_type(),
            metadata: event.metadata().clone(),
            payload: event.payload(),
            recorded_at: chrono::Utc::now(),
        }
    }
    
    /// Check if this event was caused by a specific user
    pub fn was_caused_by_user(&self, user_id: &EntityId) -> bool {
        self.metadata.user_id.as_ref() == Some(user_id)
    }
    
    /// Check if this event is part of a specific correlation
    pub fn is_correlated_with(&self, correlation_id: &str) -> bool {
        self.metadata.correlation_id.as_ref().map(|c| c == correlation_id).unwrap_or(false)
    }
    
    /// Get the age of this event
    pub fn age(&self) -> chrono::Duration {
        chrono::Utc::now() - self.recorded_at
    }
}

/// Event causation chain for tracking event relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCausation {
    /// The event that caused this event
    pub cause_event_id: EntityId,
    
    /// The event that was caused
    pub effect_event_id: EntityId,
    
    /// The type of causation relationship
    pub causation_type: CausationType,
    
    /// When this causation was recorded
    pub recorded_at: Timestamp,
}

/// Types of causation relationships between events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CausationType {
    /// Direct command-event causation
    Command,
    
    /// Event-triggered saga step
    Saga,
    
    /// Process manager decision
    Process,
    
    /// Projection update
    Projection,
    
    /// Integration event
    Integration,
}
