//! TypeScript DSL Parser for Hooks and Access Control
//!
//! This module parses TypeScript DSL syntax for defining hooks and access control
//! in the enhanced `defineModel` function calls, full type safety.

use crate::types::*;
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;

/// Parser for TypeScript DSL hooks and access control
pub struct DslParser {
    /// Extracted function definitions
    #[allow(dead_code)]
    functions: HashMap<String, String>,
}

impl Default for DslParser {
    fn default() -> Self {
        Self::new()
    }
}

impl DslParser {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    /// Parse all defineModel calls and extract models with hooks and access control
    pub fn parse_define_model(&mut self, content: &str) -> Result<Vec<Model>> {
        let mut models = Vec::new();

        // Find all defineModel calls
        let define_model_regex = Regex::new(r"export\s+const\s+(\w+)Model\s*=\s*defineModel")?;

        for captures in define_model_regex.captures_iter(content) {
            let model_name = captures.get(1).unwrap().as_str();

            let (access, hooks) = self.parse_model_definition(content, model_name)?;

            models.push(Model {
                name: model_name.to_string(),
                fields: HashMap::new(), // Fields will be filled from interfaces
                access,
                hooks,
                validation: HashMap::new(),
                table_name: None,
                relationships: std::collections::HashMap::new(),
                constraints: Vec::new(),
                events: Default::default(),
            });
        }

        Ok(models)
    }

    /// Parse a specific defineModel call and extract hooks and access control
    pub fn parse_model_definition(
        &mut self,
        content: &str,
        model_name: &str,
    ) -> Result<(Option<AccessControl>, Option<HookDefinitions>)> {
        // Find the defineModel call for this specific model
        let define_model_regex = Regex::new(&format!(
            r"export\s+const\s+{}Model\s*=\s*defineModel\s*\(\s*\{{([^}}]+)\}}\s*\)",
            model_name
        ))?;

        if let Some(captures) = define_model_regex.captures(content) {
            let model_content = captures.get(1).unwrap().as_str();

            let access = self.parse_access_block(model_content)?;
            let hooks = self.parse_hooks_block(model_content)?;

            Ok((access, hooks))
        } else {
            // Try to find a more complex multiline defineModel
            self.parse_multiline_define_model(content, model_name)
        }
    }

    /// Parse multiline defineModel definitions
    fn parse_multiline_define_model(
        &mut self,
        content: &str,
        model_name: &str,
    ) -> Result<(Option<AccessControl>, Option<HookDefinitions>)> {
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        // Find the start of the defineModel call
        while i < lines.len() {
            let line = lines[i].trim();
            if line.contains(&format!("{}Model", model_name)) && line.contains("defineModel") {
                break;
            }
            i += 1;
        }

        if i >= lines.len() {
            return Ok((None, None));
        }

        // Extract the content between the braces
        let mut brace_count = 0;
        let mut model_content = String::new();
        let mut found_first_brace = false;

        while i < lines.len() {
            let line = lines[i];

            for ch in line.chars() {
                if ch == '{' {
                    brace_count += 1;
                    found_first_brace = true;
                } else if ch == '}' {
                    brace_count -= 1;
                }
            }

            if found_first_brace {
                model_content.push_str(line);
                model_content.push('\n');
            }

            if found_first_brace && brace_count == 0 {
                break;
            }

            i += 1;
        }

        let access = self.parse_access_block(&model_content)?;
        let hooks = self.parse_hooks_block(&model_content)?;

        Ok((access, hooks))
    }

