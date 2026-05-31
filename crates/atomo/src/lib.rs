//! # Atomo - The Content Core
//!
//! Atomo is a next-generation Content Core that serves as the "Arc Reactor"
//! of your business - understanding, generating, and automating content flows.
//!
//! ## Usage as a Library
//!
//! ```text
//! use atomo::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Method 1: From schema file (similar to Prisma)
//!     let atomo = Atomo::from_schema("./schema.ts").await?;
//!     
//!     // Method 2: From embedded schema
//!     let atomo = Atomo::builder()
//!         .database_url("postgresql://...")
//!         .schema_content(include_str!("../schema.ts"))
//!         .build()
//!         .await?;
//!     
//!     // Fluent Query API (like Prisma)
//!     let posts = atomo
//!         .model("Post")
//!         .find_many()
//!         .where_("title", Contains("Rust"))
//!         .include("author")
//!         .order_by("created_at", Desc)
//!         .limit(10)
//!         .await?;
//!     
//!     // Create with type safety
//!     let new_post = atomo
//!         .model("Post")
//!         .create()
//!         .data(PostCreateData {
//!             title: "My First Post".to_string(),
//!             content: Some("Hello World!".to_string()),
//!             ..Default::default()
//!         })
//!         .await?;
//!     
//!     // Real-time subscriptions
//!     let mut post_stream = atomo
//!         .model("Post")
//!         .subscribe()
//!         .on_create()
//!         .where_("published", Equals(true));
//!
//!     while let Some(event) = post_stream.next().await {
//!         println!("New post created: {:?}", event.data);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## GraphQL Integration
//!
//! ```text
//! use atomo::prelude::*;
//! use axum::{Router, response::Json};
//!
//! async fn create_server() -> Router {
//!     let atomo = Atomo::from_schema("./schema.ts").await.unwrap();
//!     
//!     // Automatic GraphQL schema generation
//!     let graphql_schema = atomo.graphql_schema();
//!     
//!     Router::new()
//!         .route("/graphql", post(graphql_handler))
//!         .with_state(atomo)
//! }
//! ```
#![allow(dead_code)]

use anyhow::Result;
use std::sync::Arc;

pub mod ai;
pub mod cache;
pub mod client;
pub mod errors;
pub mod event_store;
pub mod events;
pub mod graphql;
pub mod hooks;
pub mod prelude;
pub mod query;
pub mod schema;
pub mod validation;
pub mod workflow;

use client::AtomoClient;
use schema::Schema;

/// The main Atomo instance - your Content Core
#[derive(Clone)]
pub struct Atomo {
    client: Arc<AtomoClient>,
    schema: Schema,
}

impl Atomo {
    /// Create Atomo instance from schema file (Prisma-like)
    pub async fn from_schema(schema_path: impl AsRef<str>) -> Result<Self> {
        let schema_content = tokio::fs::read_to_string(schema_path.as_ref()).await?;
        Self::from_schema_content(&schema_content).await
    }

    /// Create Atomo instance from schema content
    pub async fn from_schema_content(schema_content: &str) -> Result<Self> {
        let schema = schema::parse_typescript_schema(schema_content)?;
        let client = AtomoClient::new(&schema).await?;

        Ok(Self {
            client: Arc::new(client),
            schema,
        })
    }

    /// Create a builder for more advanced configuration
    pub fn builder() -> AtomoBuilder {
        AtomoBuilder::new()
    }

    /// Get a model client for type-safe operations
    pub fn model(&self, model_name: &str) -> ModelClient {
        ModelClient::new(self.client.clone(), model_name, &self.schema)
    }

    /// Get the client for direct database access (internal use)
    pub fn client(&self) -> &AtomoClient {
        &self.client
    }

    /// Get the schema definition (internal use)
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Generate GraphQL schema for integration with platform models
    pub fn graphql_schema(
        &self,
    ) -> async_graphql::Schema<graphql::Query, graphql::Mutation, graphql::Subscription> {
        graphql::build_schema(self.client.clone(), &self.schema, self.db_pool().clone())
    }

