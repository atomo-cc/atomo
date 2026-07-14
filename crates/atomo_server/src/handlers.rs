//! HTTP handlers for the Atomo server

use crate::platform_graphql::{PlatformMutation, PlatformQuery};
use async_graphql::{MergedObject, Schema as GraphQLSchema};
use async_graphql_axum::GraphQLResponse;
use atomo::graphql::UserRoleCtx;
use atomo::graphql::{Mutation as ServiceMutation, Query as ServiceQuery, Subscription};
use atomo::prelude::*;
use atomo_core::types::EntityId;
use axum::http::{header, HeaderMap};
use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{delete, get, post},
    Router,
};
use prometheus::{Encoder, TextEncoder};
use serde_json::Value;
use std::time::Instant;

/// Combined query that merges service-level and platform-level queries
#[derive(MergedObject)]
pub struct Query(ServiceQuery, PlatformQuery);

/// Combined mutation that merges service-level and platform-level mutations  
#[derive(MergedObject)]
pub struct Mutation(ServiceMutation, PlatformMutation);

pub type AtomoGraphQLSchema = GraphQLSchema<Query, Mutation, Subscription>;

/// Authenticated WebSocket handler for GraphQL subscriptions. Closes the prior bypass where
/// `/graphql/ws` was mounted with no auth. The JWT must arrive in the `connection_init` payload
/// (`{"authorization":"Bearer <jwt>"}` or a bare `token`/`authorization`); it is verified and the
/// resulting role is injected as `UserRoleCtx` so the subscription resolver can gate by access.
async fn graphql_ws_handler(
    schema: AtomoGraphQLSchema,
    auth: crate::auth::HttpAuthService,
    protocol: async_graphql_axum::GraphQLProtocol,
    ws: axum::extract::WebSocketUpgrade,
) -> axum::response::Response {
    use async_graphql_axum::GraphQLWebSocket;
    ws.protocols(async_graphql::http::ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |stream| async move {
            GraphQLWebSocket::new(stream, schema, protocol)
                .on_connection_init(move |value: serde_json::Value| async move {
                    let raw = value
                        .get("authorization")
                        .or_else(|| value.get("Authorization"))
                        .or_else(|| value.get("token"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.strip_prefix("Bearer ").unwrap_or(s).to_string());
                    let token = raw.ok_or_else(|| {
                        async_graphql::Error::new(
                            "WebSocket auth required: missing token in connection_init",
                        )
                    })?;
                    let user = auth.verify_token(&token).await.map_err(|_| {
                        async_graphql::Error::new("WebSocket auth failed: invalid or expired token")
                    })?;
                    let mut data = async_graphql::Data::default();
                    data.insert(UserRoleCtx(format!("{:?}", user.role)));
                    data.insert(atomo::graphql::UserIdCtx(user.id.clone()));
                    // Optional tenant scoping for the subscription (S3a): pass `tenant` in the
                    // connection_init payload to receive only that tenant's events.
                    if let Some(tid) = value.get("tenant").and_then(|v| v.as_str()) {
                        data.insert(atomo::graphql::TenantCtx(tid.to_string()));
                    }
                    Ok(data)
                })
                .serve()
                .await
        })
}

/// Build extended GraphQL schema that includes both service and platform queries
pub fn build_extended_schema(atomo: &Atomo) -> AtomoGraphQLSchema {
    use std::sync::Arc;

    // Get service-level components
    let client = Arc::new(atomo.client().clone());
    let schema = atomo.schema().clone();
    let pool = atomo.db_pool().clone();

    // Create service-level components
    let service_query = ServiceQuery::new(client.clone(), schema.clone());
    let service_mutation = ServiceMutation::new(client.clone(), schema.clone());
    let subscription = Subscription::new(client.clone());

    // Create platform-level components
    let platform_query = PlatformQuery::new(pool.clone());
    let platform_mutation = PlatformMutation::new(pool.clone());

    // Merge queries and mutations
    let query = Query(service_query, platform_query);
    let mutation = Mutation(service_mutation, platform_mutation);

    // Job queue handle for the `enqueueJob` mutation (shares the pool + event stream).
    let job_store = std::sync::Arc::new(crate::jobs::JobStore::new(
        atomo.db_pool().clone(),
        atomo.event_sender(),
    ));

    let mut builder = GraphQLSchema::build(query, mutation, subscription)
        .data(client)
        .data(pool)
        .data(job_store);

    // Apply GraphQL limits from env (with safe defaults)
    let max_depth: Option<usize> = std::env::var("GRAPHQL_MAX_DEPTH")
        .ok()
        .and_then(|s| s.parse().ok());
    let max_complexity: Option<usize> = std::env::var("GRAPHQL_MAX_COMPLEXITY")
        .ok()
        .and_then(|s| s.parse().ok());
    if let Some(d) = max_depth.or(Some(20)) {
        builder = builder.limit_depth(d);
    }
    if let Some(c) = max_complexity.or(Some(200)) {
        builder = builder.limit_complexity(c);
    }

    builder.finish()
}

/// Implements the workflow `MutationExecutor` seam by running the step's GraphQL query against
/// the built schema. Lets workflow `Mutation` steps actually execute (item 3) without the
/// engine depending on the server/GraphQL layer.
pub struct GraphQlMutationExecutor {
    schema: AtomoGraphQLSchema,
}

impl GraphQlMutationExecutor {
    pub fn new(schema: AtomoGraphQLSchema) -> Self {
        Self { schema }
    }
}

#[async_trait::async_trait]
impl atomo::workflow::MutationExecutor for GraphQlMutationExecutor {
    async fn execute(
        &self,
        query: &str,
        variables: &std::collections::HashMap<String, Value>,
    ) -> anyhow::Result<()> {
        let vars = async_graphql::Variables::from_json(serde_json::json!(variables));
        let resp = self
            .schema
            .execute(async_graphql::Request::new(query).variables(vars))
            .await;
        if resp.is_ok() {
            Ok(())
        } else {
            let msg = resp
                .errors
                .iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join("; ");
            anyhow::bail!("workflow mutation failed: {}", msg)
        }
    }
}

/// Decide whether an authenticated request may use a given `x-tenant-id` header value.
/// A user bound to a tenant (`user_tenant = Some`) may ONLY use its own tenant; a user with no
/// binding may pass any (legacy/single-tenant-admin). Security-critical — unit-tested below.
fn tenant_header_allowed(user_tenant: Option<&str>, header_tenant: &str) -> bool {
    user_tenant.is_none_or(|ut| ut == header_tenant)
}

pub async fn graphql_handler(
    Extension(schema): Extension<AtomoGraphQLSchema>,
    req: axum::extract::Request,
) -> GraphQLResponse {
    static GQL_COUNTER: once_cell::sync::Lazy<prometheus::IntCounterVec> =
        once_cell::sync::Lazy::new(|| {
            prometheus::register_int_counter_vec!(
                "graphql_requests_total",
                "Total GraphQL requests",
                &["operation", "status"]
            )
            .expect("register graphql_requests_total")
        });
    static GQL_HISTO: once_cell::sync::Lazy<prometheus::HistogramVec> =
        once_cell::sync::Lazy::new(|| {
            prometheus::register_histogram_vec!(
                "graphql_request_duration_seconds",
                "GraphQL request duration seconds",
                &["operation"],
                vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
            )
            .expect("register graphql_request_duration_seconds")
        });

    // Extract auth user from request extensions (set by auth middleware)
    let auth_user = req.extensions().get::<crate::auth::AuthUser>().cloned();
    let tenant_id = req
        .headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Parse GraphQL request from body
    let body = axum::body::to_bytes(req.into_body(), 2_000_000)
        .await
        .unwrap_or_default();
    let mut inner: async_graphql::Request = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => async_graphql::Request::new(""),
    };

    // Inject user role into GraphQL context
    let mut user_tenant: Option<String> = None;
    let authenticated = auth_user.is_some();
    if let Some(user) = auth_user {
        inner = inner.data(UserRoleCtx(format!("{:?}", user.role)));
        inner = inner.data(atomo::graphql::UserIdCtx(user.id.clone()));
        user_tenant = user.tenant_id.clone();
    }
    // Tenant scoping (S3b): only honor x-tenant-id for AUTHENTICATED requests, AND — when the
    // user has a tenant binding — only if the header matches it. A user bound to tenant A cannot
    // act as tenant B. Users without a binding (user_tenant=None) may still pass a header (legacy
    // / single-tenant-admin behavior). A mismatch drops the header (scoping falls back to none →
    // the request sees only unscoped/own rows, never another tenant's).
    // The tenant we'll bind for DB-enforced RLS (when ATOMO_ENABLE_RLS is on). Only set when
    // the header is allowed for this user — same gate as TenantCtx, so app-layer scoping and
    // RLS binding can never disagree.
    let mut rls_scope: Option<String> = None;
    if let Some(tid) = tenant_id {
        let allowed = authenticated && tenant_header_allowed(user_tenant.as_deref(), &tid);
        if allowed {
            rls_scope = Some(tid.clone());
            inner = inner.data(atomo::graphql::TenantCtx(tid));
        }
    }

    let op = inner
        .operation_name
        .clone()
        .unwrap_or_else(|| "anonymous".to_string());
    let start = Instant::now();
    // Bind the tenant as a task-local scope for the whole request execution: when RLS is on,
    // every query the resolvers run wraps itself in a transaction that SET LOCALs the tenant.
    // No-op when RLS is off or no tenant is in scope.
    let resp = atomo::graphql::with_tenant_scope(rls_scope, schema.execute(inner)).await;
    let status = if resp.errors.is_empty() {
        "ok"
    } else {
        "error"
    };
    let elapsed = start.elapsed().as_secs_f64();
    GQL_COUNTER.with_label_values(&[op.as_str(), status]).inc();
    GQL_HISTO.with_label_values(&[op.as_str()]).observe(elapsed);
    resp.into()
}

