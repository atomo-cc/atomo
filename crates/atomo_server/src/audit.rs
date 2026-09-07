//! Audit Log HTTP Implementation
//!
//! This module provides HTTP-specific implementations of the audit
//! interfaces defined in atomo_core.

use anyhow::Result;
use async_trait::async_trait;
use atomo_core::{
    audit::{
        AuditLogEntry, AuditOperation, AuditSearchFilters, AuditService, AuditStats, UserAuditStats,
    },
    types::EntityId,
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;

/// HTTP Audit Service Implementation
#[derive(Clone)]
pub struct HttpAuditService {
    db_pool: PgPool,
    config: atomo::history::AuditConfig,
    maintenance_cursor: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl HttpAuditService {
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            db_pool,
            config: Default::default(),
            maintenance_cursor: Default::default(),
        }
    }

    pub fn with_config(db_pool: PgPool, config: atomo::history::AuditConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            db_pool,
            config,
            maintenance_cursor: Default::default(),
        })
    }
    pub fn config(&self) -> &atomo::history::AuditConfig {
        &self.config
    }
    pub async fn diagnostics(&self) -> Result<serde_json::Value> {
        let (rows, bytes): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(pg_column_size(a)),0)::bigint FROM audit_log a",
        )
        .fetch_one(&self.db_pool)
        .await?;
        Ok(serde_json::json!({"config":self.config,"rows":rows,"estimated_bytes":bytes}))
    }
    pub async fn maintain(&self) -> Result<u64> {
        if !std::iter::once(&self.config.default)
            .chain(self.config.models.values())
            .any(|p| p.max_age_secs.is_some() || p.max_bytes.is_some())
        {
            return Ok(0);
        }
        let mut models: Vec<String> =
            sqlx::query_scalar("SELECT DISTINCT entity_type FROM audit_log ORDER BY entity_type")
                .fetch_all(&self.db_pool)
                .await?;
        if !models.is_empty() {
            let start = self
                .maintenance_cursor
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                % models.len();
            models.rotate_left(start);
        }
        let mut remaining = self.config.batch_size;
        for model in models {
            if remaining == 0 {
                break;
            }
            let policy = self.config.policy(&model);
            if policy.max_age_secs.is_none() && policy.max_bytes.is_none() {
                continue;
            }
            let removed = sqlx::query("WITH ranked AS (SELECT id, created_at, SUM(pg_column_size(a)) OVER (ORDER BY created_at DESC,id DESC) AS kept_bytes FROM audit_log a WHERE entity_type=$1), victims AS (SELECT id FROM ranked WHERE ($2::bigint IS NOT NULL AND EXTRACT(EPOCH FROM NOW())-EXTRACT(EPOCH FROM created_at) > $2::numeric) OR ($3::bigint IS NOT NULL AND kept_bytes > $3) ORDER BY created_at,id LIMIT $4) DELETE FROM audit_log a USING victims v WHERE a.id=v.id")
                .bind(model).bind(policy.max_age_secs.map(|v| v as i64)).bind(policy.max_bytes.map(|v| v as i64)).bind(i64::from(remaining)).execute(&self.db_pool).await?.rows_affected();
            remaining -= removed as u32;
        }
        Ok(u64::from(self.config.batch_size - remaining))
    }

    /// Get the database connection pool
    pub fn db_pool(&self) -> &PgPool {
        &self.db_pool
    }
}

#[async_trait]
impl AuditService<AuditLogEntry> for HttpAuditService {
    type Error = anyhow::Error;

    async fn log_audit_entry(&self, mut entry: AuditLogEntry) -> Result<(), Self::Error> {
        match self.config.policy(&entry.entity_type).mode {
            atomo::history::AuditMode::Off => return Ok(()),
            atomo::history::AuditMode::Metadata => {
                entry.operation_details = "{}".to_string();
                entry.ip_address = None;
                entry.user_agent = None;
            }
            atomo::history::AuditMode::Full => {}
        }
        sqlx::query(
            "INSERT INTO audit_log (id, entity_type, entity_id, operation, operation_details, user_id, ip_address, user_agent, created_at)
             VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, $8, $9)"
        )
        .bind(&entry.id)
        .bind(&entry.entity_type)
        .bind(entry.entity_id.to_string())
        .bind(entry.operation as i16)
        .bind(&entry.operation_details)
        .bind(&entry.user_id)
        .bind(&entry.ip_address)
        .bind(&entry.user_agent)
        .bind(entry.created_at)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    async fn get_audit_logs_for_entity(
        &self,
        entity_type: &str,
        entity_id: &EntityId,
    ) -> Result<Vec<AuditLogEntry>, Self::Error> {
        let rows = sqlx::query_as::<_, (String, String, String, i16, Option<String>, Option<String>, Option<String>, Option<String>, DateTime<Utc>)>(
            "SELECT id, entity_type, entity_id, operation, operation_details::text AS operation_details, user_id, ip_address, user_agent, created_at
             FROM audit_log 
             WHERE entity_type = $1 AND entity_id = $2
             ORDER BY created_at DESC"
        )
        .bind(entity_type)
        .bind(entity_id.to_string())
        .fetch_all(&self.db_pool)
        .await?;

        let mut entries = Vec::new();
        for row in rows {
            let operation = match row.3 {
                0 => AuditOperation::Create,
                1 => AuditOperation::Update,
                2 => AuditOperation::Delete,
                3 => AuditOperation::Read,
                _ => continue, // Skip invalid operations
            };

            entries.push(AuditLogEntry {
                id: row.0,
                entity_type: row.1,
                entity_id: EntityId::from_string(&row.2)?,
                operation,
                operation_details: row.4.unwrap_or_default(),
                user_id: row.5,
                ip_address: row.6,
                user_agent: row.7,
                created_at: row.8,
            });
        }

        Ok(entries)
    }

