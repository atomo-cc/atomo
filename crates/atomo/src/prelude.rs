//! Atomo prelude - commonly used imports
//!
//! Import everything you need to get started with Atomo:
//!
//! ```rust
//! use atomo::prelude::*;
//! ```

pub use crate::events::{EventType, ModelEvent, SubscriptionBuilder};
pub use crate::query::{
    CreateQuery, DeleteQuery, FindManyQuery, FindUniqueQuery, OrderBy, OrderDirection, UpdateQuery,
    WhereClause,
};
pub use crate::actions::{
    ActionCondition, ActionDef, ActionInputDef, ActionReturn, EventActionBinding, ModelEvents,
};
pub use crate::schema::{Field, FieldType, Model, Schema};
pub use crate::{Atomo, AtomoBuilder, ModelClient};

// Common query operators
pub use crate::query::operators::{
    Contains, EndsWith, Equals, GreaterThan, In, IsNotNull, IsNull, LessThan, NotEquals, NotIn,
    StartsWith,
};

// Re-export commonly used external types
pub use anyhow::{Context, Result};
pub use chrono::{DateTime, Utc};
pub use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

// Order direction
pub use crate::query::OrderDirection::{Asc, Desc};
