//! Platform-level models specific to the Atomo server implementation
//! 
//! These models represent core platform concepts like users, sessions,
//! and system-level entities that are specific to the server implementation.

use atomo_core::EntityId;
use async_graphql::SimpleObject;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// =============================================================================
// User Management Models
// =============================================================================

/// Platform-level user model representing a user account in the system
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "PlatformUser")]
pub struct PlatformUser {
    pub id: EntityId,
    pub email: String,
    #[serde(skip_serializing)] // Never serialize password hash
    #[graphql(skip)] // Never expose password hash in GraphQL
    pub password_hash: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub role: UserRole,
    pub is_active: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// User session for authentication and authorization
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct UserSession {
    pub id: EntityId,
    pub user_id: EntityId,
    #[graphql(skip)] // Never expose session token in GraphQL
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Platform-level user roles with hierarchical permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, async_graphql::Enum)]
pub enum UserRole {
    /// System administrator with full access
    Admin,
    /// Manager with team and data management capabilities
    Manager,
    /// Sales representative with customer data access
    Sales,
    /// Support staff with customer interaction capabilities
    Support,
    /// Read-only viewer access
    Viewer,
}

impl UserRole {
    /// Check if this role has permission to access the given resource level
    pub fn can_access(&self, required_role: UserRole) -> bool {
        use UserRole::*;
        match (self, required_role) {
            (Admin, _) => true,
            (Manager, Manager | Sales | Support | Viewer) => true,
            (Sales, Sales | Viewer) => true,
            (Support, Support | Viewer) => true,
            (Viewer, Viewer) => true,
            _ => false,
        }
    }
    
    /// Get all roles that this role can manage
    pub fn manageable_roles(&self) -> Vec<UserRole> {
        use UserRole::*;
        match self {
            Admin => vec![Admin, Manager, Sales, Support, Viewer],
            Manager => vec![Sales, Support, Viewer],
            Sales => vec![Viewer],
            Support => vec![Viewer],
            Viewer => vec![],
        }
    }
    
    /// Check if this role has permission for the given access pattern
    pub fn has_permission(&self, access_pattern: &str) -> bool {
        let roles: std::collections::HashSet<&str> = access_pattern.split('|').collect();
        
        // Admin has access to everything
        if *self == UserRole::Admin {
            return true;
        }
        
        // Check specific role permissions
        match self {
            UserRole::Admin => true,
            UserRole::Manager => roles.contains("manager") || roles.contains("sales") || roles.contains("support"),
            UserRole::Sales => roles.contains("sales"),
            UserRole::Support => roles.contains("support"),
            UserRole::Viewer => roles.contains("viewer"),
        }
    }
}

impl Default for UserRole {
    fn default() -> Self {
        UserRole::Viewer
    }
}

// Note: AuditLogEntry is already defined in atomo_core::audit
// Use that definition instead of duplicating it here
// If platform-specific extensions are needed, create a wrapper type

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::Manager => write!(f, "manager"),
            UserRole::Sales => write!(f, "sales"),
            UserRole::Support => write!(f, "support"),
            UserRole::Viewer => write!(f, "viewer"),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(UserRole::Admin),
            "manager" => Ok(UserRole::Manager),
            "sales" => Ok(UserRole::Sales),
            "support" => Ok(UserRole::Support),
            "viewer" => Ok(UserRole::Viewer),
            _ => Err(format!("Invalid user role: {}", s)),
        }
    }
}

impl From<String> for UserRole {
    fn from(s: String) -> Self {
        s.parse().unwrap_or(UserRole::Viewer)
    }
}
