use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use atomo_wasm_runtime::{
    JsRuntime, Permission, PluginManifest, PluginRuntime, RouteDef, WasmPlugin, WasmRuntime,
};
use tracing::info;

/// A loaded JS (Javy) plugin: its compiled module + the permissions it was granted.
struct JsPlugin {
    module: wasmtime::Module,
    permissions: HashSet<Permission>,
}

/// The parsed result of running a plugin route handler (see `run_route`). The
/// `transaction` batch is executed by the async route dispatcher in one DB
/// transaction; `response` is mapped to the HTTP response (unless an expectation in
/// the batch fails and substitutes an else-response).
pub struct RouteOutput {
    pub response: serde_json::Value,
    pub transaction: Vec<serde_json::Value>,
    /// Deferred effects (`emit`/`dbQuery`/`http`) the handler returned, fulfilled by
    /// the async dispatcher AFTER a successful `transaction` (so a rolled-back debit
    /// emits nothing).
    pub effects: Vec<serde_json::Value>,
    /// Whether the plugin holds `WriteDatabase` — required to run a `transaction`.
    pub can_write_db: bool,
}

pub struct WasmPluginManager {
    runtime: WasmRuntime,
    js_runtime: JsRuntime,
    plugins: HashMap<String, WasmPlugin>,
    /// JS (Javy) plugins: name -> pre-compiled module + granted permissions.
    js_plugins: HashMap<String, JsPlugin>,
    /// Permission-gated effects a JS plugin requested (emit/db/http), drained by the caller.
    js_effects: Vec<String>,
    /// Sender to publish plugin-emitted events onto the model-event stream (set at boot).
    event_sender: Option<tokio::sync::broadcast::Sender<atomo::events::ModelEvent>>,
    /// Plugin-declared HTTP routes: plugin name -> its routes (mounted by atomo-server).
    routes: HashMap<String, Vec<RouteDef>>,
    /// Per-plugin declared hooks (manifest `hooks`). A plugin absent here declared none → treated
    /// as "implements every hook" (legacy: invoked for all). Lets the runner skip plugins that
    /// explicitly don't implement a given hook.
    declared_hooks: HashMap<String, Vec<String>>,
    plugin_dir: PathBuf,
}

/// Does a plugin with these manifest-declared hooks handle `hook`? Undeclared (`None`) = legacy
/// "implements every hook" (run it); declared = only the listed hooks.
fn plugin_handles_hook(declared: Option<&Vec<String>>, hook: &str) -> bool {
    match declared {
        Some(hooks) => hooks.iter().any(|h| h == hook),
        None => true,
    }
}

#[cfg(test)]
mod hook_dispatch_tests {
    use super::plugin_handles_hook;

    #[test]
    fn declared_hooks_gate_dispatch() {
        let declared = vec!["before_create".to_string(), "after_update".to_string()];
        assert!(plugin_handles_hook(Some(&declared), "before_create"));
        assert!(plugin_handles_hook(Some(&declared), "after_update"));
        // Declared but not listed → skipped.
        assert!(!plugin_handles_hook(Some(&declared), "after_create"));
        assert!(!plugin_handles_hook(Some(&declared), "before_delete"));
        // Undeclared (legacy — also how an omitted/empty manifest `hooks` is stored) → always runs.
        assert!(plugin_handles_hook(None, "after_create"));
    }
}

impl WasmPluginManager {
    pub fn new(plugin_dir: impl Into<PathBuf>) -> Result<Self> {
        Ok(Self {
            runtime: WasmRuntime::new()?,
            js_runtime: JsRuntime::new()?,
            plugins: HashMap::new(),
            js_plugins: HashMap::new(),
            js_effects: Vec::new(),
            event_sender: None,
            routes: HashMap::new(),
            declared_hooks: HashMap::new(),
            plugin_dir: plugin_dir.into(),
        })
    }

