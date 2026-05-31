use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::info;

use crate::Projection;

pub struct ProjectorManager {
    projections: Vec<Arc<dyn Projection>>,
    pool: PgPool,
}

impl ProjectorManager {
    pub fn new(pool: PgPool) -> Self {
        Self { projections: Vec::new(), pool }
    }

    pub fn register(&mut self, projection: impl Projection + 'static) {
        self.projections.push(Arc::new(projection));
    }

    /// Process a single event through all matching projections
    pub async fn process_event(&self, event_type: &str, model_name: &str, data: &std::collections::HashMap<String, serde_json::Value>) -> Result<()> {
        for proj in &self.projections {
            if proj.source_model() == model_name {
                proj.handle_event(event_type, data, &self.pool).await?;
            }
        }
        Ok(())
    }

    /// Rebuild all projections from scratch
    pub async fn rebuild_all(&self) -> Result<()> {
        for proj in &self.projections {
            info!(projection = proj.name(), "Rebuilding projection");
            proj.rebuild(&self.pool).await?;
        }
        Ok(())
    }

    pub fn projections(&self) -> &[Arc<dyn Projection>] {
        &self.projections
    }
}
