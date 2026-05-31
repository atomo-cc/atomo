use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub models: HashMap<String, Model>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub name: String,
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
    /// pipe-OR role lists (`"sales|manager|admin"`). Non-Boolean rules currently allow.
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
            _ => AccessDecision::Allow,
        }
    }
}

/// Individual access rule that can be a boolean check or query condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessRule {
    /// Simple boolean check function
    Boolean(String), // Function code as string
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
