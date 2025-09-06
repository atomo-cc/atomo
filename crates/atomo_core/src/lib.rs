//! Atomo Core - The Fundamental Platform Infrastructure
//! 
//! This crate contains only the most essential abstractions and interfaces
//! that define the Atomo platform's core capabilities:
//! 
//! - Base types (EntityId, StreamId, etc.)
//! - Event sourcing abstractions
//! - Authentication and authorization interfaces
//! - Audit logging interfaces
//! - Core domain traits
//!
//! According to the Atomo whitepaper architecture, this core should remain
//! absolutely pure - no concrete implementations, no database code, no 
//! GraphQL schemas, no business logic. All concrete implementations belong 
//! in the server layer.

pub mod types;
pub mod events;
pub mod auth;
pub mod audit;
pub mod traits;
pub mod errors;
pub mod content;

// Re-export core types for easier access
pub use types::{EntityId, StreamId, Timestamp, UserRole};
pub use audit::{AuditOperation, AuditLogEntry};
pub use events::*;
pub use auth::*;
pub use errors::*;
pub use content::*;
