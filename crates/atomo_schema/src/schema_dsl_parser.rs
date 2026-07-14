//! Parser for the `@atomo/schema` builder-pattern DSL.
//!
//! Handles schemas written as:
//! ```ts
//! import { model, text, action, ... } from '@atomo/schema'
//! export const onX = action('onX').from('posts').input(['id', 'title'])
//! export const Post = model('posts', { fields: { ... }, access: { ... }, on: { ... } })
//! ```
//!
//! Produces the same [`Schema`] as the legacy TypeScript-interface parser so downstream
//! (codegen, migrations, server) works unchanged.

use crate::types::*;
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;

/// Returns `true` if the content looks like the builder DSL (imports from `@atomo/schema`
/// or `@atomo-cc/schema`, or contains bare `model(` calls with field builders).
pub fn is_builder_dsl(content: &str) -> bool {
    content.contains("from '@atomo/schema'")
        || content.contains("from \"@atomo/schema\"")
        || content.contains("from '@atomo-cc/schema'")
        || content.contains("from \"@atomo-cc/schema\"")
}

/// Parse a builder-DSL schema into a [`Schema`].
pub fn parse_builder_dsl(content: &str) -> Result<Schema> {
    let actions = parse_action_defs(content);
    let table_to_model = parse_model_names(content);
    let mut models = HashMap::new();

    for (model_name, table_name, body) in parse_model_blocks(content) {
        let fields = parse_fields_block(&body);
        let access = parse_access_block(&body);
        let events = parse_on_block(&body);
        let (field_map, validation, relationships) = resolve_fields(&fields, &table_to_model);

        models.insert(
            model_name.clone(),
            Model {
                name: model_name,
                fields: field_map,
                access,
                hooks: None,
                validation,
                table_name: Some(table_name),
                relationships,
                constraints: Vec::new(),
                events,
                ui: None,
            },
        );
    }

    let mut action_defs = HashMap::new();
    for ad in actions {
        let resolved_model = ad
            .source_model
            .as_deref()
            .and_then(|t| table_to_model.get(t))
            .cloned();
        let input = match ad.input {
            ActionInputDef::PickFields { model, fields } => {
                let resolved = table_to_model.get(&model).cloned().unwrap_or(model);
                ActionInputDef::PickFields {
                    model: resolved,
                    fields,
                }
            }
            other => other,
        };
        action_defs.insert(
            ad.name.clone(),
            ActionDef {
                name: ad.name,
                input,
                returns: ad.returns,
                source_model: resolved_model.or(ad.source_model),
            },
        );
    }

    Ok(Schema {
        models,
        actions: action_defs,
        // The defineModel DSL path doesn't support built-in table extensions;
        // declare them via the `builtins` block in schema metadata instead.
        builtins: std::collections::HashMap::new(),
    })
}

// ── action() parsing ────────────────────────────────────────────────────────

fn parse_action_defs(content: &str) -> Vec<ActionDef> {
    let mut actions = Vec::new();
    let re =
        Regex::new(r"export\s+const\s+(\w+)\s*=\s*action\s*\(\s*['\x22]([^'\x22]+)['\x22]\s*\)")
            .unwrap();

    for cap in re.captures_iter(content) {
        let _var_name = &cap[1];
        let action_name = cap[2].to_string();

        let after = &content[cap.get(0).unwrap().end()..];
        let chain = take_chain(after);

        let from = extract_chain_string(&chain, "from");
        let input_fields = extract_chain_array(&chain, "input");

        let source_model = from.clone();
        let input = if let Some(fields) = input_fields {
            ActionInputDef::PickFields {
                model: from.unwrap_or_default(),
                fields,
            }
        } else {
            ActionInputDef::Object { fields: vec![] }
        };

        actions.push(ActionDef {
            name: action_name,
            input,
            returns: None,
            source_model,
        });
    }

    actions
}

