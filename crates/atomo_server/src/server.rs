//! Atomo Server implementation using the new library API

use anyhow::Result;
use axum::http::{header, HeaderName, HeaderValue};
use axum::middleware;
use axum::serve;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::{AllowOrigin, Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
// use axum::{body::Body, response::Response};
// use axum::middleware::Next;
use atomo::prelude::*;
use tracing::{info, instrument};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::{config::ServerConfig, handlers::create_router};

/// Adapter: runs workflow `Plugin` steps through the WasmPluginManager (WASM + JS/Javy).
struct WasmPluginExecutorAdapter(
    std::sync::Arc<tokio::sync::Mutex<crate::wasm_plugins::WasmPluginManager>>,
);

#[async_trait::async_trait]
impl atomo::workflow::PluginExecutor for WasmPluginExecutorAdapter {
    async fn execute(
        &self,
        plugin_name: &str,
        function: &str,
        context_json: &str,
    ) -> Result<Option<String>> {
        let mut mgr = self.0.lock().await;
        mgr.call_hook(plugin_name, function, context_json)
    }
}

pub struct AtomoServer {
    config: ServerConfig,
    atomo: Atomo,
    plugin_manager:
        Option<std::sync::Arc<tokio::sync::Mutex<crate::wasm_plugins::WasmPluginManager>>>,
}

impl AtomoServer {
    /// Create a new server instance with Atomo library
    pub async fn new(config: ServerConfig) -> Result<Self> {
        info!("📊 Loading schema from: {}", config.schema_path);

        // Discover WASM plugins and bridge them into the CRUD hook lifecycle
        let mut plugin_manager = crate::wasm_plugins::WasmPluginManager::new("plugins")?;
        let loaded = plugin_manager.discover_and_load().await.unwrap_or_default();
        let mut builder = Atomo::builder()
            .schema_file(&config.schema_path)
            .database_url(&config.database_url)
            .enable_migrations(true)
            .enable_ai(config.enable_ai);
        let mut manager_handle = None;
        if !loaded.is_empty() {
            info!("🔌 Loaded {} WASM plugin(s): {:?}", loaded.len(), loaded);
            let manager = std::sync::Arc::new(tokio::sync::Mutex::new(plugin_manager));
            let mut runner = crate::wasm_hooks::WasmHookRunner::new(manager.clone());
            // Enable JS effect fulfillment with a pool from the same database.
            if let Ok(pool) = sqlx::PgPool::connect(&config.database_url).await {
                runner = runner.with_fulfillment(pool);
            }
            builder = builder.hook_runner(std::sync::Arc::new(runner));
            manager_handle = Some(manager);
        }
        let atomo = builder.build().await?;

        // Let plugin `emit` effects publish onto the model-event stream.
        if let Some(mgr) = &manager_handle {
            mgr.lock().await.set_event_sender(atomo.event_sender());
        }

        Ok(Self {
            config,
            atomo,
            plugin_manager: manager_handle,
        })
    }

    /// Create from existing Atomo instance (for testing/embedding)
    pub fn from_atomo(config: ServerConfig, atomo: Atomo) -> Self {
        Self {
            config,
            atomo,
            plugin_manager: None,
        }
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
                    tracing::warn!(
                        "JWT_SECRET is not set; using insecure default for development only"
                    );
                    "dev-insecure-secret".to_string()
                }
            }
        };
        let auth_service =
            crate::auth::HttpAuthService::new(&jwt_secret, self.atomo.db_pool().clone());
        let audit_service = crate::audit::HttpAuditService::new(self.atomo.db_pool().clone());

        // Ensure platform tables (users, sessions, audit_log) exist.
        crate::ensure_platform_tables(self.atomo.db_pool()).await?;
        info!("   ✓ Platform tables ensured");

        // Plugin marketplace registry (read API). Artifacts live in ./plugin-registry.
        let registry_store = std::sync::Arc::new(crate::registry::RegistryStore::new(
            self.atomo.db_pool().clone(),
            std::env::var("ATOMO_REGISTRY_DIR").unwrap_or_else(|_| "plugin-registry".to_string()),
        ));
        registry_store.init().await?;
        info!("   ✓ Plugin registry ready");

        // Media upload/storage (POST/GET/DELETE /media). Built before create_router consumes
        // self.atomo: grab the event sender + pool now.
        let media_state = std::sync::Arc::new(crate::media::MediaState::new(
            self.atomo.db_pool().clone(),
            crate::storage::storage_from_env().await,
            self.atomo.event_sender(),
        ));
        media_state.init().await?;
        let media_router =
            crate::media::media_router(media_state, auth_service.clone());
        info!("   ✓ Media storage ready");

        // Audit listener: record an audit entry for every model mutation event.
        {
            let audit = audit_service.clone();
            let mut rx = self.atomo.event_receiver();
            tokio::spawn(async move {
                use atomo_core::audit::AuditService;
                use atomo_core::audit::{AuditLogEntry, AuditOperation};
                use atomo_core::types::EntityId;
                while let Ok(ev) = rx.recv().await {
                    let op = match ev.event_type {
                        atomo::events::EventType::Created => AuditOperation::Create,
                        atomo::events::EventType::Updated => AuditOperation::Update,
                        atomo::events::EventType::Deleted => AuditOperation::Delete,
                        atomo::events::EventType::Custom => AuditOperation::Read,
                    };
                    let entity_id = ev
                        .data
                        .get("id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| EntityId::from_string(s).ok())
                        .unwrap_or_else(EntityId::new);
                    let details = serde_json::to_string(&ev.data).unwrap_or_default();
                    let entry = AuditLogEntry::new(
                        ev.model_name.clone(),
                        entity_id,
                        op,
                        details,
                        ev.actor.clone(),
                    );
                    if let Err(e) = audit.log_audit_entry(entry).await {
                        tracing::warn!(error = %e, "Failed to write audit entry");
                    }
                }
            });
            info!("   ✓ Audit listener started");
        }

        // Optionally seed an admin user from ADMIN_EMAIL/ADMIN_PASSWORD env vars.
        crate::seed_admin(&auth_service).await?;

        // Start CQRS projector listener: convert ModelEvent -> ProjectorEvent and feed projections.
        // Auto-register one TableProjection per schema model (maintains a `{table}_projection` read table).
        let projector_manager = {
            use atomo_projectors::{ProjectorEvent, ProjectorManager, TableProjection};
            let mut manager = ProjectorManager::new(self.atomo.db_pool().clone());
            for (name, model) in &self.atomo.schema().models {
                // Skip enum-derived pseudo-models and block sub-types (only real entities have an `id`).
                if model.fields.contains_key("_enum_type") {
                    continue;
                }
                if !model.fields.contains_key("id") {
                    continue;
                }
                let table = format!("{}_projection", crate::pluralize(name));
                let columns: Vec<String> =
                    model.fields.keys().map(|f| crate::to_snake(f)).collect();
                // Ensure the projection table exists (id + each column as TEXT/JSONB-agnostic TEXT).
                let cols_ddl: Vec<String> = columns
                    .iter()
                    .map(|c| {
                        if c == "id" {
                            format!("\"{}\" TEXT PRIMARY KEY", c)
                        } else {
                            format!("\"{}\" TEXT", c)
                        }
                    })
                    .collect();
                let ddl = format!(
                    "CREATE TABLE IF NOT EXISTS {} ({})",
                    table,
                    cols_ddl.join(", ")
                );
                let _ = sqlx::query(&ddl).execute(self.atomo.db_pool()).await;
                manager.register(TableProjection::new(name, &table, columns));
            }
            let manager = std::sync::Arc::new(manager);
            let (proj_tx, proj_rx) = tokio::sync::broadcast::channel::<ProjectorEvent>(1000);
            manager.clone().start_event_listener(proj_rx);
            let mut model_rx = self.atomo.event_receiver();
            tokio::spawn(async move {
                while let Ok(ev) = model_rx.recv().await {
                    let _ = proj_tx.send(ProjectorEvent {
                        event_type: format!("{:?}", ev.event_type),
                        model_name: ev.model_name,
                        data: ev.data,
                    });
                }
            });
            info!("   ✓ CQRS projector listener started");
            manager
        };

        // Workflow engine: durable (Postgres-backed), plus ./workflows/*.json files, then listeners.
        let mut engine = atomo::workflow::WorkflowEngine::with_pool(self.atomo.db_pool().clone());
        // Inject the executor so workflow `Mutation` steps run against the GraphQL schema (item 3).
        engine.set_mutation_executor(std::sync::Arc::new(
            crate::handlers::GraphQlMutationExecutor::new(graphql_schema.clone()),
        ));
        // Inject the plugin executor so workflow `Plugin` steps run via the WasmPluginManager.
        if let Some(mgr) = &self.plugin_manager {
            engine.set_plugin_executor(std::sync::Arc::new(
                WasmPluginExecutorAdapter(mgr.clone()),
            ));
        }
        let workflow_engine = std::sync::Arc::new(engine);
        {
            workflow_engine.init().await?; // create table + load persisted definitions
            let loaded = crate::load_workflows(&workflow_engine, "workflows").await;
            if loaded > 0 {
                info!("   ✓ Loaded {} workflow(s) from ./workflows", loaded);
            }
            workflow_engine
                .clone()
                .start_event_listener(self.atomo.event_receiver());
            workflow_engine.clone().start_scheduler();
            info!("   ✓ Workflow event listener + scheduler started");
        }

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
        let csp_val = HeaderValue::from_str(&csp_str)
            .unwrap_or_else(|_| HeaderValue::from_static("default-src 'self'"));
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
        let svc_builder = ServiceBuilder::new()
            // Generate/propagate request IDs
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(PropagateRequestIdLayer::x_request_id())
            // Structured tracing with request metadata
            .layer(
                TraceLayer::new_for_http()
                    .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
            )
            .layer(cors_layer);

        // Rate limiting
        let rate_limiter = crate::rate_limit::RateLimiter::from_env();

        // Ephemeral realtime tier (in-memory hub; durable outcomes still go via
        // the normal command path). Started once here and shared by the route.
        let realtime_router = if self.config.enable_realtime {
            let hub = atomo_realtime::Hub::new();
            info!("   ✓ Realtime hub started (ephemeral channels + presence)");
            Some(crate::realtime::realtime_router(hub, auth_service.clone()))
        } else {
            None
        };

        let mut app = create_router(graphql_schema, self.atomo, auth_service, audit_service)
            .merge(crate::handlers::workflow_router(workflow_engine.clone()))
            .merge(crate::projector_routes::projector_router(
                projector_manager.clone(),
            ))
            .merge(crate::registry_routes::registry_router(
                registry_store.clone(),
            ))
            .merge(media_router);
        if let Some(realtime_router) = realtime_router {
            app = app.merge(realtime_router);
        }

        // Optionally serve a bundled Admin UI SPA at /admin. Present in the Docker
        // image (ATOMO_ADMIN_DIR=/app/admin); absent in plain `cargo run`, where
        // this is simply skipped. Unknown /admin/* paths fall back to index.html
        // so client-side routing works.
        let admin_dir = std::env::var("ATOMO_ADMIN_DIR").unwrap_or_else(|_| "admin".to_string());
        let admin_index = std::path::Path::new(&admin_dir).join("index.html");
        if admin_index.is_file() {
            let serve = tower_http::services::ServeDir::new(&admin_dir)
                .not_found_service(tower_http::services::ServeFile::new(admin_index));
            app = app.nest_service("/admin", serve);
            info!("   ✓ Admin UI served at /admin (dir: {})", admin_dir);
        } else {
            info!("   • Admin UI not bundled ({}/index.html absent); /admin disabled", admin_dir);
        }

        let mut app = app
            .layer(svc_builder)
            .layer(middleware::from_fn(
                crate::tracing_middleware::request_tracing,
            ))
            .route_layer(middleware::from_fn_with_state(
                rate_limiter,
                crate::rate_limit::rate_limit_middleware,
            ));
        // Conditionally apply security headers
        let disable_headers = std::env::var("DISABLE_SECURITY_HEADERS")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        if !disable_headers {
            app = app.layer(sec_builder);
        }

        // Start server
        let addr = SocketAddr::new(self.config.host.parse()?, self.config.port);
        let listener = TcpListener::bind(&addr).await?;

        info!("🌐 Server running at http://{}", addr);
        info!("   GraphQL Playground: http://{}/graphql", addr);

        // Optional schema hot-reload (no Rust on the host): the schema is a
        // mounted file in the container, so a background poll detects edits and
        // exits cleanly. The orchestrator's restart policy relaunches the server,
        // which re-parses the schema + migrates on boot — edit-and-live in ~2s.
        if env_flag("ATOMO_SCHEMA_WATCH") {
            spawn_schema_watcher(self.config.schema_path.clone());
        }

        serve(listener, app).await?;

        Ok(())
    }
}

/// True for an env var set to `true`/`1`.
fn env_flag(key: &str) -> bool {
    matches!(std::env::var(key).as_deref(), Ok("true") | Ok("1"))
}

fn schema_mtime(path: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Poll the schema file's mtime; on change, exit so the orchestrator restarts the
/// server with the new schema. Polling (not inotify) is used because file events
/// don't reliably cross Docker bind mounts.
fn spawn_schema_watcher(path: String) {
    let interval_secs: u64 = std::env::var("ATOMO_SCHEMA_WATCH_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(2);
    tokio::spawn(async move {
        let mut last = schema_mtime(&path);
        info!(
            "👀 Watching {} for changes every {}s (exit-on-change reload)",
            path, interval_secs
        );
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        loop {
            ticker.tick().await;
            let current = schema_mtime(&path);
            match (last, current) {
                (Some(prev), Some(now)) if now != prev => {
                    info!("🔄 {} changed — exiting to reload (restart policy relaunches)", path);
                    std::process::exit(0);
                }
                _ => {}
            }
            if current.is_some() {
                last = current;
            }
        }
    });
}
