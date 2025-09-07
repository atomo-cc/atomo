//! Atomo Server implementation using the new library API

use anyhow::Result;
use axum::serve;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer, AllowOrigin},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
use axum::http::{HeaderValue, header, HeaderName};
use axum::{middleware, http::Request};
// use axum::{body::Body, response::Response};
// use axum::middleware::Next;
use tracing::{info, instrument};
use tracing_subscriber::{fmt, EnvFilter, prelude::*};
use std::time::Duration;
use atomo::prelude::*;

use crate::{
    config::ServerConfig,
    handlers::create_router,
};

pub struct AtomoServer {
    config: ServerConfig,
    atomo: Atomo,
}

impl AtomoServer {
    /// Create a new server instance with Atomo library
    pub async fn new(config: ServerConfig) -> Result<Self> {
        info!("📊 Loading schema from: {}", config.schema_path);
        
        // Initialize Atomo from schema file
        let atomo = Atomo::builder()
            .schema_file(&config.schema_path)
            .database_url(&config.database_url)
            .enable_migrations(true)
            .enable_ai(config.enable_ai)
            .build()
            .await?;
        
        Ok(Self { config, atomo })
    }

    /// Create from existing Atomo instance (for testing/embedding)
    pub fn from_atomo(config: ServerConfig, atomo: Atomo) -> Self {
        Self { config, atomo }
    }

    #[instrument(skip(self))]
    pub async fn run(self) -> Result<()> {
        // Initialize tracing with optional JSON format and env filter
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        let registry = tracing_subscriber::registry().with(filter);
        if matches!(std::env::var("LOG_FORMAT").as_deref(), Ok("json")) {
            registry.with(fmt::layer().json()).init();
        } else {
            registry.with(fmt::layer()).init();
        }

        info!("🚀 Starting Atomo Content Core Server");
        info!("   Host: {}", self.config.host);
        info!("   Port: {}", self.config.port);
        info!("   Database: {}", self.config.database_url);

        // Generate extended GraphQL schema that includes both service and platform queries
        let graphql_schema = crate::handlers::build_extended_schema(&self.atomo);
        info!("   ✓ Extended GraphQL schema generated (service + platform)");

        // Initialize authentication and audit services
        let env_name = std::env::var("ATOMO_ENV").unwrap_or_else(|_| "development".to_string());
        let jwt_secret = match std::env::var("JWT_SECRET") {
            Ok(v) => v,
            Err(_) => {
                if env_name == "production" {
                    anyhow::bail!("JWT_SECRET must be set in production environment");
                } else {
                    tracing::warn!("JWT_SECRET is not set; using insecure default for development only");
                    "dev-insecure-secret".to_string()
                }
            }
        };
        let auth_service = crate::auth::HttpAuthService::new(&jwt_secret, self.atomo.db_pool().clone());
        let audit_service = crate::audit::HttpAuditService::new(self.atomo.db_pool().clone());

        // Build CORS layer from configured origins
        let cors_layer = {
            let origins = &self.config.cors_origins;
            if origins.iter().any(|o| o == "*") {
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_headers(Any)
                    .allow_methods(Any)
            } else {
                let list: Vec<HeaderValue> = origins
                    .iter()
                    .filter_map(|o| HeaderValue::from_str(o).ok())
                    .collect();
                CorsLayer::new()
                    .allow_origin(AllowOrigin::list(list))
                    .allow_headers(Any)
                    .allow_methods(Any)
            }
        };

        // Basic security headers (configurable)
        let csp_str = std::env::var("CSP").unwrap_or_else(|_| "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline' 'unsafe-eval'".to_string());
        let csp_val = HeaderValue::from_str(&csp_str).unwrap_or_else(|_| HeaderValue::from_static("default-src 'self'"));
        let csp_name: HeaderName = HeaderName::from_static("content-security-policy");
        let sec_builder = ServiceBuilder::new()
            .layer(SetResponseHeaderLayer::if_not_present(
                header::STRICT_TRANSPORT_SECURITY,
                HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(
                header::REFERRER_POLICY,
                HeaderValue::from_static("no-referrer"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(csp_name, csp_val));

        // Create router with Atomo context and services
        let mut svc_builder = ServiceBuilder::new()
            // Generate/propagate request IDs
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(PropagateRequestIdLayer::x_request_id())
            // Structured tracing with request metadata
            .layer(
                TraceLayer::new_for_http()
                    .on_response(
                        DefaultOnResponse::new()
                            .level(tracing::Level::INFO)
                    )
            )
            .layer(cors_layer);

        // Global rate limit disabled (tower::limit not enabled). Consider adding a gateway/proxy for rate limiting.

        let mut app = create_router(graphql_schema, self.atomo, auth_service, audit_service)
            .layer(svc_builder);
        // Conditionally apply security headers
        let disable_headers = std::env::var("DISABLE_SECURITY_HEADERS").map(|v| v == "true" || v == "1").unwrap_or(false);
        if !disable_headers { app = app.layer(sec_builder); }
        // Custom per-IP/per-key middleware removed for now to ensure compatibility; use global RPS limiter above.

        // Start server
        let addr = SocketAddr::new(
            self.config.host.parse()?,
            self.config.port,
        );
        let listener = TcpListener::bind(&addr).await?;

        info!("🌐 Server running at http://{}", addr);
        info!("   GraphQL Playground: http://{}/graphql", addr);

        serve(listener, app).await?;

        Ok(())
    }
}