fn model_in_public_read_allowlist(config: &str, model: &str) -> bool {
    config
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .any(|value| value == model)
}

/// Fixed equality filters always applied to a public-read model, read from
/// `ATOMO_PUBLIC_READ_FILTER_<Model>` (e.g. `status:published,kind:app`). This replaces any
/// hardcoded publication convention: the operator declares per model which rows are public. A model
/// with no filter config exposes all of its public rows — restricting to e.g. published rows is an
/// explicit choice the operator makes here.
fn public_read_fixed_filters(model: &str) -> Vec<(String, String)> {
    parse_public_read_filters(
        &std::env::var(format!("ATOMO_PUBLIC_READ_FILTER_{model}")).unwrap_or_default(),
    )
}

fn parse_public_read_filters(config: &str) -> Vec<(String, String)> {
    config
        .split(',')
        .filter_map(|pair| {
            let (field, value) = pair.trim().split_once(':')?;
            let field = field.trim();
            if field.is_empty() {
                None
            } else {
                Some((field.to_string(), value.trim().to_string()))
            }
        })
        .collect()
}

/// Query fields a client may filter on for a public-read model, read from
/// `ATOMO_PUBLIC_READ_FIELDS_<Model>` (e.g. `slug,category`). A query parameter not in this list is
/// ignored, so a client can never widen the exposed rows beyond the operator's allowance, and a
/// model with no field config accepts no client filters at all.
fn public_read_query_fields(model: &str) -> Vec<String> {
    parse_public_read_fields(
        &std::env::var(format!("ATOMO_PUBLIC_READ_FIELDS_{model}")).unwrap_or_default(),
    )
}

