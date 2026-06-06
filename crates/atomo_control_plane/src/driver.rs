//! Driver abstraction — *how* a per-project `atomo-server` instance is run. (Phase 0 foundation.)
//!
//! v1 implementation: [`crate::docker`]. Phase 5 adds nomad/k8s behind this same trait
//! (see [`crate::extensions`]). The control plane talks only to this interface.

use crate::error::Result;
use crate::registry::Project;
use async_trait::async_trait;
use std::collections::HashMap;

/// A handle to a started instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceHandle {
    pub project_id: String,
    /// `host:port` or unix socket the instance listens on (the gateway's upstream).
    pub upstream: String,
    /// Driver-specific identifier (container id / pid / job id).
    pub driver_ref: String,
}

/// Observed runtime state of an instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceState {
    Running,
    Stopped,
    Failed,
    Unknown,
}

/// Runs and supervises one `atomo-server` instance per project.
///
/// Implementations receive a fully-resolved `env` map (secrets already injected by the
/// provisioner via the [`crate::SecretStore`]) and must not reach into the secret store
/// themselves.
#[async_trait]
pub trait Driver: Send + Sync {
    /// Stable driver name (e.g. `"docker"`).
    fn name(&self) -> &str;

    /// Start (or no-op if already running) an instance for `project`.
    async fn start(&self, project: &Project, env: &HashMap<String, String>)
        -> Result<InstanceHandle>;

    /// Stop the instance. Idempotent.
    async fn stop(&self, project: &Project) -> Result<()>;

    /// Restart the instance (e.g. after a schema bump). Idempotent w.r.t. final state.
    async fn restart(
        &self,
        project: &Project,
        env: &HashMap<String, String>,
    ) -> Result<InstanceHandle>;

    /// Probe the instance's current state.
    async fn state(&self, project: &Project) -> Result<InstanceState>;
}