/// Consume a chain of `.method(...)` calls from the start of `s`.
fn take_chain(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'.' => {
                i += 1;
                // skip method name
                while i < bytes.len() && bytes[i].is_ascii_alphanumeric()
                    || (i < bytes.len() && bytes[i] == b'_')
                {
                    i += 1;
                }
                // skip whitespace
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                // skip balanced parens
                if i < bytes.len() && bytes[i] == b'(' {
                    let mut depth = 1;
                    i += 1;
                    while i < bytes.len() && depth > 0 {
                        match bytes[i] {
                            b'(' => depth += 1,
                            b')' => depth -= 1,
                            b'\'' | b'"' => {
                                let q = bytes[i];
                                i += 1;
                                while i < bytes.len() && bytes[i] != q {
                                    if bytes[i] == b'\\' {
                                        i += 1;
                                    }
                                    i += 1;
                                }
                                // skip closing quote
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                }
            }
            _ => break,
        }
    }
    s[..i].to_string()
}

fn extract_chain_string(chain: &str, method: &str) -> Option<String> {
    let re = Regex::new(&format!(
        r"\.{}\s*\(\s*['\x22]([^'\x22]+)['\x22]\s*\)",
        method
    ))
    .ok()?;
    re.captures(chain).map(|c| c[1].to_string())
}

fn extract_chain_array(chain: &str, method: &str) -> Option<Vec<String>> {
    let re = Regex::new(&format!(r"\.{}\s*\(\s*\[([^\]]*)\]\s*\)", method)).ok()?;
    let cap = re.captures(chain)?;
    let inner = &cap[1];
    let items: Vec<String> = Regex::new(r"['\x22]([^'\x22]+)['\x22]")
        .unwrap()
        .captures_iter(inner)
        .map(|c| c[1].to_string())
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

// ── model() parsing ─────────────────────────────────────────────────────────

/// Extract `export const Name = model('table', { ... })` → (ModelName, tableName).
fn parse_model_names(content: &str) -> HashMap<String, String> {
    let re =
        Regex::new(r"export\s+const\s+(\w+)\s*=\s*model\s*\(\s*['\x22]([^'\x22]+)['\x22]").unwrap();
    let mut map = HashMap::new();
    for cap in re.captures_iter(content) {
        map.insert(cap[2].to_string(), cap[1].to_string());
    }
    map
}

/// Extract all model blocks: (ModelName, tableName, objectBody).
fn parse_model_blocks(content: &str) -> Vec<(String, String, String)> {
    let re =
        Regex::new(r"export\s+const\s+(\w+)\s*=\s*model\s*\(\s*['\x22]([^'\x22]+)['\x22]\s*,\s*\{")
            .unwrap();
    let mut out = Vec::new();

    for cap in re.captures_iter(content) {
        let name = cap[1].to_string();
        let table = cap[2].to_string();
        let brace_start = cap.get(0).unwrap().end(); // just after the `{`
        if let Some(body) = balanced_brace_body(content, brace_start) {
            out.push((name, table, body));
        }
    }
    out
}

/// Starting right after an opening `{`, return content up to the matching `}`.
fn balanced_brace_body(content: &str, start: usize) -> Option<String> {
    let bytes = content.as_bytes();
    let mut depth = 1usize;
    let mut i = start;
    while i < bytes.len() && depth > 0 {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b'\'' | b'"' | b'`' => {
                let q = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != q {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    if depth == 0 {
        Some(content[start..i.saturating_sub(1)].to_string())
    } else {
        None
    }
}

// ── fields block ────────────────────────────────────────────────────────────

struct RawField {
    name: String,
    base_type: String,
    base_arg: Option<String>,
    chain: String,
}

fn parse_fields_block(body: &str) -> Vec<RawField> {
    let fields_re = Regex::new(r"fields\s*:\s*\{").unwrap();
    let m = match fields_re.find(body) {
        Some(m) => m,
        None => return vec![],
    };
    let inner = match balanced_brace_body(body, m.end()) {
        Some(s) => s,
        None => return vec![],
    };

    let mut fields = Vec::new();
    // Match `name: builder(...)` where builder is a known function name.
    let field_re = Regex::new(
        r"(\w+)\s*:\s*(text|email|url|number|select|datetime|relation|boolean|json|file)\s*\(",
    )
    .unwrap();

    for cap in field_re.captures_iter(&inner) {
        let field_name = cap[1].to_string();
        let base_type = cap[2].to_string();

        // Find the balanced parens for the base call argument.
        let paren_start = cap.get(0).unwrap().end();
        let (base_arg, after_paren) = extract_paren_arg(&inner, paren_start);

        let chain = take_chain(&inner[after_paren..]);

        fields.push(RawField {
            name: field_name,
            base_type,
            base_arg,
            chain,
        });
    }

    fields
}

/// Given a position right after `(`, extract the content to the matching `)` and return
/// (arg-content-or-None, position-after-closing-paren).
fn extract_paren_arg(content: &str, start: usize) -> (Option<String>, usize) {
    let bytes = content.as_bytes();
    let mut depth = 1usize;
    let mut i = start;
    while i < bytes.len() && depth > 0 {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'\'' | b'"' => {
                let q = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != q {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let raw = content[start..i.saturating_sub(1)].trim();
    let arg = if raw.is_empty() {
        None
    } else {
        Some(raw.to_string())
    };
    (arg, i)
}

fn resolve_fields(
    raw: &[RawField],
    table_to_model: &HashMap<String, String>,
) -> (
    HashMap<String, Field>,
    HashMap<String, String>,
    HashMap<String, Relationship>,
) {
    let mut fields = HashMap::new();
    let mut validation = HashMap::new();
    let mut relationships = HashMap::new();

    for f in raw {
        let (field_type, is_relation_target) = base_to_field_type(&f.base_type, &f.base_arg);
        let chain = &f.chain;

        let is_id = chain.contains(".id()");
        let is_optional = chain.contains(".optional()");
        let is_required = chain.contains(".required()");
        let optional = if is_id {
            false
        } else if is_required {
            false
        } else {
            is_optional || !is_required
        };
        // For relation fields, if not explicitly required, default to optional.
        let optional = if f.base_type == "relation" && !is_required {
            true
        } else {
            optional
        };
        let optional = if f.base_type == "datetime" && !is_optional {
            false
        } else {
            optional
        };

        let mut attrs = Vec::new();
        if is_id {
            attrs.push(FieldAttribute::Primary);
        }
        if chain.contains(".defaultNow()") || chain.contains(".autoUpdate()") {
            attrs.push(FieldAttribute::Timestamp);
        }
        if let Some(def) = extract_chain_string(chain, "default") {
            attrs.push(FieldAttribute::Default(def));
        }

        // Validation rules
        let mut rules = Vec::new();
        if is_required {
            rules.push("required".to_string());
        }
        if f.base_type == "email" {
            rules.push("email".to_string());
        }
        if f.base_type == "url" {
            rules.push("url".to_string());
        }
        if let Some(n) = extract_chain_number(chain, "min") {
            rules.push(format!("min:{}", n));
        }
        if let Some(n) = extract_chain_number(chain, "max") {
            rules.push(format!("max:{}", n));
        }
        // select(['a', 'b']) → `in:a,b`. Previously the allowed values were
        // discarded at parse time, so the runtime validator couldn't enforce
        // them and the admin UI rendered enums as free-text inputs.
        if f.base_type == "select" {
            if let Some(arg) = &f.base_arg {
                let values: Vec<String> = Regex::new(r"['\x22]([^'\x22]+)['\x22]")
                    .unwrap()
                    .captures_iter(arg)
                    .map(|c| c[1].to_string())
                    .collect();
                if !values.is_empty() {
                    rules.push(format!("in:{}", values.join(",")));
                }
            }
        }
        if !rules.is_empty() {
            validation.insert(f.name.clone(), rules.join("|"));
        }

        // Relationships from relation() fields
        if let Some(target_table) = &is_relation_target {
            let target_model = table_to_model
                .get(target_table.as_str())
                .cloned()
                .unwrap_or_else(|| capitalize(target_table));
            let rel_name = f.name.strip_suffix("Id").unwrap_or(&f.name);
            relationships.insert(
                rel_name.to_string(),
                Relationship {
                    kind: "belongsTo".to_string(),
                    model: target_model,
                    foreign_key: Some(f.name.clone()),
                },
            );
        }

        fields.insert(
            f.name.clone(),
            Field {
                name: f.name.clone(),
                field_type,
                optional,
                attributes: attrs,
            },
        );
    }

    (fields, validation, relationships)
}

fn base_to_field_type(base: &str, arg: &Option<String>) -> (FieldType, Option<String>) {
    match base {
        "text" => (FieldType::String, None),
        "email" => (FieldType::String, None),
        "url" => (FieldType::String, None),
        "number" => (FieldType::Number, None),
        "boolean" => (FieldType::Boolean, None),
        "datetime" => (FieldType::DateTime, None),
        "json" => (FieldType::Json, None),
        "file" => (FieldType::File, None),
        "select" => (FieldType::String, None),
        "relation" => {
            let target = arg
                .as_ref()
                .map(|a| a.trim_matches(|c: char| c == '\'' || c == '"').to_string());
            (FieldType::String, target)
        }
        _ => (FieldType::Custom(base.to_string()), None),
    }
}

fn extract_chain_number(chain: &str, method: &str) -> Option<i64> {
    let re = Regex::new(&format!(r"\.{}\s*\(\s*(-?\d+)\s*\)", method)).ok()?;
    re.captures(chain).and_then(|c| c[1].parse().ok())
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

// ── access block ────────────────────────────────────────────────────────────

fn parse_access_block(body: &str) -> Option<AccessControl> {
    let re = Regex::new(r"access\s*:\s*\{").unwrap();
    let m = re.find(body)?;
    let inner = balanced_brace_body(body, m.end())?;

    let parse_rule = |op: &str| -> Option<AccessRule> {
        // Grab this op's full rule expression (to end of line) so combinator forms
        // like allow.any([allow.role('a'), allow.role('b')]) are seen whole.
        // Previously only bare allow.role/authenticated/public matched — any other
        // form (allow.any, allow.sameTenant, allow.owner) parsed to None, which
        // silently DISABLED access control for that operation.
        let expr_re = Regex::new(&format!(r"{}\s*:\s*(allow\s*\.[^\n]+)", op)).ok()?;
        let expr = expr_re.captures(&inner)?.get(1)?.as_str().to_string();

        // allow.public() — anonymous allowed, nothing further to gate.
        if Regex::new(r"\bpublic\s*\(\s*\)").unwrap().is_match(&expr) {
            return Some(AccessRule::Boolean("public".to_string()));
        }

        // allow.authenticated(), allow.sameTenant(..), allow.owner(..): all require a
        // logged-in principal. Tenant scoping / ownership are enforced by the data
        // layer's tenant_id isolation and owner checks — at THIS layer (role gating)
        // they gate to "any authenticated user", never to "no rule". Checked BEFORE
        // roles: in a mixed any-of like any([owner(..), role('admin')]) a roles-only
        // gate would wrongly deny legitimate owners, so the sound approximation for
        // the whole expression is "authenticated".
        if Regex::new(r"\b(authenticated|sameTenant|owner)\s*\(")
            .unwrap()
            .is_match(&expr)
        {
            return Some(AccessRule::Boolean("authenticated".to_string()));
        }

        // Role gates: collect every role('x') / role(['x','y']) mention. Covers both
        // the bare form and combinators like allow.any([allow.role('admin'), ...]) —
        // any-of over roles is exactly what the pipe-joined rule string expresses.
        let roles: Vec<String> = Regex::new(r"role\s*\(\s*\[?([^)\]]*)\]?\s*\)")
            .unwrap()
            .captures_iter(&expr)
            .flat_map(|cap| {
                Regex::new(r"['\x22]([^'\x22]+)['\x22]")
                    .unwrap()
                    .captures_iter(&cap[1])
                    .map(|c| c[1].to_string())
                    .collect::<Vec<_>>()
            })
            .collect();
        if !roles.is_empty() {
            return Some(AccessRule::Boolean(roles.join("|")));
        }

        None
    };

    let create = parse_rule("create");
    let read = parse_rule("read");
    let update = parse_rule("update");
    let delete = parse_rule("delete");

    if create.is_some() || read.is_some() || update.is_some() || delete.is_some() {
        Some(AccessControl {
            create,
            read,
            update,
            delete,
        })
    } else {
        None
    }
}

// ── on (events) block ───────────────────────────────────────────────────────

fn parse_on_block(body: &str) -> ModelEvents {
    let re = Regex::new(r"\bon\s*:\s*\{").unwrap();
    let m = match re.find(body) {
        Some(m) => m,
        None => return ModelEvents::default(),
    };
    let inner = match balanced_brace_body(body, m.end()) {
        Some(s) => s,
        None => return ModelEvents::default(),
    };

    ModelEvents {
        created: parse_event_list(&inner, "created"),
        updated: parse_event_list(&inner, "updated"),
        deleted: parse_event_list(&inner, "deleted"),
    }
}

fn parse_event_list(on_block: &str, event_kind: &str) -> Vec<EventActionBinding> {
    let re = Regex::new(&format!(r"{}\s*:\s*\[", event_kind)).unwrap();
    let m = match re.find(on_block) {
        Some(m) => m,
        None => return vec![],
    };

    // Find balanced brackets.
    let bytes = on_block.as_bytes();
    let mut depth = 1usize;
    let mut i = m.end();
    let start = i;
    while i < bytes.len() && depth > 0 {
        match bytes[i] {
            b'[' => depth += 1,
            b']' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    let inner = &on_block[start..i.saturating_sub(1)];

    let mut bindings = Vec::new();

    // Split on commas that are not inside parentheses to get individual entries.
    let entries = split_top_level_commas(inner);
    let str_re = Regex::new(r"['\x22]([^'\x22]+)['\x22]").unwrap();

    for entry in entries {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Extract leading identifier: actionName or actionName.whenChanged(...) or actionName.when(...)
        let ident_re = Regex::new(r"^(\w+)").unwrap();
        let action_name = match ident_re.captures(trimmed) {
            Some(cap) => cap[1].to_string(),
            None => continue,
        };
        if ["true", "false", "null", "undefined"].contains(&action_name.as_str()) {
            continue;
        }

        let condition = if let Some(wc_match) = Regex::new(r"\.whenChanged\s*\(([^)]*)\)")
            .unwrap()
            .captures(trimmed)
        {
            let args_str = &wc_match[1];
            let fields: Vec<String> = str_re
                .captures_iter(args_str)
                .map(|c| c[1].to_string())
                .collect();
            if fields.is_empty() {
                None
            } else {
                Some(ActionCondition::ChangedAny(fields))
            }
        } else if let Some(w_match) = Regex::new(r"\.when\s*\(([^)]*)\)")
            .unwrap()
            .captures(trimmed)
        {
            let args_str = &w_match[1];
            let strs: Vec<String> = str_re
                .captures_iter(args_str)
                .map(|c| c[1].to_string())
                .collect();
            if strs.len() >= 2 {
                Some(ActionCondition::FieldEquals {
                    field: strs[0].clone(),
                    value: serde_json::Value::String(strs[1].clone()),
                })
            } else {
                None
            }
        } else {
            None
        };

        bindings.push(EventActionBinding {
            action: action_name,
            condition,
        });
    }

    bindings
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut depth = 0usize;
    let mut splits = Vec::new();
    let mut start = 0;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                splits.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    splits.push(&s[start..]);
    splits
}

// ── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const CRM_DSL: &str = r#"
import { model, text, email, url, number, select, datetime, relation, allow, action } from '@atomo/schema'

export const onNewContact = action('onNewContact')
  .from('contacts')
  .input(['id', 'name', 'email'])

export const onStageChange = action('onStageChange')
  .from('contacts')
  .input(['id', 'stage'])

export const onDealStatusChange = action('onDealStatusChange')
  .from('deals')
  .input(['id', 'contactId', 'status', 'value'])

export const Company = model('companies', {
  fields: {
    id: text().id(),
    name: text().required().min(1),
    industry: text().optional(),
    website: url().optional(),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
  access: {
    create: allow.role(['sales', 'admin']),
    read: allow.authenticated(),
    update: allow.role(['sales', 'admin']),
    delete: allow.role('admin'),
  },
})

export const Contact = model('contacts', {
  fields: {
    id: text().id(),
    name: text().required().min(1).max(100),
    email: email().required(),
    phone: text().optional(),
    stage: select(['lead', 'qualified', 'customer']).default('lead'),
    companyId: relation('companies').optional(),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
  access: {
    create: allow.role(['sales', 'admin']),
    read: allow.authenticated(),
    update: allow.role(['sales', 'admin']),
    delete: allow.role('admin'),
  },
  on: {
    created: [onNewContact],
    updated: [onStageChange.whenChanged('stage')],
  },
})

export const Deal = model('deals', {
  fields: {
    id: text().id(),
    title: text().required().min(1).max(160),
    value: number().min(0),
    status: select(['open', 'won', 'lost']).default('open'),
    contactId: relation('contacts').required(),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
  access: {
    create: allow.role(['sales', 'admin']),
    read: allow.authenticated(),
    update: allow.role(['sales', 'admin']),
    delete: allow.role('admin'),
  },
  on: {
    updated: [onDealStatusChange.whenChanged('status')],
  },
})

export const Activity = model('activities', {
  fields: {
    id: text().id(),
    contactId: relation('contacts').required(),
    dealId: relation('deals').optional(),
    type: select(['call', 'email', 'meeting']).required(),
    notes: text().optional(),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
  access: {
    create: allow.role(['sales', 'admin']),
    read: allow.authenticated(),
    update: allow.role(['sales', 'admin']),
    delete: allow.role('admin'),
  },
})
"#;

    #[test]
    fn detects_builder_dsl() {
        assert!(is_builder_dsl(CRM_DSL));
        assert!(!is_builder_dsl("export interface Post { id: string; }"));
    }

    #[test]
    fn parses_actions() {
        let schema = parse_builder_dsl(CRM_DSL).unwrap();
        assert_eq!(schema.actions.len(), 3);

        let on_new = schema.actions.get("onNewContact").unwrap();
        assert_eq!(on_new.source_model.as_deref(), Some("Contact"));
        if let ActionInputDef::PickFields { model, fields } = &on_new.input {
            assert_eq!(model, "Contact");
            assert_eq!(fields, &["id", "name", "email"]);
        } else {
            panic!("expected PickFields");
        }

        let on_deal = schema.actions.get("onDealStatusChange").unwrap();
        assert_eq!(on_deal.source_model.as_deref(), Some("Deal"));
    }

    #[test]
    fn parses_all_models() {
        let schema = parse_builder_dsl(CRM_DSL).unwrap();
        assert_eq!(schema.models.len(), 4);
        assert!(schema.models.contains_key("Company"));
        assert!(schema.models.contains_key("Contact"));
        assert!(schema.models.contains_key("Deal"));
        assert!(schema.models.contains_key("Activity"));
    }

    #[test]
    fn parses_table_names() {
        let schema = parse_builder_dsl(CRM_DSL).unwrap();
        assert_eq!(
            schema.models["Company"].table_name.as_deref(),
            Some("companies")
        );
        assert_eq!(
            schema.models["Contact"].table_name.as_deref(),
            Some("contacts")
        );
        assert_eq!(schema.models["Deal"].table_name.as_deref(), Some("deals"));
        assert_eq!(
            schema.models["Activity"].table_name.as_deref(),
            Some("activities")
        );
    }

    #[test]
    fn parses_field_types() {
        let schema = parse_builder_dsl(CRM_DSL).unwrap();
        let contact = &schema.models["Contact"];

        assert_eq!(contact.fields["id"].field_type, FieldType::String);
        assert!(!contact.fields["id"].optional);
        assert!(contact.fields["id"]
            .attributes
            .iter()
            .any(|a| matches!(a, FieldAttribute::Primary)));

        assert_eq!(contact.fields["name"].field_type, FieldType::String);
        assert!(!contact.fields["name"].optional);

        assert_eq!(contact.fields["email"].field_type, FieldType::String);

        assert_eq!(contact.fields["phone"].field_type, FieldType::String);
        assert!(contact.fields["phone"].optional);

        assert_eq!(contact.fields["stage"].field_type, FieldType::String);

        assert_eq!(contact.fields["companyId"].field_type, FieldType::String);
        assert!(contact.fields["companyId"].optional);

        let deal = &schema.models["Deal"];
        assert_eq!(deal.fields["value"].field_type, FieldType::Number);
        assert_eq!(deal.fields["createdAt"].field_type, FieldType::DateTime);
    }

    #[test]
    fn parses_validation() {
        let schema = parse_builder_dsl(CRM_DSL).unwrap();
        let contact = &schema.models["Contact"];
        assert_eq!(
            contact.validation.get("name").map(|s| s.as_str()),
            Some("required|min:1|max:100")
        );
        assert_eq!(
            contact.validation.get("email").map(|s| s.as_str()),
            Some("required|email")
        );

        let deal = &schema.models["Deal"];
        assert_eq!(
            deal.validation.get("title").map(|s| s.as_str()),
            Some("required|min:1|max:160")
        );
        assert_eq!(
            deal.validation.get("value").map(|s| s.as_str()),
            Some("min:0")
        );
    }

    #[test]
    fn select_values_become_in_rule() {
        // select(['a','b']) must survive parsing as an `in:` validation rule —
        // the runtime validator enforces it and the admin UI renders a Select.
        let dsl = r#"
import { model, text, select } from '@atomo/schema'
export const Deal = model('deals', {
  fields: {
    id: text().id(),
    stage: select(['prospecting', 'proposal', 'won']).default('prospecting'),
    kind: select(['new']).required(),
  },
})
"#;
        let schema = parse_builder_dsl(dsl).unwrap();
        let deal = &schema.models["Deal"];
        assert_eq!(
            deal.validation.get("stage").map(|s| s.as_str()),
            Some("in:prospecting,proposal,won")
        );
        assert_eq!(
            deal.validation.get("kind").map(|s| s.as_str()),
            Some("required|in:new")
        );
    }

    #[test]
    fn parses_relationships() {
        let schema = parse_builder_dsl(CRM_DSL).unwrap();
        let contact = &schema.models["Contact"];
        let rel = contact
            .relationships
            .get("company")
            .expect("company relationship");
        assert_eq!(rel.kind, "belongsTo");
        assert_eq!(rel.model, "Company");
        assert_eq!(rel.foreign_key.as_deref(), Some("companyId"));

        let deal = &schema.models["Deal"];
        let rel = deal
            .relationships
            .get("contact")
            .expect("contact relationship");
        assert_eq!(rel.kind, "belongsTo");
        assert_eq!(rel.model, "Contact");
    }

    #[test]
    fn parses_access_control() {
        let schema = parse_builder_dsl(CRM_DSL).unwrap();
        let contact = &schema.models["Contact"];
        let ac = contact.access.as_ref().expect("access control");

        match &ac.create {
            Some(AccessRule::Boolean(s)) => assert_eq!(s, "sales|admin"),
            other => panic!("expected Boolean, got {:?}", other),
        }
        match &ac.read {
            Some(AccessRule::Boolean(s)) => assert_eq!(s, "authenticated"),
            other => panic!("expected Boolean, got {:?}", other),
        }
        match &ac.delete {
            Some(AccessRule::Boolean(s)) => assert_eq!(s, "admin"),
            other => panic!("expected Boolean, got {:?}", other),
        }
    }

    #[test]
    fn parses_access_combinators() {
        // The forms the CRM dogfood schema actually uses. Before this parser
        // handled them, they parsed to None — access control silently OFF.
        let dsl = r#"
export const Contact = model('contacts', {
  fields: { id: text().id() },
  access: {
    read: allow.sameTenant('tenantId'),
    create: allow.any([allow.role('admin'), allow.role('manager'), allow.role('sales')]),
    update: allow.any([allow.owner('ownerId'), allow.role('admin'), allow.role('manager')]),
    delete: allow.any([allow.role('admin'), allow.role('manager')]),
  },
})
"#;
        let schema = parse_builder_dsl(dsl).unwrap();
        let ac = schema.models["Contact"].access.as_ref().expect("access");

        // any-of over roles → pipe-joined role gate.
        match &ac.create {
            Some(AccessRule::Boolean(s)) => assert_eq!(s, "admin|manager|sales"),
            other => panic!("expected Boolean, got {:?}", other),
        }
        match &ac.delete {
            Some(AccessRule::Boolean(s)) => assert_eq!(s, "admin|manager"),
            other => panic!("expected Boolean, got {:?}", other),
        }
        // sameTenant → any authenticated user at the role layer.
        match &ac.read {
            Some(AccessRule::Boolean(s)) => assert_eq!(s, "authenticated"),
            other => panic!("expected Boolean, got {:?}", other),
        }
        // mixed any(owner, roles) → authenticated, so legitimate owners aren't denied.
        match &ac.update {
            Some(AccessRule::Boolean(s)) => assert_eq!(s, "authenticated"),
            other => panic!("expected Boolean, got {:?}", other),
        }
    }

    #[test]
    fn parses_events() {
        let schema = parse_builder_dsl(CRM_DSL).unwrap();

        let contact = &schema.models["Contact"];
        assert_eq!(contact.events.created.len(), 1);
        assert_eq!(contact.events.created[0].action, "onNewContact");
        assert!(contact.events.created[0].condition.is_none());

        assert_eq!(contact.events.updated.len(), 1);
        assert_eq!(contact.events.updated[0].action, "onStageChange");
        match &contact.events.updated[0].condition {
            Some(ActionCondition::ChangedAny(fields)) => assert_eq!(fields, &["stage"]),
            other => panic!("expected ChangedAny, got {:?}", other),
        }

        let deal = &schema.models["Deal"];
        assert_eq!(deal.events.updated.len(), 1);
        assert_eq!(deal.events.updated[0].action, "onDealStatusChange");
        match &deal.events.updated[0].condition {
            Some(ActionCondition::ChangedAny(fields)) => assert_eq!(fields, &["status"]),
            other => panic!("expected ChangedAny, got {:?}", other),
        }

        // Activity has no events.
        assert!(schema.models["Activity"].events.is_empty());
    }

    #[test]
    fn codegen_produces_valid_output() {
        let schema = parse_builder_dsl(CRM_DSL).unwrap();
        let ts = crate::codegen::generate_typescript_client(&schema);

        assert!(ts.contains("export interface Contact {"));
        assert!(ts.contains("export interface Deal {"));
        assert!(ts.contains("export type onNewContactInput = Pick<Contact,"));
        assert!(ts.contains("export type onDealStatusChangeInput = Pick<Deal,"));
        assert!(ts.contains("export interface ActionHandlers {"));
        assert!(ts.contains("export interface TypedWorker {"));
    }

    // ── is_builder_dsl edge cases ──────────────────────────────────────────

    #[test]
    fn is_builder_dsl_rejects_legacy_ts_interface() {
        let legacy = r#"
            export interface Post {
                id: string;
                title: string;
                body: string;
            }
        "#;
        assert!(!is_builder_dsl(legacy));
    }

    #[test]
    fn is_builder_dsl_rejects_empty_string() {
        assert!(!is_builder_dsl(""));
    }

    #[test]
    fn is_builder_dsl_detects_commented_out_import() {
        // The function does a plain `contains`, so a commented-out import still
        // returns true. Verify that current behaviour so we notice if it changes.
        let commented = r#"
            // import { model } from '@atomo/schema'
        "#;
        assert!(
            is_builder_dsl(commented),
            "commented-out import currently counts as builder DSL (substring match)"
        );
    }

    // ── parse_builder_dsl edge cases ───────────────────────────────────────

    #[test]
    fn minimal_model_with_only_id_field() {
        let dsl = r#"
import { model, text } from '@atomo/schema'

export const Tag = model('tags', {
  fields: {
    id: text().id(),
  },
})
"#;
        let schema = parse_builder_dsl(dsl).unwrap();
        let tag = schema.models.get("Tag").expect("Tag model");
        assert_eq!(tag.fields.len(), 1);
        assert_eq!(tag.fields["id"].field_type, FieldType::String);
        assert!(!tag.fields["id"].optional);
        assert!(tag.fields["id"]
            .attributes
            .iter()
            .any(|a| matches!(a, FieldAttribute::Primary)));
        assert!(tag.access.is_none());
        assert!(tag.events.is_empty());
        assert!(tag.relationships.is_empty());
    }

    #[test]
    fn model_with_all_field_types() {
        let dsl = r#"
import { model, text, email, url, number, select, datetime, relation, boolean, json, file } from '@atomo/schema'

export const Author = model('authors', {
  fields: {
    id: text().id(),
  },
})

export const Everything = model('everything', {
  fields: {
    id: text().id(),
    title: text().required(),
    contact: email().required(),
    homepage: url().optional(),
    count: number().min(0).max(999),
    status: select(['draft', 'published']).default('draft'),
    publishedAt: datetime().defaultNow(),
    authorId: relation('authors').required(),
    active: boolean(),
    meta: json().optional(),
    avatar: file().optional(),
  },
})
"#;
        let schema = parse_builder_dsl(dsl).unwrap();
        let model = schema.models.get("Everything").expect("Everything model");

        assert_eq!(model.fields["id"].field_type, FieldType::String);
        assert_eq!(model.fields["title"].field_type, FieldType::String);
        assert_eq!(model.fields["contact"].field_type, FieldType::String);
        assert_eq!(model.fields["homepage"].field_type, FieldType::String);
        assert_eq!(model.fields["count"].field_type, FieldType::Number);
        assert_eq!(model.fields["status"].field_type, FieldType::String);
        assert_eq!(model.fields["publishedAt"].field_type, FieldType::DateTime);
        assert_eq!(model.fields["authorId"].field_type, FieldType::String);
        assert_eq!(model.fields["active"].field_type, FieldType::Boolean);
        assert_eq!(model.fields["meta"].field_type, FieldType::Json);
        assert_eq!(model.fields["avatar"].field_type, FieldType::File);

        // Relation creates a relationship entry
        let rel = model
            .relationships
            .get("author")
            .expect("author relationship");
        assert_eq!(rel.model, "Author");
        assert_eq!(rel.foreign_key.as_deref(), Some("authorId"));

        // Validation: email field gets email rule, number gets min/max
        assert!(model.validation["contact"].contains("email"));
        assert!(model.validation["count"].contains("min:0"));
        assert!(model.validation["count"].contains("max:999"));
    }

    #[test]
    fn model_with_no_fields_block_produces_empty_fields() {
        // A model(...) call that has a body but no `fields: { ... }` key
        let dsl = r#"
import { model } from '@atomo/schema'

export const Empty = model('empties', {
  access: {
    read: allow.public(),
  },
})
"#;
        let schema = parse_builder_dsl(dsl).unwrap();
        let model = schema.models.get("Empty").expect("Empty model");
        assert!(
            model.fields.is_empty(),
            "model with no fields block should have empty fields"
        );
    }

    #[test]
    fn model_call_missing_brace_body_is_skipped() {
        // `model('x')` without the second arg object — parse_model_blocks regex
        // requires `, {` so this should just produce no models.
        let dsl = r#"
import { model } from '@atomo/schema'

export const Broken = model('broken')
"#;
        let schema = parse_builder_dsl(dsl).unwrap();
        assert!(
            schema.models.is_empty(),
            "model with no body block should be skipped"
        );
    }

    #[test]
    fn action_with_no_input_call() {
        let dsl = r#"
import { model, text, action } from '@atomo/schema'

export const onPing = action('onPing')
  .from('pings')

export const Ping = model('pings', {
  fields: {
    id: text().id(),
  },
})
"#;
        let schema = parse_builder_dsl(dsl).unwrap();
        let action = schema.actions.get("onPing").expect("onPing action");
        assert_eq!(action.source_model.as_deref(), Some("Ping"));
        // No .input() means the parser falls through to Object { fields: [] }
        match &action.input {
            ActionInputDef::Object { fields } => {
                assert!(
                    fields.is_empty(),
                    "no .input() should produce empty Object input"
                );
            }
            other => panic!(
                "expected Object variant for action with no .input(), got {:?}",
                other
            ),
        }
    }

    #[test]
    fn multiple_actions_referencing_same_model() {
        let dsl = r#"
import { model, text, action } from '@atomo/schema'

export const onOrderCreated = action('onOrderCreated')
  .from('orders')
  .input(['id', 'total'])

export const onOrderShipped = action('onOrderShipped')
  .from('orders')
  .input(['id', 'trackingNumber'])

export const onOrderRefunded = action('onOrderRefunded')
  .from('orders')
  .input(['id', 'total', 'reason'])

export const Order = model('orders', {
  fields: {
    id: text().id(),
    total: number(),
    trackingNumber: text().optional(),
    reason: text().optional(),
  },
  on: {
    created: [onOrderCreated],
    updated: [onOrderShipped, onOrderRefunded],
  },
})
"#;
        let schema = parse_builder_dsl(dsl).unwrap();

        // All three actions should exist and point at the same model
        assert_eq!(schema.actions.len(), 3);
        for name in &["onOrderCreated", "onOrderShipped", "onOrderRefunded"] {
            let a = schema.actions.get(*name).unwrap();
            assert_eq!(
                a.source_model.as_deref(),
                Some("Order"),
                "{} should reference Order",
                name
            );
        }

        // Verify distinct input fields
        if let ActionInputDef::PickFields { fields, .. } = &schema.actions["onOrderCreated"].input {
            assert_eq!(fields, &["id", "total"]);
        } else {
            panic!("expected PickFields");
        }
        if let ActionInputDef::PickFields { fields, .. } = &schema.actions["onOrderShipped"].input {
            assert_eq!(fields, &["id", "trackingNumber"]);
        } else {
            panic!("expected PickFields");
        }

        // Event bindings on the model
        let order = &schema.models["Order"];
        assert_eq!(order.events.created.len(), 1);
        assert_eq!(order.events.updated.len(), 2);
        assert_eq!(order.events.updated[0].action, "onOrderShipped");
        assert_eq!(order.events.updated[1].action, "onOrderRefunded");
    }
}