fn parse_public_read_fields(config: &str) -> Vec<String> {
    config
        .split(',')
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .map(String::from)
        .collect()
}

/// Build the `where` object for a public read: the client may only filter on operator-approved
/// query fields, and the operator's fixed filters are applied last so a client cannot override them
/// (e.g. cannot flip `status` away from a forced `published`).
fn build_public_where(
    fixed: Vec<(String, String)>,
    allowed_fields: &[String],
    params: &std::collections::HashMap<String, String>,
) -> serde_json::Map<String, Value> {
    let mut where_value = serde_json::Map::new();
    for field in allowed_fields {
        if let Some(value) = params.get(field) {
            where_value.insert(field.clone(), serde_json::json!({ "equals": value }));
        }
    }
    for (field, value) in fixed {
        where_value.insert(field, serde_json::json!({ "equals": value }));
    }
    where_value
}

fn model_declares_public_read(atomo: &Atomo, model: &str) -> bool {
    let Some(rule) = atomo
        .schema()
        .models
        .get(model)
        .and_then(|model| model.access.as_ref())
        .and_then(|access| access.read.as_ref())
    else {
        return false;
    };
    serde_json::to_value(rule)
        .ok()
        .is_some_and(|value| value == serde_json::json!({ "Boolean": "public" }))
}

