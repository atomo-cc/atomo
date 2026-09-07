use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;

/// Missing capability metadata is accepted only for legacy installations. Once present,
/// retained/off policies or a historical gap prevent destructive replay.
pub async fn require_complete_history_in(conn: &mut sqlx::PgConnection, model: &str) -> Result<()> {
    let exists: bool =
        sqlx::query_scalar("SELECT to_regclass('model_history_coverage') IS NOT NULL")
            .fetch_one(&mut *conn)
            .await?;
    if exists {
        let state: Option<(String, bool)> =
            sqlx::query_as("SELECT mode, complete FROM model_history_coverage WHERE model_name=$1")
                .bind(model)
                .fetch_optional(&mut *conn)
                .await?;
        if state.is_some_and(|(mode, complete)| mode != "full" || !complete) {
            anyhow::bail!("HISTORY_REPLAY_UNAVAILABLE: model {model} does not have complete full history; projection unchanged");
        }
    }
    Ok(())
}

/// Render a JSON value as the TEXT to store in a projection column. Strings pass through;
/// numbers/bools/etc. are stringified (previously `as_str()` returned None for non-strings,
/// silently binding "" — so numeric fields like Deal.value were lost).
fn value_to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// A projection that materializes a read model from events
#[async_trait]
pub trait Projection: Send + Sync {
    fn name(&self) -> &str;
    fn source_model(&self) -> &str;
    async fn handle_event(
        &self,
        event_type: &str,
        data: &HashMap<String, Value>,
        pool: &PgPool,
    ) -> Result<()>;
    async fn rebuild(&self, pool: &PgPool) -> Result<()>;
    /// Custom projections must opt into connection-bound rebuild to participate in atomic rebuild_all.
    fn supports_transactional_rebuild(&self) -> bool {
        false
    }
    async fn rebuild_in(&self, _conn: &mut sqlx::PgConnection) -> Result<()> {
        anyhow::bail!("PROJECTION_REBUILD_UNSUPPORTED: implement connection-bound rebuild_in first")
    }
}

/// Auto-generated table projection: maintains a denormalized read table
pub struct TableProjection {
    pub name: String,
    pub source_model: String,
    pub table_name: String,
    pub columns: Vec<String>,
}

impl TableProjection {
    pub fn new(source_model: &str, table_name: &str, columns: Vec<String>) -> Self {
        Self {
            name: format!("{}_projection", table_name),
            source_model: source_model.to_string(),
            table_name: table_name.to_string(),
            columns,
        }
    }
    async fn handle_event_connection(
        &self,
        event_type: &str,
        data: &HashMap<String, Value>,
        conn: &mut sqlx::PgConnection,
    ) -> Result<()> {
        match event_type {
            "Created" | "Restored" => {
                let cols: Vec<&str> = self
                    .columns
                    .iter()
                    .filter(|c| data.contains_key(*c))
                    .map(|s| s.as_str())
                    .collect();
                if cols.is_empty() {
                    return Ok(());
                }
                let placeholders: Vec<String> =
                    (1..=cols.len()).map(|i| format!("${}", i)).collect();
                let sql = format!(
                    "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT DO NOTHING",
                    self.table_name,
                    cols.join(", "),
                    placeholders.join(", ")
                );
                let mut query = sqlx::query(&sql);
                for col in &cols {
                    let val = data.get(*col).unwrap_or(&Value::Null);
                    query = query.bind(value_to_text(val));
                }
                query.execute(&mut *conn).await?;
            }
            "Updated" => {
                if let Some(Value::String(id)) = data.get("id") {
                    let sets: Vec<String> = self
                        .columns
                        .iter()
                        .filter(|c| *c != "id" && data.contains_key(*c))
                        .enumerate()
                        .map(|(i, c)| format!("{} = ${}", c, i + 1))
                        .collect();
                    if sets.is_empty() {
                        return Ok(());
                    }
                    let sql = format!(
                        "UPDATE {} SET {} WHERE id = ${}",
                        self.table_name,
                        sets.join(", "),
                        sets.len() + 1
                    );
                    let mut query = sqlx::query(&sql);
                    for col in self
                        .columns
                        .iter()
                        .filter(|c| *c != "id" && data.contains_key(*c))
                    {
                        let val = data.get(col).unwrap_or(&Value::Null);
                        query = query.bind(value_to_text(val));
                    }
                    query = query.bind(id);
                    query.execute(&mut *conn).await?;
                }
            }
            "Deleted" | "HardDeleted" => {
                if let Some(Value::String(id)) = data.get("id") {
                    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", self.table_name))
                        .bind(id)
                        .execute(&mut *conn)
                        .await?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[async_trait]
impl Projection for TableProjection {
    fn name(&self) -> &str {
        &self.name
    }
    fn source_model(&self) -> &str {
        &self.source_model
    }

    async fn handle_event(
        &self,
        event_type: &str,
        data: &HashMap<String, Value>,
        pool: &PgPool,
    ) -> Result<()> {
        let mut conn = pool.acquire().await?;
        self.handle_event_connection(event_type, data, &mut conn)
            .await
    }

    async fn rebuild(&self, pool: &PgPool) -> Result<()> {
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(718206091)")
            .execute(&mut *tx)
            .await?;
        require_complete_history_in(&mut tx, &self.source_model).await?;
        self.rebuild_in(&mut tx).await?;
        tx.commit().await?;
        Ok(())
    }
    fn supports_transactional_rebuild(&self) -> bool {
        true
    }
    async fn rebuild_in(&self, conn: &mut sqlx::PgConnection) -> Result<()> {
        require_complete_history_in(conn, &self.source_model).await?;
        let rows: Vec<(String, Value)> = sqlx::query_as(
            "SELECT event_type, data FROM event_log WHERE model_name = $1 ORDER BY timestamp, created_at",
        ).bind(&self.source_model).fetch_all(&mut *conn).await?;
        sqlx::query(&format!("TRUNCATE TABLE {}", self.table_name))
            .execute(&mut *conn)
            .await?;
        for (event_type, data) in rows {
            let map: HashMap<String, Value> = match data {
                Value::Object(m) => m.into_iter().collect(),
                _ => HashMap::new(),
            };
            self.handle_event_connection(&event_type, &map, conn)
                .await?;
        }
        Ok(())
    }
}
