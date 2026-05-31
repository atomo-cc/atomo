//! HTTP handlers for the Atomo server

use async_graphql::{Schema as GraphQLSchema, MergedObject};
use async_graphql_axum::{GraphQLResponse, GraphQLSubscription};
use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use atomo::prelude::*;
use atomo::graphql::UserRoleCtx;
use std::time::Instant;
use atomo_core::types::EntityId;
use atomo::graphql::{Query as ServiceQuery, Mutation as ServiceMutation, Subscription};
use crate::platform_graphql::{PlatformQuery, PlatformMutation};
use serde_json::Value;
use prometheus::{Encoder, TextEncoder};
use axum::http::{header, HeaderMap};

/// Combined query that merges service-level and platform-level queries
#[derive(MergedObject)]
pub struct Query(ServiceQuery, PlatformQuery);

/// Combined mutation that merges service-level and platform-level mutations  
#[derive(MergedObject)]
pub struct Mutation(ServiceMutation, PlatformMutation);

pub type AtomoGraphQLSchema = GraphQLSchema<Query, Mutation, Subscription>;

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
    
    let mut builder = GraphQLSchema::build(query, mutation, subscription)
        .data(client)
        .data(pool);

    // Apply GraphQL limits from env (with safe defaults)
    let max_depth: Option<usize> = std::env::var("GRAPHQL_MAX_DEPTH").ok().and_then(|s| s.parse().ok());
    let max_complexity: Option<usize> = std::env::var("GRAPHQL_MAX_COMPLEXITY").ok().and_then(|s| s.parse().ok());
    if let Some(d) = max_depth.or(Some(20)) { builder = builder.limit_depth(d); }
    if let Some(c) = max_complexity.or(Some(200)) { builder = builder.limit_complexity(c); }

    builder.finish()
}

