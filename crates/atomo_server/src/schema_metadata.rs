//! Schema metadata extraction for the Admin UI
//!
//! This module provides functionality to extract schema metadata from Atomo instances
//! and convert it into a format suitable for the Admin UI to dynamically render forms,
//! tables, and other components.

use atomo::schema::{FieldAttribute, FieldType};
use atomo::Atomo;
use serde_json::{json, Value};

/// Extract schema metadata from an Atomo instance for the Admin UI.
/// Builds the `{ models: { Name: { tableName, primaryKey, fields, relationships, validation } } }`
/// structure the UI consumes to render tables/forms — driven by the real loaded schema.
pub fn extract_schema_metadata(atomo: &Atomo) -> Value {
    let schema = atomo.schema();
    let mut models = serde_json::Map::new();

    for (name, model) in &schema.models {
        // Enum pseudo-models / block sub-types have no real `id` — skip them.
        if !model.fields.contains_key("id") {
            continue;
        }
        let mut fields = serde_json::Map::new();
        for (fname, field) in &model.fields {
            let attrs: Vec<&str> = field
                .attributes
                .iter()
                .map(|a| match a {
                    FieldAttribute::Primary => "primary",
                    FieldAttribute::Unique => "unique",
                    FieldAttribute::Index => "index",
                    FieldAttribute::ForeignKey => "foreignKey",
                    FieldAttribute::Timestamp => "timestamp",
                    FieldAttribute::Default(_) => "default",
                })
                .collect();
            fields.insert(
                fname.clone(),
                json!({
                    "name": fname,
                    "type": field_type_str(&field.field_type),
                    "optional": field.optional,
                    "attributes": attrs,
                }),
            );
        }

        let relationships: serde_json::Map<String, Value> = model
            .relationships
            .iter()
            .map(|(rn, r)| {
                (
                    rn.clone(),
                    json!({ "type": r.kind, "model": r.model, "foreignKey": r.foreign_key }),
                )
            })
            .collect();

        models.insert(
            name.clone(),
            json!({
                "tableName": atomo::query::sql_builder::table_name_for(model),
                "primaryKey": "id",
                "fields": fields,
                "relationships": relationships,
                "validation": model.validation,
            }),
        );
    }

    json!({
        "models": models,
        "version": "1.0.0",
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "platform": { "name": "Atomo", "version": env!("CARGO_PKG_VERSION") }
    })
}

/// Map a Rust FieldType to the UI's field-type string.
fn field_type_str(t: &FieldType) -> &'static str {
    match t {
        FieldType::String | FieldType::EntityId | FieldType::Reference(_) => "string",
        FieldType::File => "file",
        FieldType::Number => "number",
        FieldType::Boolean => "boolean",
        FieldType::Date => "date",
        FieldType::DateTime => "datetime",
        FieldType::Json | FieldType::Custom(_) => "json",
        // A File[] field renders the multi-file uploader, not the generic array widget.
        FieldType::Array(inner) if matches!(**inner, FieldType::File) => "file",
        FieldType::Array(_) | FieldType::Blocks => "array",
    }
}

#[cfg(test)]
mod tests {
    use super::field_type_str;
    use atomo::schema::FieldType;

    #[test]
    fn file_field_maps_to_file_metadata_type() {
        // Drives the Admin UI auto-rendering the uploader for File-declared fields.
        assert_eq!(field_type_str(&FieldType::File), "file");
        assert_eq!(field_type_str(&FieldType::String), "string");
        // File[] also renders the (multi-file) uploader, not the generic array widget.
        assert_eq!(
            field_type_str(&FieldType::Array(Box::new(FieldType::File))),
            "file"
        );
        assert_eq!(
            field_type_str(&FieldType::Array(Box::new(FieldType::String))),
            "array"
        );
    }
}
