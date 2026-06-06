//! Custom HTTP routes served by plugins.
//!
//! A plugin declares routes in its `plugin.toml` (`[[routes]]` with `method`/`path`/
//! `auth`); atomo-server mounts each under `/ext/<plugin><path>` and dispatches the
//! request to the plugin's JS handler. The handler receives a request envelope
//! (`{ method, path, query, headers, body, principal }`) and returns
//! `{ response: { status, headers, body }, transaction: [...] }`. This is the
//! extend-without-forking seam: business-logic endpoints live in a plugin, not a
//! fork of the server.
//!
//! **Transactional routes (phase 3).** When the handler returns a `transaction`
//! array, the server runs those statements atomically in ONE DB transaction with
//! bound parameters — the synchronous read-modify-write primitive that lets logic
//! like a no-overdraw debit live in a plugin instead of a sidecar. A statement may
//! carry an `expect` (`{ minRowsAffected, elseStatus?, elseBody? }`): if fewer rows
//! are affected, the whole transaction rolls back and the else-response is returned.
//! Running a `transaction` requires the plugin's `WriteDatabase` permission.

use std::sync::Arc;

use atomo_wasm_runtime::RouteDef;
use axum::{
    body::to_bytes,
    extract::Request,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{on, MethodFilter},
    Router,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::Mutex;

use crate::auth::HttpAuthService;
use crate::wasm_plugins::WasmPluginManager;

const MAX_BODY: usize = 1024 * 1024; // 1 MiB

/// Build a router mounting every plugin-declared route at `/ext/<plugin><path>`.
/// `routes` is taken from `WasmPluginManager::plugin_routes()`.
pub fn plugin_routes_router(
    manager: Arc<Mutex<WasmPluginManager>>,
    auth: HttpAuthService,
    pool: PgPool,
    routes: Vec<(String, RouteDef)>,
) -> Router {
    let http = reqwest::Client::new();
    let mut router = Router::new();
    for (plugin, route) in routes {
        let path = format!("/ext/{}{}", plugin, route.path);
        let filter = method_filter(&route.method);
        let mgr = manager.clone();
        let auth = auth.clone();
        let pool = pool.clone();
        let http = http.clone();
        let plugin = plugin.clone();
        let require_auth = route.auth;
        router = router.route(
            &path,
            on(filter, move |req: Request| {
                let mgr = mgr.clone();
                let auth = auth.clone();
                let pool = pool.clone();
                let http = http.clone();
                let plugin = plugin.clone();
                async move { dispatch(req, mgr, auth, pool, http, plugin, require_auth).await }
            }),
        );
    }
    router
}

fn method_filter(method: &str) -> MethodFilter {
    match method.to_ascii_uppercase().as_str() {
        "GET" => MethodFilter::GET,
        "PUT" => MethodFilter::PUT,
        "DELETE" => MethodFilter::DELETE,
        "PATCH" => MethodFilter::PATCH,
        _ => MethodFilter::POST,
    }
}

async fn dispatch(
    req: Request,
    manager: Arc<Mutex<WasmPluginManager>>,
    auth: HttpAuthService,
    pool: PgPool,
    http: reqwest::Client,
    plugin: String,
    require_auth: bool,
) -> Response {
    let method = req.method().to_string();
    let uri = req.uri().clone();
    let path = uri.path().to_string();
    let query = uri.query().unwrap_or("").to_string();
    let headers = header_map_to_json(req.headers());

    // Authenticate if the route requires it; the verified principal is passed on.
    let principal = if require_auth {
        let token = bearer(req.headers());
        match token {
            Some(t) => match auth.verify_token(&t).await {
                Ok(user) => json!({ "id": user.id, "role": format!("{:?}", user.role) }),
                Err(_) => {
                    return (StatusCode::UNAUTHORIZED, "invalid or expired token").into_response()
                }
            },
            None => return (StatusCode::UNAUTHORIZED, "auth required").into_response(),
        }
    } else {
        Value::Null
    };

    let body_bytes = match to_bytes(req.into_body(), MAX_BODY).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::PAYLOAD_TOO_LARGE, "body too large").into_response(),
    };
    // Pass the body as parsed JSON when it is JSON, otherwise as a raw string.
    let body: Value = serde_json::from_slice(&body_bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body_bytes).to_string()));

    let request_json = json!({
        "method": method,
        "path": path,
        "query": query,
        "headers": headers,
        "body": body,
        "principal": principal,
    })
    .to_string();

    let output = {
        let mut mgr = manager.lock().await;
        match mgr.run_route(&plugin, &request_json) {
            Ok(o) => o,
            Err(e) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, format!("plugin error: {e}"))
                    .into_response()
            }
        }
    };

    // Phase 3: run the handler's `transaction` batch atomically before responding.
    if !output.transaction.is_empty() {
        if !output.can_write_db {
            return (
                StatusCode::FORBIDDEN,
                "plugin lacks the WriteDatabase permission required for transactional routes",
            )
                .into_response();
        }
        match run_transaction(&pool, &output.transaction).await {
            // All statements met their expectations → fall through to the handler response.
            Ok(None) => {}
            // An `expect` failed → the transaction rolled back; return its else-response
            // WITHOUT running effects (a rolled-back debit must not emit events / call out).
            Ok(Some((status, body))) => return json_response(status, body),
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("transaction error: {e}"),
                )
                    .into_response()
            }
        }
    }

    // The transaction committed (or there was none): fulfill the handler's deferred
    // effects (emit/dbQuery/http), permission-gated. Previously these were silently
    // dropped on the route path.
    if !output.effects.is_empty() {
        let mut mgr = manager.lock().await;
        if let Err(e) = mgr
            .fulfill_route_effects(&plugin, &output.effects, &pool, &http)
            .await
        {
            return (StatusCode::FORBIDDEN, format!("effect denied: {e}")).into_response();
        }
    }

    build_response(&output.response.to_string())
}

