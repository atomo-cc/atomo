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

/// Adapter: runs workflow `Job` steps by enqueueing onto the durable job queue.
struct JobEnqueueAdapter(std::sync::Arc<crate::jobs::JobStore>);

#[async_trait::async_trait]
impl atomo::workflow::JobExecutor for JobEnqueueAdapter {
    async fn enqueue(
        &self,
        queue: &str,
        kind: &str,
        payload: &serde_json::Value,
        idempotency_key: Option<&str>,
    ) -> Result<String> {
        // Workflow-enqueued jobs use default retry/priority and no tenant binding.
        self.0
            .enqueue(queue, kind, payload.clone(), idempotency_key, 5, 0, None)
            .await
    }
}

pub struct AtomoServer {
    config: ServerConfig,
    atomo: Atomo,
}

impl AtomoServer {
    /// Create a new server instance with Atomo library
    pub async fn new(config: ServerConfig) -> Result<Self> {
        info!("📊 Loading schema from: {}", config.schema_path);

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
        if !self.config.public_read_models.is_empty() {
            info!("   Public-read models: {:?}", self.config.public_read_models);
        }

        // Fail loud on silent half-registration (consumer feedback #1): a model with
        // no `id` field gets its TABLE created, but is NOT registered as a model —
        // invisible to /meta/schema, the admin UI, GraphQL by-id lookups, and the
        // CQRS projector, previously with zero warning. atomo's primary key is the
        // `id` field; a declared `primaryKey: "..."` other than id is NOT honored.
        // Enum/Block pseudo-models legitimately have no `id`, so exclude them.
        // (Emitted here in run() — after the tracing subscriber is initialized above —
        // not in new(), where warnings would be dropped.)
        for (name, model) in &self.atomo.schema().models {
            let is_pseudo = model.fields.contains_key("_enum_type") || name.ends_with("Block");
            if !is_pseudo && !model.fields.contains_key("id") {
                tracing::warn!(
                    model = %name,
                    "model has no `id` field → NOT registered (invisible to /meta/schema, the \
                     admin UI, GraphQL by-id lookups, and the projector); its table is still \
                     created. atomo's primary key is the `id` field — a declared `primaryKey` is \
                     ignored. Add an `id: string` field to register this model."
                );
            }
        }

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

        // Opt-in, DB-enforced multi-tenant Row-Level Security (defense-in-depth).
        // Gated behind ATOMO_ENABLE_RLS (default OFF → no-op, behavior unchanged).
        // Model tables already exist here (created in Atomo::new() before run()).
        {
            // ServerConfig::enable_rls reads ATOMO_ENABLE_RLS (the same env the data layer's
            // per-request bind reads), so the typed config and the executor never disagree.
            let enabled = self.config.enable_rls;
            if enabled {
                let table_names: Vec<String> = self
                    .atomo
                    .schema()
                    .models
                    .values()
                    // Real entities only: skip enum-derived pseudo-models and block sub-types.
                    .filter(|m| !m.fields.contains_key("_enum_type") && m.fields.contains_key("id"))
                    .map(atomo::query::sql_builder::table_name_for)
                    .collect();
                crate::rls::ensure_rls_policies(self.atomo.db_pool(), &table_names, enabled)
                    .await?;
                info!("   ✓ Row-Level Security policies ensured (ATOMO_ENABLE_RLS)");
            }
        }

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
        let media_router = crate::media::media_router(media_state.clone(), auth_service.clone());
        info!("   ✓ Media storage ready");

        // Ephemeral realtime hub — created here (before the job wiring) so the job-progress
        // endpoint can publish live updates to it; the same hub backs the `/realtime/ws` router
        // mounted further down.
        let realtime_hub = if self.config.enable_realtime {
            Some(atomo_realtime::Hub::new())
        } else {
            None
        };
        // A long-lived system connection used to publish job progress onto `job:{id}` channels.
        let job_progress_publisher = match &realtime_hub {
            Some(hub) => Some(
                hub.connect(atomo_realtime::Principal::new("system:jobs", None))
                    .await
                    .handle,
            ),
            None => None,
        };

        // Durable job queue + worker-token auth (external-worker lease API under /jobs).
        let job_store = std::sync::Arc::new(crate::jobs::JobStore::new(
            self.atomo.db_pool().clone(),
            self.atomo.event_sender(),
        ));
        job_store.init().await?;
        let worker_tokens = std::sync::Arc::new(crate::jobs::WorkerTokenStore::new(
            self.atomo.db_pool().clone(),
        ));
        worker_tokens.init().await?;
        // Generic metered-command primitives (expiring single-use tokens + integer-unit budget
        // ledger). Library primitives a consumer composes transactionally with the job queue; the
        // tables self-init at boot like the other stores so they are available to compose. Gated by
        // ATOMO_ENABLE_METERED_COMMANDS (default on) so deployments that don't use them can skip the
        // tables.
        if self.config.enable_metered_commands {
            let expiring_tokens =
                crate::metered::ExpiringTokenStore::new(self.atomo.db_pool().clone());
            expiring_tokens.init().await?;
            let budget_ledger = crate::metered::BudgetLedger::new(self.atomo.db_pool().clone());
            budget_ledger.init().await?;
            info!("   ✓ Metered-command primitives ready");
        }
        let jobs_router = crate::job_routes::jobs_router(
            job_store.clone(),
            worker_tokens.clone(),
            auth_service.clone(),
            job_progress_publisher,
        );
        // Crash recovery: periodically return expired leases (dead/stalled workers) to the queue.
        {
            let store = job_store.clone();
            let interval_secs: u64 = std::env::var("ATOMO_JOB_RECLAIM_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&n| n >= 1)
                .unwrap_or(30);
            tokio::spawn(async move {
                let mut ticker =
                    tokio::time::interval(std::time::Duration::from_secs(interval_secs));
                loop {
                    ticker.tick().await;
                    match store.reclaim_expired().await {
                        Ok(n) if n > 0 => {
                            tracing::info!(reclaimed = n, "reclaimed expired job leases")
                        }
                        Ok(_) => {}
                        Err(e) => tracing::warn!(error = %e, "job lease reclaim failed"),
                    }
                }
            });
        }
        info!("   ✓ Job queue ready (lease API at /jobs)");

        // Action dispatcher: enqueue lifecycle action jobs when model events match schema bindings.
        crate::action_dispatcher::spawn_action_dispatcher(
            self.atomo.schema().clone(),
            job_store.clone(),
            self.atomo.event_receiver(),
        );
        info!("   ✓ Action dispatcher started");

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
                        atomo::events::EventType::Restored => AuditOperation::Update,
                        atomo::events::EventType::HardDeleted => AuditOperation::Delete,
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
        // Inject the executor so workflow `Mutation` steps run against the GraphQL schema.
        engine.set_mutation_executor(std::sync::Arc::new(
            crate::handlers::GraphQlMutationExecutor::new(graphql_schema.clone()),
        ));
        // Inject the job executor so workflow `Job` steps enqueue onto the durable job queue.
        engine.set_job_executor(std::sync::Arc::new(JobEnqueueAdapter(job_store.clone())));
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
            // Structured tracing with request metadata. The per-response completion log is at
            // DEBUG, not INFO: emitting a formatted log line for *every* request cost ~45% of HTTP
            // throughput in the benchmarks (and most deployments don't want per-request INFO spam).
            // Boot/error logs stay at INFO+, and the `request` info_span still carries request-id
            // context onto any warn/error within a request. Set `RUST_LOG=debug` for per-request logs.
            .layer(
                TraceLayer::new_for_http()
                    .on_response(DefaultOnResponse::new().level(tracing::Level::DEBUG)),
            )
            .layer(cors_layer);

        // Rate limiting
        let rate_limiter = crate::rate_limit::RateLimiter::from_env();

        // Ephemeral realtime tier (in-memory hub; durable outcomes still go via the normal command
        // path). The hub was created above (so job progress can publish to it); mount its WS route.
        let realtime_router = if let Some(hub) = realtime_hub {
            info!("   ✓ Realtime hub started (ephemeral channels + presence)");
            Some(crate::realtime::realtime_router(hub, auth_service.clone()))
        } else {
            None
        };

        let action_router = crate::action_routes::action_router(
            self.atomo.schema().clone(),
            job_store.clone(),
        );

        let crud_router = crate::crud_routes::crud_router(
            std::sync::Arc::new(self.atomo.client().clone()),
            self.atomo.schema().clone(),
            worker_tokens.clone(),
        );

        let registration =
            crate::auth::RegistrationConfig::new(self.config.enable_self_registration);
        let public_read_models = self.config.public_read_models.clone();
        let mut app = create_router(
            graphql_schema,
            self.atomo,
            auth_service,
            audit_service,
            registration,
            public_read_models,
        )
        .merge(crate::handlers::workflow_router(workflow_engine.clone()))
        .merge(crate::projector_routes::projector_router(
            projector_manager.clone(),
        ))
        .merge(crate::registry_routes::registry_router(
            registry_store.clone(),
        ))
        .merge(media_router)
        .merge(jobs_router)
        .merge(action_router)
        .merge(crud_router);
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
            // `.fallback` (not `.not_found_service`, which forces a 404 via
            // SetStatus) serves index.html with 200 for unknown /admin/* paths —
            // the SPA's client-side router then handles deep links and refreshes.
            let serve = tower_http::services::ServeDir::new(&admin_dir)
                .fallback(tower_http::services::ServeFile::new(admin_index));
            app = app.nest_service("/admin", serve);
            info!("   ✓ Admin UI served at /admin (dir: {})", admin_dir);
        } else {
            info!(
                "   • Admin UI not bundled ({}/index.html absent); /admin disabled",
                admin_dir
            );
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
                    info!(
                        "🔄 {} changed — exiting to reload (restart policy relaunches)",
                        path
                    );
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
