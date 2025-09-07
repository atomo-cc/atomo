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
pub mod auth;
pub mod audit;
pub mod event_store;
pub mod aggregate;
pub mod domain;
pub mod platform_graphql;
pub mod platform_models;
pub mod models_ext;
pub mod schema_metadata;
pub mod model_registry;
pub mod plugins;

pub use config::*;
pub use server::*;
pub use auth::*;
pub use audit::*;
pub use event_store::*;
pub use aggregate::*;
pub use domain::*;
pub use platform_graphql::*;
pub use platform_models::*;

