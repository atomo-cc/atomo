//! GraphQL integration for Atomo
//!
//! Provides automatic GraphQL schema generation and resolvers based on the
//! Atomo schema definition. Platform integration is handled at the server layer.

use async_graphql::{
    Context, Object, Result as GraphQLResult, Schema as GraphQLSchema, SimpleObject, Subscription,
};
use futures;
use futures::StreamExt;
use serde_json::Value;
use sqlx;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;

use atomo_schema::AccessRule;

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

fn check_access(
    schema: &Schema,
    model_name: &str,
    action: &str,
    ctx: &Context<'_>,
) -> GraphQLResult<()> {
    let access = schema
        .models
        .get(model_name)
        .and_then(|m| m.access.as_ref());
    let rule = match (access, action) {
        (Some(a), "create") => a.create.as_ref(),
        (Some(a), "read") => a.read.as_ref(),
        (Some(a), "update") => a.update.as_ref(),
        (Some(a), "delete") => a.delete.as_ref(),
        _ => return Ok(()),
    };
    let rule = match rule {
        Some(r) => r,
        None => return Ok(()),
    };
    let user_role = ctx.data_opt::<UserRoleCtx>();
    match rule {
        AccessRule::Boolean(roles_str) => {
            if roles_str == "authenticated" {
                if user_role.is_none() {
                    return Err(AtomoError::Unauthorized {
                        message: "Authentication required".to_string(),
                    }
                    .into());
                }
                return Ok(());
            }
            let allowed: Vec<&str> = roles_str.split('|').collect();
            match user_role {
                Some(r) if allowed.iter().any(|a| a.eq_ignore_ascii_case(&r.0)) => Ok(()),
                Some(_) => Err(AtomoError::Forbidden {
                    message: format!("Access denied: requires one of [{}]", roles_str),
                }
                .into()),
                None => Err(AtomoError::Unauthorized {
                    message: "Authentication required".to_string(),
                }
                .into()),
            }
        }
        _ => Ok(()),
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

fn parse_where(where_json: &Value) -> Vec<WhereClause> {
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
                        "isNull" => WhereOperator::IsNull,
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
        Ok(result)
    }

    /// List soft-deleted records (the "trash" view).
    async fn deleted_records(
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
            .find_deleted(
                &model,
                &where_clauses,
                &orders,
                limit.map(|l| l as usize),
                offset.map(|o| o as usize),
            )
            .await?;
        Ok(result)
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
        Ok(result)
    }

    /// Get records with pagination metadata
    async fn paginated_records(
        &self,
        ctx: &Context<'_>,
        model: String,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> GraphQLResult<PaginatedRecords> {
        check_access(&self.schema, &model, "read", ctx)?;
        let lim = limit.unwrap_or(20) as usize;
        let off = offset.unwrap_or(0) as usize;
        let tenant = ctx.data_opt::<TenantCtx>();
        let where_clauses = crate::client::scope_by_tenant(&[], tenant.map(|t| t.0.as_str()));
        let data = self
            .client
            .find_many(&model, &where_clauses, &[], Some(lim), Some(off), &[])
            .await?;
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
        let mut data = data;
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
        let result = self
            .client
            .create(
                &model,
                &data,
                &[], // include
                actor.as_deref(),
            )
            .await?;

        Ok(result)
    }

    /// Update a record
    async fn update(
        &self,
        ctx: &Context<'_>,
        model: String,
        #[graphql(name = "where")] where_: Value,
        data: HashMap<String, Value>,
    ) -> GraphQLResult<HashMap<String, Value>> {
        check_access(&self.schema, &model, "update", ctx)?;
        let tenant = ctx.data_opt::<TenantCtx>();
        let where_clauses = parse_where(&where_);
        let where_clauses =
            crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let actor = ctx.data_opt::<UserIdCtx>().map(|u| u.0.clone());
        let results = self
            .client
            .update_many(
                &model,
                &where_clauses,
                &data,
                &[], // include
                actor.as_deref(),
            )
            .await?;

        // Return the first updated record or a default one
        Ok(results.into_iter().next().unwrap_or_default())
    }

    /// Delete a record
    async fn delete(
        &self,
        ctx: &Context<'_>,
        model: String,
        #[graphql(name = "where")] where_: Value,
    ) -> GraphQLResult<i32> {
        check_access(&self.schema, &model, "delete", ctx)?;
        let tenant = ctx.data_opt::<TenantCtx>();
        let where_clauses = parse_where(&where_);
        let where_clauses =
            crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let actor = ctx.data_opt::<UserIdCtx>().map(|u| u.0.clone());
        let count = self
            .client
            .delete_many(&model, &where_clauses, actor.as_deref())
            .await?;

        Ok(count as i32)
    }

    /// Restore soft-deleted records matching the where filter.
    async fn restore(&self, ctx: &Context<'_>, model: String, r#where: Value) -> GraphQLResult<i32> {
        check_access(&self.schema, &model, "delete", ctx)?;
        let tenant = ctx.data_opt::<TenantCtx>();
        let mut where_clauses = parse_where(&r#where);
        where_clauses = crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let count = self.client.restore_many(&model, &where_clauses).await?;
        Ok(count as i32)
    }

    /// Permanently delete (purge) records matching the where filter.
    async fn hard_delete(
        &self,
        ctx: &Context<'_>,
        model: String,
        r#where: Value,
    ) -> GraphQLResult<i32> {
        check_access(&self.schema, &model, "delete", ctx)?;
        let tenant = ctx.data_opt::<TenantCtx>();
        let mut where_clauses = parse_where(&r#where);
        where_clauses = crate::client::scope_by_tenant(&where_clauses, tenant.map(|t| t.0.as_str()));
        let count = self
            .client
            .hard_delete_many(&model, &where_clauses)
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
    /// Subscribe to model changes
    async fn model_changes(&self, model: String) -> impl futures::Stream<Item = ModelEvent> + '_ {
        let rx = self.client.subscribe(&model, &[], &[]).await;
        let model_filter = model;
        BroadcastStream::new(rx).filter_map(move |result| {
            let m = model_filter.clone();
            async move { result.ok().filter(|e| e.model_name == m) }
        })
    }
}

/// Service-level schema without platform integration
/// Platform integration should be done at the server layer
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
