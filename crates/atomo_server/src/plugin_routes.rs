//! Custom HTTP routes served by plugins.
//!
//! A plugin declares routes in its `plugin.toml` (`[[routes]]` with `method`/`path`/
//! `auth`); atomo-server mounts each under `/ext/<plugin><path>` and dispatches the
//! request to the plugin's JS handler. The handler receives a request envelope
//! (`{ method, path, query, headers, body, principal }`) and returns
//! `{ response: { status, headers, body }, effects: [...] }`. This is the
//! extend-without-forking seam: business-logic endpoints live in a plugin, not a
//! fork of the server.

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
use tokio::sync::Mutex;

use crate::auth::HttpAuthService;
use crate::wasm_plugins::WasmPluginManager;

const MAX_BODY: usize = 1024 * 1024; // 1 MiB

/// Build a router mounting every plugin-declared route at `/ext/<plugin><path>`.
/// `routes` is taken from `WasmPluginManager::plugin_routes()`.
pub fn plugin_routes_router(
    manager: Arc<Mutex<WasmPluginManager>>,
    auth: HttpAuthService,
    routes: Vec<(String, RouteDef)>,
) -> Router {
    let mut router = Router::new();
    for (plugin, route) in routes {
        let path = format!("/ext/{}{}", plugin, route.path);
        let filter = method_filter(&route.method);
        let mgr = manager.clone();
        let auth = auth.clone();
        let plugin = plugin.clone();
        let require_auth = route.auth;
        router = router.route(
            &path,
            on(filter, move |req: Request| {
                let mgr = mgr.clone();
                let auth = auth.clone();
                let plugin = plugin.clone();
                async move { dispatch(req, mgr, auth, plugin, require_auth).await }
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

    let response_json = {
        let mut mgr = manager.lock().await;
        match mgr.call_route(&plugin, &request_json) {
            Ok(r) => r,
            Err(e) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, format!("plugin error: {e}"))
                    .into_response()
            }
        }
    };

    build_response(&response_json)
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
