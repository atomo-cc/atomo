use crate::types::{FieldType, Model, Schema};
use anyhow::Result;

pub struct CodeGenerator;

impl CodeGenerator {
    pub fn generate_rust_models(schema: &Schema) -> Result<String> {
        let mut code = String::new();
        
        // Add header
        code.push_str(&Self::generate_header());
        
        // Generate each model (but skip Block types as they are handled as composable content)
        for (_name, model) in &schema.models {
            // Check if this is an enum (has _enum_type field)
            if model.fields.contains_key("_enum_type") {
                code.push_str(&Self::generate_enum_struct(model)?);
                code.push('\n');
                continue;
            }
            
            // Skip block types - they will be handled as composable content
            if model.name.ends_with("Block") {
                continue;
            }
            
            code.push_str(&Self::generate_model_struct(model)?);
            code.push('\n');
            code.push_str(&Self::generate_model_impl(model)?);
            code.push('\n');
            code.push_str(&Self::generate_model_events(model)?);
            code.push('\n');
        }
        
        // Generate input types for GraphQL mutations
        code.push_str(&Self::generate_rust_input_types(schema)?);
        
        // Generate Block union type for composable content
        code.push_str(&Self::generate_block_types(schema)?);
        
        Ok(code)
    }

    pub fn generate_typescript_types(schema: &Schema) -> Result<String> {
        let mut code = String::new();
        
        // Add header
        code.push_str(&Self::generate_typescript_header());
        
        // Add imports
        code.push_str("import { \n");
        code.push_str("  BaseEntity, \n");
        code.push_str("  Block, \n");
        code.push_str("  CreateInput, \n");
        code.push_str("  UpdateInput,\n");
        code.push_str("  WhereCondition,\n");
        code.push_str("  ApiResponse,\n");
        code.push_str("  PaginationParams\n");
        code.push_str("} from './core-types';\n\n");
        
        // Generate models section
        code.push_str("// ================================\n");
        code.push_str("// 业务模型接口 (从schema.ts生成)\n");
        code.push_str("// ================================\n\n");
        
        // Generate each model interface
        for (_name, model) in &schema.models {
            // Skip block types
            if model.name.ends_with("Block") {
                continue;
            }
            
            code.push_str(&Self::generate_typescript_interface(model)?);
            code.push('\n');
        }
        
        // Generate input types
        code.push_str(&Self::generate_typescript_input_types(schema)?);
        
        // Generate API response types
        code.push_str(&Self::generate_typescript_response_types(schema)?);
        
        // Generate query options
        code.push_str(&Self::generate_typescript_query_options(schema)?);
        
        // Generate stats types
        code.push_str(&Self::generate_typescript_stats_types(schema)?);
        
        Ok(code)
    }

