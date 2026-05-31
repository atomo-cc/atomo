//! Core client for database operations and event streaming

use anyhow::Result;
use serde_json::Value;
use sqlx::{postgres::PgArguments, Arguments, Column, PgPool, Row, TypeInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::event_store::EventStore;
use crate::events::{EventType, ModelEvent};
use crate::query::sql_builder::SqlBuilder;
use crate::query::{OrderDirection, WhereClause};
use crate::schema::Schema;

/// Scope where clauses by tenant_id
pub fn scope_by_tenant(where_clauses: &[WhereClause], tenant_id: Option<&str>) -> Vec<WhereClause> {
    let mut clauses = where_clauses.to_vec();
    if let Some(tid) = tenant_id {
        clauses.push(WhereClause {
            field: "tenant_id".to_string(),
            operator: crate::query::WhereOperator::Equals,
            value: Value::String(tid.to_string()),
        });
    }
    clauses
}

/// Core Atomo client that handles all database operations
#[derive(Clone)]
pub struct AtomoClient {
    pool: PgPool,
    schema: Schema,
    event_sender: broadcast::Sender<ModelEvent>,
    event_store: EventStore,
    embedding_store: Option<std::sync::Arc<crate::ai::EmbeddingStore>>,
    hook_runner: Arc<dyn crate::hooks::HookRunner>,
    cache: crate::cache::ReadCache,
}

impl AtomoClient {
    /// Get the database connection pool
    pub fn db_pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get the embedding store (if AI is enabled)
    pub fn embedding_store(&self) -> Option<&crate::ai::EmbeddingStore> {
        self.embedding_store.as_deref()
    }

    pub async fn new(schema: &Schema) -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/atomo".to_string());

        println!("Connecting to database: {}", database_url);
        let pool = PgPool::connect(&database_url).await?;
        let (event_sender, _) = broadcast::channel(1000);

        // Auto-create tables
        let migrations = crate::schema::generate_migrations(schema)?;
        for sql in &migrations {
            sqlx::query(sql).execute(&pool).await.ok();
        }

        let event_store = EventStore::new(pool.clone());
        event_store.init().await?;

        Ok(Self {
            pool,
            schema: schema.clone(),
            event_sender,
            event_store,
            embedding_store: None,
            hook_runner: Arc::new(crate::hooks::NoopHookRunner),
            cache: crate::cache::ReadCache::new(60),
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
        // Include limit/offset in the key — otherwise two queries that differ ONLY in pagination
        // collide and the second returns the first's cached rows (page 2 == page 1). Bug caught
        // by the CRM dogfood (orderBy + offset).
        let cache_key = crate::cache::ReadCache::key(
            model_name,
            &format!("{:?}{:?}{:?}{:?}", where_clauses, order_by, limit, offset),
        );
        if let Some(cached) = self.cache.get(&cache_key).await {
            if let Ok(records) = serde_json::from_value(cached) {
                return Ok(records);
            }
        }
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let (sql, params) =
            SqlBuilder::select_active(model, where_clauses, order_by, limit, offset);
        let args = build_args(&params)?;
        let rows = sqlx::query_with(&sql, args).fetch_all(&self.pool).await?;
        let mut records: Vec<HashMap<String, Value>> = rows.iter().map(row_to_map).collect();
        if !include.is_empty() {
            for record in &mut records {
                self.resolve_includes(model_name, record, include).await?;
            }
        }
        self.cache
            .set(&cache_key, serde_json::to_value(&records)?)
            .await;
        Ok(records)
    }

    /// Find only soft-deleted records (the "trash" view). Not cached.
    pub async fn find_deleted(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
        order_by: &[(String, OrderDirection)],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<HashMap<String, Value>>> {
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let (sql, params) =
            SqlBuilder::select_deleted(model, where_clauses, order_by, limit, offset);
        let args = build_args(&params)?;
        let rows = sqlx::query_with(&sql, args).fetch_all(&self.pool).await?;
        Ok(rows.iter().map(row_to_map).collect())
    }

    /// Find a unique record
    pub async fn find_unique(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
        include: &[String],
    ) -> Result<Option<HashMap<String, Value>>> {
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let mut clauses = where_clauses.to_vec();
        clauses.push(WhereClause {
            field: "deleted_at".to_string(),
            operator: crate::query::WhereOperator::IsNull,
            value: Value::Null,
        });
        let (sql, params) = SqlBuilder::select_one(model, &clauses);
        let args = build_args(&params)?;
        let row = sqlx::query_with(&sql, args)
            .fetch_optional(&self.pool)
            .await?;
        let mut record = row.as_ref().map(row_to_map);
        if !include.is_empty() {
            if let Some(ref mut rec) = record {
                self.resolve_includes(model_name, rec, include).await?;
            }
        }
        Ok(record)
    }

    /// Create a new record
    pub async fn create(
        &self,
        model_name: &str,
        data: &HashMap<String, Value>,
        _include: &[String],
        actor: Option<&str>,
    ) -> Result<HashMap<String, Value>> {
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;

        let hook_ctx = crate::hooks::HookContext {
            model_name: model_name.to_string(),
            operation: "create".to_string(),
            data: data.clone(),
            user_id: None,
        };
        let hook_result = self
            .hook_runner
            .run_before("before_create", &hook_ctx)
            .await?;
        let data = match hook_result {
            crate::hooks::HookResult::Continue(d) => d,
            crate::hooks::HookResult::Abort(msg) => return Err(anyhow::anyhow!(msg)),
        };

        // Enforce declared validation rules in the data layer so EVERY create path (SDK,
        // internal callers, GraphQL) is validated — not just the GraphQL resolver. Runs after
        // before_create so hooks can normalize first. Only the model's explicit rules apply.
        if !model.validation.is_empty() {
            let errors = crate::validation::validate(&data, &model.validation);
            if let Some(e) = errors.first() {
                return Err(anyhow::anyhow!("validation failed: {}", e.message));
            }
        }

        let (sql, params) = SqlBuilder::insert(model, &data);
        let args = build_args(&params)?;
        let row = sqlx::query_with(&sql, args).fetch_one(&self.pool).await?;
        let record = row_to_map(&row);

        let event = ModelEvent {
            event_type: EventType::Created,
            model_name: model_name.to_string(),
            data: record.clone(),
            previous_data: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            event_id: uuid::Uuid::new_v4().to_string(),
            actor: actor.map(|s| s.to_string()),
        };
        let _ = self.event_sender.send(event.clone());
        self.event_store.persist(&event).await.ok();

        self.hook_runner
            .run_after("after_create", &hook_ctx)
            .await
            .ok();
        self.cache.invalidate_model(model_name).await;

        Ok(record)
    }

    /// Update many records
    pub async fn update_many(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
        data: &HashMap<String, Value>,
        _include: &[String],
        actor: Option<&str>,
    ) -> Result<Vec<HashMap<String, Value>>> {
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;

        let hook_ctx = crate::hooks::HookContext {
            model_name: model_name.to_string(),
            operation: "update".to_string(),
            data: data.clone(),
            user_id: None,
        };
        let hook_result = self
            .hook_runner
            .run_before("before_update", &hook_ctx)
            .await?;
        let data = match hook_result {
            crate::hooks::HookResult::Continue(d) => d,
            crate::hooks::HookResult::Abort(msg) => return Err(anyhow::anyhow!(msg)),
        };

        // Update-aware validation: only the fields present in this patch are checked, so a
        // partial update never trips `required` on an omitted field, but a field being set must
        // still satisfy its rules. Enforced in the data layer (every update path), not just GraphQL.
        if !model.validation.is_empty() {
            let errors = crate::validation::validate_partial(&data, &model.validation);
            if let Some(e) = errors.first() {
                return Err(anyhow::anyhow!("validation failed: {}", e.message));
            }
        }

        let (sql, params) = SqlBuilder::update(model, where_clauses, &data);
        let args = build_args(&params)?;
        let rows = sqlx::query_with(&sql, args).fetch_all(&self.pool).await?;
        let records: Vec<HashMap<String, Value>> = rows.iter().map(row_to_map).collect();

        for record in &records {
            let event = ModelEvent {
                event_type: EventType::Updated,
                model_name: model_name.to_string(),
                data: record.clone(),
                previous_data: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                event_id: uuid::Uuid::new_v4().to_string(),
                actor: actor.map(|s| s.to_string()),
            };
            let _ = self.event_sender.send(event.clone());
            self.event_store.persist(&event).await.ok();
        }

        self.hook_runner
            .run_after("after_update", &hook_ctx)
            .await
            .ok();
        self.cache.invalidate_model(model_name).await;

        Ok(records)
    }

    /// Delete many records (soft delete - sets deleted_at = NOW())
    pub async fn delete_many(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
        actor: Option<&str>,
    ) -> Result<usize> {
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;

        let hook_ctx = crate::hooks::HookContext {
            model_name: model_name.to_string(),
            operation: "delete".to_string(),
            data: HashMap::new(),
            user_id: None,
        };
        let hook_result = self
            .hook_runner
            .run_before("before_delete", &hook_ctx)
            .await?;
        match hook_result {
            crate::hooks::HookResult::Continue(_) => {}
            crate::hooks::HookResult::Abort(msg) => return Err(anyhow::anyhow!(msg)),
        };

        let (sql, params) = SqlBuilder::soft_delete(model, where_clauses);
        let args = build_args(&params)?;
        // `RETURNING id` gives us the affected rows so each Deleted event can carry its id
        // (projections/audit need it to remove the right row).
        let rows = sqlx::query_with(&sql, args).fetch_all(&self.pool).await?;
        let count = rows.len();

        for row in &rows {
            let record = row_to_map(row);
            let mut data = HashMap::new();
            if let Some(id) = record.get("id") {
                data.insert("id".to_string(), id.clone());
            }
            let event = ModelEvent {
                event_type: EventType::Deleted,
                model_name: model_name.to_string(),
                data,
                previous_data: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                event_id: uuid::Uuid::new_v4().to_string(),
                actor: actor.map(|s| s.to_string()),
            };
            let _ = self.event_sender.send(event.clone());
            self.event_store.persist(&event).await.ok();
        }

        self.hook_runner
            .run_after("after_delete", &hook_ctx)
            .await
            .ok();
        self.cache.invalidate_model(model_name).await;

        Ok(count)
    }

    /// Restore soft-deleted records (clears deleted_at). Returns affected count.
    pub async fn restore_many(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
    ) -> Result<usize> {
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let (sql, params) = SqlBuilder::restore(model, where_clauses);
        let args = build_args(&params)?;
        let result = sqlx::query_with(&sql, args).execute(&self.pool).await?;
        self.cache.invalidate_model(model_name).await;
        Ok(result.rows_affected() as usize)
    }

    /// Permanently delete records (hard delete - row is removed). Returns affected count.
    pub async fn hard_delete_many(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
    ) -> Result<usize> {
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let (sql, params) = SqlBuilder::delete(model, where_clauses);
        let args = build_args(&params)?;
        let result = sqlx::query_with(&sql, args).execute(&self.pool).await?;
        self.cache.invalidate_model(model_name).await;
        Ok(result.rows_affected() as usize)
    }

    /// Count records matching where clauses
    pub async fn count(&self, model_name: &str, where_clauses: &[WhereClause]) -> Result<i64> {
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let table = crate::query::sql_builder::table_name_for(model);
        let mut clauses = where_clauses.to_vec();
        clauses.push(WhereClause {
            field: "deleted_at".to_string(),
            operator: crate::query::WhereOperator::IsNull,
            value: serde_json::Value::Null,
        });
        let (where_sql, params) = crate::query::sql_builder::build_where_pub(&clauses, 0);
        let sql = if where_sql.is_empty() {
            format!("SELECT COUNT(*) as count FROM {}", table)
        } else {
            format!(
                "SELECT COUNT(*) as count FROM {} WHERE {}",
                table, where_sql
            )
        };
        let args = build_args(&params)?;
        let row = sqlx::query_with(&sql, args).fetch_one(&self.pool).await?;
        Ok(row.try_get::<i64, _>("count").unwrap_or(0))
    }

    /// Count only soft-deleted records (for the trash view's total).
    pub async fn count_deleted(&self, model_name: &str, where_clauses: &[WhereClause]) -> Result<i64> {
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let table = crate::query::sql_builder::table_name_for(model);
        let mut clauses = where_clauses.to_vec();
        clauses.push(WhereClause {
            field: "deleted_at".to_string(),
            operator: crate::query::WhereOperator::IsNotNull,
            value: serde_json::Value::Null,
        });
        let (where_sql, params) = crate::query::sql_builder::build_where_pub(&clauses, 0);
        let sql = format!("SELECT COUNT(*) as count FROM {} WHERE {}", table, where_sql);
        let args = build_args(&params)?;
        let row = sqlx::query_with(&sql, args).fetch_one(&self.pool).await?;
        Ok(row.try_get::<i64, _>("count").unwrap_or(0))
    }

    /// Subscribe to model events
    pub async fn subscribe(
        &self,
        _model_name: &str,
        _event_types: &[EventType],
        _where_clauses: &[WhereClause],
    ) -> broadcast::Receiver<ModelEvent> {
        self.event_sender.subscribe()
    }

    /// Get a broadcast receiver for all model events (for projectors/workflows)
    pub fn event_receiver(&self) -> broadcast::Receiver<ModelEvent> {
        self.event_sender.subscribe()
    }

    /// The loaded schema (used by the subscription resolver to gate by access rules).
    pub fn schema(&self) -> &crate::schema::Schema {
        &self.schema
    }

    /// Get a clonable sender to publish events onto the model-event stream
    /// (used to surface plugin-emitted events to projectors/audit/subscriptions).
    pub fn event_sender(&self) -> broadcast::Sender<ModelEvent> {
        self.event_sender.clone()
    }

    /// Resolve relationships for a record based on include list
    pub fn resolve_includes<'a>(
        &'a self,
        model_name: &'a str,
        record: &'a mut HashMap<String, Value>,
        include: &'a [String],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if !self.schema.models.contains_key(model_name) {
                return Ok(());
            }
            for rel_name in include {
                let fk_field = format!("{}Id", rel_name);
                let fk_snake = to_snake_case(&fk_field);

                if let Some(fk_value) = record
                    .get(&fk_snake)
                    .or_else(|| record.get(&fk_field))
                    .cloned()
                {
                    // belongsTo: fetch the related record by ID
                    if let Value::String(id) = fk_value {
                        let related_model = capitalize(rel_name);
                        let where_clause = WhereClause {
                            field: "id".to_string(),
                            operator: crate::query::WhereOperator::Equals,
                            value: Value::String(id),
                        };
                        if let Ok(Some(related)) =
                            self.find_unique(&related_model, &[where_clause], &[]).await
                        {
                            record.insert(
                                rel_name.clone(),
                                serde_json::to_value(related).unwrap_or(Value::Null),
                            );
                        }
                    }
                } else {
                    // hasMany: fetch records from related model where foreignKey = this record's id
                    if let Some(Value::String(id)) = record.get("id").cloned() {
                        let related_model = capitalize(rel_name);
                        let fk = format!("{}_id", to_snake_case(model_name));
                        let where_clause = WhereClause {
                            field: fk,
                            operator: crate::query::WhereOperator::Equals,
                            value: Value::String(id),
                        };
                        let singular = if related_model.ends_with('s') {
                            related_model[..related_model.len() - 1].to_string()
                        } else {
                            related_model.clone()
                        };
                        let model_to_query = if self.schema.models.contains_key(&singular) {
                            &singular
                        } else {
                            &related_model
                        };
                        if let Ok(related) = self
                            .find_many(model_to_query, &[where_clause], &[], None, None, &[])
                            .await
                        {
                            record.insert(
                                rel_name.clone(),
                                serde_json::to_value(related).unwrap_or(Value::Null),
                            );
                        }
                    }
                }
            }
            Ok(())
        })
    }
}

