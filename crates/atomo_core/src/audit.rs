use serde::{Deserialize, Serialize};
use crate::{EntityId, StreamId, Timestamp};

/// Audit log entry for tracking all entity changes
/// 
/// This is Phase 1 implementation - we use audit_log to track changes
/// before migrating to full Event Sourcing in Phase 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: EntityId,
    pub entity_type: String,
    pub entity_id: EntityId,
    pub stream_id: StreamId,
    pub operation: AuditOperation,
    pub old_data: Option<serde_json::Value>,
    pub new_data: Option<serde_json::Value>,
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: Timestamp,
}

/// Types of operations that can be audited
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditOperation {
    Create,
    Update,
    Delete,
    Read, // For sensitive operations
}

impl AuditLogEntry {
    pub fn new(
        entity_type: String,
        entity_id: EntityId,
        stream_id: StreamId,
        operation: AuditOperation,
        old_data: Option<serde_json::Value>,
        new_data: Option<serde_json::Value>,
        user_id: Option<String>,
    ) -> Self {
        Self {
            id: EntityId::new(),
            entity_type,
            entity_id,
            stream_id,
            operation,
            old_data,
            new_data,
            user_id,
            ip_address: None,
            user_agent: None,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn with_request_metadata(mut self, ip_address: Option<String>, user_agent: Option<String>) -> Self {
        self.ip_address = ip_address;
        self.user_agent = user_agent;
        self
    }
}

/// Context for audit operations
#[derive(Debug, Clone)]
pub struct AuditContext {
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl Default for AuditContext {
    fn default() -> Self {
        Self {
            user_id: None,
            ip_address: None,
            user_agent: None,
        }
    }
}
