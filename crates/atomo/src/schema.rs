//! Schema parsing and management
//!
//! This module handles parsing TypeScript schema files and converting them
//! into Rust types for the Atomo runtime.

use anyhow::Result;
use std::collections::HashMap;

// Re-export from atomo_schema for compatibility
pub use atomo_schema::{Field, FieldAttribute, FieldType, Model, Schema, TypeScriptParser};

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

    for model in schema.models.values() {
        let table = crate::query::sql_builder::table_name_for(model);
        let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (\n", table);

        let mut columns = Vec::new();
        for field in model.fields.values() {
            let col = to_snake_case(&field.name);
            let column_type = field_type_to_sql(&field.field_type);
            // Primary key: id column gets a UUID default and PRIMARY KEY constraint
            let is_primary = field.name == "id"
                || field
                    .attributes
                    .iter()
                    .any(|a| matches!(a, FieldAttribute::Primary));
            if is_primary {
                columns.push(format!(
                    "  {} {} PRIMARY KEY DEFAULT gen_random_uuid()",
                    col, column_type
                ));
                continue;
            }
            // Timestamp columns default to NOW()
            let default = match (col.as_str(), &field.field_type) {
                ("created_at" | "updated_at", FieldType::DateTime) => " DEFAULT NOW()",
                _ => "",
            };
            let nullable = if field.optional { "" } else { " NOT NULL" };
            columns.push(format!("  {} {}{}{}", col, column_type, default, nullable));
        }

        // Add soft delete column
        columns.push("  deleted_at TIMESTAMPTZ".to_string());
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
    let chars = s.chars().peekable();

    for c in chars {
        if c.is_uppercase() && !result.is_empty() {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }

    result
}