    /// Discover and load all plugins from the plugin directory
    pub async fn discover_and_load(&mut self) -> Result<Vec<String>> {
        let mut loaded = Vec::new();
        let dir = &self.plugin_dir;
        if !dir.exists() {
            return Ok(loaded);
        }

        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(name) = self.load_plugin_from_dir(&path).await {
                    loaded.push(name);
                }
            }
        }
        Ok(loaded)
    }

    async fn load_plugin_from_dir(&mut self, dir: &Path) -> Result<String> {
        let manifest_path = dir.join("plugin.toml");
        let manifest_content = tokio::fs::read_to_string(&manifest_path).await?;
        let manifest: PluginManifest = toml::from_str(&manifest_content)?;
        let wasm_path = dir.join(&manifest.entry_point);
        let name = manifest.name.clone();
        if !manifest.routes.is_empty() {
            self.routes.insert(name.clone(), manifest.routes.clone());
        }
        if !manifest.hooks.is_empty() {
            self.declared_hooks
                .insert(name.clone(), manifest.hooks.clone());
        }
        match manifest.runtime {
            PluginRuntime::Js => {
                info!(plugin = %name, "Loading JS plugin");
                let module = self.js_runtime.compile(&wasm_path)?;
                let permissions = manifest.permissions.iter().cloned().collect();
                self.js_plugins.insert(
                    name.clone(),
                    JsPlugin {
                        module,
                        permissions,
                    },
                );
            }
            PluginRuntime::Wasm => {
                info!(plugin = %name, "Loading WASM plugin");
                let plugin = self.runtime.load_plugin(&wasm_path, &manifest).await?;
                self.plugins.insert(name.clone(), plugin);
            }
        }
        Ok(name)
    }

    /// Execute a function on a named plugin
    pub fn call(
        &mut self,
        plugin_name: &str,
        function: &str,
        args: &[wasmtime::Val],
    ) -> Result<Vec<wasmtime::Val>> {
        let plugin = self
            .plugins
            .get_mut(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_name))?;
        plugin.call_function(function, args)
    }

    /// Call a hook with JSON marshalling.
    ///
    /// For compiled plugins this calls the exported `{hook}` function. For JS plugins it
    /// re-runs the Javy module with a `{ "hook": <name>, "record": <input> }` envelope on
    /// stdin; the script returns the modified record JSON (or empty/identical = no change).
    pub fn call_hook(
        &mut self,
        plugin_name: &str,
        hook: &str,
        input_json: &str,
    ) -> Result<Option<String>> {
        if let Some(js) = self.js_plugins.get(plugin_name) {
            let module = js.module.clone();
            let perms = js.permissions.clone();
            let envelope = serde_json::json!({
                "hook": hook,
                "record": serde_json::from_str::<serde_json::Value>(input_json).unwrap_or(serde_json::Value::Null),
            })
            .to_string();
            let out = self.js_runtime.run_module(&module, &envelope)?;
            let trimmed = out.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            // Output may be a bare record, or `{ "record": {...}, "effects": [...] }`.
            let parsed: serde_json::Value = serde_json::from_str(trimmed)?;
            if let Some(obj) = parsed.as_object() {
                if obj.contains_key("record") || obj.contains_key("effects") {
                    if let Some(effects) = obj.get("effects").and_then(|e| e.as_array()) {
                        self.apply_js_effects(plugin_name, &perms, effects)?;
                    }
                    let record = obj
                        .get("record")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    return Ok(Some(record.to_string()));
                }
            }
            // Bare record (no effects envelope).
            return Ok(Some(trimmed.to_string()));
        }
        let plugin = self
            .plugins
            .get_mut(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_name))?;
        plugin.call_hook(hook, input_json)
    }

    pub fn loaded_plugins(&self) -> Vec<&str> {
        self.plugins
            .keys()
            .chain(self.js_plugins.keys())
            .map(|s| s.as_str())
            .collect()
    }

    /// Loaded plugins that may handle `hook`: those that declared it in their manifest, plus any
    /// that declared no `hooks` at all (legacy — invoked for everything). A plugin that declared
    /// *some* hooks but not this one is skipped, so the runner avoids a wasted instantiate-and-run.
    pub fn plugins_for_hook(&self, hook: &str) -> Vec<&str> {
        self.loaded_plugins()
            .into_iter()
            .filter(|p| plugin_handles_hook(self.declared_hooks.get(*p), hook))
            .collect()
    }

    /// Every plugin-declared HTTP route, as `(plugin_name, route)`.
    pub fn plugin_routes(&self) -> Vec<(String, RouteDef)> {
        self.routes
            .iter()
            .flat_map(|(name, defs)| {
                let name = name.clone();
                defs.iter().cloned().map(move |d| (name.clone(), d))
            })
            .collect()
    }

    /// Run an HTTP request through a JS plugin's route handler and return the parsed
    /// plan. `request_json` is the request envelope — `{ method, path, query, headers,
    /// body, principal }`. The plugin runs with `{ "route": <request> }` on stdin and
    /// returns `{ "response": { status, headers, body }, "transaction": [...] }`.
    ///
    /// This only RUNS the JS (sync, via the Javy runtime) and parses its output. The
    /// `transaction` batch — the phase-3 atomic read-modify-write primitive — is
    /// executed by the async caller (`plugin_routes`), which owns a DB pool; the sync
    /// runtime here cannot touch the async pool.
    pub fn run_route(&mut self, plugin_name: &str, request_json: &str) -> Result<RouteOutput> {
        if let Some(js) = self.js_plugins.get(plugin_name) {
            let module = js.module.clone();
            let can_write_db = js.permissions.contains(&Permission::WriteDatabase);
            let request = serde_json::from_str::<serde_json::Value>(request_json)
                .unwrap_or(serde_json::Value::Null);
            let envelope = serde_json::json!({ "route": request }).to_string();
            let out = self.js_runtime.run_module(&module, &envelope)?;
            let trimmed = out.trim();
            let parsed: serde_json::Value = if trimmed.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::from_str(trimmed)?
            };
            let response = parsed
                .get("response")
                .cloned()
                .unwrap_or_else(|| parsed.clone());
            let transaction = parsed
                .get("transaction")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            let effects = parsed
                .get("effects")
                .and_then(|e| e.as_array())
                .cloned()
                .unwrap_or_default();
            return Ok(RouteOutput {
                response,
                transaction,
                effects,
                can_write_db,
            });
        }
        anyhow::bail!("plugin '{}' has no JS route handler", plugin_name)
    }

    /// Apply the permission-gated effects a JS plugin requested in its output.
    /// Each effect is one of `{emit}` (WriteEvents), `{dbQuery}` (ReadDatabase),
    /// `{http}` (HttpRequests). A request without the matching permission errors.
    /// Effects are recorded; `dbQuery`/`http` fulfillment is performed by the caller
    /// via the async DB/HTTP path (see `fulfill_requests`).
    fn apply_js_effects(
        &mut self,
        plugin: &str,
        perms: &HashSet<Permission>,
        effects: &[serde_json::Value],
    ) -> Result<()> {
        for effect in effects {
            let obj = match effect.as_object() {
                Some(o) => o,
                None => continue,
            };
            let (kind, required) = if obj.contains_key("emit") {
                ("emit", Permission::WriteEvents)
            } else if obj.contains_key("dbQuery") {
                ("dbQuery", Permission::ReadDatabase)
            } else if obj.contains_key("http") {
                ("http", Permission::HttpRequests)
            } else if obj.contains_key("enqueueJob") {
                ("enqueueJob", Permission::WriteDatabase)
            } else {
                continue;
            };
            // Same permission seam the compiled host functions use.
            Permission::ensure(perms, &required)
                .map_err(|e| anyhow::anyhow!("plugin '{}' effect '{}': {}", plugin, kind, e))?;
            self.js_effects.push(effect.to_string());
        }
        Ok(())
    }

    /// Drain the effects JS plugins requested during recent hook calls.
    pub fn take_js_effects(&mut self) -> Vec<String> {
        std::mem::take(&mut self.js_effects)
    }

    /// Set the event sender so plugin `emit` effects are published to the model-event
    /// stream (consumed by projectors/audit/subscriptions). Call once at boot.
    pub fn set_event_sender(
        &mut self,
        sender: tokio::sync::broadcast::Sender<atomo::events::ModelEvent>,
    ) {
        self.event_sender = Some(sender);
    }

    /// Drain and execute the recorded JS plugin effects.
    /// - `dbQuery` → constrained read via `fulfill_db_request`
    /// - `http` → request via `fulfill_http_request`
    /// - `emit` → returned in the result list (tag `"emit"`) for the caller to push to
    ///   the event stream (the manager has no event sender).
    ///
    /// Returns one JSON result string per effect. Permissions were already checked when
    /// the effects were recorded (`apply_js_effects`).
    pub async fn fulfill_js_effects(
        &mut self,
        pool: &sqlx::PgPool,
        http: &reqwest::Client,
    ) -> Vec<String> {
        let effects = self.take_js_effects();
        let mut results = Vec::new();
        for raw in effects {
            let effect: serde_json::Value = match serde_json::from_str(&raw) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(res) = self.fulfill_one_effect(&effect, pool, http).await {
                results.push(res);
            }
        }
        results
    }

    /// Fulfill ONE already-permission-checked effect (`dbQuery`/`http`/`emit`).
    /// Returns the JSON result string, or None for an unrecognized effect.
    async fn fulfill_one_effect(
        &self,
        effect: &serde_json::Value,
        pool: &sqlx::PgPool,
        http: &reqwest::Client,
    ) -> Option<String> {
        let obj = effect.as_object()?;
        if let Some(q) = obj.get("dbQuery") {
            Some(fulfill_db_request(&q.to_string(), pool).await)
        } else if let Some(r) = obj.get("http") {
            Some(fulfill_http_request(&r.to_string(), http).await)
        } else if let Some(e) = obj.get("emit") {
            // Publish onto the model-event stream, if a sender is set. A plugin may emit
            // a typed event: { model, event: Created|Updated|Deleted|Custom, data }.
            // Unspecified fields fall back to model="plugin", event=Custom, data=payload.
            if let Some(tx) = &self.event_sender {
                let model_name = e
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("plugin")
                    .to_string();
                let event_type = match e.get("event").and_then(|v| v.as_str()) {
                    Some("Created") => atomo::events::EventType::Created,
                    Some("Updated") => atomo::events::EventType::Updated,
                    Some("Deleted") => atomo::events::EventType::Deleted,
                    _ => atomo::events::EventType::Custom,
                };
                let payload = e.get("data").unwrap_or(e);
                let data = match payload {
                    serde_json::Value::Object(m) => m.clone().into_iter().collect(),
                    other => {
                        let mut m = std::collections::HashMap::new();
                        m.insert("value".to_string(), other.clone());
                        m
                    }
                };
                let event = atomo::events::ModelEvent {
                    event_type,
                    model_name,
                    data,
                    previous_data: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    event_id: uuid::Uuid::new_v4().to_string(),
                    actor: None,
                };
                let _ = tx.send(event);
            }
            Some(serde_json::json!({ "emit": e }).to_string())
        } else if let Some(j) = obj.get("enqueueJob") {
            // { queue, kind, payload?, idempotencyKey? } — enqueue a durable job for an external
            // worker (gated by WriteDatabase). Stamped with no tenant; include one in the payload
            // if needed. The job's lifecycle events flow through the same event sender.
            let queue = j.get("queue").and_then(|v| v.as_str()).unwrap_or("");
            let kind = j.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if queue.is_empty() || kind.is_empty() {
                return Some(
                    serde_json::json!({ "enqueueJob": { "error": "queue and kind required" } })
                        .to_string(),
                );
            }
            let payload = j
                .get("payload")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let idem = j.get("idempotencyKey").and_then(|v| v.as_str());
            match &self.event_sender {
                Some(tx) => {
                    let store = crate::jobs::JobStore::new(pool.clone(), tx.clone());
                    match store.enqueue(queue, kind, payload, idem, 5, 0, None).await {
                        Ok(id) => {
                            Some(serde_json::json!({ "enqueueJob": { "id": id } }).to_string())
                        }
                        Err(e) => Some(
                            serde_json::json!({ "enqueueJob": { "error": e.to_string() } })
                                .to_string(),
                        ),
                    }
                }
                None => Some(
                    serde_json::json!({ "enqueueJob": { "error": "job queue unavailable" } })
                        .to_string(),
                ),
            }
        } else {
            None
        }
    }

    /// Fulfill a route handler's deferred effects (after its `transaction` committed),
    /// permission-gated exactly like the CRUD-hook path. Each effect needs the matching
    /// grant (`emit`→WriteEvents, `dbQuery`→ReadDatabase, `http`→HttpRequests). Returns
    /// the per-effect result strings (surfaced for logging; not fed back to the handler).
    pub async fn fulfill_route_effects(
        &self,
        plugin: &str,
        effects: &[serde_json::Value],
        pool: &sqlx::PgPool,
        http: &reqwest::Client,
    ) -> Result<Vec<String>> {
        let perms = self
            .js_plugins
            .get(plugin)
            .map(|p| p.permissions.clone())
            .unwrap_or_default();
        let mut results = Vec::new();
        for effect in effects {
            let obj = match effect.as_object() {
                Some(o) => o,
                None => continue,
            };
            let required = if obj.contains_key("emit") {
                Permission::WriteEvents
            } else if obj.contains_key("dbQuery") {
                Permission::ReadDatabase
            } else if obj.contains_key("http") {
                Permission::HttpRequests
            } else if obj.contains_key("enqueueJob") {
                Permission::WriteDatabase
            } else {
                continue;
            };
            Permission::ensure(&perms, &required)
                .map_err(|e| anyhow::anyhow!("plugin '{}' route effect: {}", plugin, e))?;
            if let Some(res) = self.fulfill_one_effect(effect, pool, http).await {
                results.push(res);
            }
        }
        Ok(results)
    }

    /// Fulfill the DB/HTTP requests a plugin recorded during its last call.
    ///
    /// Drains `take_db_requests`/`take_http_requests` and executes them:
    /// - DB: a constrained read — `{ "model": "<Name>", "limit": <n> }` runs a bounded,
    ///   read-only `SELECT` against the model's table (NOT raw SQL — plugins never run
    ///   arbitrary SQL). Malformed/oversized requests are skipped.
    /// - HTTP: `{ "method", "url", "body"? }` performed via reqwest.
    ///
    /// Returns the JSON results so the caller can feed them back to the plugin
    /// (e.g. via `set_readable_events`) on a subsequent call.
    pub async fn fulfill_requests(
        &mut self,
        plugin_name: &str,
        pool: &sqlx::PgPool,
        http: &reqwest::Client,
    ) -> Result<Vec<String>> {
        let (db_reqs, http_reqs) = {
            let plugin = self
                .plugins
                .get_mut(plugin_name)
                .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_name))?;
            (plugin.take_db_requests(), plugin.take_http_requests())
        };

        let mut results = Vec::new();
        for req in db_reqs {
            results.push(fulfill_db_request(&req, pool).await);
        }
        for req in http_reqs {
            results.push(fulfill_http_request(&req, http).await);
        }
        Ok(results)
    }
}

