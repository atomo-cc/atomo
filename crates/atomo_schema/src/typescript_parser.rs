use crate::dsl_parser::DslParser;
use crate::types::*;
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;

/// TypeScript parser that implements true "Dual-Mode Schema"
/// Parses TypeScript interface definitions and extracts complete field information
///
/// Enhanced features:
/// - Interface parsing with inheritance
/// - Enum and union type support
/// - Comprehensive field attribute detection
/// - Relationship inference
/// - Generic type handling
/// - Hook and Access Control DSL parsing
pub struct TypeScriptParser {
    /// Cache for parsed enum types
    pub enums: HashMap<String, Vec<String>>,
    /// Cache for parsed type aliases
    pub type_aliases: HashMap<String, String>,
    /// DSL parser for hooks and access control
    pub dsl_parser: DslParser,
}

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeScriptParser {
    pub fn new() -> Self {
        Self {
            enums: HashMap::new(),
            type_aliases: HashMap::new(),
            dsl_parser: DslParser::new(),
        }
    }

    /// Parse TypeScript schema content and extract complete interface definitions
    pub fn parse(&self, content: &str) -> Result<Vec<Model>> {
        let mut parser = TypeScriptParser::new();

        // First pass: collect enums and type definitions
        parser.collect_type_definitions(content)?;

        // Second pass: parse interfaces with full type context
        let mut models = parser.parse_interfaces(content)?;

        // Resolve fields whose type names a known enum: an enum is a string constraint, not a
        // JSON blob, so map it to String (-> TEXT column). Without this, `stage: DealStage`
        // becomes Custom("DealStage") -> JSONB and string inserts fail.
        for model in &mut models {
            for field in model.fields.values_mut() {
                if let FieldType::Custom(name) = &field.field_type {
                    if parser.enums.contains_key(name) {
                        field.field_type = FieldType::String;
                    }
                }
            }
        }

        // Third pass: parse DSL models (defineModel calls) and merge with interfaces
        let dsl_models = parser.dsl_parser.parse_define_model(content)?;
        for dsl_model in dsl_models {
            // Find the corresponding interface model and merge
            if let Some(interface_model) = models.iter_mut().find(|m| m.name == dsl_model.name) {
                interface_model.access = dsl_model.access;
                interface_model.hooks = dsl_model.hooks;
            } else {
                // If no corresponding interface, add the DSL model as-is
                models.push(dsl_model);
            }
        }

        // Fourth pass: parse validation rules from schema const and attach to models
        let mut validation_map = Self::parse_validation_rules(content);
        for model in &mut models {
            if let Some(rules) = validation_map.remove(&model.name) {
                model.validation = rules;
            }
        }

        // Fifth pass: parse explicit `tableName` from the schema const and attach.
        let mut table_names = Self::parse_table_names(content);
        for model in &mut models {
            if let Some(t) = table_names.remove(&model.name) {
                model.table_name = Some(t);
            }
        }

        // Sixth pass: parse `access` rules from the schema const and attach. Only set when the
        // DSL pass didn't already provide access (DSL/defineModel form takes precedence).
        let mut access_map = Self::parse_access_rules(content);
        for model in &mut models {
            if model.access.is_none() {
                if let Some(ac) = access_map.remove(&model.name) {
                    model.access = Some(ac);
                }
            }
        }

        Ok(models)
    }

    /// Extract per-model `access` rules from the `export const schema` object, e.g.
    /// `Contact: { access: { create: "sales|manager|admin", read: "authenticated" } }`.
    /// Produces `AccessRule::Boolean(roles_str)` which `check_access` already understands
    /// (pipe-OR roles + the `authenticated`/`public` tokens). Accepts single/double quotes.
    fn parse_access_rules(content: &str) -> HashMap<String, AccessControl> {
        let mut result = HashMap::new();
        // Find each `ModelName: { ... access: { ... } }` and pull the access block, then the
        // per-op string rules within it.
        let model_re = Regex::new(r"(\w+)\s*:\s*\{").unwrap();
        let op_re = Regex::new(r#"(create|read|update|delete)\s*:\s*['"]([^'"]+)['"]"#).unwrap();
        let bytes = content.as_bytes();
        for cap in model_re.captures_iter(content) {
            let model_name = cap[1].to_string();
            let block_start = cap.get(0).unwrap().end();
            // Walk the model block to find a nested `access: {` and capture it (brace-balanced).
            let mut depth = 1usize;
            let mut idx = block_start;
            let mut access_block: Option<String> = None;
            while idx < bytes.len() && depth > 0 {
                match bytes[idx] {
                    b'{' => {
                        let prefix = &content[..idx];
                        if depth == 1 && prefix.trim_end().ends_with("access:") {
                            let mut d = 1usize;
                            let mut j = idx + 1;
                            let astart = j;
                            while j < bytes.len() && d > 0 {
                                match bytes[j] {
                                    b'{' => d += 1,
                                    b'}' => d -= 1,
                                    _ => {}
                                }
                                j += 1;
                            }
                            access_block = Some(content[astart..j.saturating_sub(1)].to_string());
                        }
                        depth += 1;
                    }
                    b'}' => depth -= 1,
                    _ => {}
                }
                idx += 1;
            }
            if let Some(block) = access_block {
                let mut ac = AccessControl { create: None, read: None, update: None, delete: None };
                let mut any = false;
                for c in op_re.captures_iter(&block) {
                    let rule = Some(AccessRule::Boolean(c[2].to_string()));
                    any = true;
                    match &c[1] {
                        "create" => ac.create = rule,
                        "read" => ac.read = rule,
                        "update" => ac.update = rule,
                        "delete" => ac.delete = rule,
                        _ => {}
                    }
                }
                if any {
                    result.insert(model_name, ac);
                }
            }
        }
        result
    }

    /// Extract per-model `tableName` from the `export const schema` object, e.g.
    /// `Contact: { tableName: "contact", ... }`. Accepts single- or double-quotes.
    fn parse_table_names(content: &str) -> HashMap<String, String> {
        let mut result = HashMap::new();
        // `ModelName: { ... tableName: "x"` — capture the model name then its tableName within
        // a short window (the metadata block opens with tableName as its first key in practice).
        let re = Regex::new(r#"(\w+)\s*:\s*\{\s*tableName\s*:\s*['"]([^'"]+)['"]"#).unwrap();
        for cap in re.captures_iter(content) {
            result.insert(cap[1].to_string(), cap[2].to_string());
        }
        result
    }

    /// Extract per-model validation rules from the `export const schema` object
    fn parse_validation_rules(content: &str) -> HashMap<String, HashMap<String, String>> {
        let mut result: HashMap<String, HashMap<String, String>> = HashMap::new();
        // Field rules may be single- OR double-quoted (the real CRM schema uses double quotes;
        // a single-quote-only regex silently extracted zero rules from it).
        let field_rule_re = Regex::new(r#"(\w+):\s*['"]([^'"]*)['"]"#).unwrap();
        // Find each `ModelName: {` then, within its (brace-balanced) block, locate `validation: { ... }`.
        let model_open_re = Regex::new(r"(\w+)\s*:\s*\{").unwrap();
        let bytes = content.as_bytes();
        for cap in model_open_re.captures_iter(content) {
            let model_name = cap[1].to_string();
            let block_start = cap.get(0).unwrap().end(); // just after the opening '{'
                                                         // Walk forward tracking brace depth to find this block's matching close.
            let mut depth = 1usize;
            let mut idx = block_start;
            let mut val_block: Option<String> = None;
            while idx < bytes.len() && depth > 0 {
                match bytes[idx] {
                    b'{' => {
                        // Check if this is the start of a `validation: {` sub-block.
                        let prefix = &content[..idx];
                        if depth == 1 && prefix.trim_end().ends_with("validation:") {
                            // Capture the balanced validation block.
                            let mut d = 1usize;
                            let mut j = idx + 1;
                            let vstart = j;
                            while j < bytes.len() && d > 0 {
                                match bytes[j] {
                                    b'{' => d += 1,
                                    b'}' => d -= 1,
                                    _ => {}
                                }
                                j += 1;
                            }
                            val_block = Some(content[vstart..j.saturating_sub(1)].to_string());
                        }
                        depth += 1;
                    }
                    b'}' => depth -= 1,
                    _ => {}
                }
                idx += 1;
            }
            if let Some(block) = val_block {
                let mut rules = HashMap::new();
                for field_cap in field_rule_re.captures_iter(&block) {
                    rules.insert(field_cap[1].to_string(), field_cap[2].to_string());
                }
                if !rules.is_empty() {
                    result.insert(model_name, rules);
                }
            }
        }
        result
    }

    /// Collect enum and type alias definitions
    fn collect_type_definitions(&mut self, content: &str) -> Result<()> {
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            if line.starts_with("export enum ") {
                let (enum_name, enum_values, lines_consumed) = parse_enum(&lines, i)?;
                self.enums.insert(enum_name, enum_values);
                i += lines_consumed;
            } else if line.starts_with("export type ") {
                let (type_name, type_definition, lines_consumed) = parse_type_alias(&lines, i)?;
                self.type_aliases.insert(type_name, type_definition);
                i += lines_consumed;
            } else {
                i += 1;
            }
        }

        Ok(())
    }

    /// Parse interface definitions with full type context
    fn parse_interfaces(&mut self, content: &str) -> Result<Vec<Model>> {
        let mut models = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Look for interface definitions
            if line.starts_with("export interface ") {
                if let Some(interface_name) = extract_interface_name(line) {
                    // Skip command interfaces for now
                    if interface_name.ends_with("Command") || interface_name.ends_with("Input") {
                        i += 1;
                        continue;
                    }

                    // Parse the complete interface
                    let (mut model, lines_consumed) =
                        parse_interface(&lines, i, interface_name.clone())?;

                    // Parse hooks and access control for this model
                    let (access, hooks) = self
                        .dsl_parser
                        .parse_model_definition(content, &interface_name)?;
                    model.access = access;
                    model.hooks = hooks;

                    models.push(model);
                    i += lines_consumed;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        // Also convert collected enums to models
        for (enum_name, enum_values) in &self.enums {
            println!(
                "DEBUG: Converting enum {} to model with {} values",
                enum_name,
                enum_values.len()
            );
            let mut fields = HashMap::new();

            // Create a special marker field to identify this as an enum
            fields.insert(
                "_enum_type".to_string(),
                Field {
                    name: "_enum_type".to_string(),
                    field_type: FieldType::String,
                    optional: false,
                    attributes: vec![],
                },
            );

            // Add each enum value as metadata (we'll handle this specially in the generator)
            for (i, value) in enum_values.iter().enumerate() {
                fields.insert(
                    format!("_enum_value_{}", i),
                    Field {
                        name: value.clone(),
                        field_type: FieldType::String,
                        optional: false,
                        attributes: vec![],
                    },
                );
            }

            models.push(Model {
                name: enum_name.clone(),
                fields,
                access: None,
                hooks: None,
                validation: HashMap::new(),
                table_name: None,
            });
        }

        if models.is_empty() {
            anyhow::bail!("No valid interfaces found in schema");
        }

        Ok(models)
    }
}

fn extract_interface_name(line: &str) -> Option<String> {
    // Extract name from "export interface Name {" or "export interface Name extends Base {"
    let re = Regex::new(r"export\s+interface\s+(\w+)").ok()?;
    re.captures(line)?.get(1)?.as_str().to_string().into()
}

fn parse_interface(lines: &[&str], start_index: usize, name: String) -> Result<(Model, usize)> {
    let mut fields = HashMap::new();
    let mut i = start_index;

    // Find the opening brace
    while i < lines.len() && !lines[i].contains('{') {
        i += 1;
    }

    // Collect the interface body — from the content after `{` on the brace line, through the
    // line containing the closing `}` (content before it). This handles single-line interfaces
    // (`interface X { a: string; b: string; }`) and multiple fields packed onto one line, both
    // of which the old line-at-a-time scan silently dropped.
    let mut body = String::new();
    let mut closed = false;
    if i < lines.len() {
        if let Some(pos) = lines[i].find('{') {
            let rest = &lines[i][pos + 1..];
            if let Some(end) = rest.find('}') {
                body.push_str(&rest[..end]);
                closed = true;
            } else {
                body.push_str(rest);
                body.push('\n');
            }
        }
        i += 1;
    }
    while !closed && i < lines.len() {
        if let Some(end) = lines[i].find('}') {
            body.push_str(&lines[i][..end]);
            closed = true;
        } else {
            body.push_str(lines[i]);
            body.push('\n');
        }
        i += 1;
    }

    // Each field is `;`-separated (and may also span newlines). Strip inline/line comments,
    // skip union members and stray exports, then parse the rest as field definitions.
    for raw in body.split(['\n', ';']) {
        let seg = raw.split("//").next().unwrap_or("").trim();
        if seg.is_empty()
            || seg.starts_with("/*")
            || seg.starts_with('*')
            || seg.starts_with("export ")
            || seg.contains('|')
        {
            continue;
        }
        if let Some(field) = parse_field_definition(seg) {
            fields.insert(field.name.clone(), field);
        }
    }

    let model = Model {
        name,
        fields,
        access: None, // Will be populated later by DSL parser
        hooks: None,  // Will be populated later by DSL parser
        validation: HashMap::new(),
        table_name: None,
    };
    let lines_consumed = i - start_index;

    Ok((model, lines_consumed))
}

fn parse_field_definition(line: &str) -> Option<Field> {
    // Handle patterns like:
    // id: string;
    // email?: string;
    // createdAt: Date; // ISO date string
    // stage: DealStage;

    let field_re = Regex::new(r"(\w+)(\?)?:\s*([^;/]+)").ok()?;
    let captures = field_re.captures(line)?;

    let field_name = captures.get(1)?.as_str().to_string();
    let is_optional = captures.get(2).is_some();
    let type_str = captures.get(3)?.as_str().trim();

    // Determine field type
    let field_type = match type_str {
        "string" => FieldType::String,
        "number" => FieldType::Number,
        "boolean" => FieldType::Boolean,
        "Date" => FieldType::DateTime,
        "any" => FieldType::Json,       // TypeScript any type maps to JSON
        "Block[]" => FieldType::Blocks, // Special handling for composable content
        t if t.ends_with("[]") => FieldType::Array(Box::new(parse_array_type(t)?)),
        t => FieldType::Custom(t.to_string()),
    };

    // Determine attributes based on field name and comments
    let mut attributes = Vec::new();

    // Common patterns for attributes
    if field_name == "id" {
        attributes.push(FieldAttribute::Primary);
    }
    if field_name == "email" {
        attributes.push(FieldAttribute::Unique);
    }
    if field_name.ends_with("_id") || field_name.ends_with("Id") {
        attributes.push(FieldAttribute::ForeignKey);
    }
    if field_name.contains("created") || field_name.contains("updated") {
        attributes.push(FieldAttribute::Timestamp);
    }

    Some(Field {
        name: field_name,
        field_type,
        optional: is_optional,
        attributes,
    })
}

fn parse_array_type(type_str: &str) -> Option<FieldType> {
    let inner_type = type_str.strip_suffix("[]")?;
    match inner_type {
        "string" => Some(FieldType::String),
        "number" => Some(FieldType::Number),
        "boolean" => Some(FieldType::Boolean),
        t => Some(FieldType::Custom(t.to_string())),
    }
}

/// Parse enum definition from TypeScript
fn parse_enum(lines: &[&str], start_index: usize) -> Result<(String, Vec<String>, usize)> {
    let mut i = start_index;
    let line = lines[i].trim();

    // Extract enum name from "export enum EnumName {"
    let enum_re = Regex::new(r"export\s+enum\s+(\w+)").unwrap();
    let enum_name = enum_re
        .captures(line)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| anyhow::anyhow!("Invalid enum declaration: {}", line))?;

    // Find opening brace
    while i < lines.len() && !lines[i].contains('{') {
        i += 1;
    }
    i += 1; // Move past opening brace

    let mut enum_values = Vec::new();

    // Parse enum values
    while i < lines.len() {
        let line = lines[i].trim();

        if line == "}" {
            break;
        }

        if !line.is_empty() && !line.starts_with("//") {
            // Extract enum value (handle both "VALUE" and "VALUE = 'string'" patterns)
            let value_re = Regex::new(r"(\w+)").unwrap();
            if let Some(caps) = value_re.captures(line) {
                if let Some(value) = caps.get(1) {
                    enum_values.push(value.as_str().to_string());
                }
            }
        }

        i += 1;
    }

    let lines_consumed = i - start_index + 1;

    // Debug output
    println!(
        "DEBUG: Parsed enum {} with values: {:?}",
        enum_name, enum_values
    );

    Ok((enum_name, enum_values, lines_consumed))
}

/// Parse type alias definition from TypeScript
fn parse_type_alias(lines: &[&str], start_index: usize) -> Result<(String, String, usize)> {
    let mut i = start_index;
    let line = lines[i].trim();

    // Extract type name from "export type TypeName = ..."
    let type_re = Regex::new(r"export\s+type\s+(\w+)\s*=").unwrap();
    let type_name = type_re
        .captures(line)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| anyhow::anyhow!("Invalid type declaration: {}", line))?;

    // Skip type definitions for now - they're complex and not needed for basic model generation
    // Just consume lines until we find the end
    while i < lines.len() {
        let current_line = lines[i].trim();
        if current_line.ends_with(';') {
            break;
        }
        i += 1;
    }

    let lines_consumed = i - start_index + 1;
    let placeholder_definition = "any".to_string(); // Placeholder for complex types
    Ok((type_name, placeholder_definition, lines_consumed))
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn parses_validation_with_nested_blocks() {
        // Mirrors the real CRM schema: nested access/relationships blocks precede validation.
        let content = r#"
        export const schema = {
          models: {
            Contact: {
              tableName: 'contacts',
              access: { create: 'sales|manager', read: 'authenticated' },
              relationships: { company: { type: 'belongsTo', model: 'Company' } },
              validation: {
                email: 'email',
                firstName: 'required|min:1|max:100'
              }
            }
          }
        };
        "#;
        let rules = TypeScriptParser::parse_validation_rules(content);
        let contact = rules
            .get("Contact")
            .expect("Contact validation rules missing");
        assert_eq!(contact.get("email").map(|s| s.as_str()), Some("email"));
        assert_eq!(
            contact.get("firstName").map(|s| s.as_str()),
            Some("required|min:1|max:100")
        );
    }

    #[test]
    fn parses_single_line_and_packed_interfaces() {
        // The footgun: a single-line interface (and multiple fields on one line) used to be
        // silently dropped, producing tables with missing columns and no error.
        let single = "export interface Contact { id: string; email: string; name: string; }";
        let models = TypeScriptParser::new().parse_interfaces(single).unwrap();
        let c = models.iter().find(|m| m.name == "Contact").expect("Contact parsed");
        for f in ["id", "email", "name"] {
            assert!(c.fields.contains_key(f), "single-line field '{}' must be parsed", f);
        }

        // Multi-line still works, including a comment and an optional field.
        let multi = "export interface Note {\n  id: string;\n  title?: string; // heading\n}";
        let models = TypeScriptParser::new().parse_interfaces(multi).unwrap();
        let n = models.iter().find(|m| m.name == "Note").expect("Note parsed");
        assert!(n.fields.contains_key("id") && n.fields.contains_key("title"));
    }

    #[test]
    fn parses_and_enforces_access_rules() {
        use crate::types::AccessDecision;
        // Mirrors the real CRM: double-quoted access rules in the export-const-schema format.
        let content = r#"
        export interface Contact { id: string; name: string; }
        export const schema = { models: { Contact: {
          tableName: "contact",
          access: { create: "sales|manager|admin", read: "authenticated", delete: "manager|admin" }
        } } };
        "#;
        let models = TypeScriptParser::new().parse(content).unwrap();
        let c = models.iter().find(|m| m.name == "Contact").expect("Contact parsed");
        let ac = c.access.as_ref().expect("access rules must be parsed (RBAC bypass fix)");

        // create requires sales|manager|admin
        assert_eq!(ac.decide("create", Some("Viewer")), AccessDecision::Forbidden, "viewer denied create");
        assert_eq!(ac.decide("create", Some("Sales")), AccessDecision::Allow, "sales allowed create");
        assert_eq!(ac.decide("create", None), AccessDecision::NeedsAuth, "anon needs auth");
        // read is authenticated → any logged-in role allowed
        assert_eq!(ac.decide("read", Some("Viewer")), AccessDecision::Allow, "viewer can read");
        // delete gated to manager|admin
        assert_eq!(ac.decide("delete", Some("Sales")), AccessDecision::Forbidden, "sales cannot delete");
        assert_eq!(ac.decide("delete", Some("Admin")), AccessDecision::Allow, "admin can delete");
    }
}