    fn generate_typescript_header() -> String {
        format!(r#"/**
 * 自动生成的业务模型类型
 * 
 * 这个文件由 atomo CLI 从 schema.ts 自动生成
 * 请勿手动编辑 - 所有更改将被覆盖
 * 
 * 生成时间: {}
 * 源文件: packages/atomo-crm-app/atomo/schema.ts
 */

"#, chrono::Utc::now().to_rfc3339())
    }

    fn generate_typescript_interface(model: &Model) -> Result<String> {
        let mut code = String::new();
        
        // Add JSDoc comment
        code.push_str(&format!("/** {} */\n", Self::get_model_description(&model.name)));
        code.push_str(&format!("export interface {} extends BaseEntity {{\n", model.name));
        
        for (_name, field) in &model.fields {
            // Skip base entity fields
            if Self::is_base_entity_field(&field.name) {
                continue;
            }
            
            let ts_type = Self::convert_field_type_to_typescript(&field.field_type);
            let optional_mark = if field.optional { "?" } else { "" };
            
            code.push_str(&format!("  {}{}: {};\n", field.name, optional_mark, ts_type));
        }
        
        code.push_str("}\n");
        Ok(code)
    }

    fn generate_typescript_input_types(schema: &Schema) -> Result<String> {
        let mut code = String::new();
        
        code.push_str("// ================================\n");
        code.push_str("// 输入类型（用于创建和更新）\n");
        code.push_str("// ================================\n\n");
        
        for (_name, model) in &schema.models {
            if model.name.ends_with("Block") {
                continue;
            }
            
            code.push_str(&format!("export type {}CreateInput = CreateInput<{}>;\n", model.name, model.name));
            code.push_str(&format!("export type {}UpdateInput = UpdateInput<{}>;\n", model.name, model.name));
            code.push_str(&format!("export type {}WhereInput = WhereCondition<{}>;\n\n", model.name, model.name));
        }
        
        Ok(code)
    }

    fn generate_typescript_response_types(schema: &Schema) -> Result<String> {
        let mut code = String::new();
        
        code.push_str("// ================================\n");
        code.push_str("// API响应类型\n");
        code.push_str("// ================================\n\n");
        
        for (_name, model) in &schema.models {
            if model.name.ends_with("Block") {
                continue;
            }
            
            code.push_str(&format!("export type {}Response = ApiResponse<{}>;\n", model.name, model.name));
            code.push_str(&format!("export type {}ListResponse = ApiResponse<{}[]>;\n\n", model.name, model.name));
        }
        
        Ok(code)
    }

    fn generate_typescript_query_options(schema: &Schema) -> Result<String> {
        let mut code = String::new();
        
        code.push_str("// ================================\n");
        code.push_str("// 查询选项类型\n");
        code.push_str("// ================================\n\n");
        
        for (_name, model) in &schema.models {
            if model.name.ends_with("Block") {
                continue;
            }
            
            code.push_str(&format!("export interface {}QueryOptions extends PaginationParams {{\n", model.name));
            code.push_str(&format!("  where?: {}WhereInput;\n", model.name));
            code.push_str("  include?: {\n");
            
            // Add related entity includes based on references
            for (_field_name, field) in &model.fields {
                if let FieldType::Reference(ref_name) = &field.field_type {
                    let include_name = Self::to_camel_case(ref_name);
                    code.push_str(&format!("    {}?: boolean;\n", include_name));
                }
            }
            
            code.push_str("  };\n");
            code.push_str("}\n\n");
        }
        
        Ok(code)
    }

    fn generate_typescript_stats_types(schema: &Schema) -> Result<String> {
        let mut code = String::new();
        
        code.push_str("// ================================\n");
        code.push_str("// 统计和聚合类型\n");
        code.push_str("// ================================\n\n");
        
        for (_name, model) in &schema.models {
            if model.name.ends_with("Block") {
                continue;
            }
            
            code.push_str(&format!("export interface {}Stats {{\n", model.name));
            code.push_str(&format!("  total{}s: number;\n", model.name));
            
            // Add model-specific stats
            match model.name.as_str() {
                "Deal" => {
                    code.push_str("  totalValue: number;\n");
                    code.push_str("  averageValue: number;\n");
                    code.push_str("  count: number;\n");
                    code.push_str("  winRate: number;\n");
                    code.push_str("  stageDistribution: Record<DealStage, number>;\n");
                }
                "Company" => {
                    code.push_str("  sizeDistribution: Record<CompanySize, number>;\n");
                    code.push_str("  topIndustries: Array<{ industry: string; count: number }>;\n");
                }
                "Contact" => {
                    code.push_str("  contactsWithoutCompany: number;\n");
                    code.push_str("  contactsWithDeals: number;\n");
                }
                _ => {
                    // Generic stats for other models
                    code.push_str("  createdToday: number;\n");
                    code.push_str("  createdThisWeek: number;\n");
                    code.push_str("  createdThisMonth: number;\n");
                }
            }
            
            code.push_str("}\n\n");
        }
        
        Ok(code)
    }

    fn convert_field_type_to_typescript(field_type: &FieldType) -> String {
        match field_type {
            FieldType::String => "string".to_string(),
            FieldType::Number => "number".to_string(),
            FieldType::Boolean => "boolean".to_string(),
            FieldType::Date => "string".to_string(), // ISO date string
            FieldType::DateTime => "string".to_string(), // ISO datetime string
            FieldType::EntityId => "string".to_string(),
            FieldType::Json => "any".to_string(),
            FieldType::Reference(_) => "string".to_string(), // Entity ID reference
            FieldType::Array(inner_type) => format!("{}[]", Self::convert_field_type_to_typescript(inner_type)),
            FieldType::Blocks => "Block[]".to_string(),
            FieldType::Custom(name) => name.clone(),
        }
    }

    fn get_model_description(model_name: &str) -> String {
        match model_name {
            "Contact" => "联系人实体".to_string(),
            "Company" => "公司实体".to_string(),
            "Deal" => "销售机会实体".to_string(),
            _ => format!("{}实体", model_name),
        }
    }

    fn is_base_entity_field(field_name: &str) -> bool {
        matches!(field_name, "id" | "streamId" | "createdAt" | "updatedAt" | "version")
    }

    fn to_camel_case(s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars();
        
        if let Some(first_char) = chars.next() {
            result.push(first_char.to_lowercase().next().unwrap());
            result.extend(chars);
        }
        
        result
    }

    // Convert camelCase to snake_case
    fn camel_to_snake_case(input: &str) -> String {
        let mut result = String::new();
        let mut chars = input.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch.is_uppercase() && !result.is_empty() {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        }
        
        result
    }
    
    fn generate_header() -> String {
        r#"// Auto-generated Rust models from schema.ts
// DO NOT EDIT - This file is automatically generated

use serde::{Deserialize, Serialize};
use async_graphql::{SimpleObject, Enum, InputObject, ComplexObject, Context, Result as GraphQLResult, ID};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// Type aliases for generated code
pub type EntityId = String;
pub type StreamId = String;
pub type Timestamp = DateTime<Utc>;

// Helper function to generate new EntityIds
fn new_entity_id() -> EntityId {
    Uuid::new_v4().to_string()
}

"#.to_string()
    }
    
    fn generate_enum_struct(model: &Model) -> Result<String> {
        let mut code = String::new();
        
        // Extract enum values from the special fields
        let mut enum_values = Vec::new();
        for (field_name, field) in &model.fields {
            if field_name.starts_with("_enum_value_") {
                enum_values.push(field.name.clone());
            }
        }
        
        // 注意：我们不使用 sqlx::Type derive macro，而是手动实现以避免冲突
        code.push_str(&format!("#[derive(Debug, Clone, Serialize, Deserialize, Enum, Copy, PartialEq, Eq)]\n"));
        code.push_str(&format!("pub enum {} {{\n", model.name));
        
        for value in &enum_values {
            // Convert to PascalCase for Rust enum variants
            let variant_name = value.chars()
                .map(|c| c.to_uppercase().collect::<String>())
                .collect::<String>();
            code.push_str(&format!("    #[serde(rename = \"{}\")]\n", value.to_lowercase()));
            code.push_str(&format!("    {},\n", variant_name));
        }
        
        code.push_str("}\n");
        
        // Add manual sqlx implementations
        code.push_str(&format!(r#"
impl sqlx::Type<sqlx::Postgres> for {} {{
    fn type_info() -> sqlx::postgres::PgTypeInfo {{
        <&str as sqlx::Type<sqlx::Postgres>>::type_info()
    }}
}}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for {} {{
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {{
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        match s {{"#, model.name, model.name));
        
        for value in &enum_values {
            let variant_name = value.chars()
                .map(|c| c.to_uppercase().collect::<String>())
                .collect::<String>();
            code.push_str(&format!("\n            \"{}\" => Ok({}::{}),", value.to_lowercase(), model.name, variant_name));
        }
        
        code.push_str(&format!(r#"
            _ => Err(format!("Invalid {} value: {{}}", s).into()),
        }}
    }}
}}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for {} {{
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {{
        let s = match self {{"#, model.name, model.name));
        
        for value in &enum_values {
            let variant_name = value.chars()
                .map(|c| c.to_uppercase().collect::<String>())
                .collect::<String>();
            code.push_str(&format!("\n            {}::{} => \"{}\",", model.name, variant_name, value.to_lowercase()));
        }
        
        code.push_str(r#"
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&s, buf)
    }
}
"#);
        
        Ok(code)
    }
    
    fn generate_model_struct(model: &Model) -> Result<String> {
        let mut code = String::new();
        
        // Generate database model (for sqlx FromRow)
        code.push_str(&format!("#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]\n"));
        code.push_str(&format!("pub struct {}Row {{\n", model.name));
        
        // Check if the model already has standard fields
        let has_created_at = model.fields.iter().any(|(_, field)| field.name == "createdAt");
        let has_updated_at = model.fields.iter().any(|(_, field)| field.name == "updatedAt");
        let has_version = model.fields.iter().any(|(_, field)| field.name == "version");
        
        for (_name, field) in &model.fields {
            let rust_type = Self::convert_field_type_for_database(&field.field_type);
            // Use snake_case field name for Rust struct to match database columns
            let field_name = Self::camel_to_snake_case(&field.name);
            
            if field.optional {
                code.push_str(&format!("    pub {}: Option<{}>,\n", field_name, rust_type));
            } else {
                code.push_str(&format!("    pub {}: {},\n", field_name, rust_type));
            }
        }
        
        // Add standard Atomo fields only if they don't already exist
        if !has_created_at {
            code.push_str("    pub created_at: DateTime<Utc>,\n");
        }
        if !has_updated_at {
            code.push_str("    pub updated_at: DateTime<Utc>,\n");
        }
        if !has_version {
            code.push_str("    pub version: i32,\n");
        }
        
        code.push_str("}\n\n");
        
        // Generate GraphQL model (for SimpleObject)
        code.push_str(&format!("#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]\n"));
        code.push_str(&format!("pub struct {} {{\n", model.name));
        
        for (_name, field) in &model.fields {
            let rust_type = Self::convert_field_type_for_graphql(&field.field_type);
            // Use snake_case field name for Rust struct to match database columns
            let field_name = Self::camel_to_snake_case(&field.name);
            // Store original name for GraphQL field mapping
            let graphql_field = &field.name;
            
            if field.optional {
                code.push_str(&format!("    #[graphql(name = \"{}\")]\n", graphql_field));
                code.push_str(&format!("    pub {}: Option<{}>,\n", field_name, rust_type));
            } else {
                code.push_str(&format!("    #[graphql(name = \"{}\")]\n", graphql_field));
                code.push_str(&format!("    pub {}: {},\n", field_name, rust_type));
            }
        }
        
        // Add standard Atomo fields only if they don't already exist
        if !has_created_at {
            code.push_str("    #[graphql(name = \"createdAt\")]\n");
            code.push_str("    pub created_at: DateTime<Utc>,\n");
        }
        if !has_updated_at {
            code.push_str("    #[graphql(name = \"updatedAt\")]\n");
            code.push_str("    pub updated_at: DateTime<Utc>,\n");
        }
        if !has_version {
            code.push_str("    pub version: i32,\n");
        }
        
        code.push_str("}\n\n");
        
        // Generate conversion between Row and GraphQL model
        code.push_str(&format!("impl From<{}Row> for {} {{\n", model.name, model.name));
        code.push_str(&format!("    fn from(row: {}Row) -> Self {{\n", model.name));
        code.push_str(&format!("        {} {{\n", model.name));
        
        for (_name, field) in &model.fields {
            let field_name = Self::camel_to_snake_case(&field.name);
            
            match &field.field_type {
                FieldType::Array(_) => {
                    // Convert from sqlx::types::Json<Vec<T>> to Vec<T>
                    if field.optional {
                        code.push_str(&format!("            {}: row.{}.map(|v| v.0),\n", field_name, field_name));
                    } else {
                        code.push_str(&format!("            {}: row.{}.0,\n", field_name, field_name));
                    }
                },
                _ => {
                    code.push_str(&format!("            {}: row.{},\n", field_name, field_name));
                }
            }
        }
        
        // Add standard fields
        if !has_created_at {
            code.push_str("            created_at: row.created_at,\n");
        }
        if !has_updated_at {
            code.push_str("            updated_at: row.updated_at,\n");
        }
        if !has_version {
            code.push_str("            version: row.version,\n");
        }
        
        code.push_str("        }\n");
        code.push_str("    }\n");
        code.push_str("}\n");
        
        Ok(code)
    }
    
    fn generate_model_impl(model: &Model) -> Result<String> {
        let mut code = String::new();
        
        code.push_str(&format!("impl {} {{\n", model.name));
        
        // Generate new function
        code.push_str("    pub fn new(");
        
        // Check if the model already has standard fields
        let has_created_at = model.fields.iter().any(|(_, field)| field.name == "createdAt");
        let has_updated_at = model.fields.iter().any(|(_, field)| field.name == "updatedAt");
        let has_version = model.fields.iter().any(|(_, field)| field.name == "version");
        
        let mut constructor_params = Vec::new();
        for (_name, field) in &model.fields {
            // Skip primary key fields in constructor
            if field.attributes.iter().any(|attr| matches!(attr, crate::types::FieldAttribute::Primary)) {
                continue;
            }
            // Skip timestamp fields - they should be auto-generated
            if field.attributes.iter().any(|attr| matches!(attr, crate::types::FieldAttribute::Timestamp)) {
                continue;
            }
            // Skip standard fields if they already exist in schema
            if field.name == "createdAt" || field.name == "updatedAt" || field.name == "version" {
                continue;
            }
            
            let rust_type = Self::convert_field_type_for_graphql(&field.field_type);
            // Use snake_case field name to match struct
            let field_name = Self::camel_to_snake_case(&field.name);
            
            if field.optional {
                constructor_params.push(format!("{}: Option<{}>", field_name, rust_type));
            } else {
                constructor_params.push(format!("{}: {}", field_name, rust_type));
            }
        }
        
        code.push_str(&constructor_params.join(", "));
        code.push_str(") -> Self {\n");
        code.push_str("        Self {\n");
        
        for (_name, field) in &model.fields {
            // Use snake_case field name to match struct
            let field_name = Self::camel_to_snake_case(&field.name);
            
            if field.attributes.iter().any(|attr| matches!(attr, crate::types::FieldAttribute::Primary)) {
                code.push_str(&format!("            {}: new_entity_id(),\n", field_name));
            } else if field.attributes.iter().any(|attr| matches!(attr, crate::types::FieldAttribute::Timestamp)) {
                code.push_str(&format!("            {}: Utc::now(),\n", field_name));
            } else if field.name == "createdAt" || field.name == "updatedAt" {
                // Handle schema-defined timestamp fields
                let snake_field_name = Self::camel_to_snake_case(&field.name);
                code.push_str(&format!("            {}: Utc::now(),\n", snake_field_name));
            } else if field.name == "version" {
                // Handle schema-defined version field
                code.push_str(&format!("            version: 1,\n"));
            } else {
                code.push_str(&format!("            {},\n", field_name));
            }
        }
        
        // Add Atomo standard fields only if they don't already exist in schema
        if !has_created_at {
            code.push_str("            created_at: Utc::now(),\n");
        }
        if !has_updated_at {
            code.push_str("            updated_at: Utc::now(),\n");
        }
        if !has_version {
            code.push_str("            version: 1,\n");
        }
        
        code.push_str("        }\n");
        code.push_str("    }\n");
        code.push_str("}\n");
        
        Ok(code)
    }
    
    fn generate_model_events(model: &Model) -> Result<String> {
        let mut code = String::new();
        let _model_snake = Self::to_snake_case(&model.name);
        
        // Generate domain events
        code.push_str(&format!("#[derive(Debug, Clone, Serialize, Deserialize)]\n"));
        code.push_str(&format!("pub enum {}Event {{\n", model.name));
        code.push_str(&format!("    Created({}),\n", model.name));
        code.push_str(&format!("    Updated {{ id: EntityId, changes: {}UpdateData }},\n", model.name));
        code.push_str(&format!("    Deleted {{ id: EntityId }},\n"));
        code.push_str("}\n\n");
        
        // Generate update data struct
        code.push_str(&format!("#[derive(Debug, Clone, Serialize, Deserialize)]\n"));
        code.push_str(&format!("pub struct {}UpdateData {{\n", model.name));
        
        for (_name, field) in &model.fields {
            // Skip primary key and timestamp fields in update data
            if field.attributes.iter().any(|attr| matches!(attr, 
                crate::types::FieldAttribute::Primary | crate::types::FieldAttribute::Timestamp)) {
                continue;
            }
            
            let rust_type = Self::convert_field_type(&field.field_type);
            // Use camelCase field name to match struct
            let field_name = &field.name;
            
            code.push_str(&format!("    pub {}: Option<{}>,\n", field_name, rust_type));
        }
        
        code.push_str("}\n");
        
        Ok(code)
    }
    
    fn convert_field_type(field_type: &FieldType) -> String {
        Self::convert_field_type_for_database(field_type)
    }

    // For database models (FromRow)
    fn convert_field_type_for_database(field_type: &FieldType) -> String {
        match field_type {
            FieldType::String => "String".to_string(),
            FieldType::Number => "f64".to_string(), // Use f64 for GraphQL compatibility
            FieldType::Boolean => "bool".to_string(),
            FieldType::Date => "chrono::NaiveDate".to_string(),
            FieldType::DateTime => "DateTime<Utc>".to_string(),
            FieldType::EntityId => "EntityId".to_string(),
            FieldType::Json => "serde_json::Value".to_string(),
            FieldType::Reference(name) => format!("EntityId /* Reference to {} */", name),
            FieldType::Array(inner_type) => {
                // For arrays, use sqlx::types::Json wrapper for database compatibility
                let inner = Self::convert_field_type_for_graphql(inner_type);
                format!("sqlx::types::Json<Vec<{}>>", inner)
            },
            FieldType::Blocks => "serde_json::Value /* Composable Content Blocks */".to_string(),
            FieldType::Custom(name) => {
                // Handle enum types and other custom types
                if name.starts_with('"') && name.ends_with('"') {
                    // String literal type like "call_log"
                    name.clone()
                } else {
                    // Regular custom type like CompanySize, DealStage
                    name.clone()
                }
            },
        }
    }

    // For GraphQL models (SimpleObject, InputObject)
    fn convert_field_type_for_graphql(field_type: &FieldType) -> String {
        match field_type {
            FieldType::String => "String".to_string(),
            FieldType::Number => "f64".to_string(), // Use f64 for GraphQL compatibility
            FieldType::Boolean => "bool".to_string(),
            FieldType::Date => "chrono::NaiveDate".to_string(),
            FieldType::DateTime => "DateTime<Utc>".to_string(),
            FieldType::EntityId => "EntityId".to_string(),
            FieldType::Json => "serde_json::Value".to_string(),
            FieldType::Reference(name) => format!("EntityId /* Reference to {} */", name),
            FieldType::Array(inner_type) => {
                // For GraphQL arrays, use plain Vec<T>
                let inner = Self::convert_field_type_for_graphql(inner_type);
                format!("Vec<{}>", inner)
            },
            FieldType::Blocks => "serde_json::Value /* Composable Content Blocks */".to_string(),
            FieldType::Custom(name) => {
                // Handle enum types and other custom types
                if name.starts_with('"') && name.ends_with('"') {
                    // String literal type like "call_log"
                    name.clone()
                } else {
                    // Regular custom type like CompanySize, DealStage
                    name.clone()
                }
            },
        }
    }
    
    fn generate_block_types(schema: &Schema) -> Result<String> {
        let mut code = String::new();
        let mut block_types = Vec::new();
        
        // Find all Block types
        for (_name, model) in &schema.models {
            if model.name.ends_with("Block") {
                block_types.push(model);
            }
        }
        
        if block_types.is_empty() {
            return Ok(code);
        }
        
        // Generate individual block structs
        for block_model in &block_types {
            code.push_str(&format!("#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]\n"));
            code.push_str(&format!("pub struct {} {{\n", block_model.name));
            
            for (_name, field) in &block_model.fields {
                if field.name == "type" {
                    continue; // Skip type field - handled separately
                }
                
                let rust_type = Self::convert_field_type_for_graphql(&field.field_type);
                let field_name = Self::to_snake_case(&field.name);
                
                if field.optional {
                    code.push_str(&format!("    pub {}: Option<{}>,\n", field_name, rust_type));
                } else {
                    code.push_str(&format!("    pub {}: {},\n", field_name, rust_type));
                }
            }
            
            code.push_str("}\n\n");
        }
        
        // Generate Block union enum
        code.push_str("/// Composable content blocks - Atomo's \"流动的画布\"\n");
        code.push_str("#[derive(Debug, Clone, Serialize, Deserialize)]\n");
        code.push_str("#[serde(tag = \"type\")]\n");
        code.push_str("pub enum Block {\n");
        
        for block_model in &block_types {
            let type_value = block_model.name.strip_suffix("Block").unwrap_or(&block_model.name);
            let snake_case_type = Self::to_snake_case(type_value);
            code.push_str(&format!("    #[serde(rename = \"{}\")]\n", snake_case_type));
            code.push_str(&format!("    {}({}),\n", type_value, block_model.name));
        }
        
        code.push_str("}\n\n");
        
        Ok(code)
    }
    
    fn generate_rust_input_types(schema: &Schema) -> Result<String> {
        let mut code = String::new();
        
        code.push_str("// ================================\n");
        code.push_str("// GraphQL Input Types\n");
        code.push_str("// ================================\n\n");
        
        for (_name, model) in &schema.models {
            // Skip enum and block types
            if model.fields.contains_key("_enum_type") || model.name.ends_with("Block") {
                continue;
            }
            
            // Generate Create Input
            code.push_str(&format!("/// Input type for creating {}\n", model.name));
            code.push_str("#[derive(InputObject, Debug, Clone)]\n");
            code.push_str(&format!("pub struct Create{}Input {{\n", model.name));
            
            for (_field_name, field) in &model.fields {
                // Skip base entity fields for create input
                if Self::is_base_entity_field(&field.name) {
                    continue;
                }
                
                let rust_type = Self::convert_field_type_for_graphql(&field.field_type);
                let field_def = if field.optional {
                    format!("    pub {}: Option<{}>,\n", field.name, rust_type)
                } else {
                    format!("    pub {}: {},\n", field.name, rust_type)
                };
                code.push_str(&field_def);
            }
            
            code.push_str("}\n\n");
            
            // Generate Update Data Input (nested structure)
            code.push_str(&format!("/// Input type for updating {} data\n", model.name));
            code.push_str("#[derive(InputObject, Debug, Clone)]\n");
            code.push_str(&format!("pub struct Update{}DataInput {{\n", model.name));
            
            for (_field_name, field) in &model.fields {
                // Skip base entity fields for update data input
                if Self::is_base_entity_field(&field.name) {
                    continue;
                }
                
                let rust_type = Self::convert_field_type_for_graphql(&field.field_type);
                // All fields are optional in update
                let field_def = format!("    pub {}: Option<{}>,\n", field.name, rust_type);
                code.push_str(&field_def);
            }
            
            code.push_str("}\n\n");
            
            // Generate Update Input
            code.push_str(&format!("/// Input type for updating {}\n", model.name));
            code.push_str("#[derive(InputObject, Debug, Clone)]\n");
            code.push_str(&format!("pub struct Update{}Input {{\n", model.name));
            code.push_str("    pub id: ID,\n");
            code.push_str(&format!("    pub data: Update{}DataInput,\n", model.name));
            code.push_str("}\n\n");
        }
        
        Ok(code)
    }
    
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
}