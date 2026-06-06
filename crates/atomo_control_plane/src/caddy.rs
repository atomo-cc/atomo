//! Caddy gateway — generate routing config (hostname/aliases -> upstream) from the
//! registry, write it, and reload Caddy. (Phase 1.)
//!
//! `render` produces a Caddy **native JSON** config (the format the admin API's
//! `/load` endpoint consumes). Each project becomes one route in the default
//! `srv0` HTTP server: matched primarily by `host` (hostname + aliases), with an
//! `X-Atomo-Project: <id>` header as a fallback matcher for non-DNS clients. The
//! matched route reverse-proxies to the project's `upstream`.
//!
//! `apply` writes the rendered config to `config_path` and reloads Caddy:
//! - if `admin_endpoint` is set, POST the JSON to `<admin>/load` (via `curl`);
//! - otherwise run `caddy reload --config <path>`;
//! - if neither the admin endpoint nor the `caddy` binary is reachable the file is
//!   still written (so an out-of-band reload / boot picks it up).

use crate::error::{ControlPlaneError, Result};
use crate::registry::Project;
use serde_json::{json, Value};
use tokio::process::Command;

#[derive(Clone, Default)]
pub struct CaddyGateway {
    /// Path the generated Caddy config is written to.
    pub config_path: String,
    /// Admin API endpoint used to hot-reload Caddy (e.g. `http://localhost:2019`).
    pub admin_endpoint: Option<String>,
}

impl CaddyGateway {
    pub fn new(config_path: impl Into<String>) -> Self {
        Self {
            config_path: config_path.into(),
            admin_endpoint: None,
        }
    }

    /// Set the admin endpoint for hot-reload (builder-style; additive).
    pub fn with_admin_endpoint(mut self, endpoint: Option<String>) -> Self {
        self.admin_endpoint = endpoint;
        self
    }

    /// Build the list of Caddy routes for the running projects.
    fn routes(projects: &[Project]) -> Vec<Value> {
        let mut routes = Vec::new();
        for p in projects {
            let upstream = match &p.upstream {
                Some(u) if !u.is_empty() => u.clone(),
                _ => continue, // not running / no upstream yet — skip
            };

            // Primary matcher: hostname + aliases.
            let mut hosts: Vec<String> = Vec::new();
            if let Some(h) = &p.hostname {
                if !h.is_empty() {
                    hosts.push(h.clone());
                }
            }
            for a in &p.aliases {
                if !a.is_empty() {
                    hosts.push(a.clone());
                }
            }

            let handle = json!([{
                "handler": "reverse_proxy",
                "upstreams": [{ "dial": upstream }]
            }]);

            // Host route (if any hostnames are configured).
            if !hosts.is_empty() {
                routes.push(json!({
                    "match": [{ "host": hosts }],
                    "handle": handle,
                    "terminal": true
                }));
            }

            // Fallback matcher: X-Atomo-Project header carrying the project id.
            routes.push(json!({
                "match": [{ "header": { "X-Atomo-Project": [p.id] } }],
                "handle": handle,
                "terminal": true
            }));
        }
        routes
    }

    /// Render routing config (Caddy native JSON) for all running `projects`.
    pub fn render(&self, projects: &[Project]) -> Result<String> {
        let routes = Self::routes(projects);
        let config = json!({
            "apps": {
                "http": {
                    "servers": {
                        "srv0": {
                            "listen": [":443", ":80"],
                            "routes": routes
                        }
                    }
                }
            }
        });
        Ok(serde_json::to_string_pretty(&config)?)
    }

    /// Render + write the config and trigger a Caddy reload.
    pub async fn apply(&self, projects: &[Project]) -> Result<()> {
        let rendered = self.render(projects)?;

        // Always persist the config so a manual/boot reload can recover it.
        if !self.config_path.is_empty() {
            if let Some(parent) = std::path::Path::new(&self.config_path).parent() {
                if !parent.as_os_str().is_empty() {
                    tokio::fs::create_dir_all(parent).await?;
                }
            }
            tokio::fs::write(&self.config_path, rendered.as_bytes()).await?;
        }

        if let Some(admin) = &self.admin_endpoint {
            let url = format!("{}/load", admin.trim_end_matches('/'));
            let out = Command::new("curl")
                .args([
                    "-sf",
                    "-X",
                    "POST",
                    "-H",
                    "Content-Type: application/json",
                    "-d",
                    &rendered,
                    &url,
                ])
                .output()
                .await
                .map_err(|e| ControlPlaneError::Gateway(format!("spawn curl failed: {e}")))?;
            if !out.status.success() {
                return Err(ControlPlaneError::Gateway(format!(
                    "POST {url} failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                )));
            }
        } else if !self.config_path.is_empty() {
            // No admin endpoint: ask the `caddy` binary to reload from the file.
            let out = Command::new("caddy")
                .args(["reload", "--config", &self.config_path])
                .output()
                .await;
            match out {
                Ok(o) if o.status.success() => {}
                Ok(o) => {
                    return Err(ControlPlaneError::Gateway(format!(
                        "caddy reload failed: {}",
                        String::from_utf8_lossy(&o.stderr).trim()
                    )));
                }
                // `caddy` not installed / not running — config is written; treat as soft success.
                Err(_) => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{DesiredState, ProjectStatus, SchemaRef};

    fn proj(id: &str, hostname: Option<&str>, upstream: Option<&str>) -> Project {
        let now = chrono::Utc::now();
        Project {
            id: id.to_string(),
            display_name: id.to_string(),
            hostname: hostname.map(String::from),
            aliases: vec![],
            database_url_ref: "/atomo/x/DATABASE_URL".into(),
            schema_ref: SchemaRef::Volume {
                path: "schema.ts".into(),
            },
            schema_version: None,
            upstream: upstream.map(String::from),
            env: serde_json::json!({}),
            status: ProjectStatus::Running,
            desired_state: DesiredState::Running,
            last_health: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn render_routes_each_running_project_and_skips_those_without_upstream() {
        let gw = CaddyGateway::new("/tmp/caddy.json");
        let projects = vec![
            proj("a", Some("a.example.com"), Some("127.0.0.1:4001")),
            proj("b", Some("b.example.com"), None), // no upstream → skipped
        ];
        let cfg: serde_json::Value =
            serde_json::from_str(&gw.render(&projects).unwrap()).unwrap();
        let routes = cfg["apps"]["http"]["servers"]["srv0"]["routes"]
            .as_array()
            .expect("routes array");

        let json = serde_json::to_string(&cfg).unwrap();
        // Project a: hostname route + upstream + X-Atomo-Project header fallback.
        assert!(json.contains("a.example.com"), "hostname route for a");
        assert!(json.contains("127.0.0.1:4001"), "upstream for a");
        assert!(json.contains("X-Atomo-Project"), "header fallback matcher");
        // Project b has no upstream → it must not appear at all.
        assert!(!json.contains("b.example.com"), "b has no upstream → skipped");
        // a contributes a host route + a header route (2); b contributes none.
        assert_eq!(routes.len(), 2, "only project a's two routes");
    }

    #[test]
    fn render_is_valid_json_for_empty_fleet() {
        let gw = CaddyGateway::new("/tmp/caddy.json");
        let cfg: serde_json::Value = serde_json::from_str(&gw.render(&[]).unwrap()).unwrap();
        assert_eq!(
            cfg["apps"]["http"]["servers"]["srv0"]["routes"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }
}
