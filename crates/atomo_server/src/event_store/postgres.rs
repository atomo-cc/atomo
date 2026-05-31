//! PostgreSQL Event Store Implementation
//!
//! Production-ready event store implementation using PostgreSQL
//! with JSONB storage for optimal performance and querying.

use async_trait::async_trait;
use atomo_core::{
    events::{EventEnvelope, EventStore, EventType, Snapshot, SnapshotStore},
    types::{EntityId, StreamId, Timestamp},
    AtomoError, Result,
};
use serde_json::Value;
use sqlx::{PgPool, Row};

/// PostgreSQL event store implementation
#[derive(Debug, Clone)]
pub struct PostgresEventStore {
    pool: PgPool,
}

impl PostgresEventStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialize the event store schema
    pub async fn initialize_schema(&self) -> Result<()> {
        // Create events table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                global_sequence BIGSERIAL PRIMARY KEY,
                event_id TEXT NOT NULL UNIQUE,
                stream_id TEXT NOT NULL,
                stream_version BIGINT NOT NULL,
                event_type TEXT NOT NULL,
                metadata JSONB NOT NULL,
                payload JSONB NOT NULL,
                recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                
                UNIQUE(stream_id, stream_version)
            );
            
            CREATE INDEX IF NOT EXISTS idx_events_stream_id ON events(stream_id);
            CREATE INDEX IF NOT EXISTS idx_events_event_type ON events(event_type);
            CREATE INDEX IF NOT EXISTS idx_events_recorded_at ON events(recorded_at);
            CREATE INDEX IF NOT EXISTS idx_events_metadata_user_id ON events((metadata->>'user_id'));
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        // Create snapshots table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS snapshots (
                id BIGSERIAL PRIMARY KEY,
                aggregate_id TEXT NOT NULL,
                version BIGINT NOT NULL,
                data JSONB NOT NULL,
                taken_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                
                UNIQUE(aggregate_id, version)
            );
            
            CREATE INDEX IF NOT EXISTS idx_snapshots_aggregate_id ON snapshots(aggregate_id);
            CREATE INDEX IF NOT EXISTS idx_snapshots_version ON snapshots(aggregate_id, version DESC);
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        Ok(())
    }
}

#[async_trait]
impl EventStore for PostgresEventStore {
    type Error = AtomoError;

