//! Hook and Access Control Code Generator
//!
//! This module generates Rust code for the TypeScript DSL-defined hooks and access control rules.
//! It creates type-safe Rust functions that can be executed at runtime.

use crate::types::*;
use anyhow::Result;

/// Generator for hooks and access control Rust code
pub struct HookAccessGenerator;

impl Default for HookAccessGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl HookAccessGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Generate complete hook and access control module
    pub fn generate_module(&self, models: &[Model]) -> Result<String> {
        let mut code = String::new();

        // Module header
        code.push_str(&self.generate_header());

        // Generate access control structures
        code.push_str(&self.generate_access_structures()?);

        // Generate hook structures
        code.push_str(&self.generate_hook_structures()?);

        // Generate model-specific implementations
        for model in models {
            if model.access.is_some() || model.hooks.is_some() {
                code.push_str(&self.generate_model_implementation(model)?);
            }
        }

        // Generate runtime execution framework
        code.push_str(&self.generate_runtime_framework()?);

        Ok(code)
    }

    /// Generate module header with imports
    fn generate_header(&self) -> String {
        r#"//! Auto-generated Hook and Access Control Module
//! This module contains the compiled TypeScript DSL hooks and access rules.

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

// Type definitions for the hook and access control system

"#
        .to_string()
    }

    /// Generate access control type structures
    fn generate_access_structures(&self) -> Result<String> {
        Ok(r#"/// User context for access control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessContext {
    pub user: Option<Value>,
    pub operation: String,
    pub resource_id: Option<String>,
}

/// Query builder for access control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessQuery {
    pub conditions: Vec<QueryCondition>,
    pub operator: LogicalOperator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCondition {
    pub field: String,
    pub operator: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogicalOperator {
    And,
    Or,
}

impl AccessQuery {
    pub fn new() -> Self {
        Self {
            conditions: Vec::new(),
            operator: LogicalOperator::And,
        }
    }
    
    pub fn where_clause(field: &str, operator: &str, value: Value) -> Self {
        Self {
            conditions: vec![QueryCondition {
                field: field.to_string(),
                operator: operator.to_string(),
                value,
            }],
            operator: LogicalOperator::And,
        }
    }
    
    pub fn or(queries: Vec<AccessQuery>) -> Self {
        let mut all_conditions = Vec::new();
        for query in queries {
            all_conditions.extend(query.conditions);
        }
        Self {
            conditions: all_conditions,
            operator: LogicalOperator::Or,
        }
    }
}

"#
        .to_string())
    }

    /// Generate hook type structures
    fn generate_hook_structures(&self) -> Result<String> {
        Ok(r#"/// Hook execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    pub operation: String,
    pub data: Value,
    pub original_doc: Option<Value>,
    pub user: Option<Value>,
    pub result: Option<Value>,
}

/// Hook execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    pub success: bool,
    pub data: Option<Value>,
    pub errors: Vec<String>,
}

/// Field change context for field-specific hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldChangeContext {
    pub field_name: String,
    pub old_value: Option<Value>,
    pub new_value: Value,
    pub document: Value,
    pub user: Option<Value>,
}

"#
        .to_string())
    }

    /// Generate model-specific implementation
    fn generate_model_implementation(&self, model: &Model) -> Result<String> {
        let mut code = String::new();
        let model_name = &model.name;

        code.push_str(&format!(
            "/// Generated access control and hooks for {} model\n",
            model_name
        ));
        code.push_str(&format!("pub struct {}AccessControl;\n\n", model_name));

        // Generate access control methods
        if let Some(access) = &model.access {
            code.push_str(&self.generate_access_methods(model_name, access)?);
        }

        // Generate hook methods
        if let Some(hooks) = &model.hooks {
            code.push_str(&self.generate_hook_methods(model_name, hooks)?);
        }

        code.push('\n');
        Ok(code)
    }

    /// Generate access control methods for a model
    fn generate_access_methods(&self, model_name: &str, access: &AccessControl) -> Result<String> {
        let mut code = String::new();

        code.push_str(&format!("impl {}AccessControl {{\n", model_name));

        // Generate create access method
        if let Some(create_rule) = &access.create {
            code.push_str(&self.generate_access_method("create", create_rule)?);
        }

        // Generate read access method
        if let Some(read_rule) = &access.read {
            code.push_str(&self.generate_access_method("read", read_rule)?);
        }

        // Generate update access method
        if let Some(update_rule) = &access.update {
            code.push_str(&self.generate_access_method("update", update_rule)?);
        }

        // Generate delete access method
        if let Some(delete_rule) = &access.delete {
            code.push_str(&self.generate_access_method("delete", delete_rule)?);
        }

        code.push_str("}\n\n");
        Ok(code)
    }

    /// Generate individual access method
    fn generate_access_method(&self, operation: &str, rule: &AccessRule) -> Result<String> {
        let method_code = match rule {
            AccessRule::Boolean(code) => {
                // Simple boolean check
                format!(
                    r#"    pub fn check_{}(context: &AccessContext) -> Result<bool> {{
        let user = context.user.as_ref();
        Ok({})
    }}
"#,
                    operation,
                    self.convert_boolean_rule(code)
                )
            }
            AccessRule::Query(condition) => {
                // Query-based access control
                format!(
                    r#"    pub fn check_{}(context: &AccessContext) -> Result<AccessQuery> {{
        Ok(AccessQuery::where_clause(
            "{}",
            "{}",
            {}
        ))
    }}
