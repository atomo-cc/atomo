//! GraphQL integration for Atomo
//!
//! Provides automatic GraphQL schema generation and resolvers based on the
//! Atomo schema definition. Platform integration is handled at the server layer.

use async_graphql::{
    Context, InputObject, Object, Result as GraphQLResult, Schema as GraphQLSchema, SimpleObject,
    Subscription,
};
use futures;
use futures::StreamExt;
use serde_json::Value;
use sqlx;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;

use crate::client::AtomoClient;
use crate::errors::{AtomoError, FieldError};
use crate::events::ModelEvent;
use crate::query::{OrderDirection, WhereClause, WhereOperator};
use crate::schema::Schema;

/// User role context data for RBAC checks
pub struct UserRoleCtx(pub String);

/// User id context data for audit actor attribution
pub struct UserIdCtx(pub String);

/// Tenant context for multi-tenant isolation
pub struct TenantCtx(pub String);

/// Re-export: set the request's RLS tenant scope around request execution.
/// Wrap `schema.execute(req)` in this so RLS-enabled queries bind `atomo.tenant_id`.
pub use crate::client::with_tenant_scope;

fn check_access(
    schema: &Schema,
    model_name: &str,
    action: &str,
    ctx: &Context<'_>,
) -> GraphQLResult<()> {
    let access = match schema
        .models
        .get(model_name)
        .and_then(|m| m.access.as_ref())
    {
        Some(a) => a,
        None => return Ok(()),
    };
    let user_role = ctx.data_opt::<UserRoleCtx>().map(|r| r.0.as_str());
    match access.decide(action, user_role) {
        atomo_schema::AccessDecision::Allow => Ok(()),
        atomo_schema::AccessDecision::Forbidden => Err(AtomoError::Forbidden {
            message: format!("Access denied for '{}' on '{}'", action, model_name),
        }
        .into()),
        atomo_schema::AccessDecision::NeedsAuth => Err(AtomoError::Unauthorized {
            message: "Authentication required".to_string(),
        }
        .into()),
    }
}

#[derive(SimpleObject)]
struct PageInfo {
    total_count: i64,
    has_next_page: bool,
    has_previous_page: bool,
    page_size: i32,
    offset: i32,
}

#[derive(SimpleObject)]
struct PaginatedRecords {
    data: Value,
    page_info: PageInfo,
}

#[derive(InputObject)]
struct BulkUpdateInput {
    id: String,
    data: Value,
}

/// Resolve the `id` / `where` pair into a single where value. Exactly one must
/// be provided; `id` is sugar for `{id: "<value>"}`.
fn resolve_where(id: Option<String>, where_: Option<Value>) -> GraphQLResult<Value> {
    match (id, where_) {
        (Some(id), None) => Ok(serde_json::json!({ "id": id })),
        (None, Some(w)) => Ok(w),
        (Some(_), Some(_)) => Err(async_graphql::Error::new(
            "Provide either `id` or `where`, not both",
        )),
        (None, None) => Err(async_graphql::Error::new(
            "Either `id` or `where` must be provided",
        )),
    }
}

