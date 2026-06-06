//! Control-plane error type. (Phase 0 foundation.)

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ControlPlaneError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("driver error: {0}")]
    Driver(String),

    #[error("secret store error: {0}")]
    Secret(String),

    #[error("gateway error: {0}")]
    Gateway(String),

    #[error("project not found: {0}")]
    NotFound(String),

    #[error("invalid state transition: {0}")]
    InvalidState(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, ControlPlaneError>;

impl ControlPlaneError {
    /// Convenience for "not implemented yet" scaffolding seams. Returns an error
    /// (does not panic) so partially-built phases stay safe to call.
    pub fn unimplemented(what: &str) -> Self {
        ControlPlaneError::Other(anyhow::anyhow!("not implemented yet: {what}"))
    }
}