    async fn get_audit_logs_for_user(
        &self,
        user_id: &EntityId,
    ) -> Result<Vec<AuditLogEntry>, Self::Error> {
        let rows = sqlx::query_as::<_, (String, String, String, i16, Option<String>, Option<String>, Option<String>, Option<String>, DateTime<Utc>)>(
            "SELECT id, entity_type, entity_id, operation, operation_details::text AS operation_details, user_id, ip_address, user_agent, created_at
             FROM audit_log 
             WHERE user_id = $1
             ORDER BY created_at DESC"
        )
        .bind(user_id.to_string())
        .fetch_all(&self.db_pool)
        .await?;

        let mut entries = Vec::new();
        for row in rows {
            let operation = match row.3 {
                0 => AuditOperation::Create,
                1 => AuditOperation::Update,
                2 => AuditOperation::Delete,
                3 => AuditOperation::Read,
                _ => continue,
            };

            entries.push(AuditLogEntry {
                id: row.0,
                entity_type: row.1,
                entity_id: EntityId::from_string(&row.2)?,
                operation,
                operation_details: row.4.unwrap_or_default(),
                user_id: row.5,
                ip_address: row.6,
                user_agent: row.7,
                created_at: row.8,
            });
        }

        Ok(entries)
    }

    async fn search_audit_logs(
        &self,
        filters: &AuditSearchFilters,
    ) -> Result<Vec<AuditLogEntry>, Self::Error> {
        let mut query = "SELECT id, entity_type, entity_id, operation, operation_details::text AS operation_details, user_id, ip_address, user_agent, created_at FROM audit_log WHERE 1=1".to_string();
        let mut bind_values: Vec<Box<dyn sqlx::Encode<sqlx::Postgres> + Send + Sync>> = Vec::new();
        let mut param_count = 1;

        if let Some(entity_type) = &filters.entity_type {
            query.push_str(&format!(" AND entity_type = ${}", param_count));
            bind_values.push(Box::new(entity_type.clone()));
            param_count += 1;
        }

        if let Some(entity_id) = &filters.entity_id {
            query.push_str(&format!(" AND entity_id = ${}", param_count));
            bind_values.push(Box::new(entity_id.to_string()));
            param_count += 1;
        }

        if let Some(operation) = &filters.operation {
            let op_value = match operation {
                AuditOperation::Create => 0i16,
                AuditOperation::Update => 1i16,
                AuditOperation::Delete => 2i16,
                AuditOperation::Read => 3i16,
            };
            query.push_str(&format!(" AND operation = ${}", param_count));
            bind_values.push(Box::new(op_value));
            param_count += 1;
        }

        if let Some(user_id) = &filters.user_id {
            query.push_str(&format!(" AND user_id = ${}", param_count));
            bind_values.push(Box::new(user_id.clone()));
            param_count += 1;
        }

        if let Some(start_time) = &filters.start_time {
            query.push_str(&format!(" AND created_at >= ${}", param_count));
            bind_values.push(Box::new(*start_time));
            param_count += 1;
        }

        if let Some(end_time) = &filters.end_time {
            query.push_str(&format!(" AND created_at <= ${}", param_count));
            bind_values.push(Box::new(*end_time));
        }

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filters.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = filters.offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        // For now, use a simpler approach with manual parameter binding
        // This is a simplified implementation - in production, use a query builder
        let rows = match (
            &filters.entity_type,
            &filters.entity_id,
            &filters.operation,
            &filters.user_id,
            &filters.start_time,
            &filters.end_time,
        ) {
            (None, None, None, None, None, None) => {
                sqlx::query_as::<_, (String, String, String, i16, Option<String>, Option<String>, Option<String>, Option<String>, DateTime<Utc>)>(
                    "SELECT id, entity_type, entity_id, operation, operation_details::text AS operation_details, user_id, ip_address, user_agent, created_at
                     FROM audit_log ORDER BY created_at DESC LIMIT 100"
                )
                .fetch_all(&self.db_pool)
                .await?
            }
            _ => {
                // For now, just return all entries (simplified implementation)
                sqlx::query_as::<_, (String, String, String, i16, Option<String>, Option<String>, Option<String>, Option<String>, DateTime<Utc>)>(
                    "SELECT id, entity_type, entity_id, operation, operation_details::text AS operation_details, user_id, ip_address, user_agent, created_at
                     FROM audit_log ORDER BY created_at DESC LIMIT 100"
                )
                .fetch_all(&self.db_pool)
                .await?
            }
        };

        let mut entries = Vec::new();
        for row in rows {
            let operation = match row.3 {
                0 => AuditOperation::Create,
                1 => AuditOperation::Update,
                2 => AuditOperation::Delete,
                3 => AuditOperation::Read,
                _ => continue,
            };

            entries.push(AuditLogEntry {
                id: row.0,
                entity_type: row.1,
                entity_id: EntityId::from_string(&row.2)?,
                operation,
                operation_details: row.4.unwrap_or_default(),
                user_id: row.5,
                ip_address: row.6,
                user_agent: row.7,
                created_at: row.8,
            });
        }

        Ok(entries)
    }

