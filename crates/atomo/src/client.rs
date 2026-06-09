//! Core client for database operations and event streaming

use anyhow::Result;
use serde_json::Value;
use sqlx::{postgres::PgArguments, postgres::PgPoolOptions, Arguments, Column, PgPool, Row, TypeInfo};
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

// --- RLS request-tenant scope (Phase 3 wiring) ------------------------------
// A task-local holding the current request's tenant. Set once at the request
// boundary via [`with_tenant_scope`]; read by the RLS-scoped execution helpers so
// every query in that request runs in a transaction that binds `atomo.tenant_id`.
// Non-request callers (SDK, projectors, seeding) never set it → the helpers fall
// back to the plain pool path, unchanged.
tokio::task_local! {
    static TENANT_SCOPE: Option<String>;
    static ACTION_ORIGIN: Option<String>;
}

/// Run `fut` with `tenant` bound as the request's RLS tenant scope.
///
/// When `ATOMO_ENABLE_RLS` is on, query execution within `fut` wraps each statement
/// in a transaction that `SET LOCAL`s `atomo.tenant_id` to this value, so the
/// generated RLS policies resolve to this tenant. Safe no-op when RLS is off or
/// `tenant` is `None` (the scope is simply never read). `SET LOCAL` is
/// transaction-scoped, so it is safe under PgBouncer transaction pooling.
pub async fn with_tenant_scope<F: std::future::Future>(
    tenant: Option<String>,
    fut: F,
) -> F::Output {
    TENANT_SCOPE.scope(tenant, fut).await
}

/// The current request's tenant scope, if one is active on this task.
fn current_tenant() -> Option<String> {
    TENANT_SCOPE.try_with(|t| t.clone()).ok().flatten()
}

/// Run `fut` with `origin` bound as the action origin for loop prevention.
///
/// When a worker calls back into CRUD, its action name is set here so the resulting
/// ModelEvents carry `origin = Some("processPost")`. The action dispatcher then skips
/// re-enqueuing the originating action, preventing infinite loops.
pub async fn with_action_origin<F: std::future::Future>(
    origin: Option<String>,
    fut: F,
) -> F::Output {
    ACTION_ORIGIN.scope(origin, fut).await
}

fn current_origin() -> Option<String> {
    ACTION_ORIGIN.try_with(|o| o.clone()).ok().flatten()
}

