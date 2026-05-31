//! Event Store Core Abstractions
//!
//! This module defines the core interfaces for event storage in Atomo's
//! event sourcing architecture. Concrete implementations should be in
//! the server layer.

use super::{EventEnvelope, EventType, Snapshot};
use crate::types::{EntityId, StreamId, Timestamp};
use async_trait::async_trait;

/// Core event store interface
///
/// This trait defines the fundamental operations that any event store
/// implementation must provide for Atomo's event sourcing architecture.
#[async_trait]
pub trait EventStore: Send + Sync {
    type Error: Send + Sync + 'static;

    /// Append events to a stream with optimistic concurrency control
    async fn append_events(
        &self,
        stream_id: StreamId,
        expected_version: Option<i64>,
        events: Vec<EventEnvelope>,
    ) -> Result<(), Self::Error>;

    /// Read events from a stream
    async fn read_stream(
        &self,
        stream_id: StreamId,
        from_version: Option<i64>,
        max_count: Option<usize>,
    ) -> Result<Vec<EventEnvelope>, Self::Error>;

    /// Read all events of a specific type across all streams
    async fn read_events_by_type(
        &self,
        event_type: EventType,
        from_timestamp: Option<Timestamp>,
        max_count: Option<usize>,
    ) -> Result<Vec<EventEnvelope>, Self::Error>;

    /// Read events for a specific aggregate
    async fn read_aggregate_events(
        &self,
        aggregate_id: EntityId,
        from_version: Option<i64>,
    ) -> Result<Vec<EventEnvelope>, Self::Error>;

    /// Read events by global sequence number (for projections)
    async fn read_events_from_sequence(
        &self,
        from_sequence: i64,
        max_count: Option<usize>,
    ) -> Result<Vec<EventEnvelope>, Self::Error>;

    /// Get the current version of a stream
    async fn get_stream_version(&self, stream_id: StreamId) -> Result<Option<i64>, Self::Error>;

    /// Get the latest global sequence number
    async fn get_latest_sequence(&self) -> Result<i64, Self::Error>;

    /// Check if a stream exists
    async fn stream_exists(&self, stream_id: StreamId) -> Result<bool, Self::Error>;

    /// Delete a stream (if supported by implementation)
    async fn delete_stream(&self, stream_id: StreamId) -> Result<(), Self::Error>;
}

/// Snapshot store interface for aggregate state snapshots
///
/// Snapshots are used to optimize event replay by periodically
/// capturing aggregate state.
#[async_trait]
pub trait SnapshotStore<S>: Send + Sync
where
    S: Snapshot,
{
    type Error: Send + Sync + 'static;

    /// Save a snapshot for an aggregate
    async fn save_snapshot(&self, snapshot: S) -> Result<(), Self::Error>;

    /// Load the latest snapshot for an aggregate
    async fn load_snapshot(&self, aggregate_id: EntityId) -> Result<Option<S>, Self::Error>;

    /// Load a snapshot at or before a specific version
    async fn load_snapshot_at_version(
        &self,
        aggregate_id: EntityId,
        version: i64,
    ) -> Result<Option<S>, Self::Error>;

    /// Delete old snapshots, keeping only the most recent N
    async fn cleanup_snapshots(
        &self,
        aggregate_id: EntityId,
        keep_count: usize,
    ) -> Result<(), Self::Error>;
}

/// Event store statistics for monitoring and debugging
#[derive(Debug, Clone)]
pub struct EventStoreStats {
    /// Total number of events in the store
    pub total_events: i64,

    /// Total number of streams
    pub total_streams: i64,

    /// Latest global sequence number
    pub latest_sequence: i64,

    /// Events by type
    pub events_by_type: std::collections::HashMap<String, i64>,

    /// Average events per stream
    pub avg_events_per_stream: f64,

    /// Date range of events
    pub date_range: Option<(Timestamp, Timestamp)>,
}

/// Extended event store interface with monitoring capabilities
#[async_trait]
pub trait EventStoreExt: EventStore {
    /// Get statistics about the event store
    async fn get_stats(&self) -> Result<EventStoreStats, Self::Error>;

    /// Get health status of the event store
    async fn health_check(&self) -> Result<bool, Self::Error>;

    /// Compact/optimize the event store (if supported)
    async fn compact(&self) -> Result<(), Self::Error>;
}

/// Event subscription interface for real-time event processing
#[async_trait]
pub trait EventSubscription: Send + Sync {
    type Error: Send + Sync + 'static;
    type Handle: EventSubscriptionHandle + Send + Sync;

    /// Subscribe to all new events
    async fn subscribe_all(&self) -> Result<Box<Self::Handle>, Self::Error>;

    /// Subscribe to events of a specific type
    async fn subscribe_to_type(
        &self,
        event_type: EventType,
    ) -> Result<Box<Self::Handle>, Self::Error>;

    /// Subscribe to events from a specific stream
    async fn subscribe_to_stream(
        &self,
        stream_id: StreamId,
    ) -> Result<Box<Self::Handle>, Self::Error>;
}

/// Handle for managing event subscriptions
#[async_trait]
pub trait EventSubscriptionHandle: Send + Sync {
    type Error: Send + Sync + 'static;

    /// Get the next batch of events
    async fn next_events(&mut self) -> Result<Vec<EventEnvelope>, Self::Error>;

    /// Acknowledge processing of events up to a sequence number
    async fn acknowledge(&mut self, sequence: i64) -> Result<(), Self::Error>;

    /// Close the subscription
    async fn close(self: Box<Self>) -> Result<(), Self::Error>;
}