/// Narrow anonymous read surface for explicitly allowlisted projection models.
///
/// Dual approval (default deny): a model is readable only when the operator lists it in
/// `ATOMO_PUBLIC_READ_MODELS` **and** it declares `read: allow.public()` (the resolver still
/// evaluates that rule). Which rows and which client filters are exposed is explicit per-model
/// configuration — `ATOMO_PUBLIC_READ_FILTER_<Model>` (fixed filters) and
/// `ATOMO_PUBLIC_READ_FIELDS_<Model>` (allowed query fields) — so the route assumes no `status` or
/// `slug` convention. It exposes neither mutations nor arbitrary GraphQL.
pub async fn public_records(
    axum::extract::Path(model): axum::extract::Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    Extension(schema): Extension<AtomoGraphQLSchema>,
    Extension(atomo): Extension<Atomo>,
    Extension(allowed): Extension<PublicReadModels>,
    Extension(redirects): Extension<std::sync::Arc<crate::public_read_redirects::RedirectStore>>,
) -> Result<axum::response::Response, StatusCode> {
    let model_allowed = allowed.0.iter().any(|m| m == &model);
    if !model_allowed || !model_declares_public_read(&atomo, &model) {
        return Err(StatusCode::NOT_FOUND);
    }

    let limit = params
        .get("limit")
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(100)
        .clamp(1, 100);
    let where_value = build_public_where(
        public_read_fixed_filters(&model),
        &public_read_query_fields(&model),
        &params,
    );
    let request = async_graphql::Request::new(
        r#"query($model: String!, $where: JSON!, $limit: Int!) {
            records(model: $model, where: $where, limit: $limit)
        }"#,
    )
    .variables(async_graphql::Variables::from_json(serde_json::json!({
        "model": model,
        "where": Value::Object(where_value),
        "limit": limit,
    })));
    let response = schema.execute(request).await;
    if !response.errors.is_empty() {
        return Err(StatusCode::FORBIDDEN);
    }
    let data = response
        .data
        .into_json()
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let items = data
        .get("records")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));

    // When a slug query returns empty, check for a redirect before returning.
    let items_array = items.as_array();
    if items_array.is_none_or(|a| a.is_empty()) {
        if let Some(slug) = params.get("slug") {
            if let Ok(Some(redirect)) = redirects.lookup(&model, slug).await {
                let mut target_params = params.clone();
                target_params.insert("slug".to_string(), redirect.to_slug.clone());
                let query = target_params
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
                    .collect::<Vec<_>>()
                    .join("&");
                let location = format!("/public/records/{}?{}", model, query);
                let status = if redirect.permanent {
                    StatusCode::MOVED_PERMANENTLY
                } else {
                    StatusCode::FOUND
                };
                return Ok((status, [(header::LOCATION, location)]).into_response());
            }
        }
    }

    Ok(Json(serde_json::json!({ "items": items })).into_response())
}

// --- Redirect management (admin-only) ---

#[derive(serde::Deserialize)]
pub struct CreateRedirectBody {
    pub model: String,
    pub from_slug: String,
    pub to_slug: String,
    #[serde(default = "default_permanent")]
    pub permanent: bool,
}
fn default_permanent() -> bool {
    true
}

pub async fn create_redirect(
    Extension(redirects): Extension<std::sync::Arc<crate::public_read_redirects::RedirectStore>>,
    Json(body): Json<CreateRedirectBody>,
) -> Result<StatusCode, StatusCode> {
    redirects
        .upsert(&body.model, &body.from_slug, &body.to_slug, body.permanent)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::CREATED)
}

pub async fn delete_redirect(
    axum::extract::Path((model, slug)): axum::extract::Path<(String, String)>,
    Extension(redirects): Extension<std::sync::Arc<crate::public_read_redirects::RedirectStore>>,
) -> Result<StatusCode, StatusCode> {
    let removed = redirects
        .remove(&model, &slug)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn list_redirects(
    axum::extract::Path(model): axum::extract::Path<String>,
    Extension(redirects): Extension<std::sync::Arc<crate::public_read_redirects::RedirectStore>>,
) -> Result<Json<Value>, StatusCode> {
    let items = redirects
        .list(&model)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "redirects": items })))
}

pub async fn graphql_playground() -> Html<String> {
    let source = async_graphql::http::playground_source(
        async_graphql::http::GraphQLPlaygroundConfig::new("/graphql"),
    );
    Html(source)
}

pub async fn health_check() -> &'static str {
    "OK"
}

