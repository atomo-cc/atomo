//! Phase 2 M3: JS plugin effects are permission-gated.
//!
//! A JS plugin returns `{ record, effects: [{emit|dbQuery|http: ...}] }`. The manager
//! applies effects only if the manifest grants the matching permission; otherwise the
//! hook aborts. Effects are recorded for the caller to fulfill (db/http) or emit.
//! Fixtures are committed Javy `.wasm` builds — no toolchain needed.

use std::collections::HashMap;
use std::sync::Arc;

use atomo::hooks::{HookContext, HookResult, HookRunner};
use atomo_server::wasm_hooks::WasmHookRunner;
use atomo_server::wasm_plugins::WasmPluginManager;
use serde_json::{json, Value};
use tokio::sync::Mutex;

fn dir(sub: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(sub)
}

fn ctx() -> HookContext {
    let mut data: HashMap<String, Value> = HashMap::new();
    data.insert("email".into(), json!("a@b.com"));
    HookContext { model_name: "Contact".into(), operation: "create".into(), data, user_id: None }
}

#[tokio::test]
async fn js_effect_recorded_when_permission_granted() {
    let mut mgr = WasmPluginManager::new(dir("emit-granted")).unwrap();
    mgr.discover_and_load().await.unwrap();
    let mgr = Arc::new(Mutex::new(mgr));
    let runner = WasmHookRunner::new(mgr.clone());

    let result = runner.run_before("before_create", &ctx()).await.unwrap();
    assert!(matches!(result, HookResult::Continue(_)), "granted plugin should continue");

    // The emit effect was recorded (WriteEvents granted).
    let effects = mgr.lock().await.take_js_effects();
    assert_eq!(effects.len(), 1, "expected one recorded effect, got {:?}", effects);
    assert!(effects[0].contains("welcome"), "effect should carry the emit payload: {:?}", effects);
}

#[tokio::test]
async fn js_effect_aborts_when_permission_denied() {
    let mut mgr = WasmPluginManager::new(dir("emit-denied")).unwrap();
    mgr.discover_and_load().await.unwrap();
    let runner = WasmHookRunner::new(Arc::new(Mutex::new(mgr)));

    // The plugin emits without WriteEvents -> the hook must abort.
    let result = runner.run_before("before_create", &ctx()).await.unwrap();
    match result {
        HookResult::Abort(msg) => assert!(msg.contains("WriteEvents"), "abort should cite the missing permission: {}", msg),
        HookResult::Continue(_) => panic!("denied emit should have aborted the hook"),
    }
}

#[tokio::test]
#[ignore]
async fn js_emit_effect_publishes_custom_event() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let mut mgr = WasmPluginManager::new(dir("emit-granted")).unwrap();
    mgr.discover_and_load().await.unwrap();
    // Wire an event sender and subscribe before running.
    let (tx, mut rx) = tokio::sync::broadcast::channel::<atomo::events::ModelEvent>(16);
    mgr.set_event_sender(tx);

    // Record the emit effect (before_create), then fulfill it (publishes the Custom event).
    let _ = mgr.call_hook("js-emit-granted", "before_create", r#"{"email":"a@b.com"}"#).unwrap();
    let http = reqwest::Client::new();
    let _ = mgr.fulfill_js_effects(&pool, &http).await;

    let ev = rx.try_recv().expect("a Custom event should have been published");
    assert!(matches!(ev.event_type, atomo::events::EventType::Custom));
    assert_eq!(ev.model_name, "plugin");
}
