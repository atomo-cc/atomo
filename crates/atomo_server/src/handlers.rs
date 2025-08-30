//! HTTP handlers for the Atomo server

use axum::{
    extract::Extension,
    response::Html,
    routing::{get, post},
    Router,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use async_graphql::{Schema as GraphQLSchema};
use atomo::prelude::*;
use atomo::graphql::{Query, Mutation, Subscription};

pub type AtomoGraphQLSchema = GraphQLSchema<Query, Mutation, Subscription>;

pub async fn graphql_handler(
    Extension(schema): Extension<AtomoGraphQLSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

pub async fn graphql_playground() -> Html<String> {
    let source = async_graphql::http::playground_source(
        async_graphql::http::GraphQLPlaygroundConfig::new("/graphql"),
    );
    Html(source)
}

pub async fn health_check() -> &'static str {
    "OK"
}

pub async fn atomo_info(Extension(_atomo): Extension<Atomo>) -> String {
    // You could add schema introspection here
    format!("🚀 Atomo Content Core Server\n📊 Schema loaded successfully")
}

pub fn create_router(schema: AtomoGraphQLSchema, atomo: Atomo) -> Router {
    Router::new()
        .route("/", get(|| async { "🚀 Atomo Content Core Server" }))
        .route("/health", get(health_check))
        .route("/info", get(atomo_info))
        .route("/graphql", post(graphql_handler))
        .route("/graphql", get(graphql_playground))
        .layer(Extension(schema))
        .layer(Extension(atomo))
}
