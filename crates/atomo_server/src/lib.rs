//! Atomo Server - High-performance GraphQL server
//!
//! This server is built on top of the Atomo library and provides:
//! - Automatic GraphQL API generation from schema
//! - Real-time subscriptions
//! - High-performance Rust backend
//! - Integration with the Atomo ecosystem
#![allow(dead_code)]

pub mod config;
pub mod server;
pub mod handlers;
pub mod tracing_middleware;
pub mod auth;
pub mod oauth;
pub mod audit;
pub mod rate_limit;
pub mod event_store;
pub mod aggregate;
pub mod domain;
pub mod platform_graphql;
pub mod platform_models;
pub mod models_ext;
pub mod schema_metadata;
pub mod model_registry;
pub mod plugins;
pub mod wasm_plugins;
pub mod wasm_hooks;

pub use config::*;
pub use server::*;
pub use auth::*;
pub use audit::*;
pub use event_store::*;
pub use aggregate::*;
pub use platform_graphql::*;
pub use platform_models::*;

/// Convert camelCase/PascalCase to snake_case.
pub fn to_snake(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_uppercase() && !out.is_empty() { out.push('_'); }
        out.extend(c.to_lowercase());
    }
    out
}

/// Pluralized snake_case table name for a model (matches the SQL builder convention).
pub fn pluralize(model_name: &str) -> String {
    to_snake(model_name) + "s"
}

/// Load workflow definitions from `{dir}/*.json` into the engine. Returns count loaded.
pub async fn load_workflows(engine: &atomo::workflow::WorkflowEngine, dir: &str) -> usize {
    let mut count = 0;
    let path = std::path::Path::new(dir);
    if !path.exists() { return 0; }
    if let Ok(mut entries) = tokio::fs::read_dir(path).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("json") { continue; }
            if let Ok(content) = tokio::fs::read_to_string(&p).await {
                if let Ok(wf) = serde_json::from_str::<atomo::workflow::Workflow>(&content) {
                    engine.register(wf);
                    count += 1;
                }
            }
        }
    }
    count
}

