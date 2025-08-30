//! GraphQL integration for Atomo
//!
//! Provides automatic GraphQL schema generation and resolvers based on the
//! Atomo schema definition.

use std::sync::Arc;
use std::collections::HashMap;
use async_graphql::{Schema as GraphQLSchema, Object, Subscription, Context, Result as GraphQLResult};
use serde_json::Value;

use crate::client::AtomoClient;
use crate::schema::Schema;
use crate::events::ModelEvent;

/// Root GraphQL query object
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
    /// Find many records of any model type
    async fn find_many(
        &self,
        ctx: &Context<'_>,
        model: String,
        #[graphql(desc = "Where conditions")] where_: Option<Value>,
        #[graphql(desc = "Order by fields")] order_by: Option<Value>,
        #[graphql(desc = "Limit results")] limit: Option<i32>,
        #[graphql(desc = "Offset results")] offset: Option<i32>,
    ) -> GraphQLResult<Vec<HashMap<String, Value>>> {
        // TODO: Convert GraphQL args to query parameters
        let results = self.client.find_many(
            &model,
            &[], // where_clauses
            &[], // order_by
            limit.map(|l| l as usize),
            offset.map(|o| o as usize),
            &[], // include
        ).await?;
        
        Ok(results)
    }
    
    /// Find a unique record
    async fn find_unique(
        &self,
        ctx: &Context<'_>,
        model: String,
        where_: Value,
    ) -> GraphQLResult<Option<HashMap<String, Value>>> {
        // TODO: Convert GraphQL where to query parameters
        let result = self.client.find_unique(
            &model,
            &[], // where_clauses
            &[], // include
        ).await?;
        
        Ok(result)
    }
}

/// Root GraphQL mutation object
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
    
    /// Update records
    async fn update_many(
        &self,
        ctx: &Context<'_>,
        model: String,
        where_: Value,
        data: HashMap<String, Value>,
    ) -> GraphQLResult<Vec<HashMap<String, Value>>> {
        // TODO: Convert GraphQL where to query parameters
        let results = self.client.update_many(
            &model,
            &[], // where_clauses
            &data,
            &[], // include
        ).await?;
        
        Ok(results)
    }
    
    /// Delete records
    async fn delete_many(
        &self,
        ctx: &Context<'_>,
        model: String,
        where_: Value,
    ) -> GraphQLResult<i32> {
        // TODO: Convert GraphQL where to query parameters
        let count = self.client.delete_many(
            &model,
            &[], // where_clauses
        ).await?;
        
        Ok(count as i32)
    }
}

/// Root GraphQL subscription object
pub struct Subscription {
    client: Arc<AtomoClient>,
    schema: Schema,
}

impl Subscription {
    pub fn new(client: Arc<AtomoClient>, schema: Schema) -> Self {
        Self { client, schema }
    }
}

#[Subscription]
impl Subscription {
    /// Subscribe to model changes
    async fn model_changes(
        &self,
        model: String,
    ) -> impl futures::Stream<Item = ModelEvent> {
        let receiver = self.client.subscribe(&model, &[], &[]).await;
        
        futures::stream::unfold(receiver, |mut rx| async move {
            match rx.recv().await {
                Ok(event) => Some((event.clone(), rx)),
                Err(_) => None,
            }
        })
    }
}

/// Build a complete GraphQL schema
pub fn build_schema(
    client: Arc<AtomoClient>,
    schema: &Schema,
) -> GraphQLSchema<Query, Mutation, Subscription> {
    let query = Query::new(client.clone(), schema.clone());
    let mutation = Mutation::new(client.clone(), schema.clone());
    let subscription = Subscription::new(client.clone(), schema.clone());
    
    GraphQLSchema::build(query, mutation, subscription)
        .finish()
}
