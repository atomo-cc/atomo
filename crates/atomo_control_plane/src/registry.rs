//! Project registry — the single source of truth for the fleet. (Phase 0 foundation.)
//!
//! Lives in the control plane's **own** database, never a project database.

use crate::error::{ControlPlaneError, Result};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

/// Lifecycle status of a project's instance (see the state machine in the design doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Provisioning,
    Running,
    Stopped,
    Failed,
}

impl ProjectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectStatus::Provisioning => "provisioning",
            ProjectStatus::Running => "running",
            ProjectStatus::Stopped => "stopped",
            ProjectStatus::Failed => "failed",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "provisioning" => ProjectStatus::Provisioning,
            "running" => ProjectStatus::Running,
            "stopped" => ProjectStatus::Stopped,
            "failed" => ProjectStatus::Failed,
            other => return Err(ControlPlaneError::InvalidState(format!("status={other}"))),
        })
    }
}

/// The reconciler's target state for a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesiredState {
    Running,
    Stopped,
}

impl DesiredState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DesiredState::Running => "running",
            DesiredState::Stopped => "stopped",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "stopped" => DesiredState::Stopped,
            _ => DesiredState::Running,
        }
    }
}

/// Where a project's `schema.ts` comes from.
///
/// `Git` is the production source of truth (pinned to an immutable commit SHA);
/// `Volume` is the simpler dev / single-project mode (a plain file on disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchemaRef {
    Git {
        repo: String,
        path: String,
        /// Pinned commit SHA — NOT a branch name.
        #[serde(rename = "ref")]
        git_ref: String,
    },
    Volume {
        path: String,
    },
}

/// A registered project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub display_name: String,
    pub hostname: Option<String>,
    pub aliases: Vec<String>,
    /// SSM reference (e.g. `/atomo/<id>/DATABASE_URL`), resolved by the [`crate::SecretStore`].
    pub database_url_ref: String,
    pub schema_ref: SchemaRef,
    /// Deployed commit SHA (for git schema sources); drives drift detection.
    pub schema_version: Option<String>,
    /// `host:port` or unix socket the running instance listens on.
    pub upstream: Option<String>,
    /// Non-secret per-project env overrides (feature flags, etc.).
    pub env: serde_json::Value,
    pub status: ProjectStatus,
    pub desired_state: DesiredState,
    pub last_health: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// CRUD + lookups over the `projects` registry table.
#[derive(Clone)]
pub struct ProjectRegistry {
    pool: PgPool,
}

impl ProjectRegistry {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create the registry tables. Idempotent.
    pub async fn init(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS projects (
                id             TEXT PRIMARY KEY,
                display_name   TEXT NOT NULL,
                hostname       TEXT UNIQUE,
                aliases        TEXT[] NOT NULL DEFAULT '{}',
                database_url_ref TEXT NOT NULL,
                schema_ref     JSONB NOT NULL,
                schema_version TEXT,
                upstream       TEXT,
                env            JSONB NOT NULL DEFAULT '{}'::jsonb,
                status         TEXT NOT NULL,
                desired_state  TEXT NOT NULL DEFAULT 'running',
                last_health    JSONB,
                created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS project_events (
                id          BIGSERIAL PRIMARY KEY,
                project_id  TEXT NOT NULL,
                action      TEXT NOT NULL,
                actor       TEXT,
                detail      JSONB,
                at          TIMESTAMPTZ NOT NULL DEFAULT now()
            );
            CREATE INDEX IF NOT EXISTS idx_project_events_project ON project_events(project_id);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    fn row_to_project(row: &sqlx::postgres::PgRow) -> Result<Project> {
        let schema_ref_json: serde_json::Value = row.try_get("schema_ref")?;
        let status: String = row.try_get("status")?;
        let desired: String = row.try_get("desired_state")?;
        Ok(Project {
            id: row.try_get("id")?,
            display_name: row.try_get("display_name")?,
            hostname: row.try_get("hostname")?,
            aliases: row.try_get("aliases")?,
            database_url_ref: row.try_get("database_url_ref")?,
            schema_ref: serde_json::from_value(schema_ref_json)?,
            schema_version: row.try_get("schema_version")?,
            upstream: row.try_get("upstream")?,
            env: row.try_get("env")?,
            status: ProjectStatus::parse(&status)?,
            desired_state: DesiredState::parse(&desired),
            last_health: row.try_get("last_health")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    pub async fn create(&self, p: &Project) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO projects
                (id, display_name, hostname, aliases, database_url_ref, schema_ref,
                 schema_version, upstream, env, status, desired_state, last_health)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
            "#,
        )
        .bind(&p.id)
        .bind(&p.display_name)
        .bind(&p.hostname)
        .bind(&p.aliases)
        .bind(&p.database_url_ref)
        .bind(serde_json::to_value(&p.schema_ref)?)
        .bind(&p.schema_version)
        .bind(&p.upstream)
        .bind(&p.env)
        .bind(p.status.as_str())
        .bind(p.desired_state.as_str())
        .bind(&p.last_health)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Project> {
        let row = sqlx::query("SELECT * FROM projects WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound(id.to_string()))?;
        Self::row_to_project(&row)
    }

    pub async fn list(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query("SELECT * FROM projects ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(Self::row_to_project).collect()
    }

    pub async fn resolve_by_hostname(&self, hostname: &str) -> Result<Option<Project>> {
        let row = sqlx::query(
            "SELECT * FROM projects WHERE hostname = $1 OR $1 = ANY(aliases) LIMIT 1",
        )
        .bind(hostname)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(Self::row_to_project).transpose()
    }

    pub async fn update_status(&self, id: &str, status: ProjectStatus) -> Result<()> {
        sqlx::query("UPDATE projects SET status = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(status.as_str())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_desired_state(&self, id: &str, desired: DesiredState) -> Result<()> {
        sqlx::query("UPDATE projects SET desired_state = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(desired.as_str())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_upstream(&self, id: &str, upstream: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE projects SET upstream = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(upstream)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_schema_version(&self, id: &str, version: &str) -> Result<()> {
        sqlx::query("UPDATE projects SET schema_version = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(version)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_last_health(&self, id: &str, health: serde_json::Value) -> Result<()> {
        sqlx::query("UPDATE projects SET last_health = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(health)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM projects WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Append a control-plane audit event.
    pub async fn record_event(
        &self,
        project_id: &str,
        action: &str,
        actor: Option<&str>,
        detail: serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO project_events (project_id, action, actor, detail) VALUES ($1,$2,$3,$4)",
        )
        .bind(project_id)
        .bind(action)
        .bind(actor)
        .bind(detail)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
