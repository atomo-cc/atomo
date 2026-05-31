//! Bridge WASM plugins into the CRUD lifecycle as before/after hooks.
//!
//! On each lifecycle event the runner serializes the record as JSON, passes it
//! into the guest via `call_hook`, and deserializes any modified result back.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use serde_json::Value;
use atomo::hooks::{HookContext, HookResult, HookRunner};
use crate::wasm_plugins::WasmPluginManager;

pub struct WasmHookRunner {
    manager: Arc<Mutex<WasmPluginManager>>,
}

impl WasmHookRunner {
    pub fn new(manager: Arc<Mutex<WasmPluginManager>>) -> Self {
        Self { manager }
    }
}

#[async_trait::async_trait]
impl HookRunner for WasmHookRunner {
    async fn run_before(&self, hook_name: &str, ctx: &HookContext) -> anyhow::Result<HookResult> {
        let mut mgr = self.manager.lock().await;
        let plugins: Vec<String> = mgr.loaded_plugins().iter().map(|s| s.to_string()).collect();
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
        let plugins: Vec<String> = mgr.loaded_plugins().iter().map(|s| s.to_string()).collect();
        let json = serde_json::to_string(&ctx.data)?;

        for plugin in plugins {
            let _ = mgr.call_hook(&plugin, hook_name, &json);
        }
        Ok(())
    }
}