/// Build/version transparency (consumer feedback #4): lets a consumer verify
/// exactly which build is running — "is this feature actually in the image I
/// pulled?" — without inferring from timestamps. The values are baked into the
/// image at build time via `ATOMO_VERSION` / `ATOMO_GIT_SHA` / `ATOMO_BUILD_TIME`
/// (see the Dockerfile + docker.yml); they fall back to the crate version /
/// "unknown" for a plain `cargo run`.
pub async fn version_info() -> Json<Value> {
    fn env_or(var: &str, default: &str) -> String {
        std::env::var(var)
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default.to_string())
    }
    Json(serde_json::json!({
        "name": "atomo-server",
        "version": env_or("ATOMO_VERSION", env!("CARGO_PKG_VERSION")),
        "commit": env_or("ATOMO_GIT_SHA", "unknown"),
        "buildTime": env_or("ATOMO_BUILD_TIME", "unknown"),
    }))
}

/// Readiness probe: checks DB connectivity
pub async fn ready_check(Extension(atomo): Extension<Atomo>) -> StatusCode {
    let pool = atomo.db_pool().clone();
    match sqlx::query("SELECT 1").execute(&pool).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// Prometheus metrics endpoint
pub async fn metrics(Extension(atomo): Extension<Atomo>) -> (StatusCode, HeaderMap, String) {
    // Update DB health metrics
    static DB_UP: once_cell::sync::Lazy<prometheus::IntGauge> = once_cell::sync::Lazy::new(|| {
        prometheus::register_int_gauge!("db_up", "Database connection status (1=up,0=down)")
            .expect("register db_up")
    });
    static DB_PING: once_cell::sync::Lazy<prometheus::Histogram> =
        once_cell::sync::Lazy::new(|| {
            prometheus::register_histogram!(
                "db_ping_duration_seconds",
                "Database ping duration seconds",
                vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
            )
            .expect("register db_ping_duration_seconds")
        });

    let pool = atomo.db_pool().clone();
    let start = std::time::Instant::now();
    match sqlx::query("SELECT 1").execute(&pool).await {
        Ok(_) => {
            DB_UP.set(1);
        }
        Err(_) => {
            DB_UP.set(0);
        }
    }
    DB_PING.observe(start.elapsed().as_secs_f64());

    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    if encoder.encode(&metric_families, &mut buffer).is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            HeaderMap::new(),
            String::new(),
        );
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "text/plain; version=0.0.4; charset=utf-8".parse().unwrap(),
    );
    (
        StatusCode::OK,
        headers,
        String::from_utf8_lossy(&buffer).into_owned(),
    )
}

pub async fn atomo_info(Extension(_atomo): Extension<Atomo>) -> String {
    // You could add schema introspection here
    "🚀 Atomo Content Core Server\n📊 Schema loaded successfully".to_string()
}

/// Schema metadata endpoint for Admin UI
/// Returns JSON metadata describing the entire data model for dynamic rendering
pub async fn schema_metadata(Extension(atomo): Extension<Atomo>) -> Json<Value> {
    // Extract actual schema metadata from Atomo instance including platform models
    let metadata = crate::schema_metadata::extract_schema_metadata(&atomo);

    // Extend with dynamically registered models
    let mut extended_metadata = metadata;
    if let Some(_models) = extended_metadata.get_mut("models") {
        // Note: In the future, models will be registered dynamically from service schemas
        // For now, we rely on the schema metadata extraction from Atomo instance
        // TODO: Implement dynamic model registration from individual services
    }

    Json(extended_metadata)
}
/// Shared list of model names allowed for anonymous public read (from `ServerConfig`).
#[derive(Clone)]
pub struct PublicReadModels(pub Vec<String>);

