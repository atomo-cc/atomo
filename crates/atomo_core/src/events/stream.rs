//! Event Stream utilities
//! 
//! Helper functions and types for working with event streams.

use super::{Event, EventError, EventResult, DomainEvent, EventMetadata};
use uuid::Uuid;
use ulid::Ulid;

/// Event stream builder for fluent API
pub struct EventStreamBuilder {
    stream_id: Uuid,
    events: Vec<Event>,
    metadata: EventMetadata,
}

impl EventStreamBuilder {
    pub fn new(stream_id: Uuid) -> Self {
        Self {
            stream_id,
            events: Vec::new(),
            metadata: EventMetadata::default(),
        }
    }
    
    pub fn with_metadata(mut self, metadata: EventMetadata) -> Self {
        self.metadata = metadata;
        self
    }
    
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.metadata.user_id = Some(user_id.into());
        self
    }
    
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.metadata.correlation_id = Some(correlation_id.into());
        self
    }
    
    pub fn add_event<T: DomainEvent>(mut self, event: T) -> Self {
        let event = event.to_event(self.stream_id, self.metadata.clone());
        self.events.push(event);
        self
    }
    
    pub fn build(self) -> Vec<Event> {
        self.events
    }
}

/// Generate a new ULID for event ordering
pub fn new_event_id() -> String {
    Ulid::new().to_string()
}

/// Helper function to create domain events
pub fn create_domain_event<T: DomainEvent>(
    event: T,
    stream_id: Uuid,
    metadata: EventMetadata,
) -> Event {
    let event_id = new_event_id();
    let event_type = T::event_type().to_string();
    let timestamp = chrono::Utc::now();
    
    Event {
        event_id,
        stream_id,
        event_type,
        payload: serde_json::to_value(&event).expect("Failed to serialize event"),
        metadata,
        timestamp,
        version: 0, // Will be set by event store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestEvent {
        message: String,
    }
    
    impl DomainEvent for TestEvent {
        fn event_type() -> &'static str {
            "TestEvent"
        }
        
        fn to_event(&self, stream_id: Uuid, metadata: EventMetadata) -> Event {
            create_domain_event(self.clone(), stream_id, metadata)
        }
        
        fn from_event(event: &Event) -> Result<Self, serde_json::Error> {
            serde_json::from_value(event.payload.clone())
        }
    }
    
    #[test]
    fn test_event_stream_builder() {
        let stream_id = Uuid::new_v4();
        let test_event = TestEvent {
            message: "Hello, World!".to_string(),
        };
        
        let events = EventStreamBuilder::new(stream_id)
            .with_user("test_user")
            .with_correlation_id("test_correlation")
            .add_event(test_event)
            .build();
        
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].stream_id, stream_id);
        assert_eq!(events[0].event_type, "TestEvent");
        assert_eq!(events[0].metadata.user_id, Some("test_user".to_string()));
    }
}
