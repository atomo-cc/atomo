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
    
    fn generate_header() -> String {
        r#"// Auto-generated Rust models from schema.ts
// DO NOT EDIT - This file is automatically generated

use serde::{Deserialize, Serialize};
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
    
    fn generate_model_struct(model: &Model) -> Result<String> {
        let mut code = String::new();
        
        code.push_str(&format!("#[derive(Debug, Clone, Serialize, Deserialize)]\n"));
        code.push_str(&format!("pub struct {} {{\n", model.name));
        
        for (_name, field) in &model.fields {
            let rust_type = Self::convert_field_type(&field.field_type);
            let field_name = Self::to_snake_case(&field.name);
            
            if field.optional {
                code.push_str(&format!("    pub {}: Option<{}>,\n", field_name, rust_type));
            } else {
                code.push_str(&format!("    pub {}: {},\n", field_name, rust_type));
            }
        }
        
        code.push_str("}\n");
        Ok(code)
    }
    
    fn generate_model_impl(model: &Model) -> Result<String> {
        let mut code = String::new();
        
        code.push_str(&format!("impl {} {{\n", model.name));
        
        // Generate new function
        code.push_str("    pub fn new(");
        
        let mut constructor_params = Vec::new();
        for (_name, field) in &model.fields {
            // Skip primary key fields in constructor
            if field.attributes.iter().any(|attr| matches!(attr, crate::types::FieldAttribute::Primary)) {
                continue;
            }
            
            let rust_type = Self::convert_field_type(&field.field_type);
            let field_name = Self::to_snake_case(&field.name);
            
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
            let field_name = Self::to_snake_case(&field.name);
            
            if field.attributes.iter().any(|attr| matches!(attr, crate::types::FieldAttribute::Primary)) {
                code.push_str(&format!("            {}: new_entity_id(),\n", field_name));
            } else if field.attributes.iter().any(|attr| matches!(attr, crate::types::FieldAttribute::Timestamp)) {
                code.push_str(&format!("            {}: Utc::now(),\n", field_name));
            } else {
                code.push_str(&format!("            {},\n", field_name));
            }
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
            let field_name = Self::to_snake_case(&field.name);
            
            code.push_str(&format!("    pub {}: Option<{}>,\n", field_name, rust_type));
        }
        
        code.push_str("}\n");
        
        Ok(code)
    }
    
    fn convert_field_type(field_type: &FieldType) -> String {
        match field_type {
            FieldType::String => "String".to_string(),
            FieldType::Number => "i64".to_string(),
            FieldType::Boolean => "bool".to_string(),
            FieldType::Date => "chrono::NaiveDate".to_string(),
            FieldType::DateTime => "DateTime<Utc>".to_string(),
            FieldType::EntityId => "EntityId".to_string(),
            FieldType::Json => "serde_json::Value".to_string(),
            FieldType::Reference(name) => format!("EntityId /* Reference to {} */", name),
            FieldType::Array(inner_type) => format!("Vec<{}>", Self::convert_field_type(inner_type)),
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
            code.push_str(&format!("#[derive(Debug, Clone, Serialize, Deserialize)]\n"));
            code.push_str(&format!("pub struct {} {{\n", block_model.name));
            
            for (_name, field) in &block_model.fields {
                if field.name == "type" {
                    continue; // Skip type field - handled separately
                }
                
                let rust_type = Self::convert_field_type(&field.field_type);
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