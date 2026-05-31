//! Plugin system for extending Atomo server functionality
//!
//! This module provides a plugin architecture that allows business domains
//! to extend the server's GraphQL schema and HTTP handlers without modifying
//! the core platform code.

use async_graphql::SchemaBuilder;
use axum::Router;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use crate::handlers::{Query, Mutation};
use atomo::graphql::Subscription;

/// Plugin trait for extending GraphQL schema
#[async_trait::async_trait]
pub trait GraphQLPlugin: Send + Sync {
    /// Get the plugin name
    fn name(&self) -> &'static str;

    /// Register GraphQL queries
    async fn register_queries(&self, _schema_builder: &mut SchemaBuilder<Query, Mutation, Subscription>) {}

    /// Register GraphQL mutations
    async fn register_mutations(&self, _schema_builder: &mut SchemaBuilder<Query, Mutation, Subscription>) {}

    /// Register HTTP routes
    async fn register_routes(&self, _router: Router) -> Router {
        _router
    }
}

/// Plugin manager for coordinating multiple plugins
pub struct PluginManager {
    plugins: Vec<Box<dyn GraphQLPlugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a new plugin
    pub fn register_plugin<P: GraphQLPlugin + 'static>(&mut self, plugin: P) {
        self.plugins.push(Box::new(plugin));
    }

    /// Get all registered plugins
    pub fn plugins(&self) -> &[Box<dyn GraphQLPlugin>] {
        &self.plugins
    }

    /// Apply all plugins to a GraphQL schema builder
    pub async fn apply_to_schema(&self, schema_builder: &mut SchemaBuilder<Query, Mutation, Subscription>) {
        for plugin in &self.plugins {
            plugin.register_queries(schema_builder).await;
            plugin.register_mutations(schema_builder).await;
        }
    }

    /// Apply all plugins to a router
    pub async fn apply_to_router(&self, router: Router) -> Router {
        let mut router = router;
        for plugin in &self.plugins {
            router = plugin.register_routes(router).await;
        }
        router
    }
}

/// Global plugin manager instance
static PLUGIN_MANAGER: Lazy<Mutex<PluginManager>> = Lazy::new(|| Mutex::new(PluginManager::new()));

/// CRM GraphQL Plugin
/// This plugin provides CRM-specific GraphQL operations
pub struct CrmGraphQLPlugin;

impl CrmGraphQLPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl GraphQLPlugin for CrmGraphQLPlugin {
    fn name(&self) -> &'static str {
        "crm"
    }

    async fn register_mutations(&self, _schema_builder: &mut SchemaBuilder<Query, Mutation, Subscription>) {
        // Register CRM-specific mutations
        // This would include deal position updates, etc.
    }
}

/// Initialize default plugins
pub fn init_default_plugins() {
    let crm_plugin = CrmGraphQLPlugin::new();
    let mut manager = PLUGIN_MANAGER.lock().unwrap();
    manager.register_plugin(crm_plugin);
}