pub fn create_router(
    schema: AtomoGraphQLSchema,
    atomo: Atomo,
    auth_service: crate::auth::HttpAuthService,
    audit_service: crate::audit::HttpAuditService,
    registration: crate::auth::RegistrationConfig,
    public_read_models: Vec<String>,
    redirect_store: std::sync::Arc<crate::public_read_redirects::RedirectStore>,
) -> Router {
    use crate::auth::{auth_middleware, handlers, optional_auth_middleware};
    use axum::middleware;

    let oauth_manager = crate::oauth::OAuthManager::from_env().with_pool(atomo.db_pool().clone());

    let protected_routes = Router::new()
        .route("/graphql", post(graphql_handler))
        .route_layer(middleware::from_fn_with_state(
            auth_service.clone(),
            auth_middleware,
        ));

    let semi_protected_routes = Router::new()
        .route("/meta/schema", get(schema_metadata))
        .route("/graphql", get(graphql_playground))
        .route_layer(middleware::from_fn_with_state(
            auth_service.clone(),
            optional_auth_middleware,
        ));

    Router::new()
        // Public routes (no authentication required)
        .route("/", get(|| async { "🚀 Atomo Content Core Server" }))
        .route("/health", get(health_check))
        .route("/ready", get(ready_check))
        .route("/version", get(version_info))
        .route("/metrics", get(metrics))
        .route("/info", get(atomo_info))
        .route("/public/records/{model}", get(public_records))
        .route(
            "/graphql/ws",
            get({
                let schema = schema.clone();
                let auth = auth_service.clone();
                move |protocol: async_graphql_axum::GraphQLProtocol,
                      ws: axum::extract::WebSocketUpgrade| {
                    graphql_ws_handler(schema.clone(), auth.clone(), protocol, ws)
                }
            }),
        )
        // Authentication routes. `/login` + `/refresh` are public (they mint/rotate
        // tokens). `/me` + `/logout` REQUIRE an authenticated user: they read an
        // `AuthUser` from request extensions, which only `auth_middleware` injects —
        // so they must carry that layer, or they 401 unconditionally (they did).
        .nest("/auth", {
            // `/login` + `/refresh` are public (mint/rotate tokens). `/register` is mounted only
            // when self-registration is enabled (default off) so the platform never exposes an
            // open sign-up surface by default. `/me` + `/logout` require an authenticated user.
            let mut public = Router::new()
                .route("/login", post(handlers::login))
                .route("/refresh", post(handlers::refresh));
            if registration.enabled {
                public = public.route("/register", post(handlers::register));
            }
            public
                .merge(
                    Router::new()
                        .route("/me", get(handlers::me))
                        .route("/logout", post(handlers::logout))
                        .route_layer(middleware::from_fn_with_state(
                            auth_service.clone(),
                            auth_middleware,
                        )),
                )
                .with_state(auth_service.clone())
        })
        // OAuth routes
        .nest(
            "/auth/oauth",
            Router::new()
                .route("/authorize", get(crate::oauth::oauth_authorize))
                .route("/callback/{provider}", get(crate::oauth::oauth_callback))
                .route("/providers", get(crate::oauth::oauth_providers))
                .with_state(oauth_manager),
        )
        // Audit log routes. The auth middleware must wrap these: the handlers read
        // AuthUser from request extensions for their admin/manager check, and
        // without the layer the extension is never populated — every call 401'd
        // even with a valid admin token (caught by the admin e2e smoke).
        .nest(
            "/audit",
            Router::new()
                .route("/logs", get(get_audit_logs))
                .route("/user/{user_id}/activity", get(get_user_activity))
                .route(
                    "/entity/{entity_type}/{entity_id}/audit",
                    get(get_entity_audit),
                )
                .route("/statistics", get(get_audit_statistics))
                .route_layer(middleware::from_fn_with_state(
                    auth_service.clone(),
                    optional_auth_middleware,
                ))
                .with_state(audit_service.clone()),
        )
        // Admin-only redirect management (requires auth)
        .nest(
            "/public/redirects",
            Router::new()
                .route("/", post(create_redirect))
                .route("/{model}/{slug}", delete(delete_redirect))
                .route("/{model}", get(list_redirects))
                .route_layer(middleware::from_fn_with_state(
                    auth_service.clone(),
                    auth_middleware,
                )),
        )
        // Merge protected and semi-protected routes
        .merge(protected_routes)
        .merge(semi_protected_routes)
        // Add state and extensions
        .with_state(auth_service)
        .layer(Extension(schema))
        .layer(Extension(atomo))
        .layer(Extension(registration))
        .layer(Extension(PublicReadModels(public_read_models)))
        .layer(Extension(redirect_store))
}

// ============================================================================
// Audit Log Handlers
// ============================================================================

