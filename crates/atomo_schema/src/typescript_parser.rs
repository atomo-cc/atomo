use crate::dsl_parser::DslParser;
use crate::types::*;
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;

/// Per-model metadata extracted from the `export const schema` object in one unified pass.
#[derive(Default)]
struct ModelMetadata {
    table_name: Option<String>,
    validation: HashMap<String, String>,
    access: Option<AccessControl>,
    relationships: HashMap<String, Relationship>,
}

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

        // Unified metadata pass: parse tableName / validation / access / relationships from each
        // model's block in the `export const schema` object in ONE place (replaces the prior
        // three near-identical brace-walk passes — the recurring source of parse-layer bugs).
        let mut meta = Self::parse_model_metadata(content);
        for model in &mut models {
            if let Some(m) = meta.remove(&model.name) {
                if model.table_name.is_none() {
                    model.table_name = m.table_name;
                }
                if !m.validation.is_empty() {
                    model.validation = m.validation;
                }
                // DSL/defineModel access takes precedence if already set.
                if model.access.is_none() {
                    model.access = m.access;
                }
                if !m.relationships.is_empty() {
                    model.relationships = m.relationships;
                }
            }
        }

        Ok(models)
    }

    /// Parse per-model metadata (`tableName`, `validation`, `access`, `relationships`) from the
    /// `export const schema = { models: { Name: { ... } } }` object. One robust brace-balanced
    /// extractor for ALL features — the prior code had three separate passes that each re-walked
    /// braces and drifted (quote-style, format), causing the validation/RBAC/tableName silent gaps.
    fn parse_model_metadata(content: &str) -> HashMap<String, ModelMetadata> {
        let mut result: HashMap<String, ModelMetadata> = HashMap::new();
        // Locate the `models: {` container and iterate its direct children (model blocks).
        let models_block = match Self::sub_block(content, "models") {
            Some(b) => b,
            None => return result,
        };
        for (name, block) in Self::top_level_entries(&models_block) {
            let mut m = ModelMetadata::default();
            // tableName: "x"
            let tn_re = Regex::new(r#"tableName\s*:\s*['"]([^'"]+)['"]"#).unwrap();
            if let Some(c) = tn_re.captures(&block) {
                m.table_name = Some(c[1].to_string());
            }
            // validation: { field: 'rule', ... }
            if let Some(vblock) = Self::sub_block(&block, "validation") {
                let kv = Regex::new(r#"(\w+)\s*:\s*['"]([^'"]*)['"]"#).unwrap();
                for c in kv.captures_iter(&vblock) {
                    m.validation.insert(c[1].to_string(), c[2].to_string());
                }
            }
            // access: { create: 'roles', ... }
            if let Some(ablock) = Self::sub_block(&block, "access") {
                let op =
                    Regex::new(r#"(create|read|update|delete)\s*:\s*['"]([^'"]+)['"]"#).unwrap();
                let mut ac = AccessControl {
                    create: None,
                    read: None,
                    update: None,
                    delete: None,
                };
                let mut any = false;
                for c in op.captures_iter(&ablock) {
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
                    m.access = Some(ac);
                }
            }
            // relationships: { rel: { type: 'belongsTo', model: 'X', foreignKey: 'y' }, ... }
            if let Some(rblock) = Self::sub_block(&block, "relationships") {
                for (rel_name, rdef) in Self::top_level_entries(&rblock) {
                    let field = |k: &str| {
                        Regex::new(&format!(r#"{}\s*:\s*['"]([^'"]+)['"]"#, k))
                            .ok()
                            .and_then(|re| re.captures(&rdef).map(|c| c[1].to_string()))
                    };
                    if let (Some(kind), Some(model)) = (field("type"), field("model")) {
                        m.relationships.insert(
                            rel_name,
                            crate::types::Relationship {
                                kind,
                                model,
                                foreign_key: field("foreignKey"),
                            },
                        );
                    }
                }
            }
            if m.table_name.is_some()
                || !m.validation.is_empty()
                || m.access.is_some()
                || !m.relationships.is_empty()
            {
                result.insert(name, m);
            }
        }
        result
    }

    /// Return the brace-balanced body of `key: { ... }` (content between the matching braces),
    /// or None. Shared by all metadata extraction so brace-walking lives in exactly one place.
    fn sub_block(content: &str, key: &str) -> Option<String> {
        let key_re = Regex::new(&format!(r"{}\s*:\s*\{{", regex::escape(key))).ok()?;
        let m = key_re.find(content)?;
        let bytes = content.as_bytes();
        let mut depth = 1usize;
        let mut j = m.end();
        let start = j;
        while j < bytes.len() && depth > 0 {
            match bytes[j] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            j += 1;
        }
        Some(content[start..j.saturating_sub(1)].to_string())
    }

    /// Split a block into its direct (depth-1) `Name: { ... }` entries → (name, inner-block).
    /// Used for both the `models` container and a `relationships` block.
    fn top_level_entries(block: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let entry_re = Regex::new(r"(\w+)\s*:\s*\{").unwrap();
        let bytes = block.as_bytes();
        for cap in entry_re.captures_iter(block) {
            // Only depth-0 entries within `block` (not keys nested inside a child).
            let open = cap.get(0).unwrap();
            let before = &block[..open.start()];
            let depth = before.bytes().filter(|&b| b == b'{').count() as i64
                - before.bytes().filter(|&b| b == b'}').count() as i64;
            if depth != 0 {
                continue;
            }
            let name = cap[1].to_string();
            let mut d = 1usize;
            let mut j = open.end();
            let start = j;
            while j < bytes.len() && d > 0 {
                match bytes[j] {
                    b'{' => d += 1,
                    b'}' => d -= 1,
                    _ => {}
                }
                j += 1;
            }
            out.push((name, block[start..j.saturating_sub(1)].to_string()));
        }
        out
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
                relationships: std::collections::HashMap::new(),
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
        relationships: std::collections::HashMap::new(),
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
        // File/media fields store the uploaded media id (or URL) as TEXT; the bytes live in the
        // storage backend behind /media. String-backed so no codegen/match sites change.
        "File" => FieldType::String,
        "File[]" => FieldType::Array(Box::new(FieldType::String)),
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
        // Mirrors the real CRM schema: nested access/relationships/validation in one model block.
        // Exercises the UNIFIED parser extracting all four metadata features from one block.
        let content = r#"
        export const schema = {
          models: {
            Contact: {
              tableName: 'contacts',
              access: { create: 'sales|manager', read: 'authenticated' },
              relationships: { company: { type: 'belongsTo', model: 'Company', foreignKey: 'companyId' } },
              validation: {
                email: 'email',
                firstName: 'required|min:1|max:100'
              }
            }
          }
        };
        "#;
        let meta = TypeScriptParser::parse_model_metadata(content);
        let c = meta.get("Contact").expect("Contact metadata missing");
        // tableName
        assert_eq!(c.table_name.as_deref(), Some("contacts"));
        // validation (must not be confused by the nested access/relationships blocks)
        assert_eq!(c.validation.get("email").map(|s| s.as_str()), Some("email"));
        assert_eq!(
            c.validation.get("firstName").map(|s| s.as_str()),
            Some("required|min:1|max:100")
        );
        // access
        assert!(c.access.is_some(), "access parsed");
        // relationships — schema-driven (name → declared target model + fk)
        let rel = c
            .relationships
            .get("company")
            .expect("company relationship parsed");
        assert_eq!(rel.kind, "belongsTo");
        assert_eq!(rel.model, "Company");
        assert_eq!(rel.foreign_key.as_deref(), Some("companyId"));
    }

    #[test]
    fn parses_file_field_as_string_backed() {
        // A `File` field stores the media id/url as TEXT (string-backed); `File[]` -> Array(String).
        let content = "export interface Contact { id: string; avatar: File; photos: File[]; }";
        let models = TypeScriptParser::new().parse_interfaces(content).unwrap();
        let c = models
            .iter()
            .find(|m| m.name == "Contact")
            .expect("Contact parsed");
        assert_eq!(c.fields.get("avatar").unwrap().field_type, FieldType::String);
        assert_eq!(
            c.fields.get("photos").unwrap().field_type,
            FieldType::Array(Box::new(FieldType::String))
        );
    }

    #[test]
    fn parses_single_line_and_packed_interfaces() {
        // The footgun: a single-line interface (and multiple fields on one line) used to be
        // silently dropped, producing tables with missing columns and no error.
        let single = "export interface Contact { id: string; email: string; name: string; }";
        let models = TypeScriptParser::new().parse_interfaces(single).unwrap();
        let c = models
            .iter()
            .find(|m| m.name == "Contact")
            .expect("Contact parsed");
        for f in ["id", "email", "name"] {
            assert!(
                c.fields.contains_key(f),
                "single-line field '{}' must be parsed",
                f
            );
        }

        // Multi-line still works, including a comment and an optional field.
        let multi = "export interface Note {\n  id: string;\n  title?: string; // heading\n}";
        let models = TypeScriptParser::new().parse_interfaces(multi).unwrap();
        let n = models
            .iter()
            .find(|m| m.name == "Note")
            .expect("Note parsed");
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
        let c = models
            .iter()
            .find(|m| m.name == "Contact")
            .expect("Contact parsed");
        let ac = c
            .access
            .as_ref()
            .expect("access rules must be parsed (RBAC bypass fix)");

        // create requires sales|manager|admin
        assert_eq!(
            ac.decide("create", Some("Viewer")),
            AccessDecision::Forbidden,
            "viewer denied create"
        );
        assert_eq!(
            ac.decide("create", Some("Sales")),
            AccessDecision::Allow,
            "sales allowed create"
        );
        assert_eq!(
            ac.decide("create", None),
            AccessDecision::NeedsAuth,
            "anon needs auth"
        );
        // read is authenticated → any logged-in role allowed
        assert_eq!(
            ac.decide("read", Some("Viewer")),
            AccessDecision::Allow,
            "viewer can read"
        );
        // delete gated to manager|admin
        assert_eq!(
            ac.decide("delete", Some("Sales")),
            AccessDecision::Forbidden,
            "sales cannot delete"
        );
        assert_eq!(
            ac.decide("delete", Some("Admin")),
            AccessDecision::Allow,
            "admin can delete"
        );
    }
}
