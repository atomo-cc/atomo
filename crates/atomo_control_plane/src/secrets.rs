//! Secret store — resolves secret *references* (held in the registry) into concrete
//! values, injected as env at instance start. (Phase 0 foundation.)
//!
//! Production store: **AWS SSM Parameter Store** ([`SsmSecretStore`]). [`EnvSecretStore`]
//! is a read-only dev/test fallback.

use crate::error::{ControlPlaneError, Result};
use async_trait::async_trait;

#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Resolve a parameter reference (e.g. `/atomo/<id>/DATABASE_URL`).
    async fn get(&self, reference: &str) -> Result<String>;

    /// Write/overwrite a `SecureString` parameter. Returns its version id.
    async fn put(&self, reference: &str, value: &str) -> Result<String>;
}

/// AWS SSM Parameter Store via the `aws` CLI.
///
/// CLI shell-out keeps the dependency surface minimal for v1; swap for `aws-sdk-ssm`
/// later without changing this trait. Requires the `aws` CLI on PATH with IAM scoped to
/// the project parameter paths.
pub struct SsmSecretStore;

#[async_trait]
impl SecretStore for SsmSecretStore {
    async fn get(&self, reference: &str) -> Result<String> {
        let out = tokio::process::Command::new("aws")
            .args([
                "ssm",
                "get-parameter",
                "--with-decryption",
                "--name",
                reference,
                "--query",
                "Parameter.Value",
                "--output",
                "text",
            ])
            .output()
            .await?;
        if !out.status.success() {
            return Err(ControlPlaneError::Secret(format!(
                "aws ssm get-parameter {reference} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    async fn put(&self, reference: &str, value: &str) -> Result<String> {
        let out = tokio::process::Command::new("aws")
            .args([
                "ssm",
                "put-parameter",
                "--type",
                "SecureString",
                "--overwrite",
                "--name",
                reference,
                "--value",
                value,
                "--query",
                "Version",
                "--output",
                "text",
            ])
            .output()
            .await?;
        if !out.status.success() {
            return Err(ControlPlaneError::Secret(format!(
                "aws ssm put-parameter {reference} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

/// Dev/test fallback. A reference `env:NAME` (or a bare name) resolves to that env var.
pub struct EnvSecretStore;

#[async_trait]
impl SecretStore for EnvSecretStore {
    async fn get(&self, reference: &str) -> Result<String> {
        let key = reference.trim_start_matches("env:");
        std::env::var(key).map_err(|_| ControlPlaneError::Secret(format!("env var not set: {key}")))
    }

    async fn put(&self, _reference: &str, _value: &str) -> Result<String> {
        Err(ControlPlaneError::Secret(
            "EnvSecretStore is read-only".into(),
        ))
    }
}
