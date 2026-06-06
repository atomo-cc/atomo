//! Operability — per-project backups, fleet observability, resource limits. (Phase 4.)
//!
//! All work here is **shell-outs** (`pg_dump`, `pg_restore`/`psql`, `aws s3 cp`) so the
//! crate stays free of heavy native deps. Each function operates on a single project's
//! database — the silo design means a backup or restore never touches another project.
//!
//! ## Environment variables
//! | Var | Used by | Meaning |
//! |-----|---------|---------|
//! | `ATOMO_BACKUP_S3_BUCKET` | backup | If set, upload the dump to `s3://<bucket>/...`. |
//! | `ATOMO_BACKUP_DIR` | backup | Local destination dir when no S3 bucket (default `./backups`). |
//! | `ATOMO_RESTORE_ALLOW` | restore | Must be truthy (`1`/`true`/`yes`) — restores are destructive. |
//! | `ATOMO_HEALTH_TIMEOUT_MS` | fleet_health | TCP probe timeout per instance (default `2000`). |
//!
//! The project's `DATABASE_URL` is a *secret reference* in the registry, so the public
//! [`backup_project`] / [`restore_project`] entry points resolve it through an
//! [`EnvSecretStore`] by default and delegate to the `*_with` helpers. Callers that
//! already hold a [`SecretStore`] (e.g. the reconciler with an [`SsmSecretStore`]) should
//! resolve the URL themselves and call the `*_with` helpers directly.

use crate::error::{ControlPlaneError, Result};
use crate::registry::Project;
use crate::secrets::{EnvSecretStore, SecretStore};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::process::Command;

/// Where a backup artifact should land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupDest {
    /// A local directory; the dump file is written inside it.
    LocalDir(String),
    /// An S3 bucket; the dump is `aws s3 cp`-ed to `s3://<bucket>/<key>`.
    S3Bucket(String),
}

impl BackupDest {
    /// Pick a destination from the environment: `ATOMO_BACKUP_S3_BUCKET` wins, else
    /// `ATOMO_BACKUP_DIR`, else `./backups`.
    pub fn from_env() -> Self {
        if let Ok(bucket) = std::env::var("ATOMO_BACKUP_S3_BUCKET") {
            if !bucket.trim().is_empty() {
                return BackupDest::S3Bucket(bucket.trim().to_string());
            }
        }
        let dir = std::env::var("ATOMO_BACKUP_DIR")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "./backups".to_string());
        BackupDest::LocalDir(dir)
    }
}

/// Format the artifact file name for a project at a given UTC instant.
///
/// Pure + hermetic so it can be unit-tested without a clock. Shape:
/// `atomo-<id>-<YYYYMMDDTHHMMSSZ>.dump` (custom-format `pg_dump`, restorable with
/// `pg_restore`). The id is sanitised so it is always a safe path/key component.
fn artifact_name(project_id: &str, at: chrono::DateTime<chrono::Utc>) -> String {
    let safe: String = project_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    format!("atomo-{safe}-{}.dump", at.format("%Y%m%dT%H%M%SZ"))
}

/// Run a per-project database backup (silo advantage: independent `pg_dump`/PITR).
///
/// Resolves the project's `DATABASE_URL` via an [`EnvSecretStore`] and picks the
/// destination from the environment (see module docs), then delegates to
/// [`backup_project_with`]. Returns the artifact location (local path or `s3://…` URL).
pub async fn backup_project(project: &Project) -> Result<String> {
    let database_url = EnvSecretStore.get(&project.database_url_ref).await?;
    backup_project_with(project, &database_url, &BackupDest::from_env()).await
}

