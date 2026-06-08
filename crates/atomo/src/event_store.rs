use crate::events::{EventType, ModelEvent};
use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;

#[derive(Clone)]
pub struct EventStore {
    pool: PgPool,
}

impl EventStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Ensure the event_log table exists
    pub async fn init(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS event_log (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                event_id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL,
                model_name TEXT NOT NULL,
                data JSONB NOT NULL DEFAULT '{}',
                previous_data JSONB,
                timestamp TEXT NOT NULL DEFAULT now()::text,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_event_log_model ON event_log (model_name, timestamp)",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Persist a single event (own connection / autocommit).
    pub async fn persist(&self, event: &ModelEvent) -> Result<()> {
        self.persist_in(&self.pool, event).await
    }

    /// Persist a single event using a caller-supplied executor — e.g. the **same transaction** as
    /// the write that produced it, so the row and its event commit together in **one** `fsync`
    /// instead of two. Pass `&mut *tx` (a `&mut PgConnection`) to enlist in an open transaction.
    pub async fn persist_in<'e, E>(&self, executor: E, event: &ModelEvent) -> Result<()>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query(
            "INSERT INTO event_log (event_id, event_type, model_name, data, previous_data, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (event_id) DO NOTHING"
        )
        .bind(&event.event_id)
        .bind(format!("{:?}", event.event_type))
        .bind(&event.model_name)
        .bind(serde_json::to_value(&event.data)?)
        .bind(event.previous_data.as_ref().and_then(|d| serde_json::to_value(d).ok()))
        .bind(&event.timestamp)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// Persist many events in a single **multi-row** INSERT on a caller-supplied executor (the
    /// batch's transaction) — one statement instead of N. Used by `create_many`.
    pub async fn persist_many_in<'e, E>(&self, executor: E, events: &[ModelEvent]) -> Result<()>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        if events.is_empty() {
            return Ok(());
        }
        let tuples: Vec<String> = (0..events.len())
            .map(|i| {
                let b = i * 6;
                format!(
                    "(${},${},${},${},${},${})",
                    b + 1,
                    b + 2,
                    b + 3,
                    b + 4,
                    b + 5,
                    b + 6
                )
            })
            .collect();
        let sql = format!(
            "INSERT INTO event_log (event_id, event_type, model_name, data, previous_data, timestamp) \
             VALUES {} ON CONFLICT (event_id) DO NOTHING",
            tuples.join(", ")
        );
        let mut q = sqlx::query(&sql);
        for e in events {
            q = q
                .bind(e.event_id.clone())
                .bind(format!("{:?}", e.event_type))
                .bind(e.model_name.clone())
                .bind(serde_json::to_value(&e.data)?)
                .bind(
                    e.previous_data
                        .as_ref()
                        .and_then(|d| serde_json::to_value(d).ok()),
                )
                .bind(e.timestamp.clone());
        }
        q.execute(executor).await?;
        Ok(())
    }

    /// Replay events for a model, optionally from a given timestamp
    pub async fn replay(&self, model_name: &str, since: Option<&str>) -> Result<Vec<ModelEvent>> {
        let rows = if let Some(ts) = since {
            sqlx::query_as::<_, EventRow>(
                "SELECT event_id, event_type, model_name, data, previous_data, timestamp FROM event_log WHERE model_name = $1 AND timestamp >= $2 ORDER BY timestamp"
            ).bind(model_name).bind(ts).fetch_all(&self.pool).await?
        } else {
            sqlx::query_as::<_, EventRow>(
                "SELECT event_id, event_type, model_name, data, previous_data, timestamp FROM event_log WHERE model_name = $1 ORDER BY timestamp"
            ).bind(model_name).fetch_all(&self.pool).await?
        };
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Get all events for a specific entity by ID
    pub async fn entity_history(
        &self,
        model_name: &str,
        entity_id: &str,
    ) -> Result<Vec<ModelEvent>> {
        let rows = sqlx::query_as::<_, EventRow>(
            "SELECT event_id, event_type, model_name, data, previous_data, timestamp FROM event_log WHERE model_name = $1 AND data->>'id' = $2 ORDER BY timestamp"
        ).bind(model_name).bind(entity_id).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }
}

#[derive(sqlx::FromRow)]
struct EventRow {
    event_id: String,
    event_type: String,
    model_name: String,
    data: Value,
    previous_data: Option<Value>,
    timestamp: String,
}

impl From<EventRow> for ModelEvent {
    fn from(row: EventRow) -> Self {
        let event_type = match row.event_type.as_str() {
            "Created" => EventType::Created,
            "Updated" => EventType::Updated,
            "Deleted" => EventType::Deleted,
            _ => EventType::Created,
        };
        let data = match row.data {
            Value::Object(map) => map.into_iter().collect(),
            _ => HashMap::new(),
        };
        let previous_data = row.previous_data.and_then(|v| match v {
            Value::Object(map) => Some(map.into_iter().collect()),
            _ => None,
        });
        ModelEvent {
            event_type,
            model_name: row.model_name,
            data,
            previous_data,
            timestamp: row.timestamp,
            event_id: row.event_id,
            actor: None,
        }
    }
}
