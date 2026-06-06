//! Control-plane HTTP API over the registry + provisioner. (Phase 2.)
//!
//! CRUD projects and trigger lifecycle actions. Operator-authenticated; this credential
//! never grants access to project data.

use crate::provisioner::Provisioner;
use std::sync::Arc;

/// Build the control-plane axum router. (Phase 2: add routes/handlers.)
pub fn router(_provisioner: Arc<Provisioner>) -> axum::Router {
    axum::Router::new()
}
