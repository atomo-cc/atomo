//! In-Memory Event Store Implementation
//! 
//! This module provides an in-memory implementation of the EventStore trait
//! suitable for testing and development purposes.

use atomo_core::{
    events::{EventStore, EventEnvelope, EventType, Snapshot, SnapshotStore},
    types::{EntityId, StreamId, Timestamp},
    AtomoError, Result,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// In-memory event store for testing and development
#[derive(Debug, Default, Clone)]
pub struct InMemoryEventStore {
    /// All events stored by global sequence
    events: Arc<RwLock<Vec<EventEnvelope>>>,
    /// Stream versions for concurrency control
    stream_versions: Arc<RwLock<HashMap<StreamId, i64>>>,
    /// Events indexed by stream ID
    stream_events: Arc<RwLock<HashMap<StreamId, Vec<usize>>>>, // indices into events vec
    /// Events indexed by aggregate ID
    aggregate_events: Arc<RwLock<HashMap<EntityId, Vec<usize>>>>,
    /// Current global sequence number
    global_sequence: Arc<RwLock<i64>>,
}

impl InMemoryEventStore {
    /// Create a new in-memory event store
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of events stored
    pub fn event_count(&self) -> usize {
        self.events.read().unwrap().len()
    }

    /// Clear all events (for testing)
    pub fn clear(&self) {
        self.events.write().unwrap().clear();
        self.stream_versions.write().unwrap().clear();
        self.stream_events.write().unwrap().clear();
        self.aggregate_events.write().unwrap().clear();
        *self.global_sequence.write().unwrap() = 0;
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    type Error = AtomoError;

    async fn append_events(
        &self,
        stream_id: StreamId,
        expected_version: Option<i64>,
        mut events: Vec<EventEnvelope>,
    ) -> Result<()> {
        let mut all_events = self.events.write().unwrap();
        let mut stream_versions = self.stream_versions.write().unwrap();
        let mut stream_events = self.stream_events.write().unwrap();
        let mut aggregate_events = self.aggregate_events.write().unwrap();
        let mut global_seq = self.global_sequence.write().unwrap();

        // Check optimistic concurrency control
        let current_version = stream_versions.get(&stream_id).copied().unwrap_or(0);
        if let Some(expected) = expected_version {
            if current_version != expected {
                return Err(AtomoError::concurrency_conflict(expected, current_version));
            }
        }

        // Assign stream versions and global sequences
        let mut next_stream_version = current_version + 1;
        for event in &mut events {
            event.stream_version = next_stream_version;
            *global_seq += 1;
            event.global_sequence = *global_seq;
            next_stream_version += 1;
        }

        // Store events and update indices
        let start_index = all_events.len();
        for (i, event) in events.iter().enumerate() {
            let event_index = start_index + i;
            
            // Index by stream
            stream_events.entry(stream_id).or_default().push(event_index);
            
            // Index by aggregate
            let aggregate_id = event.metadata.user_id.unwrap_or_else(|| EntityId::new());
            aggregate_events.entry(aggregate_id).or_default().push(event_index);
        }

        // Update stream version
        stream_versions.insert(stream_id, next_stream_version - 1);

        // Add events to store
        all_events.extend(events);

        Ok(())
    }

    async fn read_stream(
        &self,
        stream_id: StreamId,
        from_version: Option<i64>,
        max_count: Option<usize>,
    ) -> Result<Vec<EventEnvelope>> {
        let events = self.events.read().unwrap();
        let stream_events = self.stream_events.read().unwrap();

        let empty_vec = vec![];
        let indices = stream_events.get(&stream_id).unwrap_or(&empty_vec);
        let from_ver = from_version.unwrap_or(1);

        let result: Vec<EventEnvelope> = indices
            .iter()
            .filter_map(|&idx| events.get(idx))
            .filter(|event| event.stream_version >= from_ver)
            .take(max_count.unwrap_or(usize::MAX))
            .cloned()
            .collect();

        Ok(result)
    }

    async fn read_events_by_type(
        &self,
        event_type: EventType,
        from_timestamp: Option<Timestamp>,
        max_count: Option<usize>,
    ) -> Result<Vec<EventEnvelope>> {
        let events = self.events.read().unwrap();
        let from_time = from_timestamp.unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());

        let result: Vec<EventEnvelope> = events
            .iter()
            .filter(|event| event.event_type == event_type && event.recorded_at >= from_time)
            .take(max_count.unwrap_or(usize::MAX))
            .cloned()
            .collect();

        Ok(result)
    }

    async fn read_aggregate_events(
        &self,
        aggregate_id: EntityId,
        from_version: Option<i64>,
    ) -> Result<Vec<EventEnvelope>> {
        let events = self.events.read().unwrap();
        let aggregate_events = self.aggregate_events.read().unwrap();

        let empty_vec = vec![];
        let indices = aggregate_events.get(&aggregate_id).unwrap_or(&empty_vec);
        let from_ver = from_version.unwrap_or(1);

        let result: Vec<EventEnvelope> = indices
            .iter()
            .filter_map(|&idx| events.get(idx))
            .filter(|event| event.stream_version >= from_ver)
            .cloned()
            .collect();

        Ok(result)
    }

    async fn read_events_from_sequence(
        &self,
        from_sequence: i64,
        max_count: Option<usize>,
    ) -> Result<Vec<EventEnvelope>> {
        let events = self.events.read().unwrap();

        let result: Vec<EventEnvelope> = events
            .iter()
            .filter(|event| event.global_sequence >= from_sequence)
            .take(max_count.unwrap_or(usize::MAX))
            .cloned()
            .collect();

        Ok(result)
    }

    async fn get_stream_version(&self, stream_id: StreamId) -> Result<Option<i64>> {
        let stream_versions = self.stream_versions.read().unwrap();
        Ok(stream_versions.get(&stream_id).copied())
    }

    async fn get_latest_sequence(&self) -> Result<i64> {
        let global_seq = self.global_sequence.read().unwrap();
        Ok(*global_seq)
    }

    async fn stream_exists(&self, stream_id: StreamId) -> Result<bool> {
        let stream_versions = self.stream_versions.read().unwrap();
        Ok(stream_versions.contains_key(&stream_id))
    }

    async fn delete_stream(&self, stream_id: StreamId) -> Result<()> {
        let mut stream_versions = self.stream_versions.write().unwrap();
        let mut stream_events = self.stream_events.write().unwrap();

        stream_versions.remove(&stream_id);
        stream_events.remove(&stream_id);

        Ok(())
    }
}

/// In-memory snapshot store implementation
#[derive(Debug, Clone)]
pub struct InMemorySnapshotStore<S> {
    snapshots: Arc<RwLock<HashMap<EntityId, Vec<S>>>>,
}

impl<S> InMemorySnapshotStore<S> {
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<S> Default for InMemorySnapshotStore<S> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<S> SnapshotStore<S> for InMemorySnapshotStore<S>
where
    S: Snapshot + Send + Sync + Clone + 'static,
{
    type Error = AtomoError;

    async fn save_snapshot(&self, snapshot: S) -> Result<()> {
        let mut snapshots = self.snapshots.write().unwrap();
        let aggregate_id = snapshot.aggregate_id();
        
        snapshots.entry(aggregate_id).or_default().push(snapshot);
        
        Ok(())
    }

    async fn load_snapshot(&self, aggregate_id: EntityId) -> Result<Option<S>> {
        let snapshots = self.snapshots.read().unwrap();
        
        if let Some(aggregate_snapshots) = snapshots.get(&aggregate_id) {
            // Return the latest snapshot
            Ok(aggregate_snapshots.last().cloned())
        } else {
            Ok(None)
        }
    }

    async fn load_snapshot_at_version(
        &self,
        aggregate_id: EntityId,
        version: i64,
    ) -> Result<Option<S>> {
        let snapshots = self.snapshots.read().unwrap();
        
        if let Some(aggregate_snapshots) = snapshots.get(&aggregate_id) {
            // Find the latest snapshot at or before the specified version
            let snapshot = aggregate_snapshots
                .iter()
                .filter(|s| s.version() <= version)
                .max_by_key(|s| s.version())
                .cloned();
            Ok(snapshot)
        } else {
            Ok(None)
        }
    }

    async fn cleanup_snapshots(
        &self,
        aggregate_id: EntityId,
        keep_count: usize,
    ) -> Result<()> {
        let mut snapshots = self.snapshots.write().unwrap();
        
        if let Some(aggregate_snapshots) = snapshots.get_mut(&aggregate_id) {
            if aggregate_snapshots.len() > keep_count {
                // Sort by version and keep only the most recent
                aggregate_snapshots.sort_by_key(|s| s.version());
                aggregate_snapshots.drain(..aggregate_snapshots.len() - keep_count);
            }
        }
        
        Ok(())
    }
}
