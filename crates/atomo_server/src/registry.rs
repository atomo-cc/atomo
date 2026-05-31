//! Plugin marketplace registry — data model + read access (milestone 1).
//!
//! Stores plugin metadata in Postgres (`plugins`, `plugin_versions`) and artifacts
//! (`.wasm` bytes) in a local blob directory. This milestone is read-only: search,
//! version lookup, and artifact download. Publishing is milestone 3.

use anyhow::Result;
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::path::PathBuf;

#[derive(Clone)]
pub struct RegistryStore {
    pool: PgPool,
    blob_dir: PathBuf,
}

impl RegistryStore {
    pub fn new(pool: PgPool, blob_dir: impl Into<PathBuf>) -> Self {
        Self { pool, blob_dir: blob_dir.into() }
    }

    /// Ensure the registry tables exist. Idempotent.
    pub async fn init(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS plugins (
                name TEXT PRIMARY KEY,
                description TEXT NOT NULL DEFAULT '',
                author TEXT NOT NULL DEFAULT '',
                latest_version TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        ).execute(&self.pool).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS plugin_versions (
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                checksum TEXT NOT NULL,
                manifest JSONB NOT NULL DEFAULT '{}',
                artifact_path TEXT NOT NULL,
                signature TEXT,
                published_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (name, version)
            )",
        ).execute(&self.pool).await?;
        tokio::fs::create_dir_all(&self.blob_dir).await.ok();
        Ok(())
    }

    /// Search plugins by name/description substring (empty query = list all).
    pub async fn search(&self, query: &str) -> Result<Vec<Value>> {
        let pattern = format!("%{}%", query);
        let rows = sqlx::query(
            "SELECT name, description, author, latest_version FROM plugins
             WHERE name ILIKE $1 OR description ILIKE $1 ORDER BY name",
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.get::<String, _>("name"),
                    "description": r.get::<String, _>("description"),
                    "author": r.get::<String, _>("author"),
                    "latest_version": r.try_get::<Option<String>, _>("latest_version").ok().flatten(),
                })
            })
            .collect())
    }

    /// Get a plugin's metadata + its versions. Returns None if the plugin is unknown.
    pub async fn get_plugin(&self, name: &str) -> Result<Option<Value>> {
        let plugin = sqlx::query(
            "SELECT name, description, author, latest_version FROM plugins WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        let plugin = match plugin {
            Some(p) => p,
            None => return Ok(None),
        };
        let versions = sqlx::query(
            "SELECT version, checksum, manifest, published_at FROM plugin_versions
             WHERE name = $1 ORDER BY published_at DESC",
        )
        .bind(name)
        .fetch_all(&self.pool)
        .await?;
        let versions: Vec<Value> = versions
            .iter()
            .map(|r| {
                serde_json::json!({
                    "version": r.get::<String, _>("version"),
                    "checksum": r.get::<String, _>("checksum"),
                    "manifest": r.get::<Value, _>("manifest"),
                })
            })
            .collect();
        Ok(Some(serde_json::json!({
            "name": plugin.get::<String, _>("name"),
            "description": plugin.get::<String, _>("description"),
            "author": plugin.get::<String, _>("author"),
            "latest_version": plugin.try_get::<Option<String>, _>("latest_version").ok().flatten(),
            "versions": versions,
        })))
    }

    /// Read the artifact bytes for a specific name@version. None if unknown/missing.
    pub async fn get_artifact(&self, name: &str, version: &str) -> Result<Option<Vec<u8>>> {
        let row = sqlx::query(
            "SELECT artifact_path FROM plugin_versions WHERE name = $1 AND version = $2",
        )
        .bind(name)
        .bind(version)
        .fetch_optional(&self.pool)
        .await?;
        let path = match row {
            Some(r) => r.get::<String, _>("artifact_path"),
            None => return Ok(None),
        };
        // artifact_path is stored relative to blob_dir.
        let full = self.blob_dir.join(path);
        match tokio::fs::read(&full).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(_) => Ok(None),
        }
    }
}
