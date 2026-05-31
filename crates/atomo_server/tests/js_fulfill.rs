//! Phase 2 effect fulfillment: a JS plugin's dbQuery effect is executed.
//! Requires Postgres via DATABASE_URL.
//! Run: cargo test -p atomo_server --test js_fulfill -- --ignored

use std::collections::HashMap;
use std::sync::Arc;

use atomo::hooks::{HookContext, HookRunner};
use atomo_server::wasm_hooks::WasmHookRunner;
use atomo_server::wasm_plugins::WasmPluginManager;
use serde_json::{json, Value};
use tokio::sync::Mutex;

#[tokio::test]
#[ignore]
async fn js_dbquery_effect_is_fulfilled() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    sqlx::query("CREATE TABLE IF NOT EXISTS widgets (id TEXT PRIMARY KEY, name TEXT, deleted_at TIMESTAMPTZ)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO widgets (id, name) VALUES ('1','alpha') ON CONFLICT DO NOTHING")
        .execute(&pool)
        .await
        .unwrap();

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dbquery");
    let mut mgr = WasmPluginManager::new(dir).unwrap();
    mgr.discover_and_load().await.unwrap();
    let mgr = Arc::new(Mutex::new(mgr));
    let runner = WasmHookRunner::new(mgr.clone()).with_fulfillment(pool.clone());

    let mut data: HashMap<String, Value> = HashMap::new();
    data.insert("email".into(), json!("a@b.com"));
    let ctx = HookContext {
        model_name: "Contact".into(),
        operation: "create".into(),
        data,
        user_id: None,
    };

    // after_create runs the plugin (records the dbQuery effect) AND fulfills it.
    runner.run_after("after_create", &ctx).await.unwrap();

    // Re-run fulfillment is a no-op now (effects drained); instead assert by re-driving:
    // call the plugin hook directly to record an effect, then fulfill and inspect results.
    let mut m = mgr.lock().await;
    let _ = m.call_hook("js-dbquery", "after_create", "{}").unwrap();
    let http = reqwest::Client::new();
    let results = m.fulfill_js_effects(&pool, &http).await;
    assert_eq!(
        results.len(),
        1,
        "expected one fulfilled effect, got {:?}",
        results
    );
    assert!(
        results[0].contains("\"ok\":true"),
        "dbQuery should succeed: {}",
        results[0]
    );
    assert!(
        results[0].contains("alpha"),
        "dbQuery should return the widget row: {}",
        results[0]
    );

    sqlx::query("DROP TABLE widgets").execute(&pool).await.ok();
}
