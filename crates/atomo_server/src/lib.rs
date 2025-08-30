//! Atomo Server - High-performance GraphQL server
//!
//! This server is built on top of the Atomo library and provides:
//! - Automatic GraphQL API generation from schema
//! - Real-time subscriptions
//! - High-performance Rust backend
//! - Integration with the Atomo ecosystem

pub mod config;
pub mod server;
pub mod handlers;

pub use config::*;
pub use server::*;

