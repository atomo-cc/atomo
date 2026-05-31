//! Query operators for building where clauses

use super::{WhereClause, WhereOperator};
use serde_json::Value;

/// Equals operator
pub struct Equals(pub Value);

impl From<Equals> for WhereClause {
    fn from(op: Equals) -> Self {
        WhereClause {
            field: String::new(), // Will be set by the query builder
            operator: WhereOperator::Equals,
            value: op.0,
        }
    }
}

/// Not equals operator
pub struct NotEquals(pub Value);

impl From<NotEquals> for WhereClause {
    fn from(op: NotEquals) -> Self {
        WhereClause {
            field: String::new(),
            operator: WhereOperator::NotEquals,
            value: op.0,
        }
    }
}

/// Contains operator for string fields
pub struct Contains(pub String);

impl From<Contains> for WhereClause {
    fn from(op: Contains) -> Self {
        WhereClause {
            field: String::new(),
            operator: WhereOperator::Contains,
            value: Value::String(op.0),
        }
    }
}

/// Starts with operator for string fields
pub struct StartsWith(pub String);

impl From<StartsWith> for WhereClause {
    fn from(op: StartsWith) -> Self {
        WhereClause {
            field: String::new(),
            operator: WhereOperator::StartsWith,
            value: Value::String(op.0),
        }
    }
}

/// Ends with operator for string fields
pub struct EndsWith(pub String);

impl From<EndsWith> for WhereClause {
    fn from(op: EndsWith) -> Self {
        WhereClause {
            field: String::new(),
            operator: WhereOperator::EndsWith,
            value: Value::String(op.0),
        }
    }
}

/// Greater than operator
pub struct GreaterThan(pub Value);

impl From<GreaterThan> for WhereClause {
    fn from(op: GreaterThan) -> Self {
        WhereClause {
            field: String::new(),
            operator: WhereOperator::GreaterThan,
            value: op.0,
        }
    }
}

/// Less than operator
pub struct LessThan(pub Value);

impl From<LessThan> for WhereClause {
    fn from(op: LessThan) -> Self {
        WhereClause {
            field: String::new(),
            operator: WhereOperator::LessThan,
            value: op.0,
        }
    }
}

/// In operator for array matching
pub struct In(pub Vec<Value>);

impl From<In> for WhereClause {
    fn from(op: In) -> Self {
        WhereClause {
            field: String::new(),
            operator: WhereOperator::In,
            value: Value::Array(op.0),
        }
    }
}

/// Not in operator for array matching
pub struct NotIn(pub Vec<Value>);

impl From<NotIn> for WhereClause {
    fn from(op: NotIn) -> Self {
        WhereClause {
            field: String::new(),
            operator: WhereOperator::NotIn,
            value: Value::Array(op.0),
        }
    }
}

/// Is null operator
pub struct IsNull;

impl From<IsNull> for WhereClause {
    fn from(_: IsNull) -> Self {
        WhereClause {
            field: String::new(),
            operator: WhereOperator::IsNull,
            value: Value::Null,
        }
    }
}

/// Is not null operator
pub struct IsNotNull;

impl From<IsNotNull> for WhereClause {
    fn from(_: IsNotNull) -> Self {
        WhereClause {
            field: String::new(),
            operator: WhereOperator::IsNotNull,
            value: Value::Null,
        }
    }
}

// Convenience implementations for common types
impl From<String> for Equals {
    fn from(s: String) -> Self {
        Equals(Value::String(s))
    }
}

impl From<&str> for Equals {
    fn from(s: &str) -> Self {
        Equals(Value::String(s.to_string()))
    }
}

impl From<i64> for Equals {
    fn from(n: i64) -> Self {
        Equals(Value::Number(n.into()))
    }
}

impl From<bool> for Equals {
    fn from(b: bool) -> Self {
        Equals(Value::Bool(b))
    }
}
