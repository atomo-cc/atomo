//! Bridge WASM plugins into the CRUD lifecycle as before/after hooks.
//!
//! On each lifecycle event the runner serializes the record as JSON, passes it
//! into the guest via `call_hook`, and deserializes any modified result back.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::wasm_plugins::WasmPluginManager;
use atomo::hooks::{HookContext, HookResult, HookRunner};
use serde_json::Value;

pub struct WasmHookRunner {
    manager: Arc<Mutex<WasmPluginManager>>,
    /// When set, JS plugin effects are fulfilled after after-hooks run.
    pool: Option<sqlx::PgPool>,
    http: reqwest::Client,
}

impl WasmHookRunner {
    pub fn new(manager: Arc<Mutex<WasmPluginManager>>) -> Self {
        Self {
            manager,
            pool: None,
            http: reqwest::Client::new(),
        }
    }

    /// Enable JS effect fulfillment (dbQuery/http) using the given pool.
    pub fn with_fulfillment(mut self, pool: sqlx::PgPool) -> Self {
        self.pool = Some(pool);
        self
    }
}

#[async_trait::async_trait]
impl HookRunner for WasmHookRunner {
    async fn run_before(&self, hook_name: &str, ctx: &HookContext) -> anyhow::Result<HookResult> {
        let mut mgr = self.manager.lock().await;
        let plugins: Vec<String> = mgr
            .plugins_for_hook(hook_name)
            .iter()
            .map(|s| s.to_string())
            .collect();
        // No loaded plugin implements this hook — skip the JSON marshalling and the per-plugin
        // instantiate-and-run entirely.
        if plugins.is_empty() {
            return Ok(HookResult::Continue(ctx.data.clone()));
        }
        let json = serde_json::to_string(&ctx.data)?;
        let mut data = ctx.data.clone();

        for plugin in plugins {
            match mgr.call_hook(&plugin, hook_name, &json) {
                Ok(Some(out)) => {
                    data = serde_json::from_str::<HashMap<String, Value>>(&out)?;
                }
                Ok(None) => {}
                Err(e) => {
                    let msg = e.to_string();
                    if !msg.contains("not found") && !msg.contains("missing") {
                        return Ok(HookResult::Abort(format!("{}: {}", plugin, msg)));
                    }
                }
            }
        }
        Ok(HookResult::Continue(data))
    }

    async fn run_after(&self, hook_name: &str, ctx: &HookContext) -> anyhow::Result<()> {
        let mut mgr = self.manager.lock().await;
        let plugins: Vec<String> = mgr
            .plugins_for_hook(hook_name)
            .iter()
            .map(|s| s.to_string())
            .collect();
        // No plugin implements this hook → nothing to run and no effects to fulfill.
        if plugins.is_empty() {
            return Ok(());
        }
        let json = serde_json::to_string(&ctx.data)?;

        for plugin in plugins {
            let _ = mgr.call_hook(&plugin, hook_name, &json);
        }

        // Fulfill any effects the JS plugins recorded (emit/dbQuery/http), if enabled.
        if let Some(pool) = &self.pool {
            let results = mgr.fulfill_js_effects(pool, &self.http).await;
            for r in results {
                tracing::debug!(effect_result = %r, "fulfilled JS plugin effect");
            }
        }
        Ok(())
    }
}
