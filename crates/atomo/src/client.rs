//! Core client for database operations and event streaming

use std::collections::HashMap;
use anyhow::Result;
use serde_json::Value;
use tokio::sync::broadcast;
use sqlx::PgPool;

use crate::schema::Schema;
use crate::query::{WhereClause, OrderDirection};
use crate::events::{EventType, ModelEvent};

/// Core Atomo client that handles all database operations
pub struct AtomoClient {
    pool: PgPool,
    schema: Schema,
    event_sender: broadcast::Sender<ModelEvent>,
}

impl AtomoClient {
    pub async fn new(schema: &Schema) -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/atomo".to_string());
        
        println!("Connecting to database: {}", database_url);
        let pool = PgPool::connect(&database_url).await?;
        let (event_sender, _) = broadcast::channel(1000);
        
        Ok(Self {
            pool,
            schema: schema.clone(),
            event_sender,
        })
    }
    
    pub fn builder() -> AtomoClientBuilder {
        AtomoClientBuilder::new()
    }
    
    /// Find many records
    pub async fn find_many(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
        order_by: &[(String, OrderDirection)],
        limit: Option<usize>,
        offset: Option<usize>,
        include: &[String],
    ) -> Result<Vec<HashMap<String, Value>>> {
        // TODO: Build and execute SQL query based on schema
        // This is a simplified implementation
        Ok(vec![])
    }
    
    /// Find a unique record
    pub async fn find_unique(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
        include: &[String],
    ) -> Result<Option<HashMap<String, Value>>> {
        // TODO: Build and execute SQL query
        Ok(None)
    }
    
    /// Create a new record
    pub async fn create(
        &self,
        model_name: &str,
        data: &HashMap<String, Value>,
        include: &[String],
    ) -> Result<HashMap<String, Value>> {
        // TODO: Insert record and publish event
        let record = HashMap::new(); // Placeholder
        
        // Publish create event
        let event = ModelEvent {
            event_type: EventType::Created,
            model_name: model_name.to_string(),
            data: record.clone(),
            previous_data: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            event_id: uuid::Uuid::new_v4().to_string(),
        };
        
        let _ = self.event_sender.send(event);
        
        Ok(record)
    }
    
    /// Update many records
    pub async fn update_many(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
        data: &HashMap<String, Value>,
        include: &[String],
    ) -> Result<Vec<HashMap<String, Value>>> {
        // TODO: Update records and publish events
        Ok(vec![])
    }
    
    /// Delete many records
    pub async fn delete_many(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
    ) -> Result<usize> {
        // TODO: Delete records and publish events
        Ok(0)
    }
    
    /// Subscribe to model events
    pub async fn subscribe(
        &self,
        model_name: &str,
        event_types: &[EventType],
        where_clauses: &[WhereClause],
    ) -> broadcast::Receiver<ModelEvent> {
        self.event_sender.subscribe()
    }
}

/// Builder for AtomoClient
pub struct AtomoClientBuilder {
    database_url: Option<String>,
    enable_migrations: bool,
    enable_ai: bool,
}

impl AtomoClientBuilder {
    pub fn new() -> Self {
        Self {
            database_url: None,
            enable_migrations: true,
            enable_ai: false,
        }
    }
    
    pub fn database_url(mut self, url: impl Into<String>) -> Self {
        self.database_url = Some(url.into());
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
    
    pub async fn build(self, schema: &Schema) -> Result<AtomoClient> {
        let database_url = self.database_url
            .unwrap_or_else(|| std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost/atomo".to_string()));
        
        println!("Connecting to database: {}", database_url);
        let pool = PgPool::connect(&database_url).await?;
        let (event_sender, _) = broadcast::channel(1000);
        
        // Run migrations if enabled
        if self.enable_migrations {
            // TODO: Run database migrations based on schema
            println!("Migrations enabled but not yet implemented");
        }
        
        Ok(AtomoClient {
            pool,
            schema: schema.clone(),
            event_sender,
        })
    }
}
