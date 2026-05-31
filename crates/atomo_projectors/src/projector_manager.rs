use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

use crate::Projection;

/// Event received from the broadcast channel
#[derive(Debug, Clone)]
pub struct ProjectorEvent {
    pub event_type: String,
    pub model_name: String,
    pub data: HashMap<String, Value>,
}

pub struct ProjectorManager {
    projections: Vec<Arc<dyn Projection>>,
    pool: PgPool,
}

impl ProjectorManager {
    pub fn new(pool: PgPool) -> Self {
        Self {
            projections: Vec::new(),
            pool,
        }
    }

    pub fn register(&mut self, projection: impl Projection + 'static) {
        self.projections.push(Arc::new(projection));
    }

    /// Process a single event through all matching projections
    pub async fn process_event(
        &self,
        event_type: &str,
        model_name: &str,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        for proj in &self.projections {
            if proj.source_model() == model_name {
                proj.handle_event(event_type, data, &self.pool).await?;
            }
        }
        Ok(())
    }

    /// Rebuild all projections from scratch. Per-projection failures are logged, not fatal.
    pub async fn rebuild_all(&self) -> Result<()> {
        for proj in &self.projections {
            info!(projection = proj.name(), "Rebuilding projection");
            if let Err(e) = proj.rebuild(&self.pool).await {
                tracing::warn!(projection = proj.name(), error = %e, "Projection rebuild failed; skipping");
            }
        }
        Ok(())
    }

    pub fn projections(&self) -> &[Arc<dyn Projection>] {
        &self.projections
    }

    /// Start listening to model events and process them through projections
    pub fn start_event_listener(self: Arc<Self>, mut rx: broadcast::Receiver<ProjectorEvent>) {
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if let Err(e) = self
                            .process_event(&event.event_type, &event.model_name, &event.data)
                            .await
                        {
                            tracing::error!(error = %e, "Projection processing failed");
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Projector listener lagged by {} events", n);
                    }
                    Err(_) => break,
                }
            }
        });
    }
}
