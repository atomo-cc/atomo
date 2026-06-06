//! Reconcile loop — converge running instances to each project's `desired_state`,
//! restart crashed instances, record health, regenerate gateway config on change. (Phase 2.)

use crate::error::Result;
use crate::provisioner::Provisioner;
use std::sync::Arc;

pub struct Reconciler {
    pub provisioner: Arc<Provisioner>,
}

impl Reconciler {
    pub fn new(provisioner: Arc<Provisioner>) -> Self {
        Self { provisioner }
    }

    /// One reconcile pass over the fleet. (Phase 2: implement convergence + health.)
    pub async fn tick(&self) -> Result<()> {
        Ok(())
    }
}
