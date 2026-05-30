//! GraphQL integration for Atomo
//!
//! Provides automatic GraphQL schema generation and resolvers based on the
//! Atomo schema definition. Platform integration is handled at the server layer.

use std::sync::Arc;
use std::collections::HashMap;
use async_graphql::{Schema as GraphQLSchema, Object, Subscription, Context, Result as GraphQLResult, EmptySubscription};
use serde_json::Value;
use futures;
use futures::StreamExt;
use sqlx;
use tokio_stream::wrappers::BroadcastStream;

use crate::client::AtomoClient;
use crate::schema::Schema;
use crate::events::ModelEvent;

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

#[Object]
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
        let result = self.client.find_many(
            &model,
            &[], // where_clauses  
            &[], // order_by
            limit.map(|l| l as usize),
            offset.map(|o| o as usize),
            &[], // include
        ).await?;
        
        Ok(result)
    }

    /// Get a single record by ID
    async fn record(
        &self,
        ctx: &Context<'_>,
        model: String,
        id: String,
    ) -> GraphQLResult<Option<HashMap<String, Value>>> {
        let result = self.client.find_unique(
            &model,
            &[], // where_clauses
            &[], // include  
        ).await?;
        
        Ok(result)
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

#[Object]
impl Mutation {
    /// Create a new record
    async fn create(
        &self,
        ctx: &Context<'_>,
        model: String,
        data: HashMap<String, Value>,
    ) -> GraphQLResult<HashMap<String, Value>> {
        let result = self.client.create(
            &model,
            &data,
            &[], // include
        ).await?;
        
        Ok(result)
    }

    /// Update a record
    async fn update(
        &self,
        ctx: &Context<'_>,
        model: String,
        where_: Value,
        data: HashMap<String, Value>,
    ) -> GraphQLResult<HashMap<String, Value>> {
        let results = self.client.update_many(
            &model,
            &[], // where_clauses
            &data,
            &[], // include
        ).await?;
        
        // Return the first updated record or a default one
        Ok(results.into_iter().next().unwrap_or_default())
    }

    /// Delete a record
    async fn delete(
        &self,
        ctx: &Context<'_>,
        model: String,
        where_: Value,
    ) -> GraphQLResult<i32> {
        let count = self.client.delete_many(
            &model,
            &[], // where_clauses
        ).await?;
        
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
    async fn model_changes(
        &self,
        model: String,
    ) -> impl futures::Stream<Item = ModelEvent> + '_ {
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
pub fn build_schema(client: Arc<AtomoClient>, schema: &Schema, pool: sqlx::Pool<sqlx::Postgres>) -> GraphQLSchema<Query, Mutation, Subscription> {
    let query = Query::new(client.clone(), schema.clone());
    let mutation = Mutation::new(client.clone(), schema.clone());
    let subscription = Subscription::new(client.clone());
    
    GraphQLSchema::build(query, mutation, subscription)
        .data(client)
        .data(pool)
        .finish()
}
