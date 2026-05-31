use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// Hook execution context passed to hook functions
#[derive(Debug, Clone)]
pub struct HookContext {
    pub model_name: String,
    pub operation: String,
    pub data: HashMap<String, Value>,
    pub user_id: Option<String>,
}

/// Result of a before-hook: can modify data or abort
#[derive(Debug, Clone)]
pub enum HookResult {
    Continue(HashMap<String, Value>),
    Abort(String),
}

/// Hook runner trait - implemented by WASM runtime or in-process hooks
#[async_trait::async_trait]
pub trait HookRunner: Send + Sync {
    async fn run_before(&self, hook_name: &str, ctx: &HookContext) -> Result<HookResult>;
    async fn run_after(&self, hook_name: &str, ctx: &HookContext) -> Result<()>;
}

/// No-op hook runner (default when no plugins loaded)
pub struct NoopHookRunner;

#[async_trait::async_trait]
impl HookRunner for NoopHookRunner {
    async fn run_before(&self, _: &str, ctx: &HookContext) -> Result<HookResult> {
        Ok(HookResult::Continue(ctx.data.clone()))
    }
    async fn run_after(&self, _: &str, _: &HookContext) -> Result<()> {
        Ok(())
    }
}