    async fn append_events(
        &self,
        stream_id: StreamId,
        expected_version: Option<i64>,
        events: Vec<EventEnvelope>,
    ) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        // Start transaction for concurrency control
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AtomoError::Database(e.into()))?;

        // Check current stream version for optimistic concurrency control
        let current_version: Option<i64> = sqlx::query_scalar(
            "SELECT stream_version FROM events WHERE stream_id = $1 ORDER BY stream_version DESC LIMIT 1"
        )
        .bind(stream_id.to_string())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        let current_version = current_version.unwrap_or(0);

        if let Some(expected) = expected_version {
            if current_version != expected {
                return Err(AtomoError::concurrency_conflict(expected, current_version));
            }
        }

        // Insert events
        for event in events {
            sqlx::query(
                r#"
                INSERT INTO events (event_id, stream_id, stream_version, event_type, metadata, payload, recorded_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#
            )
            .bind(event.event_id.to_string())
            .bind(event.stream_id.to_string())
            .bind(event.stream_version)
            .bind(serde_json::to_string(&event.event_type).unwrap())
            .bind(serde_json::to_value(&event.metadata).unwrap())
            .bind(event.payload)
            .bind(event.recorded_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| AtomoError::Database(e.into()))?;
        }

        tx.commit()
            .await
            .map_err(|e| AtomoError::Database(e.into()))?;

        Ok(())
    }

    async fn read_stream(
        &self,
        stream_id: StreamId,
        from_version: Option<i64>,
        max_count: Option<usize>,
    ) -> Result<Vec<EventEnvelope>> {
        let from_ver = from_version.unwrap_or(1);
        let limit = max_count.unwrap_or(1000) as i64;

        let rows = sqlx::query(
            r#"
            SELECT global_sequence, event_id, stream_id, stream_version, event_type, metadata, payload, recorded_at
            FROM events 
            WHERE stream_id = $1 AND stream_version >= $2
            ORDER BY stream_version ASC
            LIMIT $3
            "#
        )
        .bind(stream_id.to_string())
        .bind(from_ver)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        let events = rows
            .into_iter()
            .map(|row| {
                let event_type: String = row.get("event_type");
                EventEnvelope {
                    event_id: EntityId::from_string(&row.get::<String, _>("event_id")).unwrap(),
                    stream_id: StreamId::from_string(&row.get::<String, _>("stream_id")).unwrap(),
                    stream_version: row.get("stream_version"),
                    global_sequence: row.get("global_sequence"),
                    event_type: serde_json::from_str(&event_type).unwrap(),
                    metadata: serde_json::from_value(row.get::<Value, _>("metadata")).unwrap(),
                    payload: row.get("payload"),
                    recorded_at: row.get("recorded_at"),
                }
            })
            .collect();

        Ok(events)
    }

    async fn read_events_by_type(
        &self,
        event_type: EventType,
        from_timestamp: Option<Timestamp>,
        max_count: Option<usize>,
    ) -> Result<Vec<EventEnvelope>> {
        let from_time =
            from_timestamp.unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        let limit = max_count.unwrap_or(1000) as i64;
        let event_type_str = serde_json::to_string(&event_type).unwrap();

        let rows = sqlx::query(
            r#"
            SELECT global_sequence, event_id, stream_id, stream_version, event_type, metadata, payload, recorded_at
            FROM events 
            WHERE event_type = $1 AND recorded_at >= $2
            ORDER BY recorded_at ASC
            LIMIT $3
            "#
        )
        .bind(event_type_str)
        .bind(from_time)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        let events = rows
            .into_iter()
            .map(|row| {
                let event_type: String = row.get("event_type");
                EventEnvelope {
                    event_id: EntityId::from_string(&row.get::<String, _>("event_id")).unwrap(),
                    stream_id: StreamId::from_string(&row.get::<String, _>("stream_id")).unwrap(),
                    stream_version: row.get("stream_version"),
                    global_sequence: row.get("global_sequence"),
                    event_type: serde_json::from_str(&event_type).unwrap(),
                    metadata: serde_json::from_value(row.get::<Value, _>("metadata")).unwrap(),
                    payload: row.get("payload"),
                    recorded_at: row.get("recorded_at"),
                }
            })
            .collect();

        Ok(events)
    }

    async fn read_aggregate_events(
        &self,
        aggregate_id: EntityId,
        from_version: Option<i64>,
    ) -> Result<Vec<EventEnvelope>> {
        let from_ver = from_version.unwrap_or(1);

        let rows = sqlx::query(
            r#"
            SELECT global_sequence, event_id, stream_id, stream_version, event_type, metadata, payload, recorded_at
            FROM events 
            WHERE metadata->>'user_id' = $1 AND stream_version >= $2
            ORDER BY recorded_at ASC
            "#
        )
        .bind(aggregate_id.to_string())
        .bind(from_ver)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        let events = rows
            .into_iter()
            .map(|row| {
                let event_type: String = row.get("event_type");
                EventEnvelope {
                    event_id: EntityId::from_string(&row.get::<String, _>("event_id")).unwrap(),
                    stream_id: StreamId::from_string(&row.get::<String, _>("stream_id")).unwrap(),
                    stream_version: row.get("stream_version"),
                    global_sequence: row.get("global_sequence"),
                    event_type: serde_json::from_str(&event_type).unwrap(),
                    metadata: serde_json::from_value(row.get::<Value, _>("metadata")).unwrap(),
                    payload: row.get("payload"),
                    recorded_at: row.get("recorded_at"),
                }
            })
            .collect();

        Ok(events)
    }

    async fn read_events_from_sequence(
        &self,
        from_sequence: i64,
        max_count: Option<usize>,
    ) -> Result<Vec<EventEnvelope>> {
        let limit = max_count.unwrap_or(1000) as i64;

        let rows = sqlx::query(
            r#"
            SELECT global_sequence, event_id, stream_id, stream_version, event_type, metadata, payload, recorded_at
            FROM events 
            WHERE global_sequence >= $1
            ORDER BY global_sequence ASC
            LIMIT $2
            "#
        )
        .bind(from_sequence)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        let events = rows
            .into_iter()
            .map(|row| {
                let event_type: String = row.get("event_type");
                EventEnvelope {
                    event_id: EntityId::from_string(&row.get::<String, _>("event_id")).unwrap(),
                    stream_id: StreamId::from_string(&row.get::<String, _>("stream_id")).unwrap(),
                    stream_version: row.get("stream_version"),
                    global_sequence: row.get("global_sequence"),
                    event_type: serde_json::from_str(&event_type).unwrap(),
                    metadata: serde_json::from_value(row.get::<Value, _>("metadata")).unwrap(),
                    payload: row.get("payload"),
                    recorded_at: row.get("recorded_at"),
                }
            })
            .collect();

        Ok(events)
    }

    async fn get_stream_version(&self, stream_id: StreamId) -> Result<Option<i64>> {
        let version: Option<i64> = sqlx::query_scalar(
            "SELECT stream_version FROM events WHERE stream_id = $1 ORDER BY stream_version DESC LIMIT 1"
        )
        .bind(stream_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        Ok(version)
    }

    async fn get_latest_sequence(&self) -> Result<i64> {
        let sequence: Option<i64> = sqlx::query_scalar(
            "SELECT global_sequence FROM events ORDER BY global_sequence DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        Ok(sequence.unwrap_or(0))
    }

    async fn stream_exists(&self, stream_id: StreamId) -> Result<bool> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM events WHERE stream_id = $1)")
                .bind(stream_id.to_string())
                .fetch_one(&self.pool)
                .await
                .map_err(|e| AtomoError::Database(e.into()))?;

        Ok(exists)
    }

    async fn delete_stream(&self, stream_id: StreamId) -> Result<()> {
        sqlx::query("DELETE FROM events WHERE stream_id = $1")
            .bind(stream_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| AtomoError::Database(e.into()))?;

        Ok(())
    }
}

