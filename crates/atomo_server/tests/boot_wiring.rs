//! Boot-path integration test: proves the server's plugin wiring works end-to-end the way
//! `AtomoServer::new` assembles it — a JS plugin loaded into `WasmHookRunner` (with effect
//! fulfillment + an event sender) actually fires through the live `client.create()` path:
//!   before_create normalizes the record  AND  after_create's emit reaches the event stream.
//! Requires Postgres via DATABASE_URL.
//! Run: cargo test -p atomo_server --test boot_wiring -- --ignored

use std::collections::HashMap;
use std::sync::Arc;

use atomo_server::wasm_hooks::WasmHookRunner;
use atomo_server::wasm_plugins::WasmPluginManager;
use serde_json::{json, Value};
use tokio::sync::Mutex;

fn plugins_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../services/crm-service/plugins")
}

#[tokio::test]
#[ignore]
async fn boot_wiring_runs_plugin_hooks_and_publishes_emit() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");

    // Assemble exactly like AtomoServer::new: manager -> runner(+fulfillment) -> builder,
    // then set the event sender after build.
    let mut mgr = WasmPluginManager::new(plugins_dir()).unwrap();
    mgr.discover_and_load().await.unwrap();
    let mgr = Arc::new(Mutex::new(mgr));
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let runner = WasmHookRunner::new(mgr.clone()).with_fulfillment(pool);

    let schema_ts = r#"
export interface Contact {
  id: string;
  email: string;
  name: string;
}
export const schema = { models: { Contact: { tableName: 'contacts', access: { read: 'public', create: 'public', update: 'public', delete: 'public' } } } };
export default schema;
"#;
    let atomo = atomo::Atomo::builder()
        .schema_content(schema_ts)
        .database_url(&url)
        .enable_migrations(true)
        .hook_runner(Arc::new(runner))
        .build()
        .await
        .unwrap();
    mgr.lock().await.set_event_sender(atomo.event_sender());

    let mut rx = atomo.event_receiver();

    // Drive a real create through the data layer (before/after hooks fire here).
    let mut data: HashMap<String, Value> = HashMap::new();
    data.insert("email".into(), json!("  Boot@Example.COM "));
    data.insert("name".into(), json!("  Boot User  "));
    let created = atomo
        .client()
        .create("Contact", &data, &[], None)
        .await
        .unwrap();

    // (a) before_create normalization applied through the live path.
    assert_eq!(
        created.get("email").and_then(|v| v.as_str()),
        Some("boot@example.com"),
        "email normalized at boot"
    );
    assert_eq!(
        created.get("name").and_then(|v| v.as_str()),
        Some("Boot User"),
        "name trimmed at boot"
    );

    // (b) the after_create emit reached the event stream. Two events are expected on the
    // channel: the CRUD Created (model=Contact) and the plugin emit (model=Notification).
    let mut saw_notification = false;
    for _ in 0..5 {
        match rx.try_recv() {
            Ok(ev) if ev.model_name == "Notification" => {
                assert!(matches!(ev.event_type, atomo::events::EventType::Created));
                assert_eq!(
                    ev.data.get("kind").and_then(|v| v.as_str()),
                    Some("contact_welcome")
                );
                saw_notification = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    assert!(
        saw_notification,
        "plugin emit should have published a Notification event onto the stream"
    );

    // Cleanup
    sqlx::query("DROP TABLE IF EXISTS contacts")
        .execute(atomo.db_pool())
        .await
        .ok();
}
