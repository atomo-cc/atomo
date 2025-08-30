//! Event Store - Storage and retrieval of events
//! 
//! This module provides the core event storage abstraction and implementations.

use super::{Event, EventError, EventResult, StreamInfo};
use async_trait::async_trait;
use uuid::Uuid;

/// Event store trait - abstract interface for event persistence
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append events to a stream
    /// Returns the new version number after append
    async fn append_events(
        &self,
        stream_id: Uuid,
        expected_version: Option<i64>,
        events: Vec<Event>,
    ) -> EventResult<i64>;
    
    /// Read events from a stream
    async fn read_stream(
        &self,
        stream_id: Uuid,
        from_version: Option<i64>,
        max_count: Option<usize>,
    ) -> EventResult<Vec<Event>>;
    
    /// Read all events from all streams (for projections)
    async fn read_all_events(
        &self,
        from_position: Option<i64>,
        max_count: Option<usize>,
    ) -> EventResult<Vec<Event>>;
    
    /// Get stream information
    async fn get_stream_info(&self, stream_id: Uuid) -> EventResult<Option<StreamInfo>>;
    
    /// Get global event position (for projection checkpoints)
    async fn get_global_position(&self) -> EventResult<i64>;
}

/// In-memory event store for testing and development
#[derive(Debug, Default)]
pub struct InMemoryEventStore {
    events: std::sync::Arc<std::sync::RwLock<Vec<Event>>>,
    streams: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<Uuid, StreamInfo>>>,
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn append_events(
        &self,
        stream_id: Uuid,
        expected_version: Option<i64>,
        mut events: Vec<Event>,
    ) -> EventResult<i64> {
        let mut all_events = self.events.write().unwrap();
        let mut streams = self.streams.write().unwrap();
        
        // Get current stream info
        let current_version = {
            let streams = self.streams.read().unwrap();
            streams.get(&stream_id).map(|s| s.current_version).unwrap_or(0)
        };
        
        // Check optimistic concurrency
        if let Some(expected) = expected_version {
            if current_version != expected {
                return Err(EventError::ConcurrencyError {
                    expected,
                    actual: current_version,
                });
            }
        }
        
        // Assign versions and global positions
        let start_version = current_version + 1;
        let _global_position = all_events.len() as i64;
        
        for (i, event) in events.iter_mut().enumerate() {
            event.version = start_version + i as i64;
        }
        
        // Update stream info
        let new_version = start_version + events.len() as i64 - 1;
        let now = chrono::Utc::now();
        
        {
            let mut streams = self.streams.write().unwrap();
            let existing_info = streams.get(&stream_id).cloned();
            
            streams.insert(stream_id, StreamInfo {
                stream_id,
                current_version: new_version,
                event_count: existing_info.as_ref().map(|s| s.event_count).unwrap_or(0) + events.len() as i64,
                created_at: existing_info.as_ref().map(|s| s.created_at).unwrap_or(now),
                last_event_at: now,
            });
        }
        
        // Append events
        all_events.extend(events);
        
        Ok(new_version)
    }
    
    async fn read_stream(
        &self,
        stream_id: Uuid,
        from_version: Option<i64>,
        max_count: Option<usize>,
    ) -> EventResult<Vec<Event>> {
        let events = self.events.read().unwrap();
        let from_ver = from_version.unwrap_or(0);
        
        let stream_events: Vec<Event> = events
            .iter()
            .filter(|e| e.stream_id == stream_id && e.version >= from_ver)
            .take(max_count.unwrap_or(usize::MAX))
            .cloned()
            .collect();
            
        Ok(stream_events)
    }
    
    async fn read_all_events(
        &self,
        from_position: Option<i64>,
        max_count: Option<usize>,
    ) -> EventResult<Vec<Event>> {
        let events = self.events.read().unwrap();
        let from_pos = from_position.unwrap_or(0) as usize;
        
        let result: Vec<Event> = events
            .iter()
            .skip(from_pos)
            .take(max_count.unwrap_or(usize::MAX))
            .cloned()
            .collect();
            
        Ok(result)
    }
    
    async fn get_stream_info(&self, stream_id: Uuid) -> EventResult<Option<StreamInfo>> {
        let streams = self.streams.read().unwrap();
        Ok(streams.get(&stream_id).cloned())
    }
    
    async fn get_global_position(&self) -> EventResult<i64> {
        let events = self.events.read().unwrap();
        Ok(events.len() as i64)
    }
}
