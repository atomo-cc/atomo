//! Reconcile loop — converge running instances to each project's `desired_state`,
//! restart crashed instances, record health, regenerate gateway config on change. (Phase 2.)
//!
//! The loop is the declarative half of the control plane: the registry holds *desired*
//! state, the driver reports *observed* state, and [`Reconciler::tick`] drives the two
//! together. It is deliberately defensive — a failure converging or probing one project is
//! logged and never aborts the pass for the rest of the fleet.

use crate::driver::InstanceState;
use crate::error::Result;
use crate::provisioner::Provisioner;
use crate::registry::{DesiredState, Project};
use std::sync::Arc;
use std::time::Duration;

/// What a reconcile pass should do for a project, given desired vs observed state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileAction {
    /// Already converged — do nothing.
    Noop,
    /// Bring the instance up (also used to restart a Failed instance).
    Start,
    /// Bring the instance down.
    Stop,
}

/// Pure convergence decision: `desired` vs `observed` → action. Extracted so the full
/// matrix is unit-testable without a driver, registry, or network.
pub fn decide(desired: DesiredState, observed: InstanceState) -> ReconcileAction {
    use DesiredState::{Running as WantUp, Stopped as WantDown};
    use InstanceState::{Failed, Running, Stopped, Unknown};
    match (desired, observed) {
        (WantUp, Running) => ReconcileAction::Noop,
        // Stopped / Failed / Unknown while we want it up → start (start also restarts).
        (WantUp, Stopped | Failed | Unknown) => ReconcileAction::Start,
        // Running / Failed while we want it down → stop.
        (WantDown, Running | Failed) => ReconcileAction::Stop,
        (WantDown, Stopped | Unknown) => ReconcileAction::Noop,
    }
}

pub struct Reconciler {
    pub provisioner: Arc<Provisioner>,
}

impl Reconciler {
    pub fn new(provisioner: Arc<Provisioner>) -> Self {
        Self { provisioner }
    }

    /// One reconcile pass over the fleet: for each project, converge observed → desired,
    /// then probe and record health. Per-project errors are logged, not propagated, so a
    /// single bad project never stalls the loop.
    pub async fn tick(&self) -> Result<()> {
        let projects = self.provisioner.registry.list().await?;
        for project in &projects {
            if let Err(e) = self.reconcile_one(project).await {
                tracing::warn!(project = %project.id, error = %e, "reconcile failed for project");
            }
            self.probe_health(project).await;
        }
        Ok(())
    }

    /// Converge a single project's running instance to its `desired_state`.
    async fn reconcile_one(&self, project: &Project) -> Result<()> {
        let observed = self.provisioner.driver.state(project).await?;
        match decide(project.desired_state, observed) {
            ReconcileAction::Start => {
                tracing::info!(project = %project.id, ?observed, "starting instance to match desired=running");
                self.provisioner.start(&project.id).await?;
            }
            ReconcileAction::Stop => {
                tracing::info!(project = %project.id, ?observed, "stopping instance to match desired=stopped");
                self.provisioner.stop(&project.id).await?;
            }
            ReconcileAction::Noop => {}
        }
        Ok(())
    }

    /// Probe the instance and persist the result to `last_health`. Best-effort: a probe or
    /// a registry write failure is logged but never surfaced.
    async fn probe_health(&self, project: &Project) {
        let health = self.health_of(project).await;
        if let Err(e) = self
            .provisioner
            .registry
            .set_last_health(&project.id, health)
            .await
        {
            tracing::warn!(project = %project.id, error = %e, "failed to persist health");
        }
    }

    /// Build a health JSON document for a project: a TCP connect to the upstream when one is
    /// known, otherwise `unknown`. Includes a timestamp so consumers can detect staleness.
    async fn health_of(&self, project: &Project) -> serde_json::Value {
        let now = chrono::Utc::now().to_rfc3339();
        match &project.upstream {
            Some(upstream) => {
                let reachable = tcp_reachable(upstream).await;
                serde_json::json!({
                    "status": if reachable { "healthy" } else { "unreachable" },
                    "upstream": upstream,
                    "checked_at": now,
                })
            }
            None => serde_json::json!({
                "status": "unknown",
                "reason": "no upstream registered",
                "checked_at": now,
            }),
        }
    }

    /// Loop [`tick`](Self::tick) forever on a fixed interval. A failed pass is logged and the
    /// loop continues — the control plane should never exit because of a transient registry or
    /// driver error.
    pub async fn run(self, interval_secs: u64) {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs.max(1)));
        // Skip missed ticks rather than bursting to catch up after a slow pass.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(e) = self.tick().await {
                tracing::error!(error = %e, "reconcile tick failed");
            }
        }
    }
}

/// Best-effort TCP liveness probe with a short timeout. Accepts `host:port` upstreams; a
/// unix-socket upstream (no `:port`) is treated as not-TCP-probeable and reports unreachable.
async fn tcp_reachable(upstream: &str) -> bool {
    let target = upstream
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let timeout = Duration::from_secs(2);
    matches!(
        tokio::time::timeout(timeout, tokio::net::TcpStream::connect(target)).await,
        Ok(Ok(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::InstanceState as I;
    use crate::registry::DesiredState as D;

    #[test]
    fn convergence_matrix_is_exhaustive_and_correct() {
        // desired Running: only Running is converged; everything else → Start.
        assert_eq!(decide(D::Running, I::Running), ReconcileAction::Noop);
        assert_eq!(decide(D::Running, I::Stopped), ReconcileAction::Start);
        assert_eq!(decide(D::Running, I::Failed), ReconcileAction::Start); // restart
        assert_eq!(decide(D::Running, I::Unknown), ReconcileAction::Start);

        // desired Stopped: Running/Failed → Stop; Stopped/Unknown → converged.
        assert_eq!(decide(D::Stopped, I::Running), ReconcileAction::Stop);
        assert_eq!(decide(D::Stopped, I::Failed), ReconcileAction::Stop);
        assert_eq!(decide(D::Stopped, I::Stopped), ReconcileAction::Noop);
        assert_eq!(decide(D::Stopped, I::Unknown), ReconcileAction::Noop);
    }

    #[test]
    fn tcp_unreachable_for_garbage_upstream() {
        // A clearly-dead port resolves quickly to unreachable (sanity for the probe).
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert!(!rt.block_on(tcp_reachable("127.0.0.1:1"))); // port 1: nothing listens
    }
}
