//! Core Error Types for Atomo Platform
//!
//! This module defines the standard error types used throughout
//! the Atomo platform. These errors follow the railway-oriented
//! programming pattern for robust error handling.

use crate::types::EntityId;
use thiserror::Error;

/// Primary error type for the Atomo platform
#[derive(Error, Debug)]
pub enum AtomoError {
    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },

    #[error("Conflict: {message}")]
    Conflict { message: String },

    #[error("Unauthorized: {message}")]
    Unauthorized { message: String },

    #[error("Forbidden: {message}")]
    Forbidden { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },

    #[error("Database error: {0}")]
    Database(#[from] anyhow::Error),

    #[error("Concurrency error: Expected version {expected}, but found {actual}")]
    ConcurrencyConflict { expected: i64, actual: i64 },

    #[error("Domain error: {message}")]
    Domain { message: String },

    #[error("Event store error: {message}")]
    EventStore { message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Session expired or invalid")]
    SessionExpired,

    #[error("Rate limit exceeded: {message}")]
    RateLimitExceeded { message: String },
}

impl AtomoError {
    /// Create a validation error with a message
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Create a not found error for an entity
    pub fn not_found(entity: impl Into<String>, id: impl Into<String>) -> Self {
        Self::NotFound {
            entity: entity.into(),
            id: id.into(),
        }
    }

    /// Create a not found error for an entity with EntityId
    pub fn entity_not_found(entity: impl Into<String>, id: EntityId) -> Self {
        Self::NotFound {
            entity: entity.into(),
            id: id.to_string(),
        }
    }

    /// Create a conflict error
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict {
            message: message.into(),
        }
    }

    /// Create an unauthorized error
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized {
            message: message.into(),
        }
    }

    /// Create a forbidden error
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden {
            message: message.into(),
        }
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Create a domain error
    pub fn domain(message: impl Into<String>) -> Self {
        Self::Domain {
            message: message.into(),
        }
    }

    /// Create an event store error
    pub fn event_store(message: impl Into<String>) -> Self {
        Self::EventStore {
            message: message.into(),
        }
    }

    /// Create a concurrency conflict error
    pub fn concurrency_conflict(expected: i64, actual: i64) -> Self {
        Self::ConcurrencyConflict { expected, actual }
    }

    /// Check if this error represents a client error (4xx)
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            AtomoError::Validation { .. }
                | AtomoError::NotFound { .. }
                | AtomoError::Conflict { .. }
                | AtomoError::Unauthorized { .. }
                | AtomoError::Forbidden { .. }
                | AtomoError::Domain { .. }
                | AtomoError::AuthenticationFailed { .. }
                | AtomoError::SessionExpired
                | AtomoError::RateLimitExceeded { .. }
        )
    }

    /// Check if this error represents a server error (5xx)
    pub fn is_server_error(&self) -> bool {
        matches!(
            self,
            AtomoError::Internal { .. }
                | AtomoError::Database(_)
                | AtomoError::EventStore { .. }
                | AtomoError::Serialization(_)
        )
    }

    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            AtomoError::Validation { .. } => 400,
            AtomoError::NotFound { .. } => 404,
            AtomoError::Conflict { .. } => 409,
            AtomoError::ConcurrencyConflict { .. } => 409,
            AtomoError::Unauthorized { .. } => 401,
            AtomoError::Forbidden { .. } => 403,
            AtomoError::AuthenticationFailed { .. } => 401,
            AtomoError::SessionExpired => 401,
            AtomoError::Domain { .. } => 400,
            AtomoError::RateLimitExceeded { .. } => 429,
            AtomoError::Internal { .. } => 500,
            AtomoError::Database(_) => 500,
            AtomoError::EventStore { .. } => 500,
            AtomoError::Serialization(_) => 500,
        }
    }
}

/// Standard Result type for Atomo operations
pub type Result<T> = std::result::Result<T, AtomoError>;

/// Result type for domain operations
pub type DomainResult<T> = Result<T>;

/// Result type for repository operations
pub type RepositoryResult<T> = Result<T>;

/// Result type for service operations
pub type ServiceResult<T> = Result<T>;