/// Execute a constrained, read-only DB request. Never runs raw plugin SQL.
pub(crate) async fn fulfill_db_request(req: &str, pool: &sqlx::PgPool) -> String {
    use sqlx::Row;
    let parsed: serde_json::Value = match serde_json::from_str(req) {
        Ok(v) => v,
        Err(e) => return error_json(&format!("invalid db request: {}", e)),
    };
    let model = parsed.get("model").and_then(|v| v.as_str()).unwrap_or("");
    if model.is_empty() || !model.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return error_json("db request requires a safe alphanumeric `model`");
    }
    // Pluralized snake_case table name (matches the SQL builder convention).
    let table = crate::pluralize(model);
    let limit = parsed
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .clamp(1, 100);
    let sql = format!(
        "SELECT row_to_json(t) FROM (SELECT * FROM {} WHERE deleted_at IS NULL LIMIT {}) t",
        table, limit
    );
    match sqlx::query(&sql).fetch_all(pool).await {
        Ok(rows) => {
            let items: Vec<serde_json::Value> = rows
                .iter()
                .filter_map(|r| r.try_get::<serde_json::Value, _>(0).ok())
                .collect();
            serde_json::json!({ "ok": true, "rows": items }).to_string()
        }
        Err(e) => error_json(&format!("db query failed: {}", e)),
    }
}

