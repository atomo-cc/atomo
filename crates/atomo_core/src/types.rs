//! Core types for the Atomo platform
//!
//! This module contains the fundamental types used throughout the Atomo ecosystem.
//! According to the whitepaper architecture, this should contain only the most
//! essential types needed by all layers.

use async_graphql::{Enum, InputValueError, InputValueResult, Scalar, ScalarType, Value};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display},
    str::FromStr,
};
use ulid::Ulid;
use uuid::Uuid;

/// ULID-based identifier for entities
///
/// ULIDs provide lexicographic sorting and are globally unique,
/// making them ideal for distributed systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub Ulid);

#[Scalar]
impl ScalarType for EntityId {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(s) => EntityId::from_string(&s)
                .map_err(|e| InputValueError::custom(format!("Invalid EntityId: {}", e))),
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}

impl EntityId {
    /// Create a new EntityId
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    /// Create from string representation
    pub fn from_string(s: &str) -> Result<Self, ulid::DecodeError> {
        Ok(Self(Ulid::from_string(s)?))
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for EntityId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_string(s)
    }
}

/// Stream identifier for event sourcing
///
/// Streams group related events together (e.g., all events for a specific entity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StreamId(pub Uuid);

#[Scalar]
impl ScalarType for StreamId {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(s) => StreamId::from_string(&s)
                .map_err(|e| InputValueError::custom(format!("Invalid StreamId: {}", e))),
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}

impl StreamId {
    /// Create a new StreamId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from string representation
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Default for StreamId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for StreamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for StreamId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_string(s)
    }
}

/// Timestamp type alias for consistency
pub type Timestamp = chrono::DateTime<chrono::Utc>;

/// User role enumeration
///
/// Defines the basic roles that users can have in the system.
/// This is a core business concept that should be defined at the core level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Enum)]
pub enum UserRole {
    /// Regular user with basic permissions
    User,
    /// Administrator with elevated permissions
    Admin,
    /// Manager with intermediate permissions
    Manager,
}

impl Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Admin => write!(f, "admin"),
            UserRole::Manager => write!(f, "manager"),
        }
    }
}

impl FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(UserRole::User),
            "admin" => Ok(UserRole::Admin),
            "manager" => Ok(UserRole::Manager),
            _ => Err(format!("Unknown user role: {}", s)),
        }
    }
}
