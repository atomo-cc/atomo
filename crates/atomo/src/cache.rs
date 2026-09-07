//! Bounded process-local read caching. Distributed deployments can explicitly bypass it.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheMode {
    Strong,
    Eventual,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CachePolicyOverride {
    pub enabled: Option<bool>,
    pub ttl_secs: Option<u64>,
    pub tti_secs: Option<u64>,
    pub refresh_after_secs: Option<u64>,
    pub max_entry_bytes: Option<usize>,
    pub max_entries: Option<usize>,
    pub max_bytes: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CacheConfig {
    pub enabled: bool,
    pub mode: CacheMode,
    pub ttl_secs: u64,
    pub tti_secs: Option<u64>,
    pub cleanup_interval_secs: u64,
    pub max_entries: usize,
    pub max_bytes: usize,
    pub max_entry_bytes: usize,
    pub refresh_after_secs: Option<u64>,
    pub multi_instance: bool,
    pub models: HashMap<String, CachePolicyOverride>,
}
impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: CacheMode::Strong,
            ttl_secs: 60,
            tti_secs: None,
            cleanup_interval_secs: 30,
            max_entries: 10_000,
            max_bytes: 64 * 1024 * 1024,
            max_entry_bytes: 1024 * 1024,
            refresh_after_secs: None,
            multi_instance: false,
            models: HashMap::new(),
        }
    }
}
impl CacheConfig {
    pub fn from_env() -> Result<Self, String> {
        let mut value = serde_json::to_value(Self::default()).map_err(|e| e.to_string())?;
        for (name, field, boolean) in [
            ("ENABLED", "enabled", true),
            ("MULTI_INSTANCE", "multi_instance", true),
            ("TTL_SECS", "ttl_secs", false),
            ("TTI_SECS", "tti_secs", false),
            ("CLEANUP_INTERVAL_SECS", "cleanup_interval_secs", false),
            ("MAX_ENTRIES", "max_entries", false),
            ("MAX_BYTES", "max_bytes", false),
            ("MAX_ENTRY_BYTES", "max_entry_bytes", false),
            ("REFRESH_AFTER_SECS", "refresh_after_secs", false),
        ] {
            let key = format!("ATOMO_CACHE_{name}");
            if let Ok(raw) = std::env::var(&key) {
                value[field] = if boolean {
                    Value::Bool(
                        raw.parse()
                            .map_err(|_| format!("{key} must be true or false"))?,
                    )
                } else {
                    Value::from(
                        raw.parse::<u64>()
                            .map_err(|_| format!("{key} must be a nonnegative integer"))?,
                    )
                };
            }
        }
        if let Ok(mode) = std::env::var("ATOMO_CACHE_MODE") {
            value["mode"] = Value::String(mode);
        }
        if let Ok(models) = std::env::var("ATOMO_CACHE_MODELS") {
            value["models"] =
                serde_json::from_str(&models).map_err(|e| format!("ATOMO_CACHE_MODELS: {e}"))?;
        }
        let config: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=86_400).contains(&self.cleanup_interval_secs) {
            return Err("cache cleanup_interval_secs must be in 1..=86400".into());
        }
        Ok(())
    }
}
struct CacheEntry {
    generation: u64,
    data: Value,
    inserted: Instant,
    accessed: Instant,
    bytes: usize,
    ttl: Duration,
    tti: Option<Duration>,
    refresh: Option<Duration>,
}
impl CacheEntry {
    fn expired(&self, now: Instant) -> bool {
        now.duration_since(self.inserted) >= self.ttl
            || self
                .tti
                .is_some_and(|tti| now.duration_since(self.accessed) >= tti)
    }
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct CacheMetrics {
    pub relational_bypasses: u64,
    pub hits: u64,
    pub misses: u64,
    pub expired: u64,
    pub evicted: u64,
    pub skipped: u64,
    pub stale_fills_rejected: u64,
    pub invalidations: u64,
    pub entries: usize,
    pub estimated_bytes: usize,
}
#[derive(Serialize)]
pub struct CacheStatus {
    pub config: CacheConfig,
    pub metrics: CacheMetrics,
    pub effective_enabled: bool,
    pub consistency: &'static str,
}
#[derive(Default)]
struct State {
    entries: HashMap<String, CacheEntry>,
    metrics: CacheMetrics,
}
struct Inner {
    generation: std::sync::atomic::AtomicU64,
    state: Mutex<State>,
    config: CacheConfig,
    flights: Vec<Arc<Mutex<()>>>,
    maintenance_started: std::sync::atomic::AtomicBool,
}
struct MaintenanceGuard(std::sync::Weak<Inner>);
impl Drop for MaintenanceGuard {
    fn drop(&mut self) {
        if let Some(inner) = self.0.upgrade() {
            inner
                .maintenance_started
                .store(false, std::sync::atomic::Ordering::Release);
        }
    }
}
/// Byte accounting includes serialized payload, key bytes and fixed entry overhead; it is not RSS.
#[derive(Clone)]
pub struct ReadCache {
    inner: Arc<Inner>,
}
impl ReadCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self::with_mode(ttl_secs, CacheMode::Strong)
    }
    pub fn with_mode(ttl_secs: u64, mode: CacheMode) -> Self {
        Self::with_config(CacheConfig {
            ttl_secs,
            mode,
            ..Default::default()
        })
    }
    pub fn from_env(default_ttl_secs: u64) -> Self {
        let mut config = CacheConfig::from_env().unwrap_or_else(|error| {
            tracing::error!(%error,"Invalid cache configuration; bypassing cache");
            CacheConfig {
                enabled: false,
                ttl_secs: default_ttl_secs,
                ..Default::default()
            }
        });
        if std::env::var_os("ATOMO_CACHE_TTL_SECS").is_none() {
            config.ttl_secs = default_ttl_secs;
        }
        Self::with_config(config)
    }
    pub fn with_config(mut config: CacheConfig) -> Self {
        if let Err(error) = config.validate() {
            tracing::error!(%error,"Invalid cache configuration; bypassing cache");
            config.enabled = false;
            config.cleanup_interval_secs = 30;
        }
        let inner = Arc::new(Inner {
            generation: std::sync::atomic::AtomicU64::new(0),
            state: Mutex::new(State::default()),
            config,
            maintenance_started: std::sync::atomic::AtomicBool::new(false),
            flights: (0..256).map(|_| Arc::new(Mutex::new(()))).collect(),
        });
        let cache = Self { inner };
        cache.ensure_maintenance();
        cache
    }
    fn ensure_maintenance(&self) {
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            if self
                .inner
                .maintenance_started
                .swap(true, std::sync::atomic::Ordering::AcqRel)
            {
                return;
            }
            let interval = Duration::from_secs(self.inner.config.cleanup_interval_secs);
            let weak = Arc::downgrade(&self.inner);
            let guard = MaintenanceGuard(weak.clone());
            runtime.spawn(async move {
                let _guard = guard;
                loop {
                    tokio::time::sleep(interval).await;
                    let Some(inner) = weak.upgrade() else { break };
                    Self::reclaim(&mut *inner.state.lock().await);
                }
            });
        }
    }
    fn policy(&self, key: &str) -> CachePolicyOverride {
        self.inner
            .config
            .models
            .get(key.split(':').next().unwrap_or(key))
            .cloned()
            .unwrap_or_default()
    }
    fn enabled(&self, key: &str) -> bool {
        self.inner.config.enabled
            && !self.inner.config.multi_instance
            && self.policy(key).enabled.unwrap_or(true)
    }
    /// Bounded lock stripes coalesce identical reads without an unbounded pending-key map.
    pub async fn lock_fill(&self, key: &str) -> OwnedMutexGuard<()> {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        self.inner.flights[h.finish() as usize % self.inner.flights.len()]
            .clone()
            .lock_owned()
            .await
    }
    pub async fn record_relational_bypass(&self) {
        self.inner.state.lock().await.metrics.relational_bypasses += 1;
    }
    pub async fn generation(&self) -> u64 {
        self.inner
            .generation
            .load(std::sync::atomic::Ordering::Acquire)
    }
    pub async fn get(&self, key: &str) -> Option<Value> {
        self.ensure_maintenance();
        let mut state = self.inner.state.lock().await;
        if !self.enabled(key) {
            state.metrics.skipped += 1;
            return None;
        }
        let now = Instant::now();
        if state.entries.get(key).is_some_and(|e| {
            e.expired(now)
                || (self.inner.config.mode == CacheMode::Strong
                    && e.generation
                        != self
                            .inner
                            .generation
                            .load(std::sync::atomic::Ordering::Acquire))
        }) {
            Self::remove(&mut state, key);
            state.metrics.expired += 1;
        }
        if let Some(entry) = state.entries.get_mut(key) {
            // Demand-driven refresh blocks only the requesting fill; no stale-on-error extension.
            if !entry
                .refresh
                .is_some_and(|r| now.duration_since(entry.inserted) >= r)
            {
                entry.accessed = now;
                let data = entry.data.clone();
                state.metrics.hits += 1;
                return Some(data);
            }
        }
        state.metrics.misses += 1;
        None
    }
    pub async fn set(&self, key: &str, value: Value) {
        let generation = self.generation().await;
        self.set_if_generation(key, value, generation).await;
    }
    pub async fn set_if_generation(&self, key: &str, value: Value, generation: u64) {
        self.ensure_maintenance();
        let mut state = self.inner.state.lock().await;
        if generation
            != self
                .inner
                .generation
                .load(std::sync::atomic::Ordering::Acquire)
        {
            state.metrics.stale_fills_rejected += 1;
            return;
        }
        let policy = self.policy(key);
        let config = &self.inner.config;
        let bytes = serde_json::to_vec(&value).map_or(usize::MAX, |v| {
            v.len().saturating_add(key.len()).saturating_add(128)
        });
        if !self.enabled(key)
            || config.max_entries == 0
            || policy.max_entries == Some(0)
            || bytes
                > policy
                    .max_bytes
                    .unwrap_or(config.max_bytes)
                    .min(config.max_bytes)
            || bytes
                > policy
                    .max_entry_bytes
                    .unwrap_or(config.max_entry_bytes)
                    .min(config.max_entry_bytes)
        {
            state.metrics.skipped += 1;
            return;
        }
        Self::reclaim(&mut state);
        Self::remove(&mut state, key);
        while state.entries.len() >= config.max_entries
            || state.metrics.estimated_bytes.saturating_add(bytes) > config.max_bytes
        {
            let Some(old) = state
                .entries
                .iter()
                .min_by_key(|(_, e)| e.accessed)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            Self::remove(&mut state, &old);
            state.metrics.evicted += 1;
        }
        let model_prefix = format!("{}:", key.split(':').next().unwrap_or(key));
        loop {
            let model_entries: Vec<_> = state
                .entries
                .iter()
                .filter(|(k, _)| k.starts_with(&model_prefix))
                .collect();
            let model_bytes: usize = model_entries.iter().map(|(_, e)| e.bytes).sum();
            if model_entries.len() < policy.max_entries.unwrap_or(config.max_entries)
                && model_bytes.saturating_add(bytes) <= policy.max_bytes.unwrap_or(config.max_bytes)
            {
                break;
            }
            let Some(old) = model_entries
                .into_iter()
                .min_by_key(|(_, e)| e.accessed)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            Self::remove(&mut state, &old);
            state.metrics.evicted += 1;
        }
        let now = Instant::now();
        state.entries.insert(
            key.to_owned(),
            CacheEntry {
                generation,
                data: value,
                inserted: now,
                accessed: now,
                bytes,
                ttl: Duration::from_secs(policy.ttl_secs.unwrap_or(config.ttl_secs)),
                tti: policy.tti_secs.or(config.tti_secs).map(Duration::from_secs),
                refresh: policy
                    .refresh_after_secs
                    .or(config.refresh_after_secs)
                    .map(Duration::from_secs),
            },
        );
        state.metrics.estimated_bytes += bytes;
        state.metrics.entries = state.entries.len();
    }
    fn remove(state: &mut State, key: &str) {
        if let Some(old) = state.entries.remove(key) {
            state.metrics.estimated_bytes -= old.bytes;
        }
        state.metrics.entries = state.entries.len();
    }
    fn reclaim(state: &mut State) {
        let now = Instant::now();
        let keys: Vec<_> = state
            .entries
            .iter()
            .filter(|(_, e)| e.expired(now))
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            Self::remove(state, &key);
            state.metrics.expired += 1;
        }
    }
    pub async fn invalidate_model(&self, _model: &str) {
        // Advance before the first suspension: cancellation cannot leave an old entry readable.
        self.inner
            .generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let mut state = self.inner.state.lock().await;
        state.metrics.invalidations += 1;
        // Includes can depend on any model. Conservatively evict all process-local reads.
        if self.inner.config.mode == CacheMode::Strong {
            state.entries.clear();
            state.metrics.entries = 0;
            state.metrics.estimated_bytes = 0;
        }
    }
    pub async fn invalidate(&self, key: &str) {
        self.inner
            .generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let mut state = self.inner.state.lock().await;
        Self::remove(&mut state, key);
    }
    pub async fn evict_expired(&self) {
        Self::reclaim(&mut *self.inner.state.lock().await);
    }
    pub async fn status(&self) -> CacheStatus {
        CacheStatus {
            config: self.inner.config.clone(),
            metrics: self.inner.state.lock().await.metrics.clone(),
            effective_enabled: self.inner.config.enabled && !self.inner.config.multi_instance,
            consistency: if self.inner.config.multi_instance {
                "bypass"
            } else if self.inner.config.mode == CacheMode::Strong {
                "process-local-write-invalidation"
            } else {
                "ttl-bounded-eventual"
            },
        }
    }
    pub fn key(model: &str, query_hash: &str) -> String {
        format!("{model}:{query_hash}")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[tokio::test]
    async fn capacity_bytes_disabled_and_large_values() {
        let c = ReadCache::with_config(CacheConfig {
            max_entries: 2,
            max_bytes: 400,
            max_entry_bytes: 200,
            ..Default::default()
        });
        for n in 0..10 {
            c.set(&format!("Item:{n}"), json!(n)).await;
        }
        let s = c.status().await;
        assert!(s.metrics.entries <= 2);
        assert!(s.metrics.estimated_bytes <= 400);
        assert!(s.metrics.evicted > 0);
        c.set("Item:huge", json!("x".repeat(400))).await;
        assert!(c.get("Item:huge").await.is_none());
        for config in [
            CacheConfig {
                enabled: false,
                ..Default::default()
            },
            CacheConfig {
                multi_instance: true,
                ..Default::default()
            },
        ] {
            let c = ReadCache::with_config(config);
            c.set("A:x", json!(1)).await;
            assert!(c.get("A:x").await.is_none());
            assert_eq!(c.status().await.metrics.entries, 0);
        }
    }
    #[tokio::test]
    async fn stale_fill_barrier_and_modes() {
        for mode in [CacheMode::Strong, CacheMode::Eventual] {
            let c = ReadCache::with_mode(60, mode);
            c.set("A:x", json!(1)).await;
            let generation = c.generation().await;
            c.invalidate_model("A").await;
            c.set_if_generation("A:x", json!(0), generation).await;
            assert_eq!(
                c.get("A:x").await,
                if mode == CacheMode::Strong {
                    None
                } else {
                    Some(json!(1))
                }
            );
            assert_eq!(c.status().await.metrics.stale_fills_rejected, 1);
        }
    }
    #[tokio::test]
    async fn cancelled_invalidation_still_fences_old_entries() {
        let c = ReadCache::new(60);
        c.set("A:x", json!(1)).await;
        let guard = c.inner.state.lock().await;
        let writer = c.clone();
        let task = tokio::spawn(async move {
            writer.invalidate_model("A").await;
        });
        tokio::task::yield_now().await;
        assert_eq!(c.generation().await, 1);
        task.abort();
        let _ = task.await;
        drop(guard);
        assert!(c.get("A:x").await.is_none());
    }
    #[tokio::test]
    async fn expiry_idle_refresh_and_model_override() {
        let c = ReadCache::with_config(CacheConfig {
            tti_secs: Some(0),
            ..Default::default()
        });
        c.set("A:x", json!(1)).await;
        assert!(c.get("A:x").await.is_none());
        assert_eq!(c.status().await.metrics.estimated_bytes, 0);
        let mut config = CacheConfig::default();
        config.models.insert(
            "A".into(),
            CachePolicyOverride {
                enabled: Some(false),
                ..Default::default()
            },
        );
        config.models.insert(
            "B".into(),
            CachePolicyOverride {
                refresh_after_secs: Some(0),
                ..Default::default()
            },
        );
        let c = ReadCache::with_config(config);
        c.set("A:x", json!(1)).await;
        c.set("B:x", json!(2)).await;
        c.set("C:x", json!(3)).await;
        assert!(c.get("A:x").await.is_none());
        assert!(c.get("B:x").await.is_none());
        assert_eq!(c.get("C:x").await, Some(json!(3)));
        let c = ReadCache::new(0);
        c.set("A:x", json!(1)).await;
        c.evict_expired().await;
        assert_eq!(c.status().await.metrics.entries, 0);
    }
    #[tokio::test]
    async fn absolute_ttl_never_extends_with_activity_and_models_stay_bounded() {
        let mut config = CacheConfig {
            ttl_secs: 10,
            tti_secs: Some(5),
            ..Default::default()
        };
        config.models.insert(
            "A".into(),
            CachePolicyOverride {
                max_entries: Some(1),
                ..Default::default()
            },
        );
        let c = ReadCache::with_config(config);
        c.set("A:old", json!(1)).await;
        c.set("A:new", json!(2)).await;
        assert!(c.get("A:old").await.is_none());
        assert_eq!(c.status().await.metrics.entries, 1);
        {
            let mut state = c.inner.state.lock().await;
            let entry = state.entries.get_mut("A:new").unwrap();
            entry.inserted = Instant::now() - Duration::from_secs(11);
            entry.accessed = Instant::now();
        }
        assert!(
            c.get("A:new").await.is_none(),
            "recent access cannot extend absolute TTL"
        );
    }
    #[test]
    fn config_defaults_and_rejects_unknown_overrides() {
        let config: CacheConfig = serde_json::from_value(json!({})).unwrap();
        assert_eq!(config.ttl_secs, 60);
        assert!(config.enabled);
        assert_eq!(config.mode, CacheMode::Strong);
        assert!(
            serde_json::from_value::<CacheConfig>(json!({"models":{"A":{"max_byte":1}}})).is_err()
        );
    }
    #[tokio::test]
    async fn maintenance_reclaims_without_reads_and_does_not_retain_cache() {
        let c = ReadCache::with_config(CacheConfig {
            ttl_secs: 0,
            cleanup_interval_secs: 1,
            ..Default::default()
        });
        c.set("A:x", json!(1)).await;
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(c.status().await.metrics.entries, 0);
        let weak: std::sync::Weak<Inner> = Arc::downgrade(&c.inner);
        drop(c);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn maintenance_restarts_after_runtime_shutdown() {
        let c = ReadCache::new(0);
        let first = tokio::runtime::Runtime::new().unwrap();
        first.block_on(c.set("A:x", json!(1)));
        assert!(c
            .inner
            .maintenance_started
            .load(std::sync::atomic::Ordering::Acquire));
        drop(first);
        assert!(!c
            .inner
            .maintenance_started
            .load(std::sync::atomic::Ordering::Acquire));
        let second = tokio::runtime::Runtime::new().unwrap();
        second.block_on(c.set("A:y", json!(2)));
        assert!(c
            .inner
            .maintenance_started
            .load(std::sync::atomic::Ordering::Acquire));
        drop(second);
        assert!(!c
            .inner
            .maintenance_started
            .load(std::sync::atomic::Ordering::Acquire));
    }
    #[tokio::test]
    async fn demand_refresh_is_coalesced_and_failure_never_serves_expired_data() {
        let mut config = CacheConfig::default();
        config.models.insert(
            "A".into(),
            CachePolicyOverride {
                refresh_after_secs: Some(1),
                ttl_secs: Some(3),
                max_bytes: Some(400),
                ..Default::default()
            },
        );
        let c = ReadCache::with_config(config);
        c.set("A:x", json!("old")).await;
        c.inner
            .state
            .lock()
            .await
            .entries
            .get_mut("A:x")
            .unwrap()
            .inserted = Instant::now() - Duration::from_secs(2);
        let loads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..10 {
            let c = c.clone();
            let loads = loads.clone();
            tasks.push(tokio::spawn(async move {
                let _guard = c.lock_fill("A:x").await;
                if c.get("A:x").await.is_none() {
                    loads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    c.set("A:x", json!("fresh")).await;
                }
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(loads.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(c.get("A:x").await, Some(json!("fresh")));
        c.inner
            .state
            .lock()
            .await
            .entries
            .get_mut("A:x")
            .unwrap()
            .inserted = Instant::now() - Duration::from_secs(4);
        assert!(c.get("A:x").await.is_none());
        assert_eq!(c.status().await.metrics.entries, 0);
        for n in 0..10 {
            c.set(&format!("A:{n}"), json!(n)).await;
        }
        assert!(c.status().await.metrics.estimated_bytes <= 400);
    }
    #[tokio::test]
    async fn concurrent_misses_are_coalesced() {
        let c = ReadCache::new(60);
        let loads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..20 {
            let c = c.clone();
            let loads = loads.clone();
            tasks.push(tokio::spawn(async move {
                let _guard = c.lock_fill("A:x").await;
                if c.get("A:x").await.is_none() {
                    loads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    c.set("A:x", json!(1)).await;
                }
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(loads.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
