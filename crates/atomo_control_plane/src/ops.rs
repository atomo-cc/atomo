//! Operability — per-project backups, fleet observability, resource limits. (Phase 4.)

use crate::error::{ControlPlaneError, Result};
use crate::registry::Project;

/// Run a per-project database backup (silo advantage: independent `pg_dump`/PITR).
pub async fn backup_project(_project: &Project) -> Result<()> {
    Err(ControlPlaneError::unimplemented("ops::backup_project (Phase 4)"))
}

/// Restore a single project DB from a backup artifact (does not touch other projects).
pub async fn restore_project(_project: &Project, _artifact: &str) -> Result<()> {
    Err(ControlPlaneError::unimplemented("ops::restore_project (Phase 4)"))
}

/// Aggregate per-project health/metrics into a fleet observability view.
pub async fn fleet_health(_projects: &[Project]) -> Result<serde_json::Value> {
    Err(ControlPlaneError::unimplemented("ops::fleet_health (Phase 4)"))
}
