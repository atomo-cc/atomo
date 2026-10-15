//! TypeScript DSL Parser for Hooks and Access Control
//!
//! This module parses TypeScript DSL syntax for defining hooks and access control
//! in the enhanced `defineModel` function calls, full type safety.

use crate::types::*;
use anyhow::Result;
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

        for (model_name, body) in crate::object_syntax::compatibility_blocks(content)? {
            let access = crate::schema_dsl_parser::parse_access_block(&body)?;
            let hooks = self.parse_hooks_block(&body)?;

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
                ui: None,
                tenant_policy: Default::default(),
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
        let definitions = self.parse_define_model(content)?;
        Ok(definitions
            .into_iter()
            .find(|model| model.name == model_name)
            .map(|model| (model.access, model.hooks))
            .unwrap_or((None, None)))
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
    fn callbacks_reject_independently_of_property_spelling() {
        for body in [
            "access : policy",
            "'access': {read: ({user}) => !!user}",
            "access:{read : ({user}) => !!user}",
        ] {
            let text = format!("export const NoteModel = defineModel({{{body}}});");
            assert!(DslParser::new().parse_define_model(&text).is_err());
            assert!(DslParser::new()
                .parse_model_definition(&text, "Note")
                .is_err());
        }
    }

    #[test]
    fn declarative_rules_use_the_shared_parser() {
        let text = "export const NoteModel = defineModel({'access': {'read': 'admin', update: allow.owner('ownerId')}});";
        let (access, _) = DslParser::new()
            .parse_model_definition(text, "Note")
            .unwrap();
        assert!(
            matches!(&access.unwrap().update, Some(AccessRule::Owner(field)) if field == "ownerId")
        );
    }
}
