use anyhow::Result;
use std::collections::HashMap;
use tokio::task::JoinHandle;

pub struct ProjectorManager {
    projectors: HashMap<String, JoinHandle<Result<()>>>,
}

impl ProjectorManager {
    pub fn new() -> Self {
        Self {
            projectors: HashMap::new(),
        }
    }
    
    pub async fn start_all(&mut self) -> Result<()> {
        // TODO: Start all projectors
        // This will be implemented when we have the event store
        Ok(())
    }
    
    pub async fn stop_all(&mut self) -> Result<()> {
        for (name, handle) in self.projectors.drain() {
            handle.abort();
            tracing::info!("Stopped projector: {}", name);
        }
        Ok(())
    }
}
