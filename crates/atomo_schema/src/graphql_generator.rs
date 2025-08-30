use anyhow::Result;
use crate::types::*;

/// GraphQL Schema Generator
/// Generates GraphQL type definitions from parsed TypeScript models
pub struct GraphQLGenerator;

impl GraphQLGenerator {
    pub fn new() -> Self {
        Self
    }
    
    /// Generate complete GraphQL schema from models
    pub fn generate_schema(&self, models: &[Model]) -> Result<String> {
        let mut schema = String::new();
        
        // Add schema header
        schema.push_str("# Auto-generated GraphQL schema\n");
        schema.push_str(&format!("# Generated at: {}\n", chrono::Utc::now().to_rfc3339()));
        schema.push_str("\n");
        
        // Add scalar types
        schema.push_str(include_str!("templates/scalars.graphql"));
        schema.push_str("\n");
        
        // Generate types for each model
        for model in models {
            schema.push_str(&self.generate_type_definitions(model)?);
            schema.push_str("\n");
        }
        
        // Generate Query root
        schema.push_str(&self.generate_query_type(models)?);
        schema.push_str("\n");
        
        // Generate Mutation root
        schema.push_str(&self.generate_mutation_type(models)?);
        schema.push_str("\n");
        
        // Add schema definition
        schema.push_str("schema {\n");
        schema.push_str("  query: Query\n");
        schema.push_str("  mutation: Mutation\n");
        schema.push_str("}\n");
        
        Ok(schema)
    }
    
    /// Generate GraphQL type definitions for a model
    fn generate_type_definitions(&self, model: &Model) -> Result<String> {
        let mut output = String::new();
        
        // Main type
        output.push_str(&format!("# {} entity\n", model.name));
        output.push_str(&format!("type {} {{\n", model.name));
        
        // Add fields
        for (field_name, field) in &model.fields {
            let graphql_type = self.map_field_type_to_graphql(&field.field_type, !field.optional)?;
            output.push_str(&format!("  {}: {}\n", field_name, graphql_type));
        }
        
        // Add audit fields
        output.push_str("  createdAt: DateTime!\n");
        output.push_str("  updatedAt: DateTime!\n");
        output.push_str("  version: Int!\n");
        
        output.push_str("}\n\n");
        
        // Input types for creation and updates
        output.push_str(&self.generate_input_types(model)?);
        
        // Filter and pagination types
        output.push_str(&self.generate_filter_types(model)?);
        
        Ok(output)
    }
    
    /// Generate input types for creation and updates
    fn generate_input_types(&self, model: &Model) -> Result<String> {
        let mut output = String::new();
        
        // Create input
        output.push_str(&format!("input Create{}Input {{\n", model.name));
        for (field_name, field) in &model.fields {
            if field_name == "id" || field_name == "createdAt" || field_name == "updatedAt" { 
                continue; // Skip auto-generated fields
            }
            let graphql_type = self.map_field_type_to_graphql(&field.field_type, !field.optional)?;
            output.push_str(&format!("  {}: {}\n", field_name, graphql_type));
        }
        output.push_str("}\n\n");
        
        // Update data input (nested structure)
        output.push_str(&format!("input Update{}DataInput {{\n", model.name));
        for (field_name, field) in &model.fields {
            if field_name == "id" || field_name == "createdAt" || field_name == "updatedAt" { 
                continue; // Skip non-updatable fields
            }
            let graphql_type = self.map_field_type_to_graphql(&field.field_type, false)?; // All optional for updates
            output.push_str(&format!("  {}: {}\n", field_name, graphql_type));
        }
        output.push_str("}\n\n");
        
        // Update input (with ID and data)
        output.push_str(&format!("input Update{}Input {{\n", model.name));
        output.push_str("  id: ID!\n");
        output.push_str(&format!("  data: Update{}DataInput!\n", model.name));
        output.push_str("}\n\n");
        
        Ok(output)
    }
    
    /// Generate filter and pagination types
    fn generate_filter_types(&self, model: &Model) -> Result<String> {
        let mut output = String::new();
        
        // Filter input
        output.push_str(&format!("input {}Filter {{\n", model.name));
        output.push_str("  # Search by ID\n");
        output.push_str("  id: ID\n");
        output.push_str("  ids: [ID!]\n");
        
        // Add filterable fields
        for (field_name, field) in &model.fields {
            if field_name == "id" { continue; }
            match &field.field_type {
                FieldType::String => {
                    output.push_str(&format!("  {}: String\n", field_name));
                    output.push_str(&format!("  {}Contains: String\n", field_name));
                }
                FieldType::Number => {
                    output.push_str(&format!("  {}: Float\n", field_name));
                    output.push_str(&format!("  {}Gte: Float\n", field_name));
                    output.push_str(&format!("  {}Lte: Float\n", field_name));
                }
                FieldType::Boolean => {
                    output.push_str(&format!("  {}: Boolean\n", field_name));
                }
                FieldType::Date | FieldType::DateTime => {
                    output.push_str(&format!("  {}After: DateTime\n", field_name));
                    output.push_str(&format!("  {}Before: DateTime\n", field_name));
                }
                _ => {
                    // For other types, just basic equality
                    let graphql_type = self.map_field_type_to_graphql(&field.field_type, false)?;
                    output.push_str(&format!("  {}: {}\n", field_name, graphql_type));
                }
            }
        }
        
        output.push_str("  # Date range filters\n");
        output.push_str("  createdAfter: DateTime\n");
        output.push_str("  createdBefore: DateTime\n");
        output.push_str("  updatedAfter: DateTime\n");
        output.push_str("  updatedBefore: DateTime\n");
        output.push_str("}\n\n");
        
        // Order by enum
        output.push_str(&format!("enum {}OrderBy {{\n", model.name));
        output.push_str("  CREATED_AT_ASC\n");
        output.push_str("  CREATED_AT_DESC\n");
        output.push_str("  UPDATED_AT_ASC\n");
        output.push_str("  UPDATED_AT_DESC\n");
        
        // Add field-specific ordering
        for (field_name, _) in &model.fields {
            let upper_field = field_name.to_uppercase();
            output.push_str(&format!("  {}_ASC\n", upper_field));
            output.push_str(&format!("  {}_DESC\n", upper_field));
        }
        
        output.push_str("}\n\n");
        
        // Connection types for pagination
        output.push_str(&self.generate_connection_types(model)?);
        
        Ok(output)
    }
    