/// Perform an HTTP request on the plugin's behalf.
async fn fulfill_http_request(req: &str, http: &reqwest::Client) -> String {
    let parsed: serde_json::Value = match serde_json::from_str(req) {
        Ok(v) => v,
        Err(e) => return error_json(&format!("invalid http request: {}", e)),
    };
    let method = parsed
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET");
    let url = parsed.get("url").and_then(|v| v.as_str()).unwrap_or("");
    if url.is_empty() {
        return error_json("http request requires a `url`");
    }
    let m = reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET);
    let mut rb = http.request(m, url);
    if let Some(body) = parsed.get("body") {
        rb = rb.json(body);
    }
    match rb.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            serde_json::json!({ "ok": true, "status": status, "body": text }).to_string()
        }
        Err(e) => error_json(&format!("http request failed: {}", e)),
    }
}

fn error_json(msg: &str) -> String {
    serde_json::json!({ "ok": false, "error": msg }).to_string()
}

#[cfg(test)]
mod tests {
    use super::fulfill_db_request;

    // DB-gated: requires DATABASE_URL pointing at a test database.
    #[tokio::test]
    #[ignore]
    async fn db_request_is_constrained_and_safe() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let pool = sqlx::PgPool::connect(&url).await.unwrap();
        // A real table to read from.
        sqlx::query("CREATE TABLE IF NOT EXISTS widgets (id TEXT PRIMARY KEY, name TEXT, deleted_at TIMESTAMPTZ)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO widgets (id, name) VALUES ('1','a') ON CONFLICT DO NOTHING")
            .execute(&pool)
            .await
            .unwrap();

