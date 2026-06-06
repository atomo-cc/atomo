//! `atomo project ...` — Phase 1 provisioner CLI.
//!
//! Drives the multi-project control plane: create / start / stop / list / delete an
//! isolated `atomo-server` instance (silo: its own database + container + route).
//!
//! Built from environment:
//! - `ATOMO_CP_DATABASE_URL`       — the control-plane registry database (required).
//! - `ATOMO_ADMIN_DATABASE_URL`    — Postgres maintenance DB for CREATE/DROP DATABASE.
//! - `ATOMO_SERVER_IMAGE`          — the `atomo-server` container image (Docker driver).
//! - `ATOMO_SECRET_STORE`          — `ssm` (default) | `env` (dev/test fallback).
//! - `ATOMO_CADDY_CONFIG`          — path the generated Caddy config is written to.
//! - `ATOMO_CADDY_ADMIN`           — Caddy admin endpoint for hot-reload (optional).

use std::sync::Arc;

use atomo_control_plane::caddy::CaddyGateway;
use atomo_control_plane::docker::DockerDriver;
use atomo_control_plane::registry::{
    DesiredState, Project, ProjectRegistry, ProjectStatus, SchemaRef,
};
use atomo_control_plane::secrets::{EnvSecretStore, SecretStore, SsmSecretStore};
use atomo_control_plane::Provisioner;
use clap::Subcommand;
use colored::*;
use sqlx::postgres::PgPoolOptions;

#[derive(Subcommand)]
pub enum ProjectCommands {
    /// Provision a new isolated project (create DB, place schema, start instance, route).
    Create {
        /// Stable project slug / id (e.g. "acme").
        #[arg(long)]
        id: String,
        /// Human-readable display name.
        #[arg(long)]
        name: String,
        /// Primary routing hostname (e.g. acme.example.com).
        #[arg(long)]
        hostname: Option<String>,
        /// Additional routing aliases (repeatable).
        #[arg(long = "alias")]
        aliases: Vec<String>,
        /// Secret-store reference to the project's DATABASE_URL.
        #[arg(long)]
        database_url_ref: String,
        /// Git repo for the schema source (with --schema-path and --schema-ref).
        #[arg(long, requires = "schema_path", conflicts_with = "schema_volume")]
        schema_git: Option<String>,
        /// Path to schema.ts within the git repo.
        #[arg(long)]
        schema_path: Option<String>,
        /// Pinned commit SHA for the git schema source (NOT a branch).
        #[arg(long)]
        schema_ref: Option<String>,
        /// Use a plain volume file as the schema source (dev mode), instead of git.
        #[arg(long, conflicts_with = "schema_git")]
        schema_volume: Option<String>,
    },
    /// Start (or no-op if already running) a project's instance.
    Start {
        #[arg(long)]
        id: String,
    },
    /// Stop a project's instance (keeps its database + registration).
    Stop {
        #[arg(long)]
        id: String,
    },
    /// List all registered projects and their status.
    List,
    /// Delete a project. Dropping its database is guarded behind --drop-database.
    Delete {
        #[arg(long)]
        id: String,
        /// Also DROP the project database. Destructive — take a backup first.
        #[arg(long, default_value_t = false)]
        drop_database: bool,
        /// Required acknowledgement when --drop-database is set.
        #[arg(long, default_value_t = false)]
        yes: bool,
    },
}

