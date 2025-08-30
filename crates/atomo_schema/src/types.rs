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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub field_type: FieldType,
    pub optional: bool,
    pub attributes: Vec<FieldAttribute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