        // Valid model request returns rows.
        let res = fulfill_db_request(r#"{"model":"Widget","limit":5}"#, &pool).await;
        assert!(res.contains("\"ok\":true"), "expected ok: {}", res);
        assert!(res.contains("\"name\":\"a\""), "expected row data: {}", res);

        // SQL-injection-style model is rejected (not alphanumeric) — never executes raw SQL.
        let bad = fulfill_db_request(r#"{"model":"widgets; DROP TABLE widgets"}"#, &pool).await;
        assert!(
            bad.contains("\"ok\":false"),
            "injection attempt should be rejected: {}",
            bad
        );

        // The table still exists (the injection did nothing).
        let still: (i64,) = sqlx::query_as("SELECT count(*) FROM widgets")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(still.0, 1);

        // Malformed JSON is rejected gracefully.
        assert!(fulfill_db_request("not json", &pool)
            .await
            .contains("\"ok\":false"));

        sqlx::query("DROP TABLE widgets").execute(&pool).await.ok();
    }

    // DB-gated: the `enqueueJob` plugin effect enqueues a real durable job.
    #[tokio::test]
    #[ignore]
    async fn enqueue_job_effect_creates_a_job() {
        use super::WasmPluginManager;
        use sqlx::Row;
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let pool = sqlx::PgPool::connect(&url).await.unwrap();
        let (tx, _rx) = tokio::sync::broadcast::channel(8);
        // Ensure the jobs table exists.
        crate::jobs::JobStore::new(pool.clone(), tx.clone())
            .init()
            .await
            .unwrap();

        let mut mgr = WasmPluginManager::new("plugins").unwrap();
        mgr.set_event_sender(tx);
        let http = reqwest::Client::new();
        let queue = format!("plugin-{}", uuid::Uuid::new_v4());

        let effect = serde_json::json!({
            "enqueueJob": { "queue": queue, "kind": "k", "payload": { "x": 1 } }
        });
        let res = mgr
            .fulfill_one_effect(&effect, &pool, &http)
            .await
            .expect("enqueueJob effect returns a result");
        let v: serde_json::Value = serde_json::from_str(&res).unwrap();
        let id = v["enqueueJob"]["id"].as_str().expect("job id returned");

        let row = sqlx::query("SELECT status, queue, kind FROM jobs WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("status"), "queued");
        assert_eq!(row.get::<String, _>("queue"), queue);
        assert_eq!(row.get::<String, _>("kind"), "k");

        // A blank queue/kind is reported as an error, not a panic.
        let bad = serde_json::json!({ "enqueueJob": { "queue": "", "kind": "k" } });
        let res = mgr.fulfill_one_effect(&bad, &pool, &http).await.unwrap();
        assert!(res.contains("error"), "blank queue rejected: {res}");
    }
}
