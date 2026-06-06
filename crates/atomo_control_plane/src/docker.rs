//! Docker / Compose driver — runs each project as a container on one host. (Phase 1.)

use crate::driver::{Driver, InstanceHandle, InstanceState};
use crate::error::{ControlPlaneError, Result};
use crate::registry::Project;
use async_trait::async_trait;
use std::collections::HashMap;

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
}

#[async_trait]
impl Driver for DockerDriver {
    fn name(&self) -> &str {
        "docker"
    }

    async fn start(
        &self,
        _project: &Project,
        _env: &HashMap<String, String>,
    ) -> Result<InstanceHandle> {
        Err(ControlPlaneError::unimplemented("DockerDriver::start (Phase 1)"))
    }

    async fn stop(&self, _project: &Project) -> Result<()> {
        Err(ControlPlaneError::unimplemented("DockerDriver::stop (Phase 1)"))
    }

    async fn restart(
        &self,
        _project: &Project,
        _env: &HashMap<String, String>,
    ) -> Result<InstanceHandle> {
        Err(ControlPlaneError::unimplemented(
            "DockerDriver::restart (Phase 1)",
        ))
    }

    async fn state(&self, _project: &Project) -> Result<InstanceState> {
        Ok(InstanceState::Unknown)
    }
}
