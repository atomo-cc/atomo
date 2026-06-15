//! Server configuration for Atomo server

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub schema_path: String,
    pub service_config_dir: Option<PathBuf>, // New: Path to service configuration directory
    pub cors_origins: Vec<String>,
    pub enable_ai: bool,
    pub enable_subscriptions: bool,
    /// Mount the ephemeral realtime tier (`/realtime/ws`, `/realtime/health`).
    pub enable_realtime: bool,
    /// Opt-in DB-enforced multi-tenant Row-Level Security (`ATOMO_ENABLE_RLS`). Default off;
    /// when on, the server installs `CREATE POLICY` per model table at boot and the data layer
    /// binds `atomo.tenant_id` per request. See `docs/guide/advanced/multi-tenant.md`.
    pub enable_rls: bool,
    /// Initialize the generic metered-command primitives (`ATOMO_ENABLE_METERED_COMMANDS`).
    /// Default on; creates the `expiring_tokens` and `budget_ledger` tables at boot so a consumer
    /// can compose atomic metered commands. Set false to skip them on deployments that do not use
    /// the feature. See `docs/guide/advanced/metered-command-primitives.md`.
    pub enable_metered_commands: bool,
    /// Allow anonymous self-registration via `POST /auth/register`
    /// (`ATOMO_ENABLE_SELF_REGISTRATION`). **Default off** — when disabled the route is not mounted.
    /// Provisioning (granted role, tenant binding) is configured separately; see
    /// `crate::auth::RegistrationConfig` and `docs/api/auth.md`.
    pub enable_self_registration: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3000,
            database_url: "postgresql://localhost/atomo_dev".to_string(),
            schema_path: "./schema.ts".to_string(),
            service_config_dir: None,
            cors_origins: vec!["http://localhost:3000".to_string()],
            enable_ai: false,
            enable_subscriptions: true,
            enable_realtime: true,
            enable_rls: false,
            enable_metered_commands: true,
            enable_self_registration: false,
        }
    }
}

impl ServerConfig {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .unwrap_or(3000),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost/atomo_dev".to_string()),
            schema_path: std::env::var("ATOMO_SCHEMA_PATH")
                .unwrap_or_else(|_| "./schema.ts".to_string()),
            service_config_dir: std::env::var("ATOMO_CONFIG_DIR").ok().map(PathBuf::from),
            cors_origins: std::env::var("CORS_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:3000".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            enable_ai: std::env::var("ATOMO_ENABLE_AI")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            enable_subscriptions: std::env::var("ATOMO_ENABLE_SUBSCRIPTIONS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_realtime: std::env::var("ATOMO_ENABLE_REALTIME")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_rls: std::env::var("ATOMO_ENABLE_RLS")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            enable_metered_commands: std::env::var("ATOMO_ENABLE_METERED_COMMANDS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_self_registration: std::env::var("ATOMO_ENABLE_SELF_REGISTRATION")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        }
    }
}