/// Run a plugin's `transaction` batch in ONE DB transaction with bound params.
/// Returns `Ok(None)` when every statement met its `expect` (committed), or
/// `Ok(Some((status, body)))` when a statement's expectation failed (rolled back —
/// the else-response is returned). Each statement is `{ sql, params?, expect? }`,
/// where `expect` is `{ minRowsAffected, elseStatus?, elseBody? }`.
async fn run_transaction(
    pool: &PgPool,
    statements: &[Value],
) -> Result<Option<(StatusCode, Value)>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    for stmt in statements {
        let sql = match stmt.get("sql").and_then(|s| s.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let empty: Vec<Value> = Vec::new();
        let params = stmt.get("params").and_then(|p| p.as_array()).unwrap_or(&empty);
        let mut q = sqlx::query(sql);
        for p in params {
            q = bind_value(q, p);
        }
        let result = match q.execute(&mut *tx).await {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            }
        };
        if let Some(expect) = stmt.get("expect") {
            let min = expect
                .get("minRowsAffected")
                .and_then(|m| m.as_u64())
                .unwrap_or(0);
            if result.rows_affected() < min {
                let _ = tx.rollback().await;
                let status = expect
                    .get("elseStatus")
                    .and_then(|s| s.as_u64())
                    .and_then(|s| u16::try_from(s).ok())
                    .and_then(|s| StatusCode::from_u16(s).ok())
                    .unwrap_or(StatusCode::CONFLICT);
                let body = expect.get("elseBody").cloned().unwrap_or(Value::Null);
                return Ok(Some((status, body)));
            }
        }
    }
    tx.commit().await?;
    Ok(None)
}

/// Bind one JSON parameter to a query by its underlying type. Strings/numbers/bools
/// bind as the matching SQL scalar; objects/arrays bind as JSONB (json feature).
fn bind_value<'q>(
    q: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    v: &Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match v {
        Value::String(s) => q.bind(s.clone()),
        Value::Bool(b) => q.bind(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                q.bind(i)
            } else if let Some(f) = n.as_f64() {
                q.bind(f)
            } else {
                q.bind(n.to_string())
            }
        }
        Value::Null => q.bind(Option::<String>::None),
        other => q.bind(other.clone()),
    }
}

/// Build an HTTP response from a status + JSON body (used for transaction else-responses).
fn json_response(status: StatusCode, body: Value) -> Response {
    let body_str = match body {
        Value::String(s) => s,
        Value::Null => String::new(),
        other => other.to_string(),
    };
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        body_str,
    )
        .into_response()
}

