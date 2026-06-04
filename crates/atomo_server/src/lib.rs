//! Atomo Server - High-performance GraphQL server
//!
//! This server is built on top of the Atomo library and provides:
//! - Automatic GraphQL API generation from schema
//! - Real-time subscriptions
//! - High-performance Rust backend
//! - Integration with the Atomo ecosystem
#![allow(dead_code)]
#![allow(clippy::too_many_arguments, clippy::match_like_matches_macro)]

pub mod aggregate;
pub mod audit;
pub mod auth;
pub mod config;
pub mod domain;
pub mod event_store;
pub mod handlers;
pub mod media;
pub mod model_registry;
pub mod models_ext;
pub mod oauth;
pub mod platform_graphql;
pub mod platform_models;
pub mod plugin_routes;
pub mod plugins;
pub mod projector_routes;
pub mod rate_limit;
pub mod realtime;
pub mod registry;
pub mod registry_routes;
pub mod schema_metadata;
pub mod server;
pub mod storage;
pub mod tracing_middleware;
pub mod wasm_hooks;
pub mod wasm_plugins;

pub use aggregate::*;
pub use audit::*;
pub use auth::*;
pub use config::*;
pub use event_store::*;
pub use platform_graphql::*;
pub use platform_models::*;
pub use server::*;

/// Convert camelCase/PascalCase to snake_case.
pub fn to_snake(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_uppercase() && !out.is_empty() {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// Pluralized snake_case table name for a model (matches the SQL builder convention).
pub fn pluralize(model_name: &str) -> String {
    to_snake(model_name) + "s"
}

/// Ensure platform tables (users, sessions, audit_log) exist. Idempotent.
pub async fn ensure_platform_tables(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let stmts = [
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            first_name TEXT NOT NULL DEFAULT '',
            last_name TEXT NOT NULL DEFAULT '',
            role TEXT NOT NULL DEFAULT 'viewer',
            is_active BOOLEAN NOT NULL DEFAULT true,
            tenant_id TEXT,
            last_login_at TIMESTAMPTZ,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            version INTEGER NOT NULL DEFAULT 1
        )",
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
            user_id TEXT NOT NULL,
            token TEXT NOT NULL,
            expires_at TIMESTAMPTZ NOT NULL,
            is_revoked BOOLEAN NOT NULL DEFAULT false,
            ip_address TEXT,
            user_agent TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            version INTEGER NOT NULL DEFAULT 1
        )",
        "CREATE TABLE IF NOT EXISTS audit_log (
            id TEXT PRIMARY KEY,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            operation SMALLINT NOT NULL,
            operation_details JSONB,
            user_id TEXT,
            ip_address TEXT,
            user_agent TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    ];
    for sql in stmts {
        sqlx::query(sql).execute(pool).await?;
    }
    // For pre-existing databases: add users.tenant_id if missing (idempotent).
    sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS tenant_id TEXT")
        .execute(pool)
        .await?;
    Ok(())
}

/// Create an admin user from `ADMIN_EMAIL`/`ADMIN_PASSWORD` env vars if it doesn't already exist.
/// No-op when the vars are unset. Idempotent (skips if the email already exists).
pub async fn seed_admin(auth: &crate::auth::HttpAuthService) -> anyhow::Result<()> {
    let (email, password) = match (
        std::env::var("ADMIN_EMAIL"),
        std::env::var("ADMIN_PASSWORD"),
    ) {
        (Ok(e), Ok(p)) if !e.is_empty() && !p.is_empty() => (e, p),
        _ => return Ok(()),
    };
    let pool = auth.db_pool();
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(pool)
        .await?;
    if exists.is_some() {
        return Ok(());
    }
    let id = atomo_core::types::EntityId::new().to_string();
    let hash = auth.hash_password(&password)?;
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, first_name, last_name, role, is_active)
         VALUES ($1, $2, $3, 'Admin', 'User', 'admin', true)",
    )
    .bind(&id)
    .bind(&email)
    .bind(&hash)
    .execute(pool)
    .await?;
    tracing::info!(%email, "Seeded admin user from ADMIN_EMAIL/ADMIN_PASSWORD");
    Ok(())
}

/// Load workflow definitions from `{dir}/*.{json,yaml,yml}` into the engine. Returns count
/// loaded. JSON and YAML both deserialize into the same `Workflow` struct (serde-driven).
pub async fn load_workflows(engine: &atomo::workflow::WorkflowEngine, dir: &str) -> usize {
    let mut count = 0;
    let path = std::path::Path::new(dir);
    if !path.exists() {
        return 0;
    }
    if let Ok(mut entries) = tokio::fs::read_dir(path).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let p = entry.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            let content = match tokio::fs::read_to_string(&p).await {
                Ok(c) => c,
                Err(_) => continue,
            };
            let parsed = match ext {
                "json" => serde_json::from_str::<atomo::workflow::Workflow>(&content).ok(),
                "yaml" | "yml" => serde_yaml::from_str::<atomo::workflow::Workflow>(&content).ok(),
                _ => continue,
            };
            if let Some(wf) = parsed {
                engine.register(wf);
                count += 1;
            }
        }
    }
    count
}
