//! Dynamic model registry for Atomo server
//!
//! This module provides a plugin-based system for registering business domain models
//! without hardcoding them in the platform core.

use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::RwLock;

/// Model definition structure
#[derive(Debug, Clone)]
pub struct ModelDefinition {
    pub table_name: String,
    pub primary_key: String,
    pub searchable: Vec<String>,
    pub relationships: Value,
    pub validation: Value,
    pub ui: Value,
    pub fields: Value,
}

/// Dynamic model registry
pub struct ModelRegistry {
    models: RwLock<HashMap<String, ModelDefinition>>,
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            models: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new model definition
    pub fn register_model(&self, name: String, definition: ModelDefinition) {
        let mut models = self.models.write().unwrap();
        models.insert(name, definition);
    }

    /// Get a model definition by name
    pub fn get_model(&self, name: &str) -> Option<ModelDefinition> {
        let models = self.models.read().unwrap();
        models.get(name).cloned()
    }

    /// Get all registered models as JSON metadata
    pub fn get_all_models_metadata(&self) -> Value {
        let models = self.models.read().unwrap();
        let mut result = json!({});

        for (name, definition) in models.iter() {
            result[name] = json!({
                "tableName": definition.table_name,
                "primaryKey": definition.primary_key,
                "searchable": definition.searchable,
                "relationships": definition.relationships,
                "validation": definition.validation,
                "ui": definition.ui,
                "fields": definition.fields,
            });
        }

        result
    }

    /// Check if a model is registered
    pub fn has_model(&self, name: &str) -> bool {
        let models = self.models.read().unwrap();
        models.contains_key(name)
    }
}

/// Global model registry instance
static MODEL_REGISTRY: Lazy<ModelRegistry> = Lazy::new(ModelRegistry::new);

/// Helper function to register CRM models (for backward compatibility)
/// Note: This is a temporary function for backward compatibility.
/// In the future, models should be registered dynamically from service schemas.
pub fn register_crm_models() {
    // This function is kept for backward compatibility during the transition
    // In the final architecture, models will be registered dynamically
    // from individual service schemas, not hardcoded in the platform core.

    // For now, we keep the existing hardcoded models to maintain functionality
    // TODO: Replace with dynamic model loading from service schemas
}