pub async fn graphql_handler(
    Extension(schema): Extension<AtomoGraphQLSchema>,
    req: axum::extract::Request,
) -> GraphQLResponse {
    static GQL_COUNTER: once_cell::sync::Lazy<prometheus::IntCounterVec> = once_cell::sync::Lazy::new(|| {
        prometheus::register_int_counter_vec!(
            "graphql_requests_total",
            "Total GraphQL requests",
            &["operation", "status"]
        ).expect("register graphql_requests_total")
    });
    static GQL_HISTO: once_cell::sync::Lazy<prometheus::HistogramVec> = once_cell::sync::Lazy::new(|| {
        prometheus::register_histogram_vec!(
            "graphql_request_duration_seconds",
            "GraphQL request duration seconds",
            &["operation"],
            vec![0.005,0.01,0.025,0.05,0.1,0.25,0.5,1.0,2.5,5.0,10.0]
        ).expect("register graphql_request_duration_seconds")
    });

    // Extract auth user from request extensions (set by auth middleware)
    let auth_user = req.extensions().get::<crate::auth::AuthUser>().cloned();
    let tenant_id = req.headers().get("x-tenant-id").and_then(|v| v.to_str().ok()).map(String::from);

    // Parse GraphQL request from body
    let body = axum::body::to_bytes(req.into_body(), 2_000_000).await.unwrap_or_default();
    let mut inner: async_graphql::Request = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => async_graphql::Request::new(""),
    };

    // Inject user role into GraphQL context
    if let Some(user) = auth_user {
        inner = inner.data(UserRoleCtx(format!("{:?}", user.role)));
    }
    if let Some(tid) = tenant_id {
        inner = inner.data(atomo::graphql::TenantCtx(tid));
    }

    let op = inner.operation_name.clone().unwrap_or_else(|| "anonymous".to_string());
    let start = Instant::now();
    let resp = schema.execute(inner).await;
    let status = if resp.errors.is_empty() { "ok" } else { "error" };
    let elapsed = start.elapsed().as_secs_f64();
    GQL_COUNTER.with_label_values(&[op.as_str(), status]).inc();
    GQL_HISTO.with_label_values(&[op.as_str()]).observe(elapsed);
    resp.into()
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
        prometheus::register_int_gauge!("db_up", "Database connection status (1=up,0=down)").expect("register db_up")
    });
    static DB_PING: once_cell::sync::Lazy<prometheus::Histogram> = once_cell::sync::Lazy::new(|| {
        prometheus::register_histogram!(
            "db_ping_duration_seconds",
            "Database ping duration seconds",
            vec![0.001,0.005,0.01,0.025,0.05,0.1,0.25,0.5,1.0]
        ).expect("register db_ping_duration_seconds")
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
    if let Err(_) = encoder.encode(&metric_families, &mut buffer) {
        return (StatusCode::INTERNAL_SERVER_ERROR, HeaderMap::new(), String::new());
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "text/plain; version=0.0.4; charset=utf-8".parse().unwrap(),
    );
    (StatusCode::OK, headers, String::from_utf8_lossy(&buffer).into_owned())
}

pub async fn atomo_info(Extension(_atomo): Extension<Atomo>) -> String {
    // You could add schema introspection here
    format!("🚀 Atomo Content Core Server\n📊 Schema loaded successfully")
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
}pub fn create_router(
    schema: AtomoGraphQLSchema, 
    atomo: Atomo, 
    auth_service: crate::auth::HttpAuthService,
    audit_service: crate::audit::HttpAuditService,
) -> Router {
    use crate::auth::{auth_middleware, optional_auth_middleware, handlers};
    use axum::middleware;

    let oauth_manager = crate::oauth::OAuthManager::from_env();

    let protected_routes = Router::new()
        .route("/graphql", post(graphql_handler))
        .route_layer(middleware::from_fn_with_state(auth_service.clone(), auth_middleware));

    let semi_protected_routes = Router::new()
        .route("/meta/schema", get(schema_metadata))
        .route("/graphql", get(graphql_playground))
        .route_layer(middleware::from_fn_with_state(auth_service.clone(), optional_auth_middleware));

    Router::new()
        // Public routes (no authentication required)
        .route("/", get(|| async { "🚀 Atomo Content Core Server" }))
        .route("/health", get(health_check))
        .route("/ready", get(ready_check))
        .route("/metrics", get(metrics))
        .route("/info", get(atomo_info))
        .route_service("/graphql/ws", GraphQLSubscription::new(schema.clone()))
        
        // Authentication routes  
        .nest("/auth", Router::new()
            .route("/login", post(handlers::login))
            .route("/logout", post(handlers::logout))
            .route("/refresh", post(handlers::refresh))
            .route("/me", get(handlers::me))
            .with_state(auth_service.clone())
        )
        
        // OAuth routes
        .nest("/auth/oauth", Router::new()
            .route("/authorize", get(crate::oauth::oauth_authorize))
            .route("/callback/:provider", get(crate::oauth::oauth_callback))
            .route("/providers", get(crate::oauth::oauth_providers))
            .with_state(oauth_manager)
        )
        
        // Audit log routes
        .nest("/audit", Router::new()
            .route("/logs", get(get_audit_logs))
            .route("/user/:user_id/activity", get(get_user_activity))
            .route("/entity/:entity_type/:entity_id/audit", get(get_entity_audit))
            .route("/statistics", get(get_audit_statistics))
            .with_state(audit_service.clone())
        )
        
        // Merge protected and semi-protected routes
        .merge(protected_routes)
        .merge(semi_protected_routes)
        
        // Add state and extensions
        .with_state(auth_service)
        .layer(Extension(schema))
        .layer(Extension(atomo))
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
    use atomo_core::audit::{AuditService, AuditSearchFilters, AuditOperation};
    use atomo_core::types::{EntityId, StreamId};
    
    // Check authentication
    let _user = req.extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    // TODO: Check if user has permission to view audit logs (admin/manager only)
    
    // Parse query parameters into filter
    let filters = AuditSearchFilters {
        entity_type: params.get("entity_type").cloned(),
        entity_id: params.get("entity_id")
            .and_then(|s| EntityId::from_string(s).ok()),
        stream_id: params.get("stream_id")
            .and_then(|s| StreamId::from_string(s).ok()),
        operation: params.get("operation")
            .and_then(|s| match s.as_str() {
                "create" => Some(AuditOperation::Create),
                "update" => Some(AuditOperation::Update),
                "delete" => Some(AuditOperation::Delete),
                "read" => Some(AuditOperation::Read),
                _ => None,
            }),
        user_id: params.get("user_id").cloned(),
        start_time: params.get("start_date")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        end_time: params.get("end_date")
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
    let current_user = req.extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    // Users can only view their own activity unless they're admin/manager
    if current_user.id != user_id && !matches!(current_user.role, crate::platform_models::UserRole::Admin | crate::platform_models::UserRole::Manager) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    let _limit = params.get("limit")
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
    let _user = req.extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    let entity_id = EntityId::from_string(&entity_id)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    match audit_service.get_audit_logs_for_entity(&entity_type, &entity_id).await {
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
    use atomo_core::audit::{AuditService, AuditSearchFilters};
    
    // Check authentication - only admin/manager can view statistics
    let user = req.extensions()
        .get::<crate::auth::AuthUser>()
        .cloned()
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    if !matches!(user.role, crate::platform_models::UserRole::Admin | crate::platform_models::UserRole::Manager) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    // Parse date range for statistics
    let filters = AuditSearchFilters {
        start_time: params.get("start_date")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        end_time: params.get("end_date")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        ..Default::default()
    };

    match audit_service.get_audit_stats(&filters).await {
        Ok(stats) => Ok(Json(stats)),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}
