use crate::types::*;
use crate::typescript_parser::TypeScriptParser;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;

/// Advanced TypeScript Schema Parser
///
/// Implements the "Dual-Mode Schema" concept where TypeScript interfaces serve
/// as the single source of truth. Parses models, events, and actions from the
/// schema DSL into a unified `Schema` struct.
pub struct SchemaParser;

impl SchemaParser {
    pub fn parse_file(file_path: &str) -> Result<Schema> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read schema file '{}': {}", file_path, e))?;

        let parser = TypeScriptParser::new();
        let schema = parser.parse_schema(&content)?;
        Self::validate_schema(&schema.models)?;
        Ok(schema)
    }

    /// Validate the parsed schema for consistency and completeness
    fn validate_schema(models: &HashMap<String, Model>) -> Result<()> {
        for (name, model) in models {
            // Check for required fields
            if model.fields.is_empty() {
                anyhow::bail!("Model '{}' has no fields defined", name);
            }

            // Validate that referenced types exist
            for (field_name, field) in &model.fields {
                if let FieldType::Custom(type_name) = &field.field_type {
                    // Skip built-in custom types
                    if !Self::is_builtin_type(type_name) && !models.contains_key(type_name) {
                        anyhow::bail!(
                            "Field '{}.{}' references unknown type '{}'",
                            name,
                            field_name,
                            type_name
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a type name is a built-in custom type
    fn is_builtin_type(_type_name: &str) -> bool {
        // Platform core should not have knowledge of specific business domain types
        // All domain-specific types should be defined in their respective services
        false
    }
}
