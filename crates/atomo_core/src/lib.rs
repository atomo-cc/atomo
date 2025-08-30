//! Atomo Core - The Heart of the Content Core Platform
//! 
//! This crate contains the fundamental platform infrastructure:
//! - Base types (EntityId, StreamId, etc.)
//! - Domain traits and interfaces
//! - Core platform capabilities
//!
//! Business domain models (like CRM) should be separate crates/applications
//! that use these core platform capabilities.

pub mod audit;
pub mod domain;
pub mod events;
pub mod traits;
pub mod types;
pub mod errors;

pub use audit::*;
pub use events::*;
pub use traits::*;
pub use types::*;
pub use errors::*;
