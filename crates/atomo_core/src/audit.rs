//! Audit Core Interfaces
//!
//! This module provides the core interfaces for audit logging in the Atomo platform.
//! Concrete implementations should be provided in the server layer.

use crate::{EntityId, StreamId, Timestamp};
use async_graphql::{Enum, SimpleObject};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Types of operations that can be audited
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Enum)]
pub enum AuditOperation {
    Create,
    Update,
    Delete,
    Read, // For sensitive operations
}

/// Core audit log entry structure
///
/// This represents the minimal structure for audit entries.
/// Implementations can extend this with additional fields.
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct AuditLogEntry {
    pub id: String,
    pub entity_type: String,
    pub entity_id: EntityId,
    pub operation: AuditOperation,
    pub operation_details: String,
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: Timestamp,
}

impl AuditLogEntry {
    /// Create a new audit log entry
    pub fn new(
        entity_type: impl Into<String>,
        entity_id: EntityId,
        operation: AuditOperation,
        operation_details: impl Into<String>,
        user_id: Option<String>,
    ) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            entity_type: entity_type.into(),
            entity_id,
            operation,
            operation_details: operation_details.into(),
            user_id,
            ip_address: None,
            user_agent: None,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn with_request_metadata(
        mut self,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        self.ip_address = ip_address;
        self.user_agent = user_agent;
        self
    }
}

/// Search filters for audit logs
#[derive(Debug, Clone, Default)]
pub struct AuditSearchFilters {
    pub entity_type: Option<String>,
    pub entity_id: Option<EntityId>,
    pub stream_id: Option<StreamId>,
    pub operation: Option<AuditOperation>,
    pub user_id: Option<String>,
    pub start_time: Option<Timestamp>,
    pub end_time: Option<Timestamp>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Audit statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStats {
    pub total_entries: i64,
    pub operations_by_type: std::collections::HashMap<AuditOperation, i64>,
    pub top_users: Vec<UserAuditStats>,
    pub date_range: Option<(Timestamp, Timestamp)>,
}

/// User audit statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAuditStats {
    pub user_id: String,
    pub operation_count: i64,
    pub last_activity: Timestamp,
}

/// Core audit service interface
///
/// This trait defines the audit operations that any audit implementation must provide.
#[async_trait]
pub trait AuditService<AuditEntry>: Send + Sync
where
    AuditEntry: Send + Sync,
{
    type Error: Send + Sync + 'static;

    /// Log an audit entry
    async fn log_audit_entry(&self, entry: AuditEntry) -> Result<(), Self::Error>;

    /// Search audit logs with filters
    async fn search_audit_logs(
        &self,
        filters: &AuditSearchFilters,
    ) -> Result<Vec<AuditEntry>, Self::Error>;

    /// Get audit logs for a specific entity
    async fn get_audit_logs_for_entity(
        &self,
        entity_type: &str,
        entity_id: &EntityId,
    ) -> Result<Vec<AuditEntry>, Self::Error>;

    /// Get audit logs for a specific user
    async fn get_audit_logs_for_user(
        &self,
        user_id: &EntityId,
    ) -> Result<Vec<AuditEntry>, Self::Error>;

    /// Get audit statistics
    async fn get_audit_stats(
        &self,
        filters: &AuditSearchFilters,
    ) -> Result<AuditStats, Self::Error>;
}
