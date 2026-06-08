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

/// Cache consistency mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheMode {
    /// Writes evict the model's cached reads immediately — a read never reflects state older than
    /// the last write to that model. The default.
    Strong,
    /// Writes do **not** evict; cached reads are served until their TTL expires. Reads can be stale
    /// up to the TTL, but the cache stays hot *through* writes — a much higher hit rate under a
    /// mixed read/write load. Opt in with `ATOMO_CACHE_MODE=eventual`.
    Eventual,
}

/// Read-through cache with TTL and (in strong mode) write-driven invalidation.
#[derive(Clone)]
pub struct ReadCache {
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
    ttl: Duration,
    mode: CacheMode,
}

impl ReadCache {
    /// Strong-consistency cache with the given TTL (used by tests / internal callers).
    pub fn new(ttl_secs: u64) -> Self {
        Self::with_mode(ttl_secs, CacheMode::Strong)
    }

    pub fn with_mode(ttl_secs: u64, mode: CacheMode) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_secs),
            mode,
        }
    }

    /// Build from env: `ATOMO_CACHE_MODE` (`strong` default | `eventual`) and `ATOMO_CACHE_TTL_SECS`
    /// (defaults to `default_ttl_secs`). In `eventual` mode the TTL is the **maximum read staleness**.
    pub fn from_env(default_ttl_secs: u64) -> Self {
        let ttl = std::env::var("ATOMO_CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default_ttl_secs);
        let mode = match std::env::var("ATOMO_CACHE_MODE").as_deref() {
            Ok("eventual") => CacheMode::Eventual,
            _ => CacheMode::Strong,
        };
        Self::with_mode(ttl, mode)
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

    /// Invalidate all entries for a model (on write). In `Eventual` mode this is a **no-op** — the
    /// cache stays hot through writes and the TTL alone bounds staleness.
    pub async fn invalidate_model(&self, model_name: &str) {
        if self.mode == CacheMode::Eventual {
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_format() {
        assert_eq!(ReadCache::key("Contact", "abc"), "Contact:abc");
    }

    #[tokio::test]
    async fn set_then_get_roundtrips() {
        let c = ReadCache::new(60);
        c.set("Contact:q1", json!([{"id": "1"}])).await;
        assert_eq!(c.get("Contact:q1").await, Some(json!([{"id": "1"}])));
        assert_eq!(c.get("Contact:missing").await, None);
    }

    #[tokio::test]
    async fn entries_expire_after_ttl() {
        let c = ReadCache::new(0); // 0s TTL → expires immediately
        c.set("Contact:q1", json!(1)).await;
        // expires_at = now + 0, so a later read is past expiry.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        assert_eq!(
            c.get("Contact:q1").await,
            None,
            "expired entry must not be returned"
        );
    }

    #[tokio::test]
    async fn invalidate_model_is_prefix_exact() {
        let c = ReadCache::new(60);
        c.set("Contact:a", json!(1)).await;
        c.set("Contact:b", json!(2)).await;
        c.set("Company:a", json!(3)).await;
        c.invalidate_model("Contact").await;
        // Only Contact:* dropped; Company:* (and any other model) untouched.
        assert_eq!(c.get("Contact:a").await, None);
        assert_eq!(c.get("Contact:b").await, None);
        assert_eq!(c.get("Company:a").await, Some(json!(3)));
    }

    #[tokio::test]
    async fn eventual_mode_keeps_cache_through_writes() {
        // In eventual mode a write does NOT evict — the entry survives invalidate_model and is
        // served until its TTL expires (the cache stays hot through writes).
        let c = ReadCache::with_mode(60, CacheMode::Eventual);
        c.set("Contact:a", json!(1)).await;
        c.invalidate_model("Contact").await; // a write would call this; in eventual mode it's a no-op
        assert_eq!(c.get("Contact:a").await, Some(json!(1)));

        // Strong mode (default) still evicts on write.
        let s = ReadCache::with_mode(60, CacheMode::Strong);
        s.set("Contact:a", json!(1)).await;
        s.invalidate_model("Contact").await;
        assert_eq!(s.get("Contact:a").await, None);
    }

    #[tokio::test]
    async fn evict_expired_drops_only_stale() {
        let c = ReadCache::new(0);
        c.set("Contact:a", json!(1)).await;
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        c.evict_expired().await;
        assert!(
            c.entries.read().await.is_empty(),
            "expired entries should be evicted"
        );
    }
}
