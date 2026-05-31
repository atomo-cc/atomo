//! Phase 2 M2: a JS plugin participates in the CRUD hook lifecycle via HookRunner.
//!
//! Loads `tests/fixtures/js-hook-plugin` (a Javy plugin + plugin.toml, runtime="js")
//! through WasmPluginManager, wraps it in WasmHookRunner, and verifies that a
//! `before_create` hook transforms the record (adds a "hooked" tag) — exactly the path
//! the data layer uses. No Javy toolchain needed (the .wasm fixture is committed).

use std::collections::HashMap;
use std::sync::Arc;

use atomo::hooks::{HookContext, HookResult, HookRunner};
use atomo_server::wasm_hooks::WasmHookRunner;
use atomo_server::wasm_plugins::WasmPluginManager;
use serde_json::{json, Value};
use tokio::sync::Mutex;

fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hook")
}

#[tokio::test]
async fn js_plugin_runs_in_before_create_hook() {
    let mut mgr = WasmPluginManager::new(fixtures_dir()).unwrap();
    let loaded = mgr.discover_and_load().await.unwrap();
    assert!(
        loaded.contains(&"js-hook".to_string()),
        "js plugin not loaded: {:?}",
        loaded
    );

    let runner = WasmHookRunner::new(Arc::new(Mutex::new(mgr)));

    let mut data: HashMap<String, Value> = HashMap::new();
    data.insert("email".into(), json!("a@b.com"));
    data.insert("tags".into(), json!(["x"]));
    let ctx = HookContext {
        model_name: "Contact".into(),
        operation: "create".into(),
        data,
        user_id: None,
    };

    let result = runner.run_before("before_create", &ctx).await.unwrap();
    let out = match result {
        HookResult::Continue(d) => d,
        HookResult::Abort(m) => panic!("hook aborted: {}", m),
    };
    let tags = out.get("tags").and_then(|v| v.as_array()).expect("tags");
    assert!(tags.iter().any(|t| t == "x"), "original tag preserved");
    assert!(
        tags.iter().any(|t| t == "hooked"),
        "js before_create added its tag: {:?}",
        out
    );
}
