//! Authentication and Authorization Core Interfaces
//!
//! This module provides the core interfaces and types for authentication
//! and authorization in the Atomo platform. Concrete implementations
//! should be provided in the server layer.

use crate::EntityId;
use async_trait::async_trait;

/// Authentication credentials
#[derive(Debug, Clone)]
pub struct AuthCredentials {
    pub email: String,
    pub password: String,
}

/// Authentication context for operations
#[derive(Debug, Clone, Default)]
pub struct AuthContext {
    pub user_id: Option<EntityId>,
    pub session_id: Option<EntityId>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Core authentication provider interface
///
/// This trait defines the core authentication operations that any
/// authentication implementation must provide.
#[async_trait]
pub trait AuthProvider<User, Session>: Send + Sync
where
    User: Send + Sync,
    Session: Send + Sync,
{
    type Error: Send + Sync + 'static;

    /// Authenticate user with credentials
    async fn authenticate(
        &self,
        credentials: &AuthCredentials,
    ) -> Result<Option<User>, Self::Error>;

    /// Authorize user for a specific resource and action
    async fn authorize(
        &self,
        user: &User,
        resource: &str,
        action: &str,
    ) -> Result<bool, Self::Error>;

    /// Create a new session for a user
    async fn create_session(&self, user_id: &EntityId) -> Result<Session, Self::Error>;

    /// Validate a session token and return the user
    async fn validate_session(&self, session_token: &str) -> Result<Option<User>, Self::Error>;
}

/// Authorization service interface for role-based access control
#[async_trait]
pub trait AuthorizationService<User>: Send + Sync
where
    User: Send + Sync,
{
    type Error: Send + Sync + 'static;

    /// Get a user by ID
    async fn get_user(&self, user_id: &EntityId) -> Result<Option<User>, Self::Error>;

    /// Get a user by email
    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>, Self::Error>;

    /// Create a new user
    async fn create_user(
        &self,
        email: &str,
        password: &str,
        first_name: &str,
        last_name: &str,
        role: impl Send,
    ) -> Result<User, Self::Error>;

    /// Update an existing user
    async fn update_user(&self, user: &User) -> Result<User, Self::Error>;
}
