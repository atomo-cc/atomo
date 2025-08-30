use anyhow::Result;
use crate::types::*;
use crate::typescript_parser::TypeScriptParser;
use std::collections::HashMap;
use std::fs;

/// Advanced TypeScript Schema Parser
/// 
/// This parser implements the "Dual-Mode Schema" concept where TypeScript
/// interfaces serve as the single source of truth for both frontend and backend.
/// 
/// Key features:
/// - Comprehensive TypeScript interface parsing
/// - Support for unions, enums, generics, and complex types
/// - Automatic field attribute inference
/// - Relationship detection and mapping
/// - Validation and error reporting
pub struct SchemaParser;

impl SchemaParser {
    /// Parse a TypeScript schema file and extract complete model definitions
    pub fn parse_file(file_path: &str) -> Result<Schema> {
        // Read the TypeScript schema file
        let content = fs::read_to_string(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read schema file '{}': {}", file_path, e))?;
        
        // Use the advanced TypeScript parser
        let parser = TypeScriptParser::new();
        let models = parser.parse(&content)?;
        
        // Convert Vec<Model> to HashMap for Schema
        let mut schema_models = HashMap::new();
        for model in models {
            schema_models.insert(model.name.clone(), model);
        }
        
        // Validate the parsed schema
        Self::validate_schema(&schema_models)?;
        
        Ok(Schema { models: schema_models })
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
                            name, field_name, type_name
                        );
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Check if a type name is a built-in custom type
    fn is_builtin_type(type_name: &str) -> bool {
        matches!(
            type_name,
            "DealStage" | "CompanySize" | "ContactStatus" | "TaskPriority" | 
            "LeadSource" | "OpportunityStage" | "UserRole" | "PermissionLevel"
        )
    }
}
