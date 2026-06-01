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
            // Primary key: id is TEXT (EntityId is a ULID string; matches CRM `id: string`).
            // Default generates a value DB-side so inserts without an explicit id still work.
            let is_primary = field.name == "id"
                || field
                    .attributes
                    .iter()
                    .any(|a| matches!(a, FieldAttribute::Primary));
            if is_primary {
                columns.push(format!(
                    "  {} TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text",
                    col
                ));
                continue;
            }
            // Defaults: timestamps -> NOW(); JSON array-ish fields (arrays/blocks) -> '[]'
            // so a required `notes: ContentBlock[]` doesn't force every insert to pass it.
            let default = match (col.as_str(), &field.field_type) {
                ("created_at" | "updated_at", FieldType::DateTime) => " DEFAULT NOW()".to_string(),
                (_, FieldType::Array(_) | FieldType::Blocks) => " DEFAULT '[]'::jsonb".to_string(),
                _ => String::new(),
            };
            let nullable = if field.optional { "" } else { " NOT NULL" };
            columns.push(format!("  {} {}{}{}", col, column_type, default, nullable));
        }

        // Add soft delete column
        columns.push("  deleted_at TIMESTAMPTZ".to_string());
        // Multi-tenant scoping column. Nullable so single-tenant deployments (no TenantCtx)
        // simply insert NULL and the tenant WHERE clause is never added — fully backward
        // compatible. When a TenantCtx is present, writes set it and reads filter on it.
        columns.push("  tenant_id TEXT".to_string());
        sql.push_str(&columns.join(",\n"));
        sql.push_str("\n);");

        migrations.push(sql);
    }

    // Foreign-key pass (after all tables exist, so ordering doesn't matter): for each model's
    // `belongsTo` relationship with a foreignKey, add a FK to the target table's id. This is what
    // actually enforces referential integrity (the `exists:` validation rule is a no-op). Emitted
    // as guarded ALTERs (idempotent: skip if the constraint already exists). FKs are NOT VALID-free
    // here — they validate existing rows; tables are fresh at create time so that's fine.
    let table_of: HashMap<&str, String> = schema
        .models
        .values()
        .map(|m| {
            (
                m.name.as_str(),
                crate::query::sql_builder::table_name_for(m),
            )
        })
        .collect();
    for model in schema.models.values() {
        let table = crate::query::sql_builder::table_name_for(model);
        for (rel_name, rel) in &model.relationships {
            if rel.kind != "belongsTo" {
                continue;
            }
            let fk_raw = rel
                .foreign_key
                .clone()
                .unwrap_or_else(|| format!("{}Id", rel_name));
            let fk_col = to_snake_case(&fk_raw);
            let target = match table_of.get(rel.model.as_str()) {
                Some(t) => t,
                None => continue, // target model not in schema — skip
            };
            // Skip if the model doesn't actually have the FK column (defensive).
            if !model.fields.keys().any(|f| to_snake_case(f) == fk_col) {
                continue;
            }
            let cname = format!("fk_{}_{}", table, fk_col);
            migrations.push(format!(
                "DO $$ BEGIN \
                 IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = '{cname}') THEN \
                 ALTER TABLE {table} ADD CONSTRAINT {cname} FOREIGN KEY ({fk_col}) REFERENCES {target}(id); \
                 END IF; END $$;"
            ));
        }
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
        FieldType::EntityId => "TEXT",
        FieldType::Json => "JSONB",
        FieldType::Reference(_) => "TEXT",
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
