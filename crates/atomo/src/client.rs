//! Core client for database operations and event streaming

use std::collections::HashMap;
use anyhow::Result;
use serde_json::Value;
use tokio::sync::broadcast;
use sqlx::{PgPool, Row, Column, TypeInfo, postgres::PgArguments, Arguments};

use crate::schema::Schema;
use crate::query::{WhereClause, OrderDirection};
use crate::query::sql_builder::SqlBuilder;
use crate::events::{EventType, ModelEvent};

/// Core Atomo client that handles all database operations
#[derive(Clone)]
pub struct AtomoClient {
    pool: PgPool,
    schema: Schema,
    event_sender: broadcast::Sender<ModelEvent>,
}

impl AtomoClient {
    /// Get the database connection pool
    pub fn db_pool(&self) -> &PgPool {
        &self.pool
    }

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
        _include: &[String],
    ) -> Result<Vec<HashMap<String, Value>>> {
        let model = self.schema.models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let (sql, params) = SqlBuilder::select(model, where_clauses, order_by, limit, offset);
        let args = build_args(&params)?;
        let rows = sqlx::query_with(&sql, args).fetch_all(&self.pool).await?;
        Ok(rows.iter().map(row_to_map).collect())
    }
    
    /// Find a unique record
    pub async fn find_unique(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
        _include: &[String],
    ) -> Result<Option<HashMap<String, Value>>> {
        let model = self.schema.models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let (sql, params) = SqlBuilder::select_one(model, where_clauses);
        let args = build_args(&params)?;
        let row = sqlx::query_with(&sql, args).fetch_optional(&self.pool).await?;
        Ok(row.as_ref().map(row_to_map))
    }
    
    /// Create a new record
    pub async fn create(
        &self,
        model_name: &str,
        data: &HashMap<String, Value>,
        _include: &[String],
    ) -> Result<HashMap<String, Value>> {
        let model = self.schema.models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let (sql, params) = SqlBuilder::insert(model, data);
        let args = build_args(&params)?;
        let row = sqlx::query_with(&sql, args).fetch_one(&self.pool).await?;
        let record = row_to_map(&row);

        let _ = self.event_sender.send(ModelEvent {
            event_type: EventType::Created,
            model_name: model_name.to_string(),
            data: record.clone(),
            previous_data: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            event_id: uuid::Uuid::new_v4().to_string(),
        });

        Ok(record)
    }
    
    /// Update many records
    pub async fn update_many(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
        data: &HashMap<String, Value>,
        _include: &[String],
    ) -> Result<Vec<HashMap<String, Value>>> {
        let model = self.schema.models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let (sql, params) = SqlBuilder::update(model, where_clauses, data);
        let args = build_args(&params)?;
        let rows = sqlx::query_with(&sql, args).fetch_all(&self.pool).await?;
        let records: Vec<HashMap<String, Value>> = rows.iter().map(row_to_map).collect();

        for record in &records {
            let _ = self.event_sender.send(ModelEvent {
                event_type: EventType::Updated,
                model_name: model_name.to_string(),
                data: record.clone(),
                previous_data: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                event_id: uuid::Uuid::new_v4().to_string(),
            });
        }

        Ok(records)
    }
    
    /// Delete many records
    pub async fn delete_many(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
    ) -> Result<usize> {
        let model = self.schema.models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let (sql, params) = SqlBuilder::delete(model, where_clauses);
        let args = build_args(&params)?;
        let result = sqlx::query_with(&sql, args).execute(&self.pool).await?;
        let count = result.rows_affected() as usize;

        if count > 0 {
            let _ = self.event_sender.send(ModelEvent {
                event_type: EventType::Deleted,
                model_name: model_name.to_string(),
                data: HashMap::new(),
                previous_data: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                event_id: uuid::Uuid::new_v4().to_string(),
            });
        }

        Ok(count)
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

/// Build PgArguments from a Vec of serde_json::Value
fn build_args(params: &[Value]) -> Result<PgArguments> {
    let mut args = PgArguments::default();
    for p in params {
        match p {
            Value::String(s) => args.add(s.as_str()),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    args.add(i);
                } else {
                    args.add(n.as_f64().unwrap_or(0.0));
                }
            }
            Value::Bool(b) => args.add(*b),
            Value::Null => args.add(None::<String>),
            _ => args.add(p.to_string()),
        }
    }
    Ok(args)
}

/// Convert a PgRow to HashMap<String, Value> by inspecting column types
fn row_to_map(row: &sqlx::postgres::PgRow) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    for col in row.columns() {
        let name = col.name().to_string();
        let type_name = col.type_info().name();
        let val = match type_name {
            "TEXT" | "VARCHAR" | "CHAR" | "NAME" | "BPCHAR" => {
                row.try_get::<Option<String>, _>(col.ordinal())
                    .ok().flatten().map(Value::String).unwrap_or(Value::Null)
            }
            "INT2" | "INT4" | "INT8" => {
                row.try_get::<Option<i64>, _>(col.ordinal())
                    .ok().flatten().map(|n| Value::Number(n.into())).unwrap_or(Value::Null)
            }
            "FLOAT4" | "FLOAT8" => {
                row.try_get::<Option<f64>, _>(col.ordinal())
                    .ok().flatten()
                    .and_then(|f| serde_json::Number::from_f64(f))
                    .map(Value::Number).unwrap_or(Value::Null)
            }
            "BOOL" => {
                row.try_get::<Option<bool>, _>(col.ordinal())
                    .ok().flatten().map(Value::Bool).unwrap_or(Value::Null)
            }
            "UUID" => {
                row.try_get::<Option<uuid::Uuid>, _>(col.ordinal())
                    .ok().flatten().map(|u| Value::String(u.to_string())).unwrap_or(Value::Null)
            }
            "TIMESTAMPTZ" | "TIMESTAMP" => {
                row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(col.ordinal())
                    .ok().flatten().map(|t| Value::String(t.to_rfc3339())).unwrap_or(Value::Null)
            }
            "JSONB" | "JSON" => {
                row.try_get::<Option<Value>, _>(col.ordinal())
                    .ok().flatten().unwrap_or(Value::Null)
            }
            _ => {
                row.try_get::<Option<String>, _>(col.ordinal())
                    .ok().flatten().map(Value::String).unwrap_or(Value::Null)
            }
        };
        map.insert(name, val);
    }
    map
}
