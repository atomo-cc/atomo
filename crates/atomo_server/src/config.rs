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
        }
    }
}