/// Build a [`Provisioner`] wired from environment.
async fn build_provisioner() -> anyhow::Result<Provisioner> {
    let cp_url = std::env::var("ATOMO_CP_DATABASE_URL").map_err(|_| {
        anyhow::anyhow!("ATOMO_CP_DATABASE_URL is required (the control-plane registry database)")
    })?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cp_url)
        .await?;
    let registry = ProjectRegistry::new(pool);
    registry.init().await?;

    let image = std::env::var("ATOMO_SERVER_IMAGE")
        .unwrap_or_else(|_| "ghcr.io/atomo-cc/atomo-server:latest".to_string());
    let driver = Arc::new(DockerDriver::new(image));

    let secrets: Arc<dyn SecretStore> = match std::env::var("ATOMO_SECRET_STORE")
        .unwrap_or_default()
        .as_str()
    {
        "env" => Arc::new(EnvSecretStore),
        _ => Arc::new(SsmSecretStore),
    };

    let caddy_config =
        std::env::var("ATOMO_CADDY_CONFIG").unwrap_or_else(|_| "./caddy.json".to_string());
    let gateway = CaddyGateway::new(caddy_config)
        .with_admin_endpoint(std::env::var("ATOMO_CADDY_ADMIN").ok());

    Ok(Provisioner::new(registry, driver, secrets, gateway))
}

pub async fn project_command(cmd: ProjectCommands) -> anyhow::Result<()> {
    let provisioner = build_provisioner().await?;

    match cmd {
        ProjectCommands::Create {
            id,
            name,
            hostname,
            aliases,
            database_url_ref,
            schema_git,
            schema_path,
            schema_ref,
            schema_volume,
        } => {
            let schema = if let Some(repo) = schema_git {
                let path = schema_path.ok_or_else(|| {
                    anyhow::anyhow!("--schema-path is required with --schema-git")
                })?;
                let git_ref = schema_ref.ok_or_else(|| {
                    anyhow::anyhow!(
                        "--schema-ref (pinned commit SHA) is required with --schema-git"
                    )
                })?;
                SchemaRef::Git {
                    repo,
                    path,
                    git_ref,
                }
            } else if let Some(path) = schema_volume {
                SchemaRef::Volume { path }
            } else {
                return Err(anyhow::anyhow!(
                    "provide a schema source: --schema-git/--schema-path/--schema-ref or --schema-volume"
                ));
            };

            let project = Project {
                id: id.clone(),
                display_name: name,
                hostname,
                aliases,
                database_url_ref,
                schema_ref: schema,
                schema_version: None,
                upstream: None,
                env: serde_json::json!({}),
                status: ProjectStatus::Provisioning,
                desired_state: DesiredState::Running,
                last_health: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            println!("  Provisioning project {}...", id.bright_white().bold());
            provisioner.create(project).await?;
            println!("  {} project {} is running", "✓".green(), id.bright_white());
        }

        ProjectCommands::Start { id } => {
            provisioner.start(&id).await?;
            println!("  {} started {}", "✓".green(), id.bright_white());
        }

        ProjectCommands::Stop { id } => {
            provisioner.stop(&id).await?;
            println!("  {} stopped {}", "✓".green(), id.bright_white());
        }

        ProjectCommands::List => {
            let projects = provisioner.registry.list().await?;
            if projects.is_empty() {
                println!("  {}", "no projects registered".dimmed());
            } else {
                println!(
                    "  {:<20} {:<14} {:<28} {}",
                    "ID".bold(),
                    "STATUS".bold(),
                    "HOSTNAME".bold(),
                    "UPSTREAM".bold()
                );
                for p in projects {
                    println!(
                        "  {:<20} {:<14} {:<28} {}",
                        p.id,
                        p.status.as_str(),
                        p.hostname.as_deref().unwrap_or("-"),
                        p.upstream.as_deref().unwrap_or("-"),
                    );
                }
            }
        }

        ProjectCommands::Delete {
            id,
            drop_database,
            yes,
        } => {
            if drop_database && !yes {
                return Err(anyhow::anyhow!(
                    "--drop-database is destructive; re-run with --yes to confirm (take a backup first)"
                ));
            }
            provisioner.delete(&id, drop_database).await?;
            let note = if drop_database {
                " (database dropped)"
            } else {
                ""
            };
            println!(
                "  {} deleted {}{}",
                "✓".green(),
                id.bright_white(),
                note.dimmed()
            );
        }
    }

    Ok(())
}
