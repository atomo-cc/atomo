use serde::{Deserialize, Serialize};
use uuid::Uuid;
use ulid::Ulid;
use chrono::{DateTime, Utc};
use async_graphql::{Scalar, ScalarType, InputValueError, InputValueResult, Value};

/// ULID-based identifier for entities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub Ulid);

#[Scalar]
impl ScalarType for EntityId {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(s) => {
                Ulid::from_string(&s)
                    .map(EntityId)
                    .map_err(|e| InputValueError::custom(format!("Invalid ULID: {}", e)))
            }
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.to_string())
    }
}

impl EntityId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
    
    pub fn from_string(s: &str) -> Result<Self, ulid::DecodeError> {
        Ok(Self(Ulid::from_string(s)?))
    }
    
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

/// Event ID for event sourcing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub Ulid);

impl EventId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

/// Stream ID for event streams
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StreamId(pub Uuid);

#[Scalar]
impl ScalarType for StreamId {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(s) => {
                Uuid::parse_str(&s)
                    .map(StreamId)
                    .map_err(|e| InputValueError::custom(format!("Invalid UUID: {}", e)))
            }
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.to_string())
    }
}

impl StreamId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for StreamId {
    fn default() -> Self {
        Self::new()
    }
}

/// Timestamp type for consistent time handling
pub type Timestamp = DateTime<Utc>;

/// Version for optimistic concurrency control
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version(pub i64);

impl Version {
    pub fn initial() -> Self {
        Self(0)
    }
    
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}
