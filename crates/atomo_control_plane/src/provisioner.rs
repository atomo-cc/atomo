//! Provisioner — project lifecycle: create / start / stop / schema_update / delete. (Phase 1.)
//!
//! Idempotent, registry-backed, secrets resolved at start. See the lifecycle state machine
//! in `docs/guide/advanced/multi-project-design.md`.

use crate::caddy::CaddyGateway;
use crate::driver::Driver;
use crate::error::{ControlPlaneError, Result};
use crate::registry::{Project, ProjectRegistry};
use crate::secrets::SecretStore;
use std::collections::HashMap;
use std::sync::Arc;

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

    /// Resolve the env map (secrets injected, schema path + listen addr set) for an instance.
    pub async fn resolve_env(&self, _project: &Project) -> Result<HashMap<String, String>> {
        Err(ControlPlaneError::unimplemented(
            "Provisioner::resolve_env (Phase 1)",
        ))
    }

    /// Create DB, materialize schema, start instance, register routing. Idempotent.
    pub async fn create(&self, _project: Project) -> Result<()> {
        Err(ControlPlaneError::unimplemented("Provisioner::create (Phase 1)"))
    }

    pub async fn start(&self, _id: &str) -> Result<()> {
        Err(ControlPlaneError::unimplemented("Provisioner::start (Phase 1)"))
    }

    pub async fn stop(&self, _id: &str) -> Result<()> {
        Err(ControlPlaneError::unimplemented("Provisioner::stop (Phase 1)"))
    }

    /// Bump the schema to a new commit SHA, re-materialize, restart, re-migrate.
    pub async fn schema_update(&self, _id: &str, _new_sha: &str) -> Result<()> {
        Err(ControlPlaneError::unimplemented(
            "Provisioner::schema_update (Phase 1)",
        ))
    }

    /// Stop + deregister. `drop_database` is guarded (requires explicit opt-in + backup).
    pub async fn delete(&self, _id: &str, _drop_database: bool) -> Result<()> {
        Err(ControlPlaneError::unimplemented("Provisioner::delete (Phase 1)"))
    }
}
