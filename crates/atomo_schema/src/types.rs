use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub models: HashMap<String, Model>,
    #[serde(default)]
    pub actions: HashMap<String, ActionDef>,
    /// Append-only extensions to built-in platform tables (e.g. `users`), keyed by
    /// table name: extra nullable columns and model-level constraints, so consumer
    /// integrity on platform tables lives in the schema instead of hand-written SQL.
    #[serde(default)]
    pub builtins: HashMap<String, BuiltinExtension>,
    /// Non-fatal diagnostics produced while parsing: dropped or unrecognized
    /// constructs that used to disappear silently. Surfaced as `warn!` at server
    /// boot and printed by the CLI — never treated as errors.
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl Schema {
    /// Validate runtime security semantics before a schema can serve requests.
    pub fn validate_security(&self) -> anyhow::Result<()> {
        fn rule(expression: &AccessRule, model: &Model) -> anyhow::Result<()> {
            match expression {
                AccessRule::Owner(f) | AccessRule::SameTenant(f) => {
                    anyhow::ensure!(
                        f == "tenantId"
                            || model.fields.get(f).is_some_and(|f| matches!(
                                f.field_type,
                                FieldType::String | FieldType::EntityId
                            )),
                        "Invalid security field {f}"
                    );
                }
                AccessRule::And(rs) | AccessRule::Or(rs) => {
                    for r in rs {
                        rule(r, model)?;
                    }
                }
                AccessRule::Query(_) => anyhow::bail!(
                    "Legacy query access expressions are not supported by the runtime"
                ),
                AccessRule::Boolean(value) => anyhow::ensure!(
                    value.split('|').all(|r| !r.is_empty()
                        && r.chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')),
                    "Unsupported runtime access expression"
                ),
            }
            Ok(())
        }
        for model in self.models.values() {
            for field in model.fields.values() {
                for attr in &field.attributes {
                    if let FieldAttribute::FromAuth(source) = attr {
                        anyhow::ensure!(
                            ["userId", "tenantId"].contains(&source.as_str())
                                && matches!(
                                    field.field_type,
                                    FieldType::String | FieldType::EntityId
                                ),
                            "Unsupported authentication source or field type"
                        );
                    }
                }
            }
            if let Some(access) = &model.access {
                for r in [&access.create, &access.read, &access.update, &access.delete]
                    .into_iter()
                    .flatten()
                {
                    rule(r, model)?;
                }
            }
            for (name, rel) in &model.relationships {
                anyhow::ensure!(
                    self.models.contains_key(&rel.model),
                    "Unknown relationship model {}",
                    rel.model
                );
                anyhow::ensure!(
                    ["belongsTo", "hasMany"].contains(&rel.kind.as_str()),
                    "Unsupported relationship kind"
                );
                if rel.kind == "belongsTo" {
                    let fk = rel
                        .foreign_key
                        .clone()
                        .unwrap_or_else(|| format!("{name}Id"));
                    anyhow::ensure!(
                        model.fields.contains_key(&fk),
                        "Unknown relationship field {fk}"
                    );
                } else {
                    let mut chars = model.name.chars();
                    let prefix = chars
                        .next()
                        .map(|c| c.to_lowercase().collect::<String>())
                        .unwrap_or_default();
                    let fk = rel
                        .foreign_key
                        .clone()
                        .unwrap_or_else(|| format!("{prefix}{}Id", chars.as_str()));
                    anyhow::ensure!(
                        self.models[&rel.model].fields.contains_key(&fk),
                        "Unknown relationship field {fk}"
                    );
                }
            }
        }
        for action in self.actions.values() {
            if let Some(ActionReturn::Model(name)) = &action.returns {
                anyhow::ensure!(
                    self.models.contains_key(name),
                    "Unknown action return model {name}"
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuiltinExtension {
    /// Extra columns: field name (camelCase, converted to snake_case) → SQL type.
    /// Types must be nullable — NOT NULL would break existing rows.
    #[serde(default)]
    pub columns: HashMap<String, String>,
    /// Model-level constraints, declared with the same `@@unique([..]) WHERE ..` /
    /// `@@index([..])` / `@@check(..)` annotation strings used on models.
    #[serde(default)]
    pub constraints: Vec<ModelConstraint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDef {
    pub name: String,
    pub input: ActionInputDef,
    #[serde(default)]
    pub returns: Option<ActionReturn>,
    #[serde(default)]
    pub source_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionInputDef {
    PickFields { model: String, fields: Vec<String> },
    Object { fields: Vec<ActionInputField> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionInputField {
    pub name: String,
    pub field_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub validation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionReturn {
    Model(String),
    Void,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TenantPolicy {
    #[default]
    Required,
    Global,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub name: String,
    #[serde(default)]
    pub tenant_policy: TenantPolicy,
    pub fields: HashMap<String, Field>,
    pub access: Option<AccessControl>,
    pub hooks: Option<HookDefinitions>,
    #[serde(default)]
    pub validation: HashMap<String, String>,
    /// Explicit `tableName` from the schema metadata; falls back to pluralized name when None.
    #[serde(default)]
    pub table_name: Option<String>,
    /// Declared relationships from the schema metadata, keyed by relationship name (e.g.
    /// `company`, `deals`). Lets relationship resolution be schema-driven instead of guessing
    /// the target model from the name.
    #[serde(default)]
    pub relationships: HashMap<String, Relationship>,
    /// Model-level constraints from `@@unique([..])` / `@@index([..])` / `@@check(..)`
    /// annotations — composite uniqueness/indexes and CHECK rules the per-field
    /// annotations can't express.
    #[serde(default)]
    pub constraints: Vec<ModelConstraint>,
    /// Lifecycle event bindings: which actions to enqueue when a record is created/updated/deleted.
    #[serde(default)]
    pub events: ModelEvents,
    /// Optional UI hints from the schema (e.g. which fields to show in the admin list view).
    #[serde(default)]
    pub ui: Option<UiConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default, rename = "listView")]
    pub list_view: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelEvents {
    #[serde(default)]
    pub created: Vec<EventActionBinding>,
    #[serde(default)]
    pub updated: Vec<EventActionBinding>,
    #[serde(default)]
    pub deleted: Vec<EventActionBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventActionBinding {
    pub action: String,
    #[serde(default)]
    pub condition: Option<ActionCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionCondition {
    ChangedAny(Vec<String>),
    FieldEquals {
        field: String,
        value: serde_json::Value,
    },
}

impl ActionCondition {
    pub fn matches(
        &self,
        current: &HashMap<String, serde_json::Value>,
        previous: Option<&HashMap<String, serde_json::Value>>,
    ) -> bool {
        match self {
            ActionCondition::ChangedAny(fields) => {
                let prev = match previous {
                    Some(p) => p,
                    None => return true,
                };
                fields.iter().any(|f| current.get(f) != prev.get(f))
            }
            ActionCondition::FieldEquals { field, value } => current.get(field) == Some(value),
        }
    }
}

impl ActionDef {
    pub fn is_callable(&self) -> bool {
        self.returns.is_some()
    }

    pub fn is_lifecycle_only(&self) -> bool {
        self.returns.is_none()
    }

    pub fn build_input(
        &self,
        doc: &HashMap<String, serde_json::Value>,
    ) -> HashMap<String, serde_json::Value> {
        match &self.input {
            ActionInputDef::PickFields { fields, .. } => fields
                .iter()
                .filter_map(|f| doc.get(f).map(|v| (f.clone(), v.clone())))
                .collect(),
            ActionInputDef::Object { .. } => doc.clone(),
        }
    }
}

impl ModelEvents {
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.updated.is_empty() && self.deleted.is_empty()
    }
}

/// A model-level constraint declared via an `@@` annotation in `schema.ts`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelConstraint {
    /// `@@unique([a, b])` — composite uniqueness (field names; snake-cased at DDL time).
    Unique(Vec<String>),
    /// `@@index([a, b])` — composite secondary index.
    Index(Vec<String>),
    /// `@@check(expr)` — a raw SQL CHECK expression, e.g. `amount <> 0`.
    Check(String),
    /// `@@unique([a, b]) WHERE <predicate>` — a PARTIAL unique index. The predicate
    /// is raw SQL over column names (snake_case, like `@@check`), e.g.
    /// `@@unique([storeAccountId]) WHERE store_account_id IS NOT NULL` for a nullable
    /// anti-abuse anchor where NULLs must not collide.
    UniqueWhere(Vec<String>, String),
    /// `@@index([a, b]) WHERE <predicate>` — a partial secondary index.
    IndexWhere(Vec<String>, String),
}

/// A declared relationship: `{ type: 'belongsTo'|'hasMany', model: 'Company', foreignKey: 'companyId' }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub kind: String,  // "belongsTo" | "hasMany"
    pub model: String, // target model name
    pub foreign_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub field_type: FieldType,
    pub optional: bool,
    pub attributes: Vec<FieldAttribute>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FieldType {
    String,
    Number,
    Boolean,
    Date,
    DateTime,
    EntityId,
    Json,
    Reference(String),
    Array(Box<FieldType>),
    Blocks,
    /// An uploaded file/media reference (stored as TEXT — the media id/url). Behaves like
    /// String for storage/codegen but surfaces as `file` in metadata so the UI renders an uploader.
    File,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldAttribute {
    Primary,
    Unique,
    Index,
    ForeignKey,
    Timestamp,
    Default(String),
    FromAuth(String),
}

// =============================================================================
// Access Control System
// =============================================================================

/// Access control configuration for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControl {
    pub create: Option<AccessRule>,
    pub read: Option<AccessRule>,
    pub update: Option<AccessRule>,
    pub delete: Option<AccessRule>,
}

/// Outcome of an access decision — lets callers distinguish "no rule / allowed" from the two
/// denial reasons so they can return the right error (401 vs 403).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessDecision {
    Allow,
    NeedsAuth,
    Forbidden,
}

impl AccessControl {
    pub fn rule_for(&self, action: &str) -> Option<&AccessRule> {
        match action {
            "create" => self.create.as_ref(),
            "read" => self.read.as_ref(),
            "update" => self.update.as_ref(),
            "delete" => self.delete.as_ref(),
            _ => None,
        }
    }

    /// The single RBAC decision seam, shared by the GraphQL resolver and the data layer.
    /// `role` is the caller's role (None = unauthenticated). No rule → Allow.
    pub fn decide(&self, action: &str, role: Option<&str>) -> AccessDecision {
        match self.rule_for(action) {
            Some(r) => r.decide(role),
            None => AccessDecision::Allow,
        }
    }
}

impl AccessRule {
    /// Decide a single rule against a role. Handles the `public`/`authenticated` tokens and
    /// pipe-OR role lists. Row-dependent rules require further shared-layer filtering.
    pub fn decide(&self, role: Option<&str>) -> AccessDecision {
        match self {
            AccessRule::Boolean(roles_str) => {
                if roles_str == "public" {
                    return AccessDecision::Allow;
                }
                if roles_str == "authenticated" {
                    return match role {
                        Some(_) => AccessDecision::Allow,
                        None => AccessDecision::NeedsAuth,
                    };
                }
                match role {
                    Some(r) if roles_str.split('|').any(|a| a.eq_ignore_ascii_case(r)) => {
                        AccessDecision::Allow
                    }
                    Some(_) => AccessDecision::Forbidden,
                    None => AccessDecision::NeedsAuth,
                }
            }
            AccessRule::Owner(_) | AccessRule::SameTenant(_) => {
                if role.is_some() {
                    AccessDecision::Allow
                } else {
                    AccessDecision::NeedsAuth
                }
            }
            AccessRule::And(rules) => {
                let decisions: Vec<_> = rules.iter().map(|r| r.decide(role)).collect();
                if decisions.contains(&AccessDecision::NeedsAuth) {
                    AccessDecision::NeedsAuth
                } else if decisions.contains(&AccessDecision::Forbidden) {
                    AccessDecision::Forbidden
                } else {
                    AccessDecision::Allow
                }
            }
            AccessRule::Or(rules) => {
                let decisions: Vec<_> = rules.iter().map(|r| r.decide(role)).collect();
                if decisions.contains(&AccessDecision::Allow) {
                    AccessDecision::Allow
                } else if decisions.contains(&AccessDecision::NeedsAuth) {
                    AccessDecision::NeedsAuth
                } else {
                    AccessDecision::Forbidden
                }
            }
            AccessRule::Query(_) => AccessDecision::Forbidden,
        }
    }
}

/// Individual access rule that can be a boolean check or query condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessRule {
    /// Simple boolean check function
    Boolean(String), // Role token or pipe-separated role list
    Owner(String),
    SameTenant(String),
    /// Complex query condition
    Query(QueryCondition),
    /// Combination of conditions
    And(Vec<AccessRule>),
    Or(Vec<AccessRule>),
}

/// Query condition for access control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCondition {
    pub field: String,
    pub operator: QueryOperator,
    pub value: QueryValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryOperator {
    Equals,
    NotEquals,
    In,
    NotIn,
    GreaterThan,
    LessThan,
    Like,
    IsNull,
    IsNotNull,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<String>),
    UserProperty(String), // Reference to user property like user.id
}

// =============================================================================
// Hook System
// =============================================================================

/// Hook definitions for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDefinitions {
    pub before_operation: Option<Vec<Hook>>,
    pub after_operation: Option<Vec<Hook>>,
    pub before_change: Option<Vec<FieldHook>>,
    pub after_read: Option<Vec<Hook>>,
}

/// Individual hook definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub name: String,
    pub operation_type: Option<OperationType>, // None means all operations
    pub function_code: String,                 // TypeScript function code
    pub async_hook: bool,
}

/// Field-specific hook for monitoring field changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldHook {
    pub field_name: String,
    pub function_code: String,
    pub async_hook: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    Create,
    Update,
    Delete,
}