/// Whether DB-enforced RLS is enabled (`ATOMO_ENABLE_RLS`). Mirrors `atomo_server::rls`.
fn rls_enabled() -> bool {
    std::env::var("ATOMO_ENABLE_RLS")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

/// `SET LOCAL atomo.tenant_id = <tenant>` for the current transaction, via the
/// parameter-safe `set_config(..., is_local := true)` (no SQL-injection surface).
async fn bind_tenant_local(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant: &str,
) -> Result<()> {
    sqlx::query("SELECT set_config('atomo.tenant_id', $1, true)")
        .bind(tenant)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Build a connection pool sized by `DATABASE_POOL_MAX` (default 20).
async fn pool_from_env(url: &str) -> Result<PgPool> {
    let max = std::env::var("DATABASE_POOL_MAX")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20u32);
    Ok(PgPoolOptions::new()
        .max_connections(max)
        .connect(url)
        .await?)
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

    // --- RLS-scoped execution (Phase 3) -------------------------------------
    // When `ATOMO_ENABLE_RLS` is on AND a request tenant scope is active (set via
    // `with_tenant_scope` at the request boundary), each statement runs inside its own
    // transaction that binds `atomo.tenant_id` (SET LOCAL) so the RLS policies resolve to
    // this tenant. When RLS is off or no scope is active, the statement runs directly on
    // the pool — byte-for-byte the prior path (zero behavior change by default).

    async fn fetch_all_scoped(
        &self,
        sql: &str,
        args: PgArguments,
    ) -> Result<Vec<sqlx::postgres::PgRow>> {
        if rls_enabled() {
            if let Some(tid) = current_tenant() {
                let mut tx = self.pool.begin().await?;
                bind_tenant_local(&mut tx, &tid).await?;
                let rows = sqlx::query_with(sql, args).fetch_all(&mut *tx).await?;
                tx.commit().await?;
                return Ok(rows);
            }
        }
        Ok(sqlx::query_with(sql, args).fetch_all(&self.pool).await?)
    }

    async fn fetch_one_scoped(
        &self,
        sql: &str,
        args: PgArguments,
    ) -> Result<sqlx::postgres::PgRow> {
        if rls_enabled() {
            if let Some(tid) = current_tenant() {
                let mut tx = self.pool.begin().await?;
                bind_tenant_local(&mut tx, &tid).await?;
                let row = sqlx::query_with(sql, args).fetch_one(&mut *tx).await?;
                tx.commit().await?;
                return Ok(row);
            }
        }
        Ok(sqlx::query_with(sql, args).fetch_one(&self.pool).await?)
    }

    async fn fetch_optional_scoped(
        &self,
        sql: &str,
        args: PgArguments,
    ) -> Result<Option<sqlx::postgres::PgRow>> {
        if rls_enabled() {
            if let Some(tid) = current_tenant() {
                let mut tx = self.pool.begin().await?;
                bind_tenant_local(&mut tx, &tid).await?;
                let row = sqlx::query_with(sql, args).fetch_optional(&mut *tx).await?;
                tx.commit().await?;
                return Ok(row);
            }
        }
        Ok(sqlx::query_with(sql, args)
            .fetch_optional(&self.pool)
            .await?)
    }

    async fn execute_scoped(&self, sql: &str, args: PgArguments) -> Result<u64> {
        if rls_enabled() {
            if let Some(tid) = current_tenant() {
                let mut tx = self.pool.begin().await?;
                bind_tenant_local(&mut tx, &tid).await?;
                let r = sqlx::query_with(sql, args).execute(&mut *tx).await?;
                tx.commit().await?;
                return Ok(r.rows_affected());
            }
        }
        Ok(sqlx::query_with(sql, args)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }

    pub async fn new(schema: &Schema) -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/atomo".to_string());

        println!("Connecting to database: {}", database_url);
        let pool = pool_from_env(&database_url).await?;
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
            cache: crate::cache::ReadCache::from_env(60),
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
        // Include the active tenant scope in the cache key so a cached read can never be
        // served across tenants when RLS is on (defense-in-depth above the DB). In the
        // normal path the tenant is also in `where_clauses` via scope_by_tenant, but keying
        // on it here protects the RLS-only ("forgot the WHERE") case too. None when unscoped.
        let cache_key = crate::cache::ReadCache::key(
            model_name,
            &format!(
                "{:?}{:?}{:?}{:?}{:?}",
                where_clauses,
                order_by,
                limit,
                offset,
                current_tenant()
            ),
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
        let rows = self.fetch_all_scoped(&sql, args).await?;
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
        let rows = self.fetch_all_scoped(&sql, args).await?;
        Ok(rows.iter().map(row_to_map).collect())
    }

    /// Find a unique record. Cached (point read) under the same per-model key space as `find_many`,
    /// so a write to the model invalidates it in `strong` mode (and the TTL bounds it in `eventual`).
    pub async fn find_unique(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
        include: &[String],
    ) -> Result<Option<HashMap<String, Value>>> {
        let cache_key = crate::cache::ReadCache::key(
            model_name,
            &format!("unique:{:?}{:?}{:?}", where_clauses, include, current_tenant()),
        );
        if let Some(cached) = self.cache.get(&cache_key).await {
            if let Ok(record) = serde_json::from_value(cached) {
                return Ok(record);
            }
        }
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
        let row = self.fetch_optional_scoped(&sql, args).await?;
        let mut record = row.as_ref().map(row_to_map);
        if !include.is_empty() {
            if let Some(ref mut rec) = record {
                self.resolve_includes(model_name, rec, include).await?;
            }
        }
        self.cache
            .set(&cache_key, serde_json::to_value(&record)?)
            .await;
        Ok(record)
    }

    /// Create a new record. **System/trusted API — does NOT enforce RBAC** (no caller role).
    /// Use this only from trusted server code (seeding, migrations, internal jobs). For
    /// request-handling, call [`create_checked`](Self::create_checked) so access rules apply.
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

        // The row insert and its `event_log` write commit together in ONE transaction — a single
        // `fsync`, not two. (Previously these were two autocommit statements ⇒ two fsyncs ≈ 2× the
        // create latency on durable storage.) This also makes create **atomic**: a row is never
        // persisted without its event. The tenant bind (RLS) lives in the same tx, as before.
        let mut tx = self.pool.begin().await?;
        if rls_enabled() {
            if let Some(tid) = current_tenant() {
                bind_tenant_local(&mut tx, &tid).await?;
            }
        }
        let row = sqlx::query_with(&sql, args).fetch_one(&mut *tx).await?;
        let record = row_to_map(&row);

        let event = ModelEvent {
            event_type: EventType::Created,
            model_name: model_name.to_string(),
            data: record.clone(),
            previous_data: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            event_id: uuid::Uuid::new_v4().to_string(),
            actor: actor.map(|s| s.to_string()),
            origin: current_origin(),
        };
        self.event_store.persist_in(&mut *tx, &event).await?;
        tx.commit().await?;

        // Post-commit: in-memory fan-out + after-hook + cache invalidation (unchanged).
        let _ = self.event_sender.send(event.clone());
        self.hook_runner
            .run_after("after_create", &hook_ctx)
            .await
            .ok();
        self.cache.invalidate_model(model_name).await;

        Ok(record)
    }

    /// Insert many records in **one transaction** — one `fsync` for the whole batch instead of one
    /// per row. For bulk loads (imports, seeding) this is dramatically faster than calling
    /// [`create`](Self::create) in a loop, where each call pays its own commit. The batch is
    /// **atomic**: if any row fails (validation, constraint), the whole transaction rolls back and
    /// nothing is persisted. `before_create` hooks + validation run per record *before* the
    /// transaction (so an early reject costs no DB work); `after_create` + cache invalidation run
    /// once after commit.
    pub async fn create_many(
        &self,
        model_name: &str,
        records: &[HashMap<String, Value>],
        actor: Option<&str>,
    ) -> Result<Vec<HashMap<String, Value>>> {
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        if records.is_empty() {
            return Ok(Vec::new());
        }

        // Per-record before_create hook + validation, up front (no DB work if any rejects).
        let mut prepared = Vec::with_capacity(records.len());
        for data in records {
            let hook_ctx = crate::hooks::HookContext {
                model_name: model_name.to_string(),
                operation: "create".to_string(),
                data: data.clone(),
                user_id: None,
            };
            let data = match self
                .hook_runner
                .run_before("before_create", &hook_ctx)
                .await?
            {
                crate::hooks::HookResult::Continue(d) => d,
                crate::hooks::HookResult::Abort(msg) => return Err(anyhow::anyhow!(msg)),
            };
            if !model.validation.is_empty() {
                let errors = crate::validation::validate(&data, &model.validation);
                if let Some(e) = errors.first() {
                    return Err(anyhow::anyhow!("validation failed: {}", e.message));
                }
            }
            prepared.push(data);
        }

        // One transaction for the whole batch — a single fsync.
        let mut tx = self.pool.begin().await?;
        if rls_enabled() {
            if let Some(tid) = current_tenant() {
                bind_tenant_local(&mut tx, &tid).await?;
            }
        }
        // Rows: one multi-row INSERT for a homogeneous batch (one round trip), else per-row in the
        // same transaction. RETURNING preserves VALUES order, so records line up with `prepared`.
        let records_out: Vec<HashMap<String, Value>> =
            if let Some((sql, params)) = SqlBuilder::insert_many(model, &prepared) {
                let args = build_args(&params)?;
                let rows = sqlx::query_with(&sql, args).fetch_all(&mut *tx).await?;
                rows.iter().map(row_to_map).collect()
            } else {
                let mut out = Vec::with_capacity(prepared.len());
                for data in &prepared {
                    let (sql, params) = SqlBuilder::insert(model, data);
                    let args = build_args(&params)?;
                    let row = sqlx::query_with(&sql, args).fetch_one(&mut *tx).await?;
                    out.push(row_to_map(&row));
                }
                out
            };
        // Events: one multi-row INSERT into event_log for the whole batch.
        let origin = current_origin();
        let events: Vec<ModelEvent> = records_out
            .iter()
            .map(|record| ModelEvent {
                event_type: EventType::Created,
                model_name: model_name.to_string(),
                data: record.clone(),
                previous_data: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                event_id: uuid::Uuid::new_v4().to_string(),
                actor: actor.map(|s| s.to_string()),
                origin: origin.clone(),
            })
            .collect();
        self.event_store.persist_many_in(&mut tx, &events).await?;
        tx.commit().await?;

        // Post-commit: fan-out + after-hooks + a single cache invalidation for the whole batch.
        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        for record in &records_out {
            let after_ctx = crate::hooks::HookContext {
                model_name: model_name.to_string(),
                operation: "create".to_string(),
                data: record.clone(),
                user_id: None,
            };
            self.hook_runner
                .run_after("after_create", &after_ctx)
                .await
                .ok();
        }
        self.cache.invalidate_model(model_name).await;

        Ok(records_out)
    }

    /// Update many records. **System/trusted API — does NOT enforce RBAC.** For request-handling
    /// use [`update_many_checked`](Self::update_many_checked).
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

        // The UPDATE and its events commit in ONE transaction — one `fsync`, not 1 + N autocommits
        // — and the event writes now propagate (were `.ok()`-swallowed), so an update is never
        // recorded without its events.
        let mut tx = self.pool.begin().await?;
        if rls_enabled() {
            if let Some(tid) = current_tenant() {
                bind_tenant_local(&mut tx, &tid).await?;
            }
        }
        let rows = sqlx::query_with(&sql, args).fetch_all(&mut *tx).await?;
        let records: Vec<HashMap<String, Value>> = rows.iter().map(row_to_map).collect();
        let origin = current_origin();
        let events: Vec<ModelEvent> = records
            .iter()
            .map(|record| ModelEvent {
                event_type: EventType::Updated,
                model_name: model_name.to_string(),
                data: record.clone(),
                previous_data: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                event_id: uuid::Uuid::new_v4().to_string(),
                actor: actor.map(|s| s.to_string()),
                origin: origin.clone(),
            })
            .collect();
        self.event_store.persist_many_in(&mut tx, &events).await?;
        tx.commit().await?;

        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        self.hook_runner
            .run_after("after_update", &hook_ctx)
            .await
            .ok();
        self.cache.invalidate_model(model_name).await;

        Ok(records)
    }

    /// Delete many records (soft delete - sets deleted_at = NOW()). **System/trusted API — does
    /// NOT enforce RBAC.** For request-handling use [`delete_many_checked`](Self::delete_many_checked).
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
        // (projections/audit need it to remove the right row). The soft-delete UPDATE and its
        // events commit in ONE transaction (one `fsync`, not 1 + N), and the event writes now
        // propagate (were `.ok()`-swallowed).
        let mut tx = self.pool.begin().await?;
        if rls_enabled() {
            if let Some(tid) = current_tenant() {
                bind_tenant_local(&mut tx, &tid).await?;
            }
        }
        let rows = sqlx::query_with(&sql, args).fetch_all(&mut *tx).await?;
        let count = rows.len();
        let origin = current_origin();
        let events: Vec<ModelEvent> = rows
            .iter()
            .map(|row| {
                let record = row_to_map(row);
                let mut data = HashMap::new();
                if let Some(id) = record.get("id") {
                    data.insert("id".to_string(), id.clone());
                }
                ModelEvent {
                    event_type: EventType::Deleted,
                    model_name: model_name.to_string(),
                    data,
                    previous_data: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    event_id: uuid::Uuid::new_v4().to_string(),
                    actor: actor.map(|s| s.to_string()),
                    origin: origin.clone(),
                }
            })
            .collect();
        self.event_store.persist_many_in(&mut tx, &events).await?;
        tx.commit().await?;

        for event in &events {
            let _ = self.event_sender.send(event.clone());
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
        actor: Option<&str>,
    ) -> Result<usize> {
        let model = self
            .schema
            .models
            .get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        let (sql, params) = SqlBuilder::restore(model, where_clauses);
        let args = build_args(&params)?;

        let mut tx = self.pool.begin().await?;
        if rls_enabled() {
            if let Some(tid) = current_tenant() {
                bind_tenant_local(&mut tx, &tid).await?;
            }
        }
        let rows = sqlx::query_with(&sql, args).fetch_all(&mut *tx).await?;
        let count = rows.len();
        let origin = current_origin();
        let events: Vec<ModelEvent> = rows
            .iter()
            .map(|row| {
                let record = row_to_map(row);
                let mut data = HashMap::new();
                if let Some(id) = record.get("id") {
                    data.insert("id".to_string(), id.clone());
                }
                ModelEvent {
                    event_type: EventType::Restored,
                    model_name: model_name.to_string(),
                    data,
                    previous_data: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    event_id: uuid::Uuid::new_v4().to_string(),
                    actor: actor.map(|s| s.to_string()),
                    origin: origin.clone(),
                }
            })
            .collect();
        self.event_store.persist_many_in(&mut tx, &events).await?;
        tx.commit().await?;

        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        self.cache.invalidate_model(model_name).await;
        Ok(count)
    }

    /// Permanently delete records (hard delete - row is removed). Returns affected count.
    pub async fn hard_delete_many(
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
        let (sql, params) = SqlBuilder::delete(model, where_clauses);
        let args = build_args(&params)?;

        let mut tx = self.pool.begin().await?;
        if rls_enabled() {
            if let Some(tid) = current_tenant() {
                bind_tenant_local(&mut tx, &tid).await?;
            }
        }
        let rows = sqlx::query_with(&sql, args).fetch_all(&mut *tx).await?;
        let count = rows.len();
        let origin = current_origin();
        let events: Vec<ModelEvent> = rows
            .iter()
            .map(|row| {
                let record = row_to_map(row);
                let mut data = HashMap::new();
                if let Some(id) = record.get("id") {
                    data.insert("id".to_string(), id.clone());
                }
                ModelEvent {
                    event_type: EventType::HardDeleted,
                    model_name: model_name.to_string(),
                    data,
                    previous_data: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    event_id: uuid::Uuid::new_v4().to_string(),
                    actor: actor.map(|s| s.to_string()),
                    origin: origin.clone(),
                }
            })
            .collect();
        self.event_store.persist_many_in(&mut tx, &events).await?;
        tx.commit().await?;

        for event in &events {
            let _ = self.event_sender.send(event.clone());
        }
        self.cache.invalidate_model(model_name).await;
        Ok(count)
    }

    /// Count records matching where clauses
    pub async fn count(&self, model_name: &str, where_clauses: &[WhereClause]) -> Result<i64> {
        let cache_key = crate::cache::ReadCache::key(
            model_name,
            &format!("count:{:?}{:?}", where_clauses, current_tenant()),
        );
        if let Some(cached) = self.cache.get(&cache_key).await {
            if let Some(n) = cached.as_i64() {
                return Ok(n);
            }
        }
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
        let row = self.fetch_one_scoped(&sql, args).await?;
        let n = row.try_get::<i64, _>("count").unwrap_or(0);
        self.cache
            .set(&cache_key, serde_json::Value::Number(n.into()))
            .await;
        Ok(n)
    }

    /// Count only soft-deleted records (for the trash view's total).
    pub async fn count_deleted(
        &self,
        model_name: &str,
        where_clauses: &[WhereClause],
    ) -> Result<i64> {
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
        let sql = format!(
            "SELECT COUNT(*) as count FROM {} WHERE {}",
            table, where_sql
        );
        let args = build_args(&params)?;
        let row = self.fetch_one_scoped(&sql, args).await?;
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

    /// Enforce a model's access rule for `action` against `role` (None = unauthenticated).
    /// Returns `Err` (Unauthorized/Forbidden) when denied. This is the data-layer RBAC seam:
    /// callers that have a role (SDK, server handlers, plugins) call this BEFORE a mutation so
    /// access is enforced outside the GraphQL resolver too. Uses the same
    /// `AccessControl::decide` seam the GraphQL `check_access` uses. No `access` rule = allow.
    pub fn enforce_access(&self, model_name: &str, action: &str, role: Option<&str>) -> Result<()> {
        use atomo_schema::AccessDecision;
        let access = match self
            .schema
            .models
            .get(model_name)
            .and_then(|m| m.access.as_ref())
        {
            Some(a) => a,
            None => return Ok(()),
        };
        match access.decide(action, role) {
            AccessDecision::Allow => Ok(()),
            AccessDecision::Forbidden => {
                anyhow::bail!(
                    "Access denied: '{}' on '{}' requires a permitted role",
                    action,
                    model_name
                )
            }
            AccessDecision::NeedsAuth => anyhow::bail!(
                "Authentication required for '{}' on '{}'",
                action,
                model_name
            ),
        }
    }

    /// RBAC-enforced mutations: check the model's access rule for `role` (None = unauthenticated)
    /// then delegate. These are the data-layer enforcement entry points — the GraphQL resolvers
    /// route through them so access is enforced inside the data layer (one seam), and any
    /// SDK/internal caller that has a role can use them instead of the unchecked variants.
    pub async fn create_checked(
        &self,
        role: Option<&str>,
        model_name: &str,
        data: &HashMap<String, Value>,
        include: &[String],
        actor: Option<&str>,
    ) -> Result<HashMap<String, Value>> {
        self.enforce_access(model_name, "create", role)?;
        self.create(model_name, data, include, actor).await
    }

    pub async fn update_many_checked(
        &self,
        role: Option<&str>,
        model_name: &str,
        where_clauses: &[WhereClause],
        data: &HashMap<String, Value>,
        include: &[String],
        actor: Option<&str>,
    ) -> Result<Vec<HashMap<String, Value>>> {
        self.enforce_access(model_name, "update", role)?;
        self.update_many(model_name, where_clauses, data, include, actor)
            .await
    }

    pub async fn delete_many_checked(
        &self,
        role: Option<&str>,
        model_name: &str,
        where_clauses: &[WhereClause],
        actor: Option<&str>,
    ) -> Result<usize> {
        self.enforce_access(model_name, "delete", role)?;
        self.delete_many(model_name, where_clauses, actor).await
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
                // Schema-driven first: if the model declares this relationship, use its declared
                // target model / kind / foreignKey (so a rel named differently from its model —
                // e.g. `owner: { model: "User" }` — resolves correctly). Falls back to the
                // name-convention heuristic when no declaration exists.
                let declared = self
                    .schema
                    .models
                    .get(model_name)
                    .and_then(|m| m.relationships.get(rel_name));

                if let Some(rel) = declared {
                    if rel.kind == "belongsTo" {
                        let fk = rel
                            .foreign_key
                            .clone()
                            .unwrap_or_else(|| format!("{}Id", rel_name));
                        let fk_snake = to_snake_case(&fk);
                        if let Some(Value::String(id)) =
                            record.get(&fk_snake).or_else(|| record.get(&fk)).cloned()
                        {
                            let wc = WhereClause {
                                field: "id".to_string(),
                                operator: crate::query::WhereOperator::Equals,
                                value: Value::String(id),
                            };
                            if let Ok(Some(related)) =
                                self.find_unique(&rel.model, &[wc], &[]).await
                            {
                                record.insert(
                                    rel_name.clone(),
                                    serde_json::to_value(related).unwrap_or(Value::Null),
                                );
                            }
                        }
                    } else {
                        // hasMany: related.<foreignKey or this_model_id> == this record's id
                        if let Some(Value::String(id)) = record.get("id").cloned() {
                            let fk = rel
                                .foreign_key
                                .clone()
                                .map(|f| to_snake_case(&f))
                                .unwrap_or_else(|| format!("{}_id", to_snake_case(model_name)));
                            let wc = WhereClause {
                                field: fk,
                                operator: crate::query::WhereOperator::Equals,
                                value: Value::String(id),
                            };
                            if let Ok(related) = self
                                .find_many(&rel.model, &[wc], &[], None, None, &[])
                                .await
                            {
                                record.insert(
                                    rel_name.clone(),
                                    serde_json::to_value(related).unwrap_or(Value::Null),
                                );
                            }
                        }
                    }
                    continue;
                }

                // --- Convention fallback (no declared relationship) ---
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
        let pool = pool_from_env(&database_url).await?;
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
            cache: crate::cache::ReadCache::from_env(60),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn action_origin_scopes_and_clears() {
        // Outside any scope, current_origin is None.
        assert_eq!(current_origin(), None);

        // Inside with_action_origin, it returns the set value.
        let inside = with_action_origin(Some("myAction".into()), async {
            current_origin()
        }).await;
        assert_eq!(inside, Some("myAction".to_string()));

        // After the scope, it's None again.
        assert_eq!(current_origin(), None);
    }

    #[tokio::test]
    async fn action_origin_none_scope_returns_none() {
        let inside = with_action_origin(None, async {
            current_origin()
        }).await;
        assert_eq!(inside, None);
    }
}
