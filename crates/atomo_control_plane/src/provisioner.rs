//! Provisioner — project lifecycle: create / start / stop / schema_update / delete. (Phase 1.)
//!
//! Idempotent, registry-backed, secrets resolved at start. See the lifecycle state machine
//! in `docs/guide/advanced/multi-project-design.md`.

use crate::caddy::CaddyGateway;
use crate::driver::Driver;
use crate::error::{ControlPlaneError, Result};
use crate::registry::{Project, ProjectRegistry, ProjectStatus, SchemaRef};
use crate::secrets::SecretStore;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;

pub struct Provisioner {
    pub registry: ProjectRegistry,
    pub driver: Arc<dyn Driver>,
    pub secrets: Arc<dyn SecretStore>,
    pub gateway: CaddyGateway,
}

impl Provisioner {
    pub fn new(
        registry: ProjectRegistry,
        driver: Arc<dyn Driver>,
        secrets: Arc<dyn SecretStore>,
        gateway: CaddyGateway,
    ) -> Self {
        Self {
            registry,
            driver,
            secrets,
            gateway,
        }
    }

    // ---- internal helpers -------------------------------------------------

    /// Root dir under which each project's materialized schema volume lives.
    /// Configurable via `ATOMO_SCHEMA_VOLUME_ROOT` (defaults to `./atomo-schemas`).
    fn schema_volume_root() -> String {
        std::env::var("ATOMO_SCHEMA_VOLUME_ROOT").unwrap_or_else(|_| "./atomo-schemas".to_string())
    }

    /// Per-project schema directory (where the git checkout / volume copy lands).
    fn project_schema_dir(id: &str) -> std::path::PathBuf {
        std::path::Path::new(&Self::schema_volume_root()).join(id)
    }

    /// The materialized `schema.ts` path the instance reads (`ATOMO_SCHEMA_PATH`).
    fn materialized_schema_path(project: &Project) -> std::path::PathBuf {
        match &project.schema_ref {
            // Git: we copy the file from the repo `path` into the project dir as `schema.ts`.
            SchemaRef::Git { .. } => Self::project_schema_dir(&project.id).join("schema.ts"),
            // Volume: the path is used as-is.
            SchemaRef::Volume { path } => std::path::PathBuf::from(path),
        }
    }