/// Turn the plugin's `{ status, headers, body }` JSON into an HTTP response.
fn build_response(response_json: &str) -> Response {
    let resp: Value = serde_json::from_str(response_json).unwrap_or(Value::Null);
    let status = resp
        .get("status")
        .and_then(|s| s.as_u64())
        .and_then(|s| u16::try_from(s).ok())
        .and_then(|s| StatusCode::from_u16(s).ok())
        .unwrap_or(StatusCode::OK);
    let body = match resp.get("body") {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    };
    let content_type = resp
        .get("headers")
        .and_then(|h| h.get("content-type"))
        .and_then(|c| c.as_str())
        .unwrap_or("application/json")
        .to_string();
    (
        status,
        [(header::CONTENT_TYPE, content_type)],
        body,
    )
        .into_response()
}

fn header_map_to_json(headers: &HeaderMap) -> Value {
    let mut map = serde_json::Map::new();
    for (name, value) in headers {
        if let Ok(v) = value.to_str() {
            map.insert(name.as_str().to_string(), Value::String(v.to_string()));
        }
    }
    Value::Object(map)
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    // Phase-3 atomic debit: the canonical no-overdraw primitive. Needs Postgres via
    // DATABASE_URL. Run: cargo test -p atomo_server --lib plugin_routes -- --ignored
    #[tokio::test]
    #[ignore]
    async fn run_transaction_atomic_debit_and_rollback() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let pool = sqlx::PgPool::connect(&url).await.unwrap();
        sqlx::query("DROP TABLE IF EXISTS bal_test").execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE bal_test (user_id TEXT PRIMARY KEY, balance BIGINT NOT NULL)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO bal_test (user_id, balance) VALUES ('u1', 10)")
            .execute(&pool).await.unwrap();

        let debit = |cost: i64| {
            vec![json!({
                "sql": "UPDATE bal_test SET balance = balance - $1 WHERE user_id = $2 AND balance >= $1",
                "params": [cost, "u1"],
                "expect": { "minRowsAffected": 1, "elseStatus": 402, "elseBody": { "error": "insufficient" } }
            })]
        };

        // Sufficient: applies and commits → Ok(None); balance 10 - 4 = 6.
        let r = run_transaction(&pool, &debit(4)).await.unwrap();
        assert!(r.is_none(), "sufficient debit should commit, got {:?}", r);
        let (bal,): (i64,) = sqlx::query_as("SELECT balance FROM bal_test WHERE user_id='u1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(bal, 6);

        // Insufficient: 0 rows affected → rollback + 402 else-response; balance unchanged.
        let r2 = run_transaction(&pool, &debit(100)).await.unwrap();
        match r2 {
            Some((status, body)) => {
                assert_eq!(status, StatusCode::PAYMENT_REQUIRED);
                assert_eq!(body["error"], "insufficient");
            }
            None => panic!("insufficient debit should return the else-response"),
        }
        let (bal2,): (i64,) = sqlx::query_as("SELECT balance FROM bal_test WHERE user_id='u1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(bal2, 6, "balance must be unchanged after a rolled-back debit");

        sqlx::query("DROP TABLE IF EXISTS bal_test").execute(&pool).await.unwrap();
    }

    #[test]
    fn method_filter_maps_verbs_and_defaults_to_post() {
        assert_eq!(method_filter("get"), MethodFilter::GET);
        assert_eq!(method_filter("PUT"), MethodFilter::PUT);
        assert_eq!(method_filter("Delete"), MethodFilter::DELETE);
        assert_eq!(method_filter("patch"), MethodFilter::PATCH);
        // Unknown / POST both resolve to POST.
        assert_eq!(method_filter("post"), MethodFilter::POST);
        assert_eq!(method_filter("frobnicate"), MethodFilter::POST);
    }

    #[test]
    fn build_response_uses_status_and_string_body() {
        let resp = build_response(r#"{"status":201,"body":"created"}"#);
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    #[test]
    fn build_response_defaults_to_200_when_status_absent() {
        let resp = build_response(r#"{"body":{"ok":true}}"#);
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn build_response_tolerates_garbage() {
        // A handler that returns non-JSON should not panic the server.
        let resp = build_response("not json at all");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn bearer_extracts_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer abc.def.ghi"),
        );
        assert_eq!(bearer(&headers).as_deref(), Some("abc.def.ghi"));
    }

    #[test]
    fn bearer_absent_or_malformed_is_none() {
        assert_eq!(bearer(&HeaderMap::new()), None);
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Basic xyz"));
        assert_eq!(bearer(&headers), None);
    }

    #[test]
    fn header_map_to_json_collects_string_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let json = header_map_to_json(&headers);
        assert_eq!(json.get("content-type").and_then(|v| v.as_str()), Some("application/json"));
    }
}