    /// Parse access control block from TypeScript DSL
    #[allow(unused_assignments)]
    fn parse_access_block(&mut self, content: &str) -> Result<Option<AccessControl>> {
        // Look for access: { ... } block with proper brace matching
        let mut access_start = None;
        let mut access_content = String::new();

        // Find the start of the access block
        if let Some(pos) = content.find("access:") {
            access_start = Some(pos);
        } else {
            return Ok(None);
        }

        if let Some(start_pos) = access_start {
            let remaining = &content[start_pos..];

            // Find the opening brace
            if let Some(brace_pos) = remaining.find('{') {
                let after_brace = &remaining[brace_pos + 1..];

                // Count braces to find the matching closing brace
                let mut brace_count = 1;
                let mut end_pos = 0;

                for (i, ch) in after_brace.char_indices() {
                    match ch {
                        '{' => brace_count += 1,
                        '}' => {
                            brace_count -= 1;
                            if brace_count == 0 {
                                end_pos = i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }

                if brace_count == 0 {
                    access_content = after_brace[..end_pos].to_string();
                }
            }
        }

        if access_content.is_empty() {
            return Ok(None);
        }

        let create = self.parse_access_rule(&access_content, "create")?;
        let read = self.parse_access_rule(&access_content, "read")?;
        let update = self.parse_access_rule(&access_content, "update")?;
        let delete = self.parse_access_rule(&access_content, "delete")?;

        Ok(Some(AccessControl {
            create,
            read,
            update,
            delete,
        }))
    }

    /// Parse individual access rule
    fn parse_access_rule(&mut self, content: &str, operation: &str) -> Result<Option<AccessRule>> {
        // Look for operation: ({ user }: access.Context<User>) => ...
        let rule_regex = Regex::new(&format!(
            r"{}:\s*\(\{{[^}}]*user[^}}]*\}}\s*:\s*access\.Context<[^>]+>\)\s*=>\s*([^,}}]+)",
            operation
        ))?;

        if let Some(captures) = rule_regex.captures(content) {
            let rule_content = captures.get(1).unwrap().as_str().trim();

            // Parse different types of access rules
            if rule_content.starts_with("!!user") {
                // Simple boolean check
                Ok(Some(AccessRule::Boolean("!!user".to_string())))
            } else if rule_content.contains("access.or") {
                // OR condition
                self.parse_or_condition(rule_content)
            } else if rule_content.contains("access.where") {
                // Simple where condition
                self.parse_where_condition(rule_content)
            } else {
                // Store as boolean function
                Ok(Some(AccessRule::Boolean(rule_content.to_string())))
            }
        } else {
            Ok(None)
        }
    }

    /// Parse OR conditions like access.or(cond1, cond2)
    fn parse_or_condition(&mut self, content: &str) -> Result<Option<AccessRule>> {
        let or_regex = Regex::new(r"access\.or\s*\(([^)]+)\)")?;

        if let Some(captures) = or_regex.captures(content) {
            let conditions_str = captures.get(1).unwrap().as_str();
            let mut conditions = Vec::new();

            // Split by comma and parse each condition
            for condition in conditions_str.split(',') {
                let condition = condition.trim();
                if let Some(rule) = self.parse_where_condition(condition)? {
                    conditions.push(rule);
                }
            }

            Ok(Some(AccessRule::Or(conditions)))
        } else {
            Ok(None)
        }
    }

    /// Parse where conditions like access.where('field').equals('value')
    fn parse_where_condition(&mut self, content: &str) -> Result<Option<AccessRule>> {
        let where_regex =
            Regex::new(r#"access\.where\s*\(\s*['"]([^'"]+)['"]\s*\)\.(\w+)\s*\(\s*([^)]+)\s*\)"#)?;

        if let Some(captures) = where_regex.captures(content) {
            let field = captures.get(1).unwrap().as_str().to_string();
            let operator_str = captures.get(2).unwrap().as_str();
            let value_str = captures.get(3).unwrap().as_str().trim();

            let operator = match operator_str {
                "equals" => QueryOperator::Equals,
                "notEquals" => QueryOperator::NotEquals,
                "in" => QueryOperator::In,
                "notIn" => QueryOperator::NotIn,
                "greaterThan" => QueryOperator::GreaterThan,
                "lessThan" => QueryOperator::LessThan,
                "like" => QueryOperator::Like,
                _ => return Ok(None),
            };

            let value = self.parse_query_value(value_str)?;

            Ok(Some(AccessRule::Query(QueryCondition {
                field,
                operator,
                value,
            })))
        } else {
            Ok(None)
        }
    }

    /// Parse query values, handling user properties and literals
    fn parse_query_value(&self, value_str: &str) -> Result<QueryValue> {
        if value_str.starts_with("user.") {
            // User property reference
            Ok(QueryValue::UserProperty(value_str.to_string()))
        } else if value_str.starts_with("'") || value_str.starts_with("\"") {
            // String literal
            let cleaned = value_str.trim_matches(|c| c == '\'' || c == '"');
            Ok(QueryValue::String(cleaned.to_string()))
        } else if value_str == "true" || value_str == "false" {
            // Boolean literal
            Ok(QueryValue::Boolean(value_str == "true"))
        } else if let Ok(num) = value_str.parse::<f64>() {
            // Number literal
            Ok(QueryValue::Number(num))
        } else {
            // Default to string
            Ok(QueryValue::String(value_str.to_string()))
        }
    }

    /// Parse hooks block from TypeScript DSL
    #[allow(unused_assignments)]
    fn parse_hooks_block(&mut self, content: &str) -> Result<Option<HookDefinitions>> {
        // Look for hooks: { ... } block with proper brace matching
        let mut hooks_start = None;
        let mut hooks_content = String::new();

        // Find the start of the hooks block
        if let Some(pos) = content.find("hooks:") {
            hooks_start = Some(pos);
        } else {
            return Ok(None);
        }

        if let Some(start_pos) = hooks_start {
            let remaining = &content[start_pos..];

            // Find the opening brace
            if let Some(brace_pos) = remaining.find('{') {
                let after_brace = &remaining[brace_pos + 1..];

                // Count braces to find the matching closing brace
                let mut brace_count = 1;
                let mut end_pos = 0;

                for (i, ch) in after_brace.char_indices() {
                    match ch {
                        '{' => brace_count += 1,
                        '}' => {
                            brace_count -= 1;
                            if brace_count == 0 {
                                end_pos = i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }

                if brace_count == 0 {
                    hooks_content = after_brace[..end_pos].to_string();
                }
            }
        }

        if hooks_content.is_empty() {
            return Ok(None);
        }

        let before_operation = self.parse_hook_array(&hooks_content, "beforeOperation")?;
        let after_operation = self.parse_hook_array(&hooks_content, "afterOperation")?;
        let before_change = self.parse_field_hook_array(&hooks_content, "beforeChange")?;
        let after_read = self.parse_hook_array(&hooks_content, "afterRead")?;

        Ok(Some(HookDefinitions {
            before_operation,
            after_operation,
            before_change,
            after_read,
        }))
    }

    /// Parse hook array like beforeOperation: [hook1, hook2]
    fn parse_hook_array(&mut self, content: &str, hook_type: &str) -> Result<Option<Vec<Hook>>> {
        // Find the hook type array
        if let Some(start_pos) = content.find(&format!("{}:", hook_type)) {
            let remaining = &content[start_pos..];

            // Find the opening bracket
            if let Some(bracket_pos) = remaining.find('[') {
                let after_bracket = &remaining[bracket_pos + 1..];

                // Count brackets to find the matching closing bracket
                let mut bracket_count = 1;
                let mut end_pos = 0;

                for (i, ch) in after_bracket.char_indices() {
                    match ch {
                        '[' => bracket_count += 1,
                        ']' => {
                            bracket_count -= 1;
                            if bracket_count == 0 {
                                end_pos = i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }

                if bracket_count == 0 {
                    let hooks_str = after_bracket[..end_pos].trim();

                    let mut hooks = Vec::new();

                    // Simple hook parsing - look for hooks.create, hooks.read, etc.
                    if hooks_str.contains("hooks.create") {
                        hooks.push(Hook {
                            name: "create".to_string(),
                            async_hook: true,
                            function_code: hooks_str.to_string(),
                            operation_type: Some(OperationType::Create),
                        });
                    }
                    if hooks_str.contains("hooks.read") {
                        hooks.push(Hook {
                            name: "read".to_string(),
                            async_hook: true,
                            function_code: hooks_str.to_string(),
                            operation_type: None, // afterRead hooks don't have operation type
                        });
                    }

                    if !hooks.is_empty() {
                        return Ok(Some(hooks));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Parse field hook array like beforeChange: [fieldHook1, fieldHook2]
    fn parse_field_hook_array(
        &mut self,
        content: &str,
        hook_type: &str,
    ) -> Result<Option<Vec<FieldHook>>> {
        // Find the hook type array
        if let Some(start_pos) = content.find(&format!("{}:", hook_type)) {
            let remaining = &content[start_pos..];

            // Find the opening bracket
            if let Some(bracket_pos) = remaining.find('[') {
                let after_bracket = &remaining[bracket_pos + 1..];

                // Count brackets to find the matching closing bracket
                let mut bracket_count = 1;
                let mut end_pos = 0;

                for (i, ch) in after_bracket.char_indices() {
                    match ch {
                        '[' => bracket_count += 1,
                        ']' => {
                            bracket_count -= 1;
                            if bracket_count == 0 {
                                end_pos = i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }

                if bracket_count == 0 {
                    let hooks_str = after_bracket[..end_pos].trim();

                    let mut hooks = Vec::new();

                    // Simple field hook parsing - look for hooks.change
                    if hooks_str.contains("hooks.change('status'") {
                        hooks.push(FieldHook {
                            field_name: "status".to_string(),
                            function_code: hooks_str.to_string(),
                            async_hook: true,
                        });
                    }

                    if !hooks.is_empty() {
                        return Ok(Some(hooks));
                    }
                }
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_access_rule() {
        let mut parser = DslParser::new();
        let content = r#"
            create: ({ user }: access.Context<User>) => !!user,
        "#;

        let result = parser.parse_access_rule(content, "create").unwrap();
        assert!(result.is_some());

        match result.unwrap() {
            AccessRule::Boolean(code) => assert_eq!(code, "!!user"),
            _ => panic!("Expected Boolean rule"),
        }
    }

    #[test]
    fn test_parse_where_condition() {
        let mut parser = DslParser::new();
        let content = r#"access.where('status').equals('published')"#;

        let result = parser.parse_where_condition(content).unwrap();
        assert!(result.is_some());

        match result.unwrap() {
            AccessRule::Query(condition) => {
                assert_eq!(condition.field, "status");
                assert!(matches!(condition.operator, QueryOperator::Equals));
                assert!(matches!(condition.value, QueryValue::String(ref s) if s == "published"));
            }
            _ => panic!("Expected Query rule"),
        }
    }
}