/// PostgreSQL snapshot store implementation
#[derive(Debug, Clone)]
pub struct PostgresSnapshotStore {
    pool: PgPool,
}

impl PostgresSnapshotStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl<S> SnapshotStore<S> for PostgresSnapshotStore
where
    S: Snapshot + Send + Sync + serde::Serialize + for<'de> serde::Deserialize<'de> + 'static,
{
    type Error = AtomoError;

    async fn save_snapshot(&self, snapshot: S) -> Result<()> {
        let data = serde_json::to_value(&snapshot).map_err(AtomoError::Serialization)?;

        sqlx::query(
            r#"
            INSERT INTO snapshots (aggregate_id, version, data, taken_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (aggregate_id, version) DO UPDATE SET
                data = EXCLUDED.data,
                taken_at = EXCLUDED.taken_at
            "#,
        )
        .bind(snapshot.aggregate_id().to_string())
        .bind(snapshot.version())
        .bind(data)
        .bind(snapshot.taken_at())
        .execute(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        Ok(())
    }

    async fn load_snapshot(&self, aggregate_id: EntityId) -> Result<Option<S>> {
        let row = sqlx::query(
            "SELECT data FROM snapshots WHERE aggregate_id = $1 ORDER BY version DESC LIMIT 1",
        )
        .bind(aggregate_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        if let Some(row) = row {
            let data: Value = row.get("data");
            let snapshot = serde_json::from_value(data).map_err(AtomoError::Serialization)?;
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }

    async fn load_snapshot_at_version(
        &self,
        aggregate_id: EntityId,
        version: i64,
    ) -> Result<Option<S>> {
        let row = sqlx::query(
            "SELECT data FROM snapshots WHERE aggregate_id = $1 AND version <= $2 ORDER BY version DESC LIMIT 1"
        )
        .bind(aggregate_id.to_string())
        .bind(version)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        if let Some(row) = row {
            let data: Value = row.get("data");
            let snapshot = serde_json::from_value(data).map_err(AtomoError::Serialization)?;
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }

    async fn cleanup_snapshots(&self, aggregate_id: EntityId, keep_count: usize) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM snapshots 
            WHERE aggregate_id = $1 
            AND version NOT IN (
                SELECT version FROM snapshots 
                WHERE aggregate_id = $1 
                ORDER BY version DESC 
                LIMIT $2
            )
            "#,
        )
        .bind(aggregate_id.to_string())
        .bind(keep_count as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| AtomoError::Database(e.into()))?;

        Ok(())
    }
}