"#,
                    operation,
                    condition.field,
                    self.convert_query_operator(&condition.operator),
                    self.convert_query_value(&condition.value)?
                )
            }
            AccessRule::Or(rules) => {
                // OR combination of rules
                let mut query_parts = Vec::new();
                for rule in rules {
                    match rule {
                        AccessRule::Query(condition) => {
                            query_parts.push(format!(
                                "AccessQuery::where_clause(\"{}\", \"{}\", {})",
                                condition.field,
                                self.convert_query_operator(&condition.operator),
                                self.convert_query_value(&condition.value)?
                            ));
                        }
                        _ => {
                            // Handle other rule types if needed
                        }
                    }
                }

                format!(
                    r#"    pub fn check_{}(context: &AccessContext) -> Result<AccessQuery> {{
        Ok(AccessQuery::or(vec![
            {}
        ]))
    }}
"#,
                    operation,
                    query_parts.join(",\n            ")
                )
            }
            AccessRule::And(_) => {
                // AND combination - similar to OR but with different operator
                format!(
                    r#"    pub fn check_{}(context: &AccessContext) -> Result<AccessQuery> {{
        // AND rule implementation
        Ok(AccessQuery::new())
    }}
"#,
                    operation
                )
            }
        };

        Ok(method_code)
    }

    /// Convert boolean rule code from TypeScript to Rust
    fn convert_boolean_rule(&self, code: &str) -> String {
        match code {
            "!!user" => "user.is_some()".to_string(),
            _ => "true".to_string(), // Default fallback
        }
    }

    /// Convert query operator to string
    fn convert_query_operator(&self, operator: &QueryOperator) -> &str {
        match operator {
            QueryOperator::Equals => "equals",
            QueryOperator::NotEquals => "not_equals",
            QueryOperator::In => "in",
            QueryOperator::NotIn => "not_in",
            QueryOperator::GreaterThan => "greater_than",
            QueryOperator::LessThan => "less_than",
            QueryOperator::Like => "like",
            QueryOperator::IsNull => "is_null",
            QueryOperator::IsNotNull => "is_not_null",
        }
    }

    /// Convert query value to Rust code
    fn convert_query_value(&self, value: &QueryValue) -> Result<String> {
        match value {
            QueryValue::String(s) => Ok(format!(r#"serde_json::json!("{}")"#, s)),
            QueryValue::Number(n) => Ok(format!("serde_json::json!({})", n)),
            QueryValue::Boolean(b) => Ok(format!("serde_json::json!({})", b)),
            QueryValue::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|s| format!(r#""{}""#, s)).collect();
                Ok(format!("serde_json::json!([{}])", items.join(", ")))
            }
            QueryValue::UserProperty(prop) => {
                // Handle user property references like user.id
                if let Some(field) = prop.strip_prefix("user.") {
                    // Remove "user." prefix
                    Ok(format!(
                        r#"context.user.as_ref()
                            .and_then(|u| u.get("{}"))
                            .cloned()
                            .unwrap_or(serde_json::Value::Null)"#,
                        field
                    ))
                } else {
                    Ok(format!(r#"serde_json::json!("{}")"#, prop))
                }
            }
        }
    }

    /// Generate hook methods for a model
    fn generate_hook_methods(&self, model_name: &str, hooks: &HookDefinitions) -> Result<String> {
        let mut code = String::new();

        code.push_str(&format!("pub struct {}Hooks;\n\n", model_name));
        code.push_str(&format!("impl {}Hooks {{\n", model_name));

        // Generate beforeOperation hooks
        if let Some(before_ops) = &hooks.before_operation {
            for (i, hook) in before_ops.iter().enumerate() {
                code.push_str(
                    &self.generate_hook_method(&format!("before_operation_{}", i), hook)?,
                );
            }
        }

        // Generate afterOperation hooks
        if let Some(after_ops) = &hooks.after_operation {
            for (i, hook) in after_ops.iter().enumerate() {
                code.push_str(&self.generate_hook_method(&format!("after_operation_{}", i), hook)?);
            }
        }

        // Generate afterRead hooks
        if let Some(after_reads) = &hooks.after_read {
            for (i, hook) in after_reads.iter().enumerate() {
                code.push_str(&self.generate_hook_method(&format!("after_read_{}", i), hook)?);
            }
        }

        // Generate beforeChange hooks
        if let Some(before_changes) = &hooks.before_change {
            for (i, hook) in before_changes.iter().enumerate() {
                code.push_str(
                    &self.generate_field_hook_method(&format!("before_change_{}", i), hook)?,
                );
            }
        }

        code.push_str("}\n\n");
        Ok(code)
    }

    /// Generate individual hook method
    fn generate_hook_method(&self, method_name: &str, hook: &Hook) -> Result<String> {
        let async_keyword = if hook.async_hook { "async " } else { "" };
        let return_type = if hook.async_hook {
            "Result<HookResult>"
        } else {
            "Result<HookResult>"
        };

        Ok(format!(
            r#"    pub {}fn {}(context: &mut HookContext) -> {} {{
        // Converted from TypeScript: {}
        // Operation type: {:?}
        
        // Placeholder implementation - actual logic would be converted from TypeScript
        Ok(HookResult {{
            success: true,
            data: Some(context.data.clone()),
            errors: Vec::new(),
        }})
    }}

"#,
            async_keyword,
            method_name,
            return_type,
            hook.function_code.replace('\n', "\\n"),
            hook.operation_type
        ))
    }

    /// Generate field-specific hook method
    fn generate_field_hook_method(&self, method_name: &str, hook: &FieldHook) -> Result<String> {
        let async_keyword = if hook.async_hook { "async " } else { "" };
        let return_type = if hook.async_hook {
            "Result<HookResult>"
        } else {
            "Result<HookResult>"
        };

        Ok(format!(
            r#"    pub {}fn {}(context: &FieldChangeContext) -> {} {{
        // Field-specific hook for: {}
        // Converted from TypeScript: {}
        
        // Placeholder implementation
        Ok(HookResult {{
            success: true,
            data: None,
            errors: Vec::new(),
        }})
    }}

"#,
            async_keyword,
            method_name,
            return_type,
            hook.field_name,
            hook.function_code.replace('\n', "\\n")
        ))
    }

    /// Generate runtime execution framework
    fn generate_runtime_framework(&self) -> Result<String> {
        Ok(
            r#"/// Runtime execution framework for hooks and access control
pub struct HookAccessRuntime;

impl HookAccessRuntime {
    pub fn new() -> Self {
        Self
    }
    
    /// Execute access control check
    pub fn check_access(
        &self,
        model: &str,
        operation: &str,
        context: &AccessContext,
    ) -> Result<bool> {
        // This would dispatch to the appropriate model's access control methods
        // based on the model name and operation
        
        match (model, operation) {
            ("Product", "create") => {
                // Example: ProductAccessControl::check_create(context)
                Ok(context.user.is_some())
            }
            _ => Ok(true), // Default allow
        }
    }
    
    /// Execute hook
    pub async fn execute_hook(
        &self,
        model: &str,
        hook_type: &str,
        context: &mut HookContext,
    ) -> Result<HookResult> {
        // This would dispatch to the appropriate model's hook methods
        // based on the model name and hook type
        
        Ok(HookResult {
            success: true,
            data: Some(context.data.clone()),
            errors: Vec::new(),
        })
    }
}

"#
            .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_generate_access_structures() {
        let generator = HookAccessGenerator::new();
        let result = generator.generate_access_structures().unwrap();
        assert!(result.contains("AccessContext"));
        assert!(result.contains("AccessQuery"));
    }

    #[test]
    fn test_generate_boolean_access_method() {
        let generator = HookAccessGenerator::new();
        let rule = AccessRule::Boolean("!!user".to_string());
        let result = generator.generate_access_method("create", &rule).unwrap();
        assert!(result.contains("check_create"));
        assert!(result.contains("user.is_some()"));
    }
}
