//! Automatic table projections follow current base state, independently of retained history.
use crate::{Projection, TableProjection};
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use std::collections::HashMap;

fn ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
fn table(value: &str) -> String {
    value.split('.').map(ident).collect::<Vec<_>>().join(".")
}

pub struct CurrentStateProjection {
    projection: TableProjection,
    source_table: String,
}
impl CurrentStateProjection {
    pub fn new(
        model: &str,
        source_table: &str,
        projection_table: &str,
        columns: Vec<String>,
    ) -> Self {
        Self {
            projection: TableProjection::new(model, projection_table, columns),
            source_table: source_table.into(),
        }
    }
    fn columns(&self) -> String {
        self.projection
            .columns
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(",")
    }
    fn values(&self) -> String {
        self.projection
            .columns
            .iter()
            .map(|c| format!("COALESCE(b.{}::text,'')", ident(c)))
            .collect::<Vec<_>>()
            .join(",")
    }
    fn conflict(&self, skip_unchanged: bool) -> String {
        let updates = self
            .projection
            .columns
            .iter()
            .filter(|c| c.as_str() != "id")
            .map(|c| format!("{}=EXCLUDED.{}", ident(c), ident(c)))
            .collect::<Vec<_>>();
        let update = if updates.is_empty() {
            "DO UPDATE SET id=EXCLUDED.id".to_string()
        } else {
            format!("DO UPDATE SET {}", updates.join(","))
        };
        if !skip_unchanged {
            return update;
        }
        let current = self
            .projection
            .columns
            .iter()
            .map(|c| format!("p.{}", ident(c)))
            .collect::<Vec<_>>()
            .join(",");
        let next = self
            .projection
            .columns
            .iter()
            .map(|c| format!("EXCLUDED.{}", ident(c)))
            .collect::<Vec<_>>()
            .join(",");
        format!("{update} WHERE ROW({current}) IS DISTINCT FROM ROW({next})")
    }
    /// Add missing nullable TEXT columns and reconcile current rows atomically. This never
    /// edits coverage metadata or claims an event-history replay has become available.
    pub async fn initialize_and_sync(&self, pool: &PgPool) -> Result<u64> {
        if !self.projection.columns.iter().any(|c| c == "id") {
            bail!("CURRENT_PROJECTION_SCHEMA_INVALID: id is required");
        }
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock_shared(718206091)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,718206092))")
            .bind(&self.projection.table_name)
            .execute(&mut *tx)
            .await?;
        // Base writers are paused for this initial snapshot; live refresh always re-reads
        // current state, so notifications queued before this synchronization cannot regress it.
        sqlx::query(&format!(
            "LOCK TABLE {} IN SHARE MODE",
            table(&self.source_table)
        ))
        .execute(&mut *tx)
        .await?;
        let ddl = self
            .projection
            .columns
            .iter()
            .map(|c| {
                format!(
                    "{} TEXT{}",
                    ident(c),
                    if c == "id" { " PRIMARY KEY" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        sqlx::query(&format!(
            "CREATE TABLE IF NOT EXISTS {} ({ddl})",
            table(&self.projection.table_name)
        ))
        .execute(&mut *tx)
        .await?;
        for column in &self.projection.columns {
            let existing:Option<String>=sqlx::query_scalar("SELECT format_type(a.atttypid,a.atttypmod) FROM pg_attribute a WHERE a.attrelid=to_regclass($1) AND a.attname=$2 AND a.attnum>0 AND NOT a.attisdropped").bind(table(&self.projection.table_name)).bind(column).fetch_optional(&mut *tx).await?;
            match existing {
                Some(kind) if kind!="text" => bail!("CURRENT_PROJECTION_SCHEMA_CONFLICT: existing projection column {column} requires an explicit migration"),
                Some(_) => {},
                None if column=="id" => bail!("CURRENT_PROJECTION_SCHEMA_CONFLICT: existing projection has no primary identity"),
                None => {sqlx::query(&format!("ALTER TABLE {} ADD COLUMN IF NOT EXISTS {} TEXT",table(&self.projection.table_name),ident(column))).execute(&mut *tx).await?;}
            }
        }
        sqlx::query(&format!(
            "LOCK TABLE {} IN SHARE ROW EXCLUSIVE MODE",
            table(&self.projection.table_name)
        ))
        .execute(&mut *tx)
        .await?;
        let synced=sqlx::query(&format!("INSERT INTO {} AS p ({}) SELECT {} FROM {} b WHERE b.deleted_at IS NULL ON CONFLICT (id) {}",table(&self.projection.table_name),self.columns(),self.values(),table(&self.source_table),self.conflict(true))).execute(&mut *tx).await?.rows_affected();
        sqlx::query(&format!("DELETE FROM {} p WHERE NOT EXISTS (SELECT 1 FROM {} b WHERE b.id::text=p.id AND b.deleted_at IS NULL)",table(&self.projection.table_name),table(&self.source_table))).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(synced)
    }
    async fn refresh(&self, id: &str, pool: &PgPool) -> Result<()> {
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock_shared(718206091)")
            .execute(&mut *tx)
            .await?;
        let key = serde_json::to_string(&(&self.projection.table_name, id))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,718206093))")
            .bind(key)
            .execute(&mut *tx)
            .await?;
        // Acquire the source relation before INSERT acquires its projection target. This
        // also avoids inversion when schema DDL is queued behind startup's source lock.
        sqlx::query(&format!(
            "LOCK TABLE {} IN ACCESS SHARE MODE",
            table(&self.source_table)
        ))
        .execute(&mut *tx)
        .await?;
        let inserted=sqlx::query(&format!("INSERT INTO {} ({}) SELECT {} FROM {} b WHERE b.id::text=$1 AND b.deleted_at IS NULL ON CONFLICT (id) {}",table(&self.projection.table_name),self.columns(),self.values(),table(&self.source_table),self.conflict(false))).bind(id).execute(&mut *tx).await?.rows_affected();
        if inserted == 0 {
            sqlx::query(&format!(
                "DELETE FROM {} WHERE id=$1",
                table(&self.projection.table_name)
            ))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
#[async_trait]
impl Projection for CurrentStateProjection {
    fn name(&self) -> &str {
        self.projection.name()
    }
    fn source_model(&self) -> &str {
        self.projection.source_model()
    }
    async fn handle_event(
        &self,
        _event_type: &str,
        data: &HashMap<String, Value>,
        pool: &PgPool,
    ) -> Result<()> {
        if let Some(id) = data.get("id").and_then(Value::as_str) {
            self.refresh(id, pool).await?;
        }
        Ok(())
    }
    async fn rebuild(&self, pool: &PgPool) -> Result<()> {
        self.projection.rebuild(pool).await
    }
    fn supports_transactional_rebuild(&self) -> bool {
        true
    }
    async fn rebuild_in(&self, conn: &mut PgConnection) -> Result<()> {
        self.projection.rebuild_in(conn).await
    }
}