/// Backup core: `pg_dump --format=custom` the given DB to `dest`, returning the artifact
/// location. For S3 the dump is staged to a temp file then `aws s3 cp`-ed and the temp
/// file removed; for a local dir the file is written directly (dir created if missing).
pub async fn backup_project_with(
    project: &Project,
    database_url: &str,
    dest: &BackupDest,
) -> Result<String> {
    if database_url.trim().is_empty() {
        return Err(ControlPlaneError::Driver(format!(
            "backup {}: empty DATABASE_URL",
            project.id
        )));
    }
    let name = artifact_name(&project.id, chrono::Utc::now());

    match dest {
        BackupDest::LocalDir(dir) => {
            tokio::fs::create_dir_all(dir).await?;
            let path = format!("{}/{}", dir.trim_end_matches(['/', '\\']), name);
            run_pg_dump(database_url, &path).await?;
            tracing::info!(project = %project.id, artifact = %path, "backup complete (local)");
            Ok(path)
        }
        BackupDest::S3Bucket(bucket) => {
            // Stage in the OS temp dir, upload, then clean up.
            let tmp = std::env::temp_dir().join(&name);
            let tmp_str = tmp.to_string_lossy().to_string();
            run_pg_dump(database_url, &tmp_str).await?;

            let s3_uri = format!("s3://{}/{}", bucket.trim_end_matches('/'), name);
            let out = Command::new("aws")
                .args(["s3", "cp", &tmp_str, &s3_uri])
                .output()
                .await?;
            let _ = tokio::fs::remove_file(&tmp).await;
            if !out.status.success() {
                return Err(ControlPlaneError::Driver(format!(
                    "aws s3 cp -> {s3_uri} failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                )));
            }
            tracing::info!(project = %project.id, artifact = %s3_uri, "backup complete (s3)");
            Ok(s3_uri)
        }
    }
}

/// `pg_dump --format=custom --no-owner --no-privileges <url> --file <path>`.
async fn run_pg_dump(database_url: &str, out_path: &str) -> Result<()> {
    let out = Command::new("pg_dump")
        .args([
            "--format=custom",
            "--no-owner",
            "--no-privileges",
            "--file",
            out_path,
            database_url,
        ])
        .output()
        .await?;
    if !out.status.success() {
        return Err(ControlPlaneError::Driver(format!(
            "pg_dump failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

/// True when an env var holds a truthy value (`1`/`true`/`yes`/`on`, case-insensitive).
fn env_truthy(key: &str) -> bool {
    std::env::var(key)
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

/// Restore a single project DB from a backup artifact (does not touch other projects).
///
/// **Destructive** — refuses unless `ATOMO_RESTORE_ALLOW` is truthy. Resolves the
/// project's `DATABASE_URL` via an [`EnvSecretStore`] and delegates to
/// [`restore_project_with`].
pub async fn restore_project(project: &Project, artifact: &str) -> Result<()> {
    if !env_truthy("ATOMO_RESTORE_ALLOW") {
        return Err(ControlPlaneError::InvalidState(format!(
            "restore {} refused: set ATOMO_RESTORE_ALLOW=1 (restores are destructive)",
            project.id
        )));
    }
    let database_url = EnvSecretStore.get(&project.database_url_ref).await?;
    restore_project_with(project, &database_url, artifact).await
}

/// Restore core: run `pg_restore --clean --if-exists` for a custom/dir dump, or `psql`
/// for a plain `.sql` artifact, into `database_url`. The `allow` guard is the caller's
/// responsibility here (the public [`restore_project`] enforces the env flag).
pub async fn restore_project_with(
    project: &Project,
    database_url: &str,
    artifact: &str,
) -> Result<()> {
    if database_url.trim().is_empty() {
        return Err(ControlPlaneError::Driver(format!(
            "restore {}: empty DATABASE_URL",
            project.id
        )));
    }

    let is_sql = artifact.to_ascii_lowercase().ends_with(".sql");
    let out = if is_sql {
        Command::new("psql")
            .args(["--dbname", database_url, "--file", artifact])
            .output()
            .await?
    } else {
        Command::new("pg_restore")
            .args([
                "--clean",
                "--if-exists",
                "--no-owner",
                "--no-privileges",
                "--dbname",
                database_url,
                artifact,
            ])
            .output()
            .await?
    };

    if !out.status.success() {
        let tool = if is_sql { "psql" } else { "pg_restore" };
        return Err(ControlPlaneError::Driver(format!(
            "{tool} restore of {artifact} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    tracing::info!(project = %project.id, %artifact, "restore complete");
    Ok(())
}

/// Probe timeout from `ATOMO_HEALTH_TIMEOUT_MS` (default 2000ms).
fn health_timeout() -> Duration {
    let ms = std::env::var("ATOMO_HEALTH_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(2000);
    Duration::from_millis(ms)
}

/// Probe one project's upstream: classify into `"up"`, `"down"`, `"timeout"`, or
/// `"unconfigured"` (no upstream set). Never errors — failures become a status string.
async fn probe_upstream(upstream: Option<&str>, timeout: Duration) -> &'static str {
    let Some(addr) = upstream else {
        return "unconfigured";
    };
    match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
        Ok(Ok(_)) => "up",
        Ok(Err(_)) => "down",
        Err(_) => "timeout",
    }
}

/// Aggregate per-project health/metrics into a fleet observability view.
///
/// For each project: TCP-connects to `project.upstream` (short timeout) and surfaces the
/// last reconciler-written `last_health`. One unreachable instance never fails the call —
/// it is marked in its own entry. Returns a JSON array of
/// `{ id, status, upstream, last_health }`.
pub async fn fleet_health(projects: &[Project]) -> Result<serde_json::Value> {
    let timeout = health_timeout();
    let mut entries = Vec::with_capacity(projects.len());
    for p in projects {
        let status = probe_upstream(p.upstream.as_deref(), timeout).await;
        entries.push(serde_json::json!({
            "id": p.id,
            "status": status,
            "upstream": p.upstream,
            "last_health": p.last_health,
        }));
    }
    Ok(serde_json::Value::Array(entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> chrono::DateTime<chrono::Utc> {
        use chrono::TimeZone;
        chrono::Utc.with_ymd_and_hms(2026, 6, 6, 13, 5, 9).unwrap()
    }

    #[test]
    fn artifact_name_shape() {
        assert_eq!(artifact_name("blog", ts()), "atomo-blog-20260606T130509Z.dump");
    }

    #[test]
    fn artifact_name_sanitises_unsafe_chars() {
        // Slashes / dots / spaces must not leak into a path or S3 key.
        assert_eq!(
            artifact_name("a/b .c", ts()),
            "atomo-a_b__c-20260606T130509Z.dump"
        );
        let n = artifact_name("../etc", ts());
        assert!(!n.contains('/'), "name must not contain a path separator: {n}");
        assert!(!n.contains('.') || n.ends_with(".dump"));
    }

    #[test]
    fn backup_dest_from_env_prefers_s3() {
        // Hermetic: set then clear within the same test (serialised by default? guard).
        std::env::set_var("ATOMO_BACKUP_S3_BUCKET", "my-bucket");
        assert_eq!(
            BackupDest::from_env(),
            BackupDest::S3Bucket("my-bucket".into())
        );
        std::env::remove_var("ATOMO_BACKUP_S3_BUCKET");

        std::env::set_var("ATOMO_BACKUP_DIR", "/tmp/atomo-bk");
        assert_eq!(
            BackupDest::from_env(),
            BackupDest::LocalDir("/tmp/atomo-bk".into())
        );
        std::env::remove_var("ATOMO_BACKUP_DIR");

        // default
        assert_eq!(BackupDest::from_env(), BackupDest::LocalDir("./backups".into()));
    }

    #[test]
    fn env_truthy_variants() {
        std::env::set_var("ATOMO_TEST_TRUTHY", "YES");
        assert!(env_truthy("ATOMO_TEST_TRUTHY"));
        std::env::set_var("ATOMO_TEST_TRUTHY", "0");
        assert!(!env_truthy("ATOMO_TEST_TRUTHY"));
        std::env::remove_var("ATOMO_TEST_TRUTHY");
        assert!(!env_truthy("ATOMO_TEST_DEFINITELY_UNSET"));
    }

    #[tokio::test]
    async fn probe_unconfigured_is_marked_not_errored() {
        assert_eq!(probe_upstream(None, Duration::from_millis(50)).await, "unconfigured");
    }

    #[tokio::test]
    async fn fleet_health_does_not_fail_on_unreachable() {
        // An unroutable / closed address must yield an entry, not an error.
        let p = Project {
            id: "p1".into(),
            display_name: "P1".into(),
            hostname: None,
            aliases: vec![],
            database_url_ref: "env:NOPE".into(),
            schema_ref: crate::registry::SchemaRef::Volume { path: "schema.ts".into() },
            schema_version: None,
            upstream: Some("127.0.0.1:1".into()), // almost certainly closed
            env: serde_json::json!({}),
            status: crate::registry::ProjectStatus::Running,
            desired_state: crate::registry::DesiredState::Running,
            last_health: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let v = fleet_health(std::slice::from_ref(&p)).await.unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "p1");
        let st = arr[0]["status"].as_str().unwrap();
        assert!(matches!(st, "down" | "timeout"), "unexpected status: {st}");
    }
}