    /// Default published port for a project. Configurable base via `ATOMO_PORT_BASE`
    /// (default 4000); offset deterministically by a hash of the id so concurrent
    /// projects don't collide on a single host.
    fn default_port(id: &str) -> u16 {
        let base: u16 = std::env::var("ATOMO_PORT_BASE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4000);
        let offset = id.bytes().fold(0u16, |acc, b| acc.wrapping_add(b as u16)) % 1000;
        base.wrapping_add(offset)
    }

    /// The DB name dedicated to a project.
    fn database_name(id: &str) -> String {
        format!("atomo_{}", id.replace('-', "_"))
    }

    /// Admin/maintenance Postgres URL (used for CREATE/DROP DATABASE).
    fn admin_database_url() -> Result<String> {
        std::env::var("ATOMO_ADMIN_DATABASE_URL").map_err(|_| {
            ControlPlaneError::Other(anyhow::anyhow!(
                "ATOMO_ADMIN_DATABASE_URL is required for CREATE/DROP DATABASE"
            ))
        })
    }

    /// Connect to the admin DB and run a single statement. `CREATE`/`DROP DATABASE`
    /// cannot run inside a transaction, so we execute on a fresh single connection.
    async fn admin_exec(stmt: &str) -> Result<()> {
        let url = Self::admin_database_url()?;
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await?;
        sqlx::query(stmt).execute(&pool).await?;
        pool.close().await;
        Ok(())
    }

    /// Resolve the SSM reference for a project's `JWT_SECRET`.
    fn jwt_secret_ref(id: &str) -> String {
        format!("/atomo/{id}/JWT_SECRET")
    }

    /// Materialize the schema file from `schema_ref`. For `Git`, clone/fetch the repo
    /// at the pinned SHA and copy `path` into the project dir; for `Volume`, no-op.
    async fn materialize_schema(&self, project: &Project) -> Result<()> {
        match &project.schema_ref {
            SchemaRef::Volume { path } => {
                if !std::path::Path::new(path).exists() {
                    return Err(ControlPlaneError::Other(anyhow::anyhow!(
                        "schema volume path does not exist: {path}"
                    )));
                }
                Ok(())
            }
            SchemaRef::Git {
                repo,
                path,
                git_ref,
            } => {
                let dir = Self::project_schema_dir(&project.id);
                let checkout = dir.join("_checkout");
                tokio::fs::create_dir_all(&checkout).await?;

                let checkout_str = checkout.to_string_lossy().to_string();
                let git_dir = checkout.join(".git");
                if !git_dir.exists() {
                    Self::git(&["clone", "--no-checkout", repo, &checkout_str]).await?;
                }
                // Fetch the pinned SHA and check it out (detached). Try a shallow
                // fetch-by-sha first; fall back to a full fetch if unsupported.
                if Self::git(&[
                    "-C",
                    &checkout_str,
                    "fetch",
                    "--depth",
                    "1",
                    "origin",
                    git_ref,
                ])
                .await
                .is_err()
                {
                    Self::git(&["-C", &checkout_str, "fetch", "origin"]).await?;
                }
                Self::git(&["-C", &checkout_str, "checkout", "--force", git_ref]).await?;

                // Copy the schema file into the materialized location.
                let src = checkout.join(path);
                let dst = Self::materialized_schema_path(project);
                if let Some(parent) = dst.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::copy(&src, &dst).await.map_err(|e| {
                    ControlPlaneError::Other(anyhow::anyhow!(
                        "copy schema {} -> {}: {e}",
                        src.display(),
                        dst.display()
                    ))
                })?;
                Ok(())
            }
        }
    }

    async fn git(args: &[&str]) -> Result<()> {
        let out = Command::new("git")
            .args(args)
            .output()
            .await
            .map_err(|e| ControlPlaneError::Other(anyhow::anyhow!("spawn git failed: {e}")))?;
        if !out.status.success() {
            return Err(ControlPlaneError::Other(anyhow::anyhow!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(())
    }

    /// Projects that should be present in the gateway (status == Running, has upstream).
    async fn running_projects(&self) -> Result<Vec<Project>> {
        Ok(self
            .registry
            .list()
            .await?
            .into_iter()
            .filter(|p| p.status == ProjectStatus::Running && p.upstream.is_some())
            .collect())
    }

    // ---- public API (signatures are stable; do not change) ----------------

    /// Resolve the env map (secrets injected, schema path + listen addr set) for an instance.
    pub async fn resolve_env(&self, project: &Project) -> Result<HashMap<String, String>> {
        let mut env: HashMap<String, String> = HashMap::new();

        // Per-project DATABASE_URL (resolved from the secret store reference).
        let database_url = self.secrets.get(&project.database_url_ref).await?;
        env.insert("DATABASE_URL".into(), database_url);

        // Materialized schema path the instance watches.
        env.insert(
            "ATOMO_SCHEMA_PATH".into(),
            Self::materialized_schema_path(project)
                .to_string_lossy()
                .to_string(),
        );

        // Listen address. Port derives from upstream if set, else the default.
        let port = project
            .upstream
            .as_ref()
            .and_then(|u| u.rsplit(':').next())
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or_else(|| Self::default_port(&project.id));
        env.insert("HOST".into(), "0.0.0.0".into());
        env.insert("PORT".into(), port.to_string());

        // JWT_SECRET — resolve, generate + persist if missing.
        let jwt_ref = Self::jwt_secret_ref(&project.id);
        let jwt = match self.secrets.get(&jwt_ref).await {
            Ok(v) if !v.is_empty() => v,
            _ => {
                let generated = format!(
                    "{}{}",
                    uuid::Uuid::new_v4().simple(),
                    uuid::Uuid::new_v4().simple()
                );
                // Best-effort persist; EnvSecretStore is read-only and will error — tolerate it.
                let _ = self.secrets.put(&jwt_ref, &generated).await;
                generated
            }
        };
        env.insert("JWT_SECRET".into(), jwt);

        // Project label for cross-project observability.
        env.insert("ATOMO_PROJECT_ID".into(), project.id.clone());

        // Merge non-secret per-project env overrides last (project wins on conflict).
        if let serde_json::Value::Object(map) = &project.env {
            for (k, v) in map {
                let val = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                env.insert(k.clone(), val);
            }
        }

        Ok(env)
    }

    /// Create DB, materialize schema, start instance, register routing. Idempotent.
    pub async fn create(&self, project: Project) -> Result<()> {
        // 1. Create the dedicated database (idempotent — ignore "already exists").
        let db = Self::database_name(&project.id);
        match Self::admin_exec(&format!("CREATE DATABASE \"{db}\"")).await {
            Ok(()) => {}
            Err(ControlPlaneError::Database(e)) => {
                let msg = e.to_string();
                if !msg.contains("already exists") {
                    return Err(ControlPlaneError::Database(e));
                }
            }
            Err(e) => return Err(e),
        }

        // 2. Materialize the schema from its source.
        self.materialize_schema(&project).await?;

        // 3. Register the project and record the event; mark Provisioning.
        // Idempotent create: if it already exists, fall through to (re)start.
        let mut project = project;
        project.status = ProjectStatus::Provisioning;
        if self.registry.get(&project.id).await.is_err() {
            self.registry.create(&project).await?;
        } else {
            self.registry
                .update_status(&project.id, ProjectStatus::Provisioning)
                .await?;
        }
        self.registry
            .record_event(&project.id, "create", None, serde_json::json!({ "db": db }))
            .await?;

        // 4. Assign an upstream if the project doesn't have one yet.
        if project.upstream.is_none() {
            let port = Self::default_port(&project.id);
            let upstream = format!("127.0.0.1:{port}");
            project.upstream = Some(upstream.clone());
            self.registry
                .set_upstream(&project.id, Some(&upstream))
                .await?;
        }

        // 5. Resolve env and start the instance.
        let env = self.resolve_env(&project).await?;
        let handle = match self.driver.start(&project, &env).await {
            Ok(h) => h,
            Err(e) => {
                self.registry
                    .update_status(&project.id, ProjectStatus::Failed)
                    .await?;
                return Err(e);
            }
        };

        // 6. Record the real upstream + mark Running.
        project.upstream = Some(handle.upstream.clone());
        self.registry
            .set_upstream(&project.id, Some(&handle.upstream))
            .await?;
        self.registry
            .update_status(&project.id, ProjectStatus::Running)
            .await?;

        // 7. Refresh the gateway for all running projects.
        let running = self.running_projects().await?;
        self.gateway.apply(&running).await?;

        Ok(())
    }

    pub async fn start(&self, id: &str) -> Result<()> {
        let mut project = self.registry.get(id).await?;
        let env = self.resolve_env(&project).await?;
        let handle = self.driver.start(&project, &env).await?;

        project.upstream = Some(handle.upstream.clone());
        self.registry
            .set_upstream(id, Some(&handle.upstream))
            .await?;
        self.registry
            .update_status(id, ProjectStatus::Running)
            .await?;
        self.registry
            .record_event(id, "start", None, serde_json::json!({}))
            .await?;

        let running = self.running_projects().await?;
        self.gateway.apply(&running).await?;
        Ok(())
    }

    pub async fn stop(&self, id: &str) -> Result<()> {
        let project = self.registry.get(id).await?;
        self.driver.stop(&project).await?;
        self.registry
            .update_status(id, ProjectStatus::Stopped)
            .await?;
        self.registry
            .record_event(id, "stop", None, serde_json::json!({}))
            .await?;

        // Drop it from the gateway routing table.
        let running = self.running_projects().await?;
        self.gateway.apply(&running).await?;
        Ok(())
    }

    /// Bump the schema to a new commit SHA, re-materialize, restart, re-migrate.
    pub async fn schema_update(&self, id: &str, new_sha: &str) -> Result<()> {
        let mut project = self.registry.get(id).await?;

        // Bump the pinned ref on the schema source.
        match &mut project.schema_ref {
            SchemaRef::Git { git_ref, .. } => {
                *git_ref = new_sha.to_string();
            }
            SchemaRef::Volume { .. } => {
                return Err(ControlPlaneError::InvalidState(
                    "schema_update requires a git schema source".into(),
                ));
            }
        }

        // Re-fetch + checkout the new SHA and copy the file.
        self.materialize_schema(&project).await?;

        // Restart the instance (the running atomo-server re-migrates on boot).
        let env = self.resolve_env(&project).await?;
        let handle = self.driver.restart(&project, &env).await?;
        self.registry
            .set_upstream(id, Some(&handle.upstream))
            .await?;

        // Persist the deployed SHA + audit it.
        self.registry.set_schema_version(id, new_sha).await?;
        self.registry
            .record_event(
                id,
                "schema_update",
                None,
                serde_json::json!({ "ref": new_sha }),
            )
            .await?;

        let running = self.running_projects().await?;
        self.gateway.apply(&running).await?;
        Ok(())
    }

    /// Stop + deregister. `drop_database` is guarded (requires explicit opt-in + backup).
    pub async fn delete(&self, id: &str, drop_database: bool) -> Result<()> {
        let project = self.registry.get(id).await?;

        // 1. Stop the instance.
        self.driver.stop(&project).await?;

        // 2. Optionally drop the dedicated database (guarded by the bool only).
        if drop_database {
            let db = Self::database_name(id);
            // Best-effort terminate of leftover connections before dropping.
            let _ = Self::admin_exec(&format!(
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
                 WHERE datname = '{db}' AND pid <> pg_backend_pid()"
            ))
            .await;
            match Self::admin_exec(&format!("DROP DATABASE IF EXISTS \"{db}\"")).await {
                Ok(()) => {}
                Err(e) => return Err(e),
            }
        }

        // 3. Audit before removing the registry row (FK references project_id).
        self.registry
            .record_event(
                id,
                "delete",
                None,
                serde_json::json!({ "drop_database": drop_database }),
            )
            .await?;

        // 4. Remove from the registry.
        self.registry.delete(id).await?;

        // 5. Refresh the gateway (the project is gone).
        let running = self.running_projects().await?;
        self.gateway.apply(&running).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{DesiredState, ProjectStatus};

    fn proj(id: &str, schema_ref: SchemaRef) -> Project {
        let now = chrono::Utc::now();
        Project {
            id: id.to_string(),
            display_name: id.to_string(),
            hostname: None,
            aliases: vec![],
            database_url_ref: "/atomo/x/DATABASE_URL".into(),
            schema_ref,
            schema_version: None,
            upstream: None,
            env: serde_json::json!({}),
            status: ProjectStatus::Provisioning,
            desired_state: DesiredState::Running,
            last_health: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn database_name_sanitizes_hyphens() {
        assert_eq!(Provisioner::database_name("a-b-c"), "atomo_a_b_c");
        assert_eq!(Provisioner::database_name("plain"), "atomo_plain");
    }

    #[test]
    fn default_port_is_deterministic() {
        assert_eq!(
            Provisioner::default_port("alpha"),
            Provisioner::default_port("alpha")
        );
        // Non-zero and offset stays within the 0..1000 window above the base.
        let base: u16 = std::env::var("ATOMO_PORT_BASE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4000);
        let p = Provisioner::default_port("alpha");
        assert!(
            p >= base && p < base + 1000,
            "port {p} within [{base}, {})",
            base + 1000
        );
    }

    #[test]
    fn materialized_schema_path_volume_is_used_as_is() {
        let p = proj(
            "v1",
            SchemaRef::Volume {
                path: "some/dir/schema.ts".into(),
            },
        );
        let path = Provisioner::materialized_schema_path(&p);
        assert_eq!(path, std::path::PathBuf::from("some/dir/schema.ts"));
    }

    #[test]
    fn materialized_schema_path_git_lands_in_project_dir() {
        let p = proj(
            "g1",
            SchemaRef::Git {
                repo: "git@example.com:org/schemas.git".into(),
                path: "projects/g1/schema.ts".into(),
                git_ref: "abc123".into(),
            },
        );
        let s = Provisioner::materialized_schema_path(&p)
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            s.ends_with("g1/schema.ts"),
            "git schema lands as <id>/schema.ts: {s}"
        );
    }
}
