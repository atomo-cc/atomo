use thiserror::Error;

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
    
    #[error("Internal error: {message}")]
    Internal { message: String },
    
    #[error("Database error: {0}")]
    Database(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, AtomoError>;
