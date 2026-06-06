//! Docker / Compose driver — runs each project as a container on one host. (Phase 1.)
//!
//! Shells out to the `docker` CLI via `tokio::process::Command`. One container per
//! project, named `atomo-<id>`, started from the `atomo-server` image with the
//! provisioner-resolved env injected and the upstream port published.

use crate::driver::{Driver, InstanceHandle, InstanceState};
use crate::error::{ControlPlaneError, Result};
use crate::registry::Project;
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::process::Command;

/// Runs each project as a container from the `atomo-server` image.
pub struct DockerDriver {
    pub image: String,
}

impl DockerDriver {
    pub fn new(image: impl Into<String>) -> Self {
        Self {
            image: image.into(),
        }
    }

    /// Deterministic container name for a project.
    fn container_name(project: &Project) -> String {
        format!("atomo-{}", project.id)
    }

    /// Derive the published port from `project.upstream` (`host:port` or `:port`),
    /// falling back to the `PORT` env the provisioner set, then to 3000.
    fn resolve_port(project: &Project, env: &HashMap<String, String>) -> u16 {
        if let Some(up) = &project.upstream {
            if let Some(p) = up.rsplit(':').next().and_then(|s| s.parse::<u16>().ok()) {
                return p;
            }
        }
        env.get("PORT")
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(3000)
    }

    /// Run `docker <args...>`; return stdout on success, a `Driver` error otherwise.
    async fn docker(args: &[&str]) -> Result<String> {
        let out = Command::new("docker")
            .args(args)
            .output()
            .await
            .map_err(|e| ControlPlaneError::Driver(format!("spawn docker failed: {e}")))?;
        if !out.status.success() {
            return Err(ControlPlaneError::Driver(format!(
                "docker {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Inspect a container's `.State.Status`; `None` if the container does not exist.
    async fn inspect_status(name: &str) -> Result<Option<String>> {
        let out = Command::new("docker")
            .args(["inspect", "-f", "{{.State.Status}}", name])
            .output()
            .await
            .map_err(|e| ControlPlaneError::Driver(format!("spawn docker inspect failed: {e}")))?;
        if !out.status.success() {
            // Non-zero typically means "no such object" → not running / absent.
            return Ok(None);
        }
        Ok(Some(
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
        ))
    }
}

#[async_trait]
impl Driver for DockerDriver {
    fn name(&self) -> &str {
        "docker"
    }

    async fn start(
        &self,
        project: &Project,
        env: &HashMap<String, String>,
    ) -> Result<InstanceHandle> {
        let name = Self::container_name(project);
        let port = Self::resolve_port(project, env);
        let upstream = project
            .upstream
            .clone()
            .unwrap_or_else(|| format!("127.0.0.1:{port}"));

        // Idempotent: if a container with this name already exists and is running, reuse it.
        if let Some(status) = Self::inspect_status(&name).await? {
            if status == "running" {
                let id = Self::docker(&["inspect", "-f", "{{.Id}}", &name]).await?;
                return Ok(InstanceHandle {
                    project_id: project.id.clone(),
                    upstream,
                    driver_ref: id,
                });
            }
            // Exists but not running — remove so we can recreate cleanly with current env.
            let _ = Self::docker(&["rm", "-f", &name]).await;
        }

        // Build `docker run -d --name atomo-<id> -e K=V ... -p port:port image`.
        let port_map = format!("{port}:{port}");
        let mut args: Vec<String> = vec![
            "run".into(),
            "-d".into(),
            "--restart".into(),
            "unless-stopped".into(),
            "--name".into(),
            name.clone(),
            "-p".into(),
            port_map,
        ];
        for (k, v) in env {
            args.push("-e".into());
            args.push(format!("{k}={v}"));
        }
        args.push(self.image.clone());

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let id = Self::docker(&arg_refs).await?;

        Ok(InstanceHandle {
            project_id: project.id.clone(),
            upstream,
            driver_ref: id,
        })
    }

    async fn stop(&self, project: &Project) -> Result<()> {
        let name = Self::container_name(project);
        // Idempotent: ignore "no such container".
        if Self::inspect_status(&name).await?.is_some() {
            Self::docker(&["rm", "-f", &name]).await?;
        }
        Ok(())
    }

    async fn restart(
        &self,
        project: &Project,
        env: &HashMap<String, String>,
    ) -> Result<InstanceHandle> {
        self.stop(project).await?;
        self.start(project, env).await
    }

    async fn state(&self, project: &Project) -> Result<InstanceState> {
        let name = Self::container_name(project);
        Ok(match Self::inspect_status(&name).await? {
            None => InstanceState::Stopped,
            Some(s) => match s.as_str() {
                "running" => InstanceState::Running,
                "exited" | "created" | "paused" | "dead" => InstanceState::Stopped,
                "restarting" => InstanceState::Failed,
                _ => InstanceState::Unknown,
            },
        })
    }
}
