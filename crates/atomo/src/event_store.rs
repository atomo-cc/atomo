use crate::events::{EventType, ModelEvent};
use crate::history::{HistoryConfig, HistoryMode};
use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;

#[derive(sqlx::FromRow, serde::Serialize)]
struct HistoryCoverage {
    #[serde(rename = "model")]
    model_name: String,
    mode: String,
    complete: bool,
    first_gap_at: Option<String>,
    retained_after: Option<String>,
    removed_events: i64,
}

#[derive(Clone)]
pub struct EventStore {
    pool: PgPool,
    config: HistoryConfig,
    maintenance_cursor: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl EventStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            config: HistoryConfig::default(),
            maintenance_cursor: Default::default(),
        }
    }

    pub fn with_config(pool: PgPool, config: HistoryConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            pool,
            config,
            maintenance_cursor: Default::default(),
        })
    }
    pub fn config(&self) -> &HistoryConfig {
        &self.config
    }
    pub async fn initialize_models<'a>(
        &self,
        models: impl Iterator<Item = &'a String>,
    ) -> Result<()> {
        for model in models {
            let mode = serde_json::to_value(self.config.policy(model).mode)?
                .as_str()
                .unwrap()
                .to_string();
            sqlx::query("INSERT INTO model_history_coverage (model_name, mode) SELECT $1,$2 FROM pg_advisory_xact_lock_shared(718206091) ON CONFLICT(model_name) DO UPDATE SET mode=EXCLUDED.mode")
                .bind(model).bind(mode).execute(&self.pool).await?;
        }
        Ok(())
    }

    /// Ensure the event_log table exists
    pub async fn init(&self) -> Result<()> {
        sqlx::query("CREATE TABLE IF NOT EXISTS model_history_coverage (model_name TEXT PRIMARY KEY, mode TEXT NOT NULL DEFAULT 'full', complete BOOLEAN NOT NULL DEFAULT TRUE, first_gap_at TIMESTAMPTZ, retained_after TIMESTAMPTZ, removed_events BIGINT NOT NULL DEFAULT 0)").execute(&self.pool).await?;
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
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_event_log_retention ON event_log (model_name, created_at, id)").execute(&self.pool).await?;
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
        if self.config.policy(&event.model_name).mode == HistoryMode::Off {
            sqlx::query("INSERT INTO model_history_coverage (model_name, mode, complete, first_gap_at) SELECT $1,'off',FALSE,NOW() FROM pg_advisory_xact_lock_shared(718206091) WHERE NOT EXISTS (SELECT 1 FROM model_history_coverage WHERE model_name=$1 AND complete=FALSE AND first_gap_at IS NOT NULL) ON CONFLICT(model_name) DO UPDATE SET complete=FALSE, first_gap_at=COALESCE(model_history_coverage.first_gap_at,NOW()) WHERE model_history_coverage.complete OR model_history_coverage.first_gap_at IS NULL")
                .bind(&event.model_name).execute(executor).await?;
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO event_log (event_id, event_type, model_name, data, previous_data, actor, timestamp)
             SELECT $1, $2, $3, $4, $5, $6, $7 FROM pg_advisory_xact_lock_shared(718206091)
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
        sqlx::query("SELECT pg_advisory_xact_lock_shared(718206091)")
            .execute(&mut *conn)
            .await?;
        // 7 bind params per event; 4000 → 28 000 params/chunk, comfortably under the 65535 limit.
        const CHUNK: usize = 4000;
        let mut stored = Vec::new();
        let mut skipped = std::collections::HashSet::new();
        for event in events {
            if self.config.policy(&event.model_name).mode == HistoryMode::Off {
                if skipped.insert(&event.model_name) {
                    self.persist_in(&mut *conn, event).await?;
                }
            } else {
                stored.push(event);
            }
        }
        for chunk in stored.chunks(CHUNK) {
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
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(718206091)")
            .execute(&mut *tx)
            .await?;
        let coverage: Option<(String, bool)> =
            sqlx::query_as("SELECT mode,complete FROM model_history_coverage WHERE model_name=$1")
                .bind(model_name)
                .fetch_optional(&mut *tx)
                .await?;
        if self.config.policy(model_name).mode != HistoryMode::Full
            || coverage.is_some_and(|(mode, complete)| mode != "full" || !complete)
        {
            anyhow::bail!(
                "HISTORY_REPLAY_UNAVAILABLE: model {model_name} has incomplete or non-full history"
            );
        }
        let rows = if let Some(ts) = since {
            sqlx::query_as::<_, EventRow>(
                "SELECT event_id, event_type, model_name, data, previous_data, actor, timestamp FROM event_log WHERE model_name = $1 AND timestamp >= $2 ORDER BY timestamp"
            ).bind(model_name).bind(ts).fetch_all(&mut *tx).await?
        } else {
            sqlx::query_as::<_, EventRow>(
                "SELECT event_id, event_type, model_name, data, previous_data, actor, timestamp FROM event_log WHERE model_name = $1 ORDER BY timestamp"
            ).bind(model_name).fetch_all(&mut *tx).await?
        };
        tx.commit().await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Get the available events for an entity; consult diagnostics for coverage gaps.
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

    /// Logical row bytes, not physical relation/WAL size; no payloads are returned.
    pub async fn diagnostics(&self) -> Result<Value> {
        let rows: Vec<HistoryCoverage> = sqlx::query_as("SELECT model_name, mode, complete, first_gap_at::text AS first_gap_at, retained_after::text AS retained_after, removed_events FROM model_history_coverage ORDER BY model_name").fetch_all(&self.pool).await?;
        let (count, bytes): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(pg_column_size(e)),0)::bigint FROM event_log e",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(
            serde_json::json!({"config":self.config,"events":count,"estimated_bytes":bytes,"coverage":rows}),
        )
    }

    /// One bounded deletion batch per configured model. Skipped writes and deletions never
    /// regain complete-replay capability when a later deployment switches back to full.
    pub async fn maintain(&self) -> Result<u64> {
        if !std::iter::once(&self.config.default)
            .chain(self.config.models.values())
            .any(|p| p.mode == HistoryMode::Retained)
        {
            return Ok(0);
        }
        let mut models: Vec<String> =
            sqlx::query_scalar("SELECT DISTINCT model_name FROM event_log ORDER BY model_name")
                .fetch_all(&self.pool)
                .await?;
        if !models.is_empty() {
            let start = self
                .maintenance_cursor
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                % models.len();
            models.rotate_left(start);
        }
        let mut removed = 0;
        let mut remaining = self.config.batch_size;
        for model in models {
            if remaining == 0 {
                break;
            }
            let policy = self.config.policy(&model);
            if policy.mode != HistoryMode::Retained {
                continue;
            }
            let mut tx = self.pool.begin().await?;
            sqlx::query("SELECT pg_advisory_xact_lock(718206091)")
                .execute(&mut *tx)
                .await?;
            let deleted: Vec<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
                "WITH ranked AS (SELECT id, created_at, SUM(pg_column_size(e)) OVER (ORDER BY created_at DESC,id DESC) AS kept_bytes FROM event_log e WHERE model_name=$1), victims AS (SELECT id FROM ranked WHERE ($2::bigint IS NOT NULL AND EXTRACT(EPOCH FROM NOW())-EXTRACT(EPOCH FROM created_at) > $2::numeric) OR ($3::bigint IS NOT NULL AND kept_bytes > $3) ORDER BY created_at,id LIMIT $4) DELETE FROM event_log e USING victims v WHERE e.id=v.id RETURNING e.created_at")
                .bind(&model).bind(policy.max_age_secs.map(|v| v as i64)).bind(policy.max_bytes.map(|v| v as i64)).bind(i64::from(remaining)).fetch_all(&mut *tx).await?;
            if !deleted.is_empty() {
                sqlx::query("INSERT INTO model_history_coverage (model_name,mode,complete,first_gap_at,retained_after,removed_events) VALUES ($1,'retained',FALSE,NOW(),$2::timestamptz,$3) ON CONFLICT(model_name) DO UPDATE SET complete=FALSE,first_gap_at=COALESCE(model_history_coverage.first_gap_at,NOW()),retained_after=GREATEST(model_history_coverage.retained_after,EXCLUDED.retained_after),removed_events=model_history_coverage.removed_events+EXCLUDED.removed_events")
                    .bind(&model).bind(deleted.iter().max()).bind(deleted.len() as i64).execute(&mut *tx).await?;
            }
            tx.commit().await?;
            remaining -= deleted.len() as u32;
            removed += deleted.len() as u64;
        }
        Ok(removed)
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
