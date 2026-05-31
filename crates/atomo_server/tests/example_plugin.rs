//! Phase 3 §8: integration test for the real in-repo example plugin
//! (services/crm-service/plugins/normalize-contact). Unlike the synthetic fixtures, this
//! drives the committed example end-to-end through the hook runner.

use std::collections::HashMap;
use std::sync::Arc;

use atomo::hooks::{HookContext, HookResult, HookRunner};
use atomo_server::wasm_hooks::WasmHookRunner;
use atomo_server::wasm_plugins::WasmPluginManager;
use serde_json::{json, Value};
use tokio::sync::Mutex;

/// The CRM service plugins dir (manager scans subdirs for plugin.toml; only
/// normalize-contact has one, so it's the only plugin loaded).
fn plugins_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../services/crm-service/plugins")
}

fn ctx(op: &str, email: &str, name: &str) -> HookContext {
    let mut data: HashMap<String, Value> = HashMap::new();
    data.insert("email".into(), json!(email));
    data.insert("name".into(), json!(name));
    HookContext { model_name: "Contact".into(), operation: op.into(), data, user_id: None }
}

#[tokio::test]
async fn example_plugin_normalizes_contact() {
    let mut mgr = WasmPluginManager::new(plugins_dir()).unwrap();
    mgr.discover_and_load().await.unwrap();
    let runner = WasmHookRunner::new(Arc::new(Mutex::new(mgr)));

    let result = runner.run_before("before_create", &ctx("create", "  Alice@Example.COM ", "  Alice  ")).await.unwrap();
    match result {
        HookResult::Continue(data) => {
            assert_eq!(data.get("email").and_then(|v| v.as_str()), Some("alice@example.com"), "email normalized");
            assert_eq!(data.get("name").and_then(|v| v.as_str()), Some("Alice"), "name trimmed");
        }
        HookResult::Abort(m) => panic!("normalize should not abort: {}", m),
    }
}

#[tokio::test]
#[ignore] // DB-gated: fulfillment needs a pool
async fn example_plugin_emits_welcome_notification() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let mut mgr = WasmPluginManager::new(plugins_dir()).unwrap();
    mgr.discover_and_load().await.unwrap();
    let (tx, mut rx) = tokio::sync::broadcast::channel::<atomo::events::ModelEvent>(16);
    mgr.set_event_sender(tx);

    // after_create records the emit effect; fulfill publishes it.
    let _ = mgr.call_hook("normalize-contact", "after_create", r#"{"email":"a@b.com"}"#).unwrap();
    let http = reqwest::Client::new();
    let _ = mgr.fulfill_js_effects(&pool, &http).await;

    let ev = rx.try_recv().expect("a Notification event should be published");
    assert!(matches!(ev.event_type, atomo::events::EventType::Created));
    assert_eq!(ev.model_name, "Notification");
    assert_eq!(ev.data.get("kind").and_then(|v| v.as_str()), Some("contact_welcome"));
}
