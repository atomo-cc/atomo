//! Schema parsing and management
//!
//! This module handles parsing TypeScript schema files and converting them
//! into Rust types for the Atomo runtime.

use std::collections::HashMap;
use anyhow::Result;

// Re-export from atomo_schema for compatibility
pub use atomo_schema::{TypeScriptParser, Schema, Model, Field, FieldType, FieldAttribute};

/// Parse a TypeScript schema string into a Schema object
pub fn parse_typescript_schema(content: &str) -> Result<Schema> {
    let parser = TypeScriptParser::new();
    let models = parser.parse(content)?;
    
    // Convert Vec<Model> to HashMap<String, Model>
    let mut schema_models = HashMap::new();
    for model in models {
        schema_models.insert(model.name.clone(), model);
    }
    
    Ok(Schema {
        models: schema_models,
    })
}

/// Generate database migrations from schema
pub fn generate_migrations(schema: &Schema) -> Result<Vec<String>> {
    let mut migrations = Vec::new();
    
    for (_, model) in &schema.models {
        let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (\n", to_snake_case(&model.name));
        
        let mut columns = Vec::new();
        for (_, field) in &model.fields {
            let column_type = field_type_to_sql(&field.field_type);
            let nullable = if field.optional { "" } else { " NOT NULL" };
            
            columns.push(format!("  {} {}{}", to_snake_case(&field.name), column_type, nullable));
        }
        
        sql.push_str(&columns.join(",\n"));
        sql.push_str("\n);");
        
        migrations.push(sql);
    }
    
    Ok(migrations)
}

/// Convert FieldType to SQL type
fn field_type_to_sql(field_type: &FieldType) -> &'static str {
    match field_type {
        FieldType::String => "TEXT",
        FieldType::Number => "BIGINT",
        FieldType::Boolean => "BOOLEAN",
        FieldType::Date => "DATE",
        FieldType::DateTime => "TIMESTAMPTZ",
        FieldType::EntityId => "UUID",
        FieldType::Json => "JSONB",
        FieldType::Reference(_) => "UUID",
        FieldType::Array(_) => "JSONB",
        FieldType::Blocks => "JSONB",
        FieldType::Custom(_) => "JSONB",
    }
}

/// Convert camelCase to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c.is_uppercase() && !result.is_empty() {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    
    result
}