/// Get audit logs with optional filters
pub async fn get_audit_logs(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(audit_service): State<crate::audit::HttpAuditService>,
    req: axum::extract::Request,
) -> Result<Json<Vec<atomo_core::audit::AuditLogEntry>>, axum::http::StatusCode> {
    use atomo_core::audit::{AuditOperation, AuditSearchFilters, AuditService};
    use atomo_core::types::{EntityId, StreamId};

    // Check authentication
    let user = req
        .extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    if !matches!(
        user.role,
        crate::platform_models::UserRole::Admin | crate::platform_models::UserRole::Manager
    ) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    // Parse query parameters into filter
    let filters = AuditSearchFilters {
        entity_type: params.get("entity_type").cloned(),
        entity_id: params
            .get("entity_id")
            .and_then(|s| EntityId::from_string(s).ok()),
        stream_id: params
            .get("stream_id")
            .and_then(|s| StreamId::from_string(s).ok()),
        operation: params.get("operation").and_then(|s| match s.as_str() {
            "create" => Some(AuditOperation::Create),
            "update" => Some(AuditOperation::Update),
            "delete" => Some(AuditOperation::Delete),
            "read" => Some(AuditOperation::Read),
            _ => None,
        }),
        user_id: params.get("user_id").cloned(),
        start_time: params
            .get("start_date")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        end_time: params
            .get("end_date")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        limit: params.get("limit").and_then(|s| s.parse().ok()),
        offset: params.get("offset").and_then(|s| s.parse().ok()),
    };

    match audit_service.search_audit_logs(&filters).await {
        Ok(logs) => Ok(Json(logs)),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Get user activity summary
pub async fn get_user_activity(
    axum::extract::Path(user_id): axum::extract::Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(audit_service): State<crate::audit::HttpAuditService>,
    req: axum::extract::Request,
) -> Result<Json<Vec<atomo_core::audit::AuditLogEntry>>, axum::http::StatusCode> {
    use atomo_core::audit::AuditService;

    // Check authentication
    let current_user = req
        .extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    // Users can only view their own activity unless they're admin/manager
    if current_user.id != user_id
        && !matches!(
            current_user.role,
            crate::platform_models::UserRole::Admin | crate::platform_models::UserRole::Manager
        )
    {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    let _limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let user_entity_id = match EntityId::from_string(&user_id) {
        Ok(id) => id,
        Err(_) => return Err(axum::http::StatusCode::BAD_REQUEST),
    };

    match audit_service.get_audit_logs_for_user(&user_entity_id).await {
        Ok(entries) => Ok(Json(entries)),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Get entity audit summary
pub async fn get_entity_audit(
    axum::extract::Path((entity_type, entity_id)): axum::extract::Path<(String, String)>,
    State(audit_service): State<crate::audit::HttpAuditService>,
    req: axum::extract::Request,
) -> Result<Json<Vec<atomo_core::audit::AuditLogEntry>>, axum::http::StatusCode> {
    use atomo_core::audit::AuditService;
    use atomo_core::types::EntityId;

    // Check authentication
    let user = req
        .extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    if !matches!(
        user.role,
        crate::platform_models::UserRole::Admin | crate::platform_models::UserRole::Manager
    ) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    let entity_id =
        EntityId::from_string(&entity_id).map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    match audit_service
        .get_audit_logs_for_entity(&entity_type, &entity_id)
        .await
    {
        Ok(entries) => Ok(Json(entries)),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Get audit statistics
pub async fn get_audit_statistics(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(audit_service): State<crate::audit::HttpAuditService>,
    req: axum::extract::Request,
) -> Result<Json<atomo_core::audit::AuditStats>, axum::http::StatusCode> {
    use atomo_core::audit::{AuditSearchFilters, AuditService};

    // Check authentication - only admin/manager can view statistics
    let user = req
        .extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    if !matches!(
        user.role,
        crate::platform_models::UserRole::Admin | crate::platform_models::UserRole::Manager
    ) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    // Parse date range for statistics
    let filters = AuditSearchFilters {
        start_time: params
            .get("start_date")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        end_time: params
            .get("end_date")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        ..Default::default()
    };

    match audit_service.get_audit_stats(&filters).await {
        Ok(stats) => Ok(Json(stats)),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// ============================================================================
// Workflow REST API
// ============================================================================

use atomo::workflow::{Workflow, WorkflowEngine};
use serde_json::json;
use std::sync::Arc;

/// Router for workflow management, scoped to a shared WorkflowEngine.
pub fn workflow_router(engine: Arc<WorkflowEngine>) -> Router {
    Router::new()
        .route("/workflows", get(list_workflows).post(register_workflow))
        .route(
            "/workflows/{name}",
            get(get_workflow).delete(delete_workflow),
        )
        .route("/workflows/{name}/run", post(run_workflow))
        .with_state(engine)
}

/// GET /workflows/{name} - fetch a single workflow definition
async fn get_workflow(
    State(engine): State<Arc<WorkflowEngine>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<Workflow>, StatusCode> {
    engine.get(&name).map(Json).ok_or(StatusCode::NOT_FOUND)
}

/// GET /workflows - list registered workflow names
async fn list_workflows(State(engine): State<Arc<WorkflowEngine>>) -> Json<Vec<String>> {
    Json(engine.list())
}

/// DELETE /workflows/{name} - remove a registered workflow
async fn delete_workflow(
    State(engine): State<Arc<WorkflowEngine>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if engine.remove(&name) {
        engine.unpersist(&name).await.ok();
        Ok(Json(json!({ "removed": name })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// POST /workflows - register a workflow definition
async fn register_workflow(
    State(engine): State<Arc<WorkflowEngine>>,
    Json(workflow): Json<Workflow>,
) -> Result<Json<Value>, StatusCode> {
    let name = workflow.name.clone();
    engine.register(workflow.clone());
    engine.persist(&workflow).await.ok();
    Ok(Json(json!({ "registered": name })))
}

/// POST /workflows/{name}/run - execute a workflow with the provided context
async fn run_workflow(
    State(engine): State<Arc<WorkflowEngine>>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(context): Json<std::collections::HashMap<String, Value>>,
) -> Result<Json<Value>, StatusCode> {
    match engine.execute(&name, context).await {
        Ok(exec) => Ok(Json(json!({
            "workflow": exec.workflow_name,
            "status": format!("{:?}", exec.status),
            "steps_run": exec.current_step + 1,
            "errors": exec.errors,
        }))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_public_where, model_in_public_read_allowlist, parse_public_read_fields,
        parse_public_read_filters, tenant_header_allowed,
    };
    use std::collections::HashMap;

    // S3b: a user bound to a tenant may only use its own; an unbound user may use any.
    #[test]
    fn tenant_header_validation() {
        // Bound user: own tenant allowed, other tenant denied.
        assert!(tenant_header_allowed(Some("tenant-a"), "tenant-a"));
        assert!(
            !tenant_header_allowed(Some("tenant-a"), "tenant-b"),
            "bound user must not act as another tenant"
        );
        // Unbound user (no users.tenant_id): may pass any (legacy/single-tenant-admin).
        assert!(tenant_header_allowed(None, "tenant-a"));
    }

    #[test]
    fn public_read_model_allowlist_is_exact() {
        let config = "PublicListing, PublicArticle";
        assert!(model_in_public_read_allowlist(config, "PublicListing"));
        assert!(!model_in_public_read_allowlist(config, "PublicationRecord"));
        assert!(!model_in_public_read_allowlist(config, "Public"));
    }

    #[test]
    fn public_read_filter_config_parses_field_value_pairs() {
        let parsed = parse_public_read_filters("status:published, kind:app");
        assert_eq!(
            parsed,
            vec![
                ("status".to_string(), "published".to_string()),
                ("kind".to_string(), "app".to_string()),
            ]
        );
        // Empty / malformed entries are ignored.
        assert!(parse_public_read_filters("").is_empty());
        assert!(parse_public_read_filters("noseparator").is_empty());
    }

    #[test]
    fn public_read_fields_config_parses_list() {
        assert_eq!(
            parse_public_read_fields("slug, category"),
            vec!["slug".to_string(), "category".to_string()]
        );
        assert!(parse_public_read_fields("").is_empty());
    }

    #[test]
    fn build_public_where_honors_only_allowed_fields_and_fixed_filters_win() {
        let params: HashMap<String, String> = [
            ("slug".to_string(), "logo-maker".to_string()),
            ("category".to_string(), "design".to_string()),
            // Not in the allowed list -> must be ignored.
            ("secret".to_string(), "x".to_string()),
            // Attempts to override a fixed filter -> must not win.
            ("status".to_string(), "draft".to_string()),
        ]
        .into_iter()
        .collect();
        let fixed = vec![("status".to_string(), "published".to_string())];
        let allowed = vec!["slug".to_string(), "status".to_string()];
        let where_value = build_public_where(fixed, &allowed, &params);

        // Allowed query field passes through.
        assert_eq!(where_value["slug"]["equals"], "logo-maker");
        // Fixed filter overrides a client attempt to change it.
        assert_eq!(where_value["status"]["equals"], "published");
        // Non-allowlisted query field is never exposed.
        assert!(!where_value.contains_key("category"));
        assert!(!where_value.contains_key("secret"));
    }
}
