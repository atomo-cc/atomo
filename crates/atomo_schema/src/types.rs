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