    /// Get the database connection pool for direct SQL operations
    pub fn db_pool(&self) -> &sqlx::PgPool {
        self.client.db_pool()
    }

    /// Get a broadcast receiver for model events (for projectors/workflows)
    pub fn event_receiver(&self) -> tokio::sync::broadcast::Receiver<events::ModelEvent> {
        self.client.event_receiver()
    }
}

/// Builder for Atomo configuration
pub struct AtomoBuilder {
    database_url: Option<String>,
    schema_content: Option<String>,
    schema_file: Option<String>,
    enable_migrations: bool,
    enable_ai: bool,
    hook_runner: Option<Arc<dyn hooks::HookRunner>>,
}

impl Default for AtomoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomoBuilder {
    pub fn new() -> Self {
        Self {
            database_url: None,
            schema_content: None,
            schema_file: None,
            enable_migrations: true,
            enable_ai: false,
            hook_runner: None,
        }
    }

    /// Set a custom hook runner (e.g. WASM plugin bridge)
    pub fn hook_runner(mut self, runner: Arc<dyn hooks::HookRunner>) -> Self {
        self.hook_runner = Some(runner);
        self
    }

    pub fn database_url(mut self, url: impl Into<String>) -> Self {
        self.database_url = Some(url.into());
        self
    }

    pub fn schema_content(mut self, content: impl Into<String>) -> Self {
        self.schema_content = Some(content.into());
        self
    }

    pub fn schema_file(mut self, path: impl Into<String>) -> Self {
        self.schema_file = Some(path.into());
        self
    }

    pub fn enable_migrations(mut self, enable: bool) -> Self {
        self.enable_migrations = enable;
        self
    }

    pub fn enable_ai(mut self, enable: bool) -> Self {
        self.enable_ai = enable;
        self
    }

    pub async fn build(self) -> Result<Atomo> {
        let schema_content = match (self.schema_content, self.schema_file) {
            (Some(content), _) => content,
            (None, Some(file)) => tokio::fs::read_to_string(file).await?,
            (None, None) => {
                return Err(anyhow::anyhow!(
                    "Either schema_content or schema_file must be provided"
                ))
            }
        };

        let schema = schema::parse_typescript_schema(&schema_content)?;
        let mut client_builder = AtomoClient::builder();

        if let Some(url) = self.database_url {
            client_builder = client_builder.database_url(url);
        }
        if let Some(runner) = self.hook_runner {
            client_builder = client_builder.hook_runner(runner);
        }

        let client = client_builder
            .enable_migrations(self.enable_migrations)
            .enable_ai(self.enable_ai)
            .build(&schema)
            .await?;

        Ok(Atomo {
            client: Arc::new(client),
            schema,
        })
    }
}

/// Model-specific client for type-safe operations
pub struct ModelClient {
    client: Arc<AtomoClient>,
    model_name: String,
    schema: Schema,
}

impl ModelClient {
    pub fn new(client: Arc<AtomoClient>, model_name: &str, schema: &Schema) -> Self {
        Self {
            client,
            model_name: model_name.to_string(),
            schema: schema.clone(),
        }
    }

    /// Start a find query
    pub fn find_many(&self) -> query::FindManyQuery {
        query::FindManyQuery::new(self.model_name.clone())
    }

    /// Find a single record by ID
    pub fn find_unique(&self) -> query::FindUniqueQuery {
        query::FindUniqueQuery::new(self.model_name.clone())
    }

    /// Create a new record
    pub fn create(&self) -> query::CreateQuery {
        query::CreateQuery::new(self.model_name.clone())
    }

    /// Update records
    pub fn update(&self) -> query::UpdateQuery {
        query::UpdateQuery::new(self.model_name.clone())
    }

    /// Delete records
    pub fn delete(&self) -> query::DeleteQuery {
        query::DeleteQuery::new(self.model_name.clone())
    }

    /// Subscribe to real-time events
    pub fn subscribe(&self) -> events::SubscriptionBuilder {
        events::SubscriptionBuilder::new(self.client.clone(), &self.model_name)
    }
}
