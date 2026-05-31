use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use atomo_wasm_runtime::{JsRuntime, Permission, PluginManifest, PluginRuntime, WasmPlugin, WasmRuntime};
use tracing::info;

/// A loaded JS (Javy) plugin: its compiled module + the permissions it was granted.
struct JsPlugin {
    module: wasmtime::Module,
    permissions: HashSet<Permission>,
}

pub struct WasmPluginManager {
    runtime: WasmRuntime,
    js_runtime: JsRuntime,
    plugins: HashMap<String, WasmPlugin>,
    /// JS (Javy) plugins: name -> pre-compiled module + granted permissions.
    js_plugins: HashMap<String, JsPlugin>,
    /// Permission-gated effects a JS plugin requested (emit/db/http), drained by the caller.
    js_effects: Vec<String>,
    plugin_dir: PathBuf,
}

impl WasmPluginManager {
    pub fn new(plugin_dir: impl Into<PathBuf>) -> Result<Self> {
        Ok(Self {
            runtime: WasmRuntime::new()?,
            js_runtime: JsRuntime::new()?,
            plugins: HashMap::new(),
            js_plugins: HashMap::new(),
            js_effects: Vec::new(),
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
        match manifest.runtime {
            PluginRuntime::Js => {
                info!(plugin = %name, "Loading JS plugin");
                let module = self.js_runtime.compile(&wasm_path)?;
                let permissions = manifest.permissions.iter().cloned().collect();
                self.js_plugins.insert(name.clone(), JsPlugin { module, permissions });
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
                    let record = obj.get("record").cloned().unwrap_or(serde_json::Value::Null);
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
            } else {
                continue;
            };
            if !perms.contains(&required) {
                anyhow::bail!("plugin '{}' requested '{}' without {:?} permission", plugin, kind, required);
            }
            self.js_effects.push(effect.to_string());
        }
        Ok(())
    }

    /// Drain the effects JS plugins requested during recent hook calls.
    pub fn take_js_effects(&mut self) -> Vec<String> {
        std::mem::take(&mut self.js_effects)
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
            let obj = match effect.as_object() {
                Some(o) => o,
                None => continue,
            };
            if let Some(q) = obj.get("dbQuery") {
                results.push(fulfill_db_request(&q.to_string(), pool).await);
            } else if let Some(r) = obj.get("http") {
                results.push(fulfill_http_request(&r.to_string(), http).await);
            } else if let Some(e) = obj.get("emit") {
                results.push(serde_json::json!({ "emit": e }).to_string());
            }
        }
        results
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
    let method = parsed.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
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
            .execute(&pool).await.unwrap();

        // Valid model request returns rows.
        let res = fulfill_db_request(r#"{"model":"Widget","limit":5}"#, &pool).await;
        assert!(res.contains("\"ok\":true"), "expected ok: {}", res);
        assert!(res.contains("\"name\":\"a\""), "expected row data: {}", res);

        // SQL-injection-style model is rejected (not alphanumeric) — never executes raw SQL.
        let bad = fulfill_db_request(r#"{"model":"widgets; DROP TABLE widgets"}"#, &pool).await;
        assert!(bad.contains("\"ok\":false"), "injection attempt should be rejected: {}", bad);

        // The table still exists (the injection did nothing).
        let still: (i64,) = sqlx::query_as("SELECT count(*) FROM widgets").fetch_one(&pool).await.unwrap();
        assert_eq!(still.0, 1);

        // Malformed JSON is rejected gracefully.
        assert!(fulfill_db_request("not json", &pool).await.contains("\"ok\":false"));

        sqlx::query("DROP TABLE widgets").execute(&pool).await.ok();
    }
}
