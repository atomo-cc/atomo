//! Atomo Server implementation using the new library API

use anyhow::Result;
use axum::serve;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{info, instrument};
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
        // Initialize tracing
        tracing_subscriber::fmt::init();

        info!("🚀 Starting Atomo Content Core Server");
        info!("   Host: {}", self.config.host);
        info!("   Port: {}", self.config.port);
        info!("   Database: {}", self.config.database_url);

        // Generate GraphQL schema from Atomo
        let graphql_schema = self.atomo.graphql_schema();
        info!("   ✓ GraphQL schema generated from TypeScript schema");

        // Create router with Atomo context
        let app = create_router(graphql_schema, self.atomo)
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(
                        CorsLayer::new()
                            .allow_origin(Any)
                            .allow_headers(Any)
                            .allow_methods(Any),
                    ),
            );

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
