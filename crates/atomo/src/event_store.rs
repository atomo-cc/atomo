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
                actor TEXT,
                timestamp TEXT NOT NULL DEFAULT now()::text,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query("ALTER TABLE event_log ADD COLUMN IF NOT EXISTS actor TEXT")
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_event_log_model ON event_log (model_name, timestamp)",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_event_log_type ON event_log (model_name, event_type, timestamp)",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_event_log_ts ON event_log (timestamp)")
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_event_log_record ON event_log ((data->>'id')) WHERE data->>'id' IS NOT NULL",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_event_log_actor ON event_log (actor) WHERE actor IS NOT NULL",
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
            "INSERT INTO event_log (event_id, event_type, model_name, data, previous_data, actor, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (event_id) DO NOTHING"
        )
        .bind(&event.event_id)
        .bind(format!("{:?}", event.event_type))
        .bind(&event.model_name)
        .bind(serde_json::to_value(&event.data)?)
        .bind(event.previous_data.as_ref().and_then(|d| serde_json::to_value(d).ok()))
        .bind(&event.actor)
        .bind(&event.timestamp)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// Persist many events on a caller-supplied connection (the batch's transaction) using
    /// **multi-row INSERTs, chunked** to stay under Postgres' 65535 bind-param ceiling — one
    /// statement per chunk instead of one per event. Used by the bulk write paths (`create_many`,
    /// `update_many`, `delete_many`), so a bulk op affecting tens of thousands of rows is safe.
    pub async fn persist_many_in(
        &self,
        conn: &mut sqlx::PgConnection,
        events: &[ModelEvent],
    ) -> Result<()> {
        // 7 bind params per event; 4000 → 28 000 params/chunk, comfortably under the 65535 limit.
        const CHUNK: usize = 4000;
        for chunk in events.chunks(CHUNK) {
            let tuples: Vec<String> = (0..chunk.len())
                .map(|i| {
                    let b = i * 7;
                    format!(
                        "(${},${},${},${},${},${},${})",
                        b + 1,
                        b + 2,
                        b + 3,
                        b + 4,
                        b + 5,
                        b + 6,
                        b + 7
                    )
                })
                .collect();
            let sql = format!(
                "INSERT INTO event_log (event_id, event_type, model_name, data, previous_data, actor, timestamp) \
                 VALUES {} ON CONFLICT (event_id) DO NOTHING",
                tuples.join(", ")
            );
            let mut q = sqlx::query(&sql);
            for e in chunk {
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
                    .bind(e.actor.clone())
                    .bind(e.timestamp.clone());
            }
            q.execute(&mut *conn).await?;
        }
        Ok(())
    }

    /// Replay events for a model, optionally from a given timestamp
    pub async fn replay(&self, model_name: &str, since: Option<&str>) -> Result<Vec<ModelEvent>> {
        let rows = if let Some(ts) = since {
            sqlx::query_as::<_, EventRow>(
                "SELECT event_id, event_type, model_name, data, previous_data, actor, timestamp FROM event_log WHERE model_name = $1 AND timestamp >= $2 ORDER BY timestamp"
            ).bind(model_name).bind(ts).fetch_all(&self.pool).await?
        } else {
            sqlx::query_as::<_, EventRow>(
                "SELECT event_id, event_type, model_name, data, previous_data, actor, timestamp FROM event_log WHERE model_name = $1 ORDER BY timestamp"
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
            "SELECT event_id, event_type, model_name, data, previous_data, actor, timestamp FROM event_log WHERE model_name = $1 AND data->>'id' = $2 ORDER BY timestamp"
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
    actor: Option<String>,
    timestamp: String,
}

impl From<EventRow> for ModelEvent {
    fn from(row: EventRow) -> Self {
        let event_type = match row.event_type.as_str() {
            "Created" => EventType::Created,
            "Updated" => EventType::Updated,
            "Deleted" => EventType::Deleted,
            "Restored" => EventType::Restored,
            "HardDeleted" => EventType::HardDeleted,
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
            actor: row.actor,
            origin: None,
        }
    }
}
