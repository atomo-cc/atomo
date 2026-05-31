//! Bridge WASM plugins into the CRUD lifecycle as before/after hooks.
//!
//! On each lifecycle event the runner invokes a conventionally named exported
//! function (e.g. `before_create`) on every loaded plugin. A trap/error in a
//! before-hook aborts the operation; after-hooks are fire-and-forget.
//!
//! NOTE: structured record data is not yet marshalled into WASM linear memory;
//! plugins currently act as validation/notification hooks via their exports.

use std::sync::Arc;
use tokio::sync::Mutex;

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
        for plugin in plugins {
            // Only abort if the plugin exposes the hook and it traps.
            if let Err(e) = mgr.call(&plugin, hook_name, &[]) {
                let msg = e.to_string();
                if !msg.contains("not found") {
                    return Ok(HookResult::Abort(format!("{}: {}", plugin, msg)));
                }
            }
        }
        Ok(HookResult::Continue(ctx.data.clone()))
    }

    async fn run_after(&self, hook_name: &str, _ctx: &HookContext) -> anyhow::Result<()> {
        let mut mgr = self.manager.lock().await;
        let plugins: Vec<String> = mgr.loaded_plugins().iter().map(|s| s.to_string()).collect();
        for plugin in plugins {
            let _ = mgr.call(&plugin, hook_name, &[]);
        }
        Ok(())
    }
}
