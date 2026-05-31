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

// EntityId/StreamId intentionally expose inherent to_string() alongside Display.
#![allow(clippy::inherent_to_string_shadow_display)]

pub mod audit;
pub mod auth;
pub mod content;
pub mod errors;
pub mod events;
pub mod traits;
pub mod types;

// Re-export core types for easier access
pub use audit::{AuditLogEntry, AuditOperation};
pub use auth::*;
pub use content::*;
pub use errors::*;
pub use events::*;
pub use types::{EntityId, StreamId, Timestamp, UserRole};
