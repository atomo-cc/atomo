//! Caddy gateway — generate routing config (hostname/aliases -> upstream) from the
//! registry, write it, and reload Caddy. (Phase 1.)

use crate::error::{ControlPlaneError, Result};
use crate::registry::Project;

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

    /// Render routing config for all running `projects`.
    pub fn render(&self, _projects: &[Project]) -> Result<String> {
        Err(ControlPlaneError::unimplemented("CaddyGateway::render (Phase 1)"))
    }

    /// Render + write the config and trigger a Caddy reload.
    pub async fn apply(&self, _projects: &[Project]) -> Result<()> {
        Err(ControlPlaneError::unimplemented("CaddyGateway::apply (Phase 1)"))
    }
}