    /// Generate connection types for cursor-based pagination
    fn generate_connection_types(&self, model: &Model) -> Result<String> {
        let mut output = String::new();
        
        output.push_str(&format!("type {}Edge {{\n", model.name));
        output.push_str(&format!("  node: {}!\n", model.name));
        output.push_str("  cursor: String!\n");
        output.push_str("}\n\n");
        
        output.push_str(&format!("type {}Connection {{\n", model.name));
        output.push_str(&format!("  edges: [{}Edge!]!\n", model.name));
        output.push_str("  pageInfo: PageInfo!\n");
        output.push_str("  totalCount: Int!\n");
        output.push_str("}\n\n");
        
        Ok(output)
    }
    
    /// Generate Query type with all model queries
    fn generate_query_type(&self, models: &[Model]) -> Result<String> {
        let mut output = String::new();
        
        output.push_str("type Query {\n");
        output.push_str("  # Health check\n");
        output.push_str("  health: String!\n\n");
        
        for model in models {
            let _model_lower = model.name.to_lowercase();
            let model_plural = format!("{}s", model.name.to_lowercase()); // Simple pluralization
            
            // Single entity queries
            output.push_str(&format!("  # Get single {} by ID\n", model.name));
            output.push_str(&format!("  {}: {}\n", model.name.to_lowercase(), model.name));
            output.push_str(&format!("  {}ById(id: ID!): {}\n", model.name.to_lowercase(), model.name));
            
            // List queries with filtering and pagination
            output.push_str(&format!("  # List {} with filtering and pagination\n", model.name));
            output.push_str(&format!(
                "  {}(\n    filter: {}Filter\n    orderBy: {}OrderBy\n    first: Int\n    after: String\n    last: Int\n    before: String\n  ): {}Connection!\n", 
                model_plural, model.name, model.name, model.name
            ));
            output.push_str("\n");
        }
        
        output.push_str("}\n");
        
        Ok(output)
    }
    
    /// Generate Mutation type with all model mutations
    fn generate_mutation_type(&self, models: &[Model]) -> Result<String> {
        let mut output = String::new();
        
        output.push_str("type Mutation {\n");
        
        for model in models {
            let model_lower = model.name.to_lowercase();
            
            // Create mutation
            output.push_str(&format!("  # Create new {}\n", model.name));
            output.push_str(&format!(
                "  create{}(input: Create{}Input!): {}!\n", 
                model.name, model.name, model.name
            ));
            
            // Update mutation
            output.push_str(&format!("  # Update existing {}\n", model.name));
            output.push_str(&format!(
                "  update{}(input: Update{}Input!): {}!\n", 
                model.name, model.name, model.name
            ));
            
            // Delete mutation
            output.push_str(&format!("  # Delete {} by ID\n", model.name));
            output.push_str(&format!(
                "  delete{}(id: ID!): Boolean!\n", 
                model.name
            ));
            
            output.push_str("\n");
        }
        
        output.push_str("}\n");
        
        Ok(output)
    }
    
    /// Map TypeScript field types to GraphQL types
    fn map_field_type_to_graphql(&self, field_type: &FieldType, required: bool) -> Result<String> {
        let base_type = match field_type {
            FieldType::String => "String",
            FieldType::Number => "Float",
            FieldType::Boolean => "Boolean",
            FieldType::Date => "Date",
            FieldType::DateTime => "DateTime",
            FieldType::EntityId => "ID",
            FieldType::Json => "JSON",
            FieldType::Reference(_) => "ID",
            FieldType::Array(inner) => {
                let inner_type = self.map_field_type_to_graphql(inner, true)?;
                return Ok(if required {
                    format!("[{}!]!", inner_type)
                } else {
                    format!("[{}!]", inner_type)
                });
            }
            FieldType::Blocks => "JSON",
            FieldType::Custom(type_name) => type_name,
        };
        
        Ok(if required {
            format!("{}!", base_type)
        } else {
            base_type.to_string()
        })
    }
}

impl Default for GraphQLGenerator {
    fn default() -> Self {
        Self::new()
    }
}
