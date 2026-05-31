//! In-memory read cache with event-driven invalidation

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone)]
struct CacheEntry {
    data: Value,
    expires_at: Instant,
}

/// Read-through cache with TTL and event-based invalidation
#[derive(Clone)]
pub struct ReadCache {
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
    ttl: Duration,
}

impl ReadCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    /// Get a cached value by key
    pub async fn get(&self, key: &str) -> Option<Value> {
        let entries = self.entries.read().await;
        entries.get(key).and_then(|e| {
            if e.expires_at > Instant::now() {
                Some(e.data.clone())
            } else {
                None
            }
        })
    }

    /// Set a cached value
    pub async fn set(&self, key: &str, value: Value) {
        let mut entries = self.entries.write().await;
        entries.insert(
            key.to_string(),
            CacheEntry {
                data: value,
                expires_at: Instant::now() + self.ttl,
            },
        );
    }

    /// Invalidate all entries for a model
    pub async fn invalidate_model(&self, model_name: &str) {
        let mut entries = self.entries.write().await;
        entries.retain(|k, _| !k.starts_with(&format!("{}:", model_name)));
    }

    /// Invalidate a specific entry
    pub async fn invalidate(&self, key: &str) {
        let mut entries = self.entries.write().await;
        entries.remove(key);
    }

    /// Clear all expired entries
    pub async fn evict_expired(&self) {
        let mut entries = self.entries.write().await;
        let now = Instant::now();
        entries.retain(|_, e| e.expires_at > now);
    }

    /// Generate a cache key for a model query
    pub fn key(model: &str, query_hash: &str) -> String {
        format!("{}:{}", model, query_hash)
    }
}
