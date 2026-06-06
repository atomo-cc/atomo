//! Optional extensions — multi-host drivers (nomad/k8s) + shared identity/SSO. (Phase 5.)
//!
//! Built only on demonstrated need; see the design doc's non-goals. These are stubs that
//! describe the seams without committing the implementation.

use crate::error::{ControlPlaneError, Result};

/// Shared identity (SSO) plane across projects. Deferred per the confirmed v1 decision
/// (per-project identity). Implement only when a real cross-project-login need appears.
pub async fn sso_resolve_principal(_token: &str) -> Result<()> {
    Err(ControlPlaneError::unimplemented("extensions::sso (Phase 5)"))
}

/// Marker for a multi-host driver (nomad/k8s). A real impl lives behind [`crate::Driver`].
pub const MULTI_HOST_DRIVERS: &[&str] = &["nomad", "k8s"];
