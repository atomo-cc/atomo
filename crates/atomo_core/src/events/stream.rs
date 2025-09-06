//! Event Stream Core Abstractions
//!
//! This module defines the core interfaces for event streams in Atomo's
//! event sourcing architecture.

use async_trait::async_trait;
use crate::types::StreamId;
use super::DomainEvent;

/// Event stream interface
/// 
/// Represents a sequence of related events that can be replayed
/// to reconstruct entity state.
#[async_trait]
pub trait EventStream<Event>: Send + Sync 
where
    Event: DomainEvent,
{
    type Error: Send + Sync + 'static;

    /// Get the stream ID
    fn stream_id(&self) -> StreamId;

    /// Get all events in the stream
    async fn events(&self) -> Result<Vec<Event>, Self::Error>;

    /// Get events from a specific version
    async fn events_from_version(&self, version: i64) -> Result<Vec<Event>, Self::Error>;

    /// Get the current version of the stream
    async fn current_version(&self) -> Result<i64, Self::Error>;

    /// Check if the stream is empty
    async fn is_empty(&self) -> Result<bool, Self::Error>;
}

/// Projector interface for building read models from events
/// 
/// Projectors consume events and build materialized views
/// for efficient querying.
#[async_trait]
pub trait Projector<Event>: Send + Sync 
where
    Event: DomainEvent,
{
    type Error: Send + Sync + 'static;

    /// Handle a single event
    async fn handle_event(&mut self, event: &Event) -> Result<(), Self::Error>;

    /// Handle multiple events in sequence
    async fn handle_events(&mut self, events: &[Event]) -> Result<(), Self::Error> {
        for event in events {
            self.handle_event(event).await?;
        }
        Ok(())
    }

    /// Reset the projector state
    async fn reset(&mut self) -> Result<(), Self::Error>;

    /// Get the last processed event position
    async fn last_processed_position(&self) -> Result<Option<i64>, Self::Error>;
}