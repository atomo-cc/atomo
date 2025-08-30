//! Atomo prelude - commonly used imports
//!
//! Import everything you need to get started with Atomo:
//!
//! ```rust
//! use atomo::prelude::*;
//! ```

pub use crate::{Atomo, AtomoBuilder, ModelClient};
pub use crate::query::{
    FindManyQuery, FindUniqueQuery, CreateQuery, UpdateQuery, DeleteQuery,
    WhereClause, OrderBy, OrderDirection,
};
pub use crate::events::{SubscriptionBuilder, EventType, ModelEvent};
pub use crate::schema::{Schema, Model, Field, FieldType};

// Common query operators
pub use crate::query::operators::{
    Equals, NotEquals, Contains, StartsWith, EndsWith,
    GreaterThan, LessThan, In, NotIn, IsNull, IsNotNull,
};

// Re-export commonly used external types
pub use anyhow::{Result, Context};
pub use serde::{Serialize, Deserialize};
pub use uuid::Uuid;
pub use chrono::{DateTime, Utc};

// Order direction
pub use crate::query::OrderDirection::{Asc, Desc};