/// Convert a `snake_case` string to `camelCase`. Already-camelCase input passes through unchanged.
fn snake_to_camel(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut upper_next = false;
    for c in s.chars() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            result.extend(c.to_uppercase());
            upper_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert all keys in a record from snake_case to camelCase for API output.
fn camel_keys(record: HashMap<String, Value>) -> HashMap<String, Value> {
    record
        .into_iter()
        .map(|(k, v)| (snake_to_camel(&k), v))
        .collect()
}

/// Normalize mutation input keys so both `camelCase` and `snake_case` are accepted.
/// For each incoming key, if it matches a schema field name directly, keep it.
/// Otherwise, try to find a schema field whose snake_case form matches the key's
/// snake_case form, and use the canonical schema name. Unknown keys pass through
/// as-is (the DB will reject them if the column doesn't exist).
fn normalize_input_keys(
    data: HashMap<String, Value>,
    schema: &Schema,
    model_name: &str,
) -> HashMap<String, Value> {
    let model = match schema.models.get(model_name) {
        Some(m) => m,
        None => return data,
    };
    data.into_iter()
        .map(|(k, v)| {
            if model.fields.contains_key(&k) {
                return (k, v);
            }
            let k_snake = crate::query::sql_builder::to_snake_case(&k);
            for field_name in model.fields.keys() {
                if crate::query::sql_builder::to_snake_case(field_name) == k_snake {
                    return (field_name.clone(), v);
                }
            }
            (k, v)
        })
        .collect()
}

pub fn parse_where(where_json: &Value) -> Vec<WhereClause> {
    let mut clauses = Vec::new();
    if let Value::Object(map) = where_json {
        for (field, condition) in map {
            if let Value::Object(ops) = condition {
                for (op, val) in ops {
                    let operator = match op.as_str() {
                        "equals" | "eq" => WhereOperator::Equals,
                        "not" | "neq" => WhereOperator::NotEquals,
                        "contains" | "like" => WhereOperator::Contains,
                        "startsWith" => WhereOperator::StartsWith,
                        "endsWith" => WhereOperator::EndsWith,
                        "gt" => WhereOperator::GreaterThan,
                        "gte" => WhereOperator::GreaterThanOrEqual,
                        "lt" => WhereOperator::LessThan,
                        "lte" => WhereOperator::LessThanOrEqual,
                        "in" => WhereOperator::In,
                        "notIn" => WhereOperator::NotIn,
                        // `isNull: true` → IS NULL; `isNull: false` → IS NOT NULL.
                        // Previously the value was ignored, so "is not null" was
                        // inexpressible through the GraphQL where JSON.
                        "isNull" => {
                            if val.as_bool() == Some(false) {
                                WhereOperator::IsNotNull
                            } else {
                                WhereOperator::IsNull
                            }
                        }
                        _ => continue,
                    };
                    clauses.push(WhereClause {
                        field: field.clone(),
                        operator,
                        value: val.clone(),
                    });
                }
            } else {
                clauses.push(WhereClause {
                    field: field.clone(),
                    operator: WhereOperator::Equals,
                    value: condition.clone(),
                });
            }
        }
    }
    clauses
}

fn parse_order_by(order_json: &Value) -> Vec<(String, OrderDirection)> {
    let mut orders = Vec::new();
    match order_json {
        Value::Object(map) => {
            for (field, dir) in map {
                let direction = match dir.as_str().unwrap_or("asc").to_lowercase().as_str() {
                    "desc" => OrderDirection::Desc,
                    _ => OrderDirection::Asc,
                };
                orders.push((field.clone(), direction));
            }
        }
        Value::Array(arr) => {
            for item in arr {
                if let Value::Object(obj) = item {
                    let field = obj
                        .get("field")
                        .and_then(|f| f.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let dir = obj
                        .get("direction")
                        .and_then(|d| d.as_str())
                        .unwrap_or("asc");
                    let direction = if dir.eq_ignore_ascii_case("desc") {
                        OrderDirection::Desc
                    } else {
                        OrderDirection::Asc
                    };
                    orders.push((field, direction));
                }
            }
        }
        _ => {}
    }
    orders
}

/// Service-specific GraphQL queries
/// Platform queries are handled separately in the server layer
pub struct Query {
    client: Arc<AtomoClient>,
    schema: Schema,
}

impl Query {
    pub fn new(client: Arc<AtomoClient>, schema: Schema) -> Self {
        Self { client, schema }
    }
}

#[Object(name = "ServiceQuery")]
impl Query {
    /// Get records with filtering and pagination
    async fn records(
        &self,
        ctx: &Context<'_>,
        model: String,
        #[graphql(name = "where")] where_: Option<Value>,
        #[graphql(name = "orderBy")] order_by: Option<Value>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> GraphQLResult<Vec<HashMap<String, Value>>> {
        check_access(&self.schema, &model, "read", ctx)?;
        let where_clauses = where_.as_ref().map(parse_where).unwrap_or_default();
        let tenant = ctx.data_opt::<TenantCtx>();
        let where_clauses =
            crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let orders = order_by.as_ref().map(parse_order_by).unwrap_or_default();
        let result = self
            .client
            .find_many(
                &model,
                &where_clauses,
                &orders,
                limit.map(|l| l as usize),
                offset.map(|o| o as usize),
                &[],
            )
            .await?;
        Ok(result.into_iter().map(camel_keys).collect())
    }

    /// List soft-deleted records (the "trash" view) with pagination metadata.
    async fn deleted_records(
        &self,
        ctx: &Context<'_>,
        model: String,
        #[graphql(name = "where")] where_: Option<Value>,
        #[graphql(name = "orderBy")] order_by: Option<Value>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> GraphQLResult<PaginatedRecords> {
        check_access(&self.schema, &model, "read", ctx)?;
        let lim = limit.unwrap_or(20) as usize;
        let off = offset.unwrap_or(0) as usize;
        let where_clauses = where_.as_ref().map(parse_where).unwrap_or_default();
        let tenant = ctx.data_opt::<TenantCtx>();
        let where_clauses =
            crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let orders = order_by.as_ref().map(parse_order_by).unwrap_or_default();
        let data = self
            .client
            .find_deleted(&model, &where_clauses, &orders, Some(lim), Some(off))
            .await?;
        let data: Vec<_> = data.into_iter().map(camel_keys).collect();
        let total_count = self
            .client
            .count_deleted(&model, &where_clauses)
            .await
            .unwrap_or(0);
        let page_info = PageInfo {
            total_count,
            has_next_page: (off + lim) < total_count as usize,
            has_previous_page: off > 0,
            page_size: lim as i32,
            offset: off as i32,
        };
        Ok(PaginatedRecords {
            data: serde_json::to_value(&data)?,
            page_info,
        })
    }

    /// Get a single record by ID
    async fn record(
        &self,
        ctx: &Context<'_>,
        model: String,
        id: String,
    ) -> GraphQLResult<Option<HashMap<String, Value>>> {
        check_access(&self.schema, &model, "read", ctx)?;
        let where_clauses = vec![WhereClause {
            field: "id".to_string(),
            operator: WhereOperator::Equals,
            value: Value::String(id),
        }];
        let tenant = ctx.data_opt::<TenantCtx>();
        let where_clauses =
            crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let result = self.client.find_unique(&model, &where_clauses, &[]).await?;
        Ok(result.map(camel_keys))
    }

    /// Get records with pagination metadata
    async fn paginated_records(
        &self,
        ctx: &Context<'_>,
        model: String,
        #[graphql(name = "where")] where_: Option<Value>,
        #[graphql(name = "orderBy")] order_by: Option<Value>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> GraphQLResult<PaginatedRecords> {
        check_access(&self.schema, &model, "read", ctx)?;
        let lim = limit.unwrap_or(20) as usize;
        let off = offset.unwrap_or(0) as usize;
        let where_clauses = where_.as_ref().map(parse_where).unwrap_or_default();
        let tenant = ctx.data_opt::<TenantCtx>();
        let where_clauses =
            crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let orders = order_by.as_ref().map(parse_order_by).unwrap_or_default();
        let data = self
            .client
            .find_many(&model, &where_clauses, &orders, Some(lim), Some(off), &[])
            .await?;
        let data: Vec<_> = data.into_iter().map(camel_keys).collect();
        let total_count = self.client.count(&model, &where_clauses).await.unwrap_or(0);
        let page_info = PageInfo {
            total_count,
            has_next_page: (off + lim) < total_count as usize,
            has_previous_page: off > 0,
            page_size: lim as i32,
            offset: off as i32,
        };
        Ok(PaginatedRecords {
            data: serde_json::to_value(&data)?,
            page_info,
        })
    }
}

/// Service-specific GraphQL mutations
pub struct Mutation {
    client: Arc<AtomoClient>,
    schema: Schema,
}

impl Mutation {
    pub fn new(client: Arc<AtomoClient>, schema: Schema) -> Self {
        Self { client, schema }
    }
}

#[Object(name = "ServiceMutation")]
impl Mutation {
    /// Create a new record
    async fn create(
        &self,
        ctx: &Context<'_>,
        model: String,
        data: HashMap<String, Value>,
    ) -> GraphQLResult<HashMap<String, Value>> {
        check_access(&self.schema, &model, "create", ctx)?;
        let mut data = normalize_input_keys(data, &self.schema, &model);
        if let Some(t) = ctx.data_opt::<TenantCtx>() {
            data.insert("tenant_id".to_string(), Value::String(t.0.clone()));
        }
        if let Some(model_def) = self.schema.models.get(&model) {
            let rules: HashMap<String, String> = if !model_def.validation.is_empty() {
                model_def.validation.clone()
            } else {
                model_def
                    .fields
                    .iter()
                    .filter(|(_, f)| {
                        !f.optional
                            && f.name != "id"
                            && f.name != "createdAt"
                            && f.name != "updatedAt"
                    })
                    .map(|(name, _)| (name.clone(), "required".to_string()))
                    .collect()
            };
            let errors = crate::validation::validate(&data, &rules);
            if !errors.is_empty() {
                let field_errors: Vec<FieldError> = errors
                    .into_iter()
                    .map(|e| FieldError {
                        field: e.field,
                        message: e.message,
                        code: e.code,
                    })
                    .collect();
                return Err(AtomoError::ValidationFailed {
                    errors: field_errors,
                }
                .into());
            }
        }
        let actor = ctx.data_opt::<UserIdCtx>().map(|u| u.0.clone());
        let role = ctx.data_opt::<UserRoleCtx>().map(|r| r.0.clone());
        let result = self
            .client
            .create_checked(
                role.as_deref(),
                &model,
                &data,
                &[], // include
                actor.as_deref(),
            )
            .await?;

        Ok(camel_keys(result))
    }

    /// Update a record. Accepts either `id` (shorthand for `where: {id: "..."}`)
    /// or `where` for complex filters. At least one must be provided.
    async fn update(
        &self,
        ctx: &Context<'_>,
        model: String,
        id: Option<String>,
        #[graphql(name = "where")] where_: Option<Value>,
        data: HashMap<String, Value>,
    ) -> GraphQLResult<HashMap<String, Value>> {
        check_access(&self.schema, &model, "update", ctx)?;
        let data = normalize_input_keys(data, &self.schema, &model);
        let where_value = resolve_where(id, where_)?;
        let tenant = ctx.data_opt::<TenantCtx>();
        let where_clauses = parse_where(&where_value);
        let where_clauses =
            crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let actor = ctx.data_opt::<UserIdCtx>().map(|u| u.0.clone());
        let role = ctx.data_opt::<UserRoleCtx>().map(|r| r.0.clone());
        let results = self
            .client
            .update_many_checked(
                role.as_deref(),
                &model,
                &where_clauses,
                &data,
                &[], // include
                actor.as_deref(),
            )
            .await?;

        Ok(camel_keys(results.into_iter().next().unwrap_or_default()))
    }

    /// Bulk-update multiple records by id in a single request.
    /// Each entry carries its own `id` and `data`; all updates run in one
    /// transaction so the request count drops from N to 1.
    async fn update_many(
        &self,
        ctx: &Context<'_>,
        model: String,
        updates: Vec<BulkUpdateInput>,
    ) -> GraphQLResult<Vec<HashMap<String, Value>>> {
        check_access(&self.schema, &model, "update", ctx)?;
        let tenant = ctx.data_opt::<TenantCtx>();
        let actor = ctx.data_opt::<UserIdCtx>().map(|u| u.0.clone());
        let role = ctx.data_opt::<UserRoleCtx>().map(|r| r.0.clone());
        let mut out = Vec::with_capacity(updates.len());
        for entry in updates {
            let where_value = serde_json::json!({ "id": entry.id });
            let where_clauses = parse_where(&where_value);
            let where_clauses =
                crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
            let data: HashMap<String, Value> = match entry.data {
                Value::Object(map) => map.into_iter().collect(),
                _ => {
                    return Err(async_graphql::Error::new(
                        "Each update's `data` must be a JSON object",
                    ))
                }
            };
            let data = normalize_input_keys(data, &self.schema, &model);
            let results = self
                .client
                .update_many_checked(
                    role.as_deref(),
                    &model,
                    &where_clauses,
                    &data,
                    &[],
                    actor.as_deref(),
                )
                .await?;
            if let Some(row) = results.into_iter().next() {
                out.push(camel_keys(row));
            }
        }
        Ok(out)
    }

    /// Delete a record. Accepts either `id` or `where`.
    async fn delete(
        &self,
        ctx: &Context<'_>,
        model: String,
        id: Option<String>,
        #[graphql(name = "where")] where_: Option<Value>,
    ) -> GraphQLResult<i32> {
        check_access(&self.schema, &model, "delete", ctx)?;
        let where_value = resolve_where(id, where_)?;
        let tenant = ctx.data_opt::<TenantCtx>();
        let where_clauses = parse_where(&where_value);
        let where_clauses =
            crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let actor = ctx.data_opt::<UserIdCtx>().map(|u| u.0.clone());
        let role = ctx.data_opt::<UserRoleCtx>().map(|r| r.0.clone());
        let count = self
            .client
            .delete_many_checked(role.as_deref(), &model, &where_clauses, actor.as_deref())
            .await?;

        Ok(count as i32)
    }

    /// Restore soft-deleted records. Accepts either `id` or `where`.
    async fn restore(
        &self,
        ctx: &Context<'_>,
        model: String,
        id: Option<String>,
        #[graphql(name = "where")] where_: Option<Value>,
    ) -> GraphQLResult<i32> {
        check_access(&self.schema, &model, "delete", ctx)?;
        let where_value = resolve_where(id, where_)?;
        let tenant = ctx.data_opt::<TenantCtx>();
        let where_clauses = parse_where(&where_value);
        let where_clauses =
            crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let actor = ctx.data_opt::<UserIdCtx>().map(|u| u.0.clone());
        let count = self
            .client
            .restore_many(&model, &where_clauses, actor.as_deref())
            .await?;
        Ok(count as i32)
    }

    /// Permanently delete (purge) records. Accepts either `id` or `where`.
    async fn hard_delete(
        &self,
        ctx: &Context<'_>,
        model: String,
        id: Option<String>,
        #[graphql(name = "where")] where_: Option<Value>,
    ) -> GraphQLResult<i32> {
        check_access(&self.schema, &model, "delete", ctx)?;
        let where_value = resolve_where(id, where_)?;
        let tenant = ctx.data_opt::<TenantCtx>();
        let where_clauses = parse_where(&where_value);
        let where_clauses =
            crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let actor = ctx.data_opt::<UserIdCtx>().map(|u| u.0.clone());
        let count = self
            .client
            .hard_delete_many(&model, &where_clauses, actor.as_deref())
            .await?;
        Ok(count as i32)
    }
}

/// Service-specific GraphQL subscriptions
pub struct Subscription {
    client: Arc<AtomoClient>,
}

impl Subscription {
    pub fn new(client: Arc<AtomoClient>) -> Self {
        Self { client }
    }
}

#[Subscription]
impl Subscription {
    /// Subscribe to model changes. Gated by the model's `read` access rule using the role
    /// injected at WebSocket connection_init — closes the unauthenticated-subscription bypass.
    async fn model_changes(
        &self,
        ctx: &Context<'_>,
        model: String,
    ) -> async_graphql::Result<impl futures::Stream<Item = ModelEvent> + '_> {
        if let Some(access) = self
            .client
            .schema()
            .models
            .get(&model)
            .and_then(|m| m.access.as_ref())
        {
            let role = ctx.data_opt::<UserRoleCtx>().map(|r| r.0.as_str());
            match access.decide("read", role) {
                atomo_schema::AccessDecision::Allow => {}
                atomo_schema::AccessDecision::Forbidden => {
                    return Err(AtomoError::Forbidden {
                        message: format!("Subscription denied: read access to '{}'", model),
                    }
                    .into())
                }
                atomo_schema::AccessDecision::NeedsAuth => {
                    return Err(AtomoError::Unauthorized {
                        message: "Authentication required to subscribe".to_string(),
                    }
                    .into())
                }
            }
        }
        let rx = self.client.subscribe(&model, &[], &[]).await;
        let model_filter = model;
        // S3a: scope the stream to the subscriber's tenant (from connection_init), so events
        // don't leak across tenants. None = single-tenant/unscoped → no tenant filtering.
        let tenant = ctx.data_opt::<TenantCtx>().map(|t| t.0.clone());
        Ok(BroadcastStream::new(rx).filter_map(move |result| {
            let m = model_filter.clone();
            let t = tenant.clone();
            async move {
                result
                    .ok()
                    .filter(|e| {
                        if e.model_name != m {
                            return false;
                        }
                        match &t {
                            Some(tid) => {
                                e.data.get("tenant_id").and_then(|v| v.as_str())
                                    == Some(tid.as_str())
                            }
                            None => true,
                        }
                    })
                    .map(|mut e| {
                        e.data = camel_keys(e.data);
                        if let Some(prev) = e.previous_data.take() {
                            e.previous_data = Some(camel_keys(prev));
                        }
                        e
                    })
            }
        }))
    }
}

pub fn build_schema(
    client: Arc<AtomoClient>,
    schema: &Schema,
    pool: sqlx::Pool<sqlx::Postgres>,
) -> GraphQLSchema<Query, Mutation, Subscription> {
    let query = Query::new(client.clone(), schema.clone());
    let mutation = Mutation::new(client.clone(), schema.clone());
    let subscription = Subscription::new(client.clone());

    GraphQLSchema::build(query, mutation, subscription)
        .data(client)
        .data(pool)
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_to_camel_conversions() {
        assert_eq!(snake_to_camel("first_name"), "firstName");
        assert_eq!(snake_to_camel("created_at"), "createdAt");
        assert_eq!(snake_to_camel("id"), "id");
        assert_eq!(snake_to_camel("tenant_id"), "tenantId");
        assert_eq!(snake_to_camel("firstName"), "firstName");
        assert_eq!(snake_to_camel("some_long_field_name"), "someLongFieldName");
    }

    #[test]
    fn camel_keys_converts_map() {
        let mut m = HashMap::new();
        m.insert("first_name".to_string(), Value::String("Jane".into()));
        m.insert("id".to_string(), Value::String("1".into()));
        let out = camel_keys(m);
        assert!(out.contains_key("firstName"));
        assert!(out.contains_key("id"));
        assert!(!out.contains_key("first_name"));
    }

    #[test]
    fn parse_where_is_null_honors_bool_value() {
        use crate::query::WhereOperator;
        let clauses = parse_where(&serde_json::json!({
            "closedAt": { "isNull": true },
            "ownerId": { "isNull": false },
        }));
        let op_for = |f: &str| {
            clauses
                .iter()
                .find(|c| c.field == f)
                .map(|c| c.operator.clone())
        };
        assert!(matches!(op_for("closedAt"), Some(WhereOperator::IsNull)));
        assert!(matches!(op_for("ownerId"), Some(WhereOperator::IsNotNull)));
    }
}