    async fn get_audit_stats(
        &self,
        _filters: &AuditSearchFilters,
    ) -> Result<AuditStats, Self::Error> {
        // Get total count
        let total_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_log")
            .fetch_one(&self.db_pool)
            .await?;

        // Get operations by type
        let operation_counts = sqlx::query_as::<_, (i16, i64)>(
            "SELECT operation, COUNT(*) FROM audit_log GROUP BY operation",
        )
        .fetch_all(&self.db_pool)
        .await?;

        let mut operations_by_type = HashMap::new();
        for (op, count) in operation_counts {
            let operation = match op {
                0 => AuditOperation::Create,
                1 => AuditOperation::Update,
                2 => AuditOperation::Delete,
                3 => AuditOperation::Read,
                _ => continue,
            };
            operations_by_type.insert(operation, count);
        }

        // Get top users
        let top_users_data = sqlx::query_as::<_, (String, i64, DateTime<Utc>)>(
            "SELECT user_id, COUNT(*) as count, MAX(created_at) as last_activity 
             FROM audit_log 
             WHERE user_id IS NOT NULL 
             GROUP BY user_id 
             ORDER BY count DESC 
             LIMIT 10",
        )
        .fetch_all(&self.db_pool)
        .await?;

        let top_users = top_users_data
            .into_iter()
            .map(|(user_id, count, last_activity)| UserAuditStats {
                user_id,
                operation_count: count,
                last_activity,
            })
            .collect();

        // Get date range
        let date_range = sqlx::query_as::<_, (Option<DateTime<Utc>>, Option<DateTime<Utc>>)>(
            "SELECT MIN(created_at), MAX(created_at) FROM audit_log",
        )
        .fetch_one(&self.db_pool)
        .await?;

        let date_range = match date_range {
            (Some(min), Some(max)) => Some((min, max)),
            _ => None,
        };

        Ok(AuditStats {
            total_entries: total_count.0,
            operations_by_type,
            top_users,
            date_range,
        })
    }
}

/// Model notifications are best-effort post-commit delivery, independent of history storage.
/// Dropping the owner stops the listener; lag is reported and does not kill future auditing.
pub fn spawn_model_audit_listener(
    audit: HttpAuditService,
    mut rx: tokio::sync::broadcast::Receiver<atomo::events::ModelEvent>,
) -> atomo::history::MaintenanceTask {
    atomo::history::MaintenanceTask(tokio::spawn(async move {
        use atomo_core::audit::AuditService;
        use atomo_core::audit::{AuditLogEntry, AuditOperation};
        use atomo_core::types::EntityId;
        loop {
            let ev = match rx.recv().await {
                Ok(ev) => ev,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                    tracing::warn!(
                        count,
                        "audit listener lagged; notifications are not a durable outbox"
                    );
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            let op = match ev.event_type {
                atomo::events::EventType::Created => AuditOperation::Create,
                atomo::events::EventType::Updated => AuditOperation::Update,
                atomo::events::EventType::Deleted => AuditOperation::Delete,
                atomo::events::EventType::Restored => AuditOperation::Update,
                atomo::events::EventType::HardDeleted => AuditOperation::Delete,
                atomo::events::EventType::Custom => AuditOperation::Read,
            };
            let entity_id = ev
                .data
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| EntityId::from_string(s).ok())
                .unwrap_or_else(EntityId::new);
            let details = serde_json::to_string(&ev.data).unwrap_or_default();
            let entry = AuditLogEntry::new(
                ev.model_name.clone(),
                entity_id,
                op,
                details,
                ev.actor.clone(),
            );
            if let Err(e) = audit.log_audit_entry(entry).await {
                tracing::warn!(error = %e, "Failed to write audit entry");
            }
        }
    }))
}