/// Builder for AtomoClient
pub struct AtomoClientBuilder {
    database_url: Option<String>,
    enable_migrations: bool,
    enable_ai: bool,
    hook_runner: Option<Arc<dyn crate::hooks::HookRunner>>,
}

impl Default for AtomoClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomoClientBuilder {
    pub fn new() -> Self {
        Self {
            database_url: None,
            enable_migrations: true,
            enable_ai: false,
            hook_runner: None,
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

    pub fn hook_runner(mut self, runner: Arc<dyn crate::hooks::HookRunner>) -> Self {
        self.hook_runner = Some(runner);
        self
    }

    pub async fn build(self, schema: &Schema) -> Result<AtomoClient> {
        let database_url = self.database_url.unwrap_or_else(|| {
            std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost/atomo".to_string())
        });

        println!("Connecting to database: {}", database_url);
        let pool = PgPool::connect(&database_url).await?;
        let (event_sender, _) = broadcast::channel(1000);
        let event_store = EventStore::new(pool.clone());
        event_store.init().await?;

        // Run migrations if enabled
        if self.enable_migrations {
            let migrations = crate::schema::generate_migrations(schema)?;
            for sql in &migrations {
                sqlx::query(sql).execute(&pool).await.ok();
            }
        }

        let embedding_store = if self.enable_ai {
            let store = crate::ai::EmbeddingStore::new(pool.clone());
            store.init().await.ok(); // Don't fail if pgvector not installed
            Some(std::sync::Arc::new(store))
        } else {
            None
        };

        Ok(AtomoClient {
            pool,
            schema: schema.clone(),
            event_sender,
            event_store,
            embedding_store,
            hook_runner: self
                .hook_runner
                .unwrap_or_else(|| Arc::new(crate::hooks::NoopHookRunner)),
            cache: crate::cache::ReadCache::new(60),
        })
    }
}

/// Build PgArguments from a Vec of serde_json::Value
fn build_args(params: &[Value]) -> Result<PgArguments> {
    let mut args = PgArguments::default();
    for p in params {
        match p {
            // Bind all strings as text. WHERE comparisons cast the column to text
            // (e.g. `id::text = $1`) so this works for both TEXT and UUID columns.
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
            // Arrays/objects bind as native JSON so JSONB columns accept them.
            other => args.add(other.clone()),
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
            "TEXT" | "VARCHAR" | "CHAR" | "NAME" | "BPCHAR" => row
                .try_get::<Option<String>, _>(col.ordinal())
                .ok()
                .flatten()
                .map(Value::String)
                .unwrap_or(Value::Null),
            "INT2" | "INT4" | "INT8" => row
                .try_get::<Option<i64>, _>(col.ordinal())
                .ok()
                .flatten()
                .map(|n| Value::Number(n.into()))
                .unwrap_or(Value::Null),
            "FLOAT4" | "FLOAT8" => row
                .try_get::<Option<f64>, _>(col.ordinal())
                .ok()
                .flatten()
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            "BOOL" => row
                .try_get::<Option<bool>, _>(col.ordinal())
                .ok()
                .flatten()
                .map(Value::Bool)
                .unwrap_or(Value::Null),
            "UUID" => row
                .try_get::<Option<uuid::Uuid>, _>(col.ordinal())
                .ok()
                .flatten()
                .map(|u| Value::String(u.to_string()))
                .unwrap_or(Value::Null),
            "TIMESTAMPTZ" | "TIMESTAMP" => row
                .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(col.ordinal())
                .ok()
                .flatten()
                .map(|t| Value::String(t.to_rfc3339()))
                .unwrap_or(Value::Null),
            "JSONB" | "JSON" => row
                .try_get::<Option<Value>, _>(col.ordinal())
                .ok()
                .flatten()
                .unwrap_or(Value::Null),
            _ => row
                .try_get::<Option<String>, _>(col.ordinal())
                .ok()
                .flatten()
                .map(Value::String)
                .unwrap_or(Value::Null),
        };
        map.insert(name, val);
    }
    map
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        if c.is_uppercase() && !result.is_empty() {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
