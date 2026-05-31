//! Query builder module

use serde_json::{Map, Value};

pub mod operators;
pub mod sql_builder;

/// Where clause for filtering queries
#[derive(Debug, Clone)]
pub struct WhereClause {
    pub field: String,
    pub operator: WhereOperator,
    pub value: Value,
}

/// Supported where operators
#[derive(Debug, Clone)]
pub enum WhereOperator {
    Equals,
    NotEquals,
    Contains,
    StartsWith,
    EndsWith,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    In,
    NotIn,
    IsNull,
    IsNotNull,
}

/// Order direction for sorting
#[derive(Debug, Clone)]
pub enum OrderDirection {
    Asc,
    Desc,
}

/// Order by clause
#[derive(Debug, Clone)]
pub struct OrderBy {
    pub field: String,
    pub direction: OrderDirection,
}

/// Query builder for finding many records
#[derive(Debug, Clone)]
pub struct FindManyQuery {
    pub model: String,
    pub where_clauses: Vec<WhereClause>,
    pub order_by: Vec<OrderBy>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub include: Vec<String>,
}

impl FindManyQuery {
    pub fn new(model: String) -> Self {
        Self {
            model,
            where_clauses: Vec::new(),
            order_by: Vec::new(),
            limit: None,
            offset: None,
            include: Vec::new(),
        }
    }

    pub fn where_(mut self, field: String, operator: WhereOperator, value: Value) -> Self {
        self.where_clauses.push(WhereClause {
            field,
            operator,
            value,
        });
        self
    }

    pub fn order_by(mut self, field: String, direction: OrderDirection) -> Self {
        self.order_by.push(OrderBy { field, direction });
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: i32) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn include(mut self, relation: String) -> Self {
        self.include.push(relation);
        self
    }
}

/// Query builder for finding a unique record
#[derive(Debug, Clone)]
pub struct FindUniqueQuery {
    pub model: String,
    pub where_clauses: Vec<WhereClause>,
    pub include: Vec<String>,
}

impl FindUniqueQuery {
    pub fn new(model: String) -> Self {
        Self {
            model,
            where_clauses: Vec::new(),
            include: Vec::new(),
        }
    }

    pub fn where_(mut self, field: String, operator: WhereOperator, value: Value) -> Self {
        self.where_clauses.push(WhereClause {
            field,
            operator,
            value,
        });
        self
    }

    pub fn include(mut self, relation: String) -> Self {
        self.include.push(relation);
        self
    }
}

/// Query builder for creating records
#[derive(Debug, Clone)]
pub struct CreateQuery {
    pub model: String,
    pub data: Map<String, Value>,
    pub include: Vec<String>,
}

impl CreateQuery {
    pub fn new(model: String) -> Self {
        Self {
            model,
            data: Map::new(),
            include: Vec::new(),
        }
    }

    pub fn data(mut self, field: String, value: Value) -> Self {
        self.data.insert(field, value);
        self
    }

    pub fn include(mut self, relation: String) -> Self {
        self.include.push(relation);
        self
    }
}

/// Query builder for updating records
#[derive(Debug, Clone)]
pub struct UpdateQuery {
    pub model: String,
    pub where_clauses: Vec<WhereClause>,
    pub data: Map<String, Value>,
    pub include: Vec<String>,
}

impl UpdateQuery {
    pub fn new(model: String) -> Self {
        Self {
            model,
            where_clauses: Vec::new(),
            data: Map::new(),
            include: Vec::new(),
        }
    }

    pub fn where_(mut self, field: String, operator: WhereOperator, value: Value) -> Self {
        self.where_clauses.push(WhereClause {
            field,
            operator,
            value,
        });
        self
    }

    pub fn data(mut self, field: String, value: Value) -> Self {
        self.data.insert(field, value);
        self
    }

    pub fn include(mut self, relation: String) -> Self {
        self.include.push(relation);
        self
    }
}

/// Query builder for deleting records
#[derive(Debug, Clone)]
pub struct DeleteQuery {
    pub model: String,
    pub where_clauses: Vec<WhereClause>,
}

impl DeleteQuery {
    pub fn new(model: String) -> Self {
        Self {
            model,
            where_clauses: Vec::new(),
        }
    }

    pub fn where_(mut self, field: String, operator: WhereOperator, value: Value) -> Self {
        self.where_clauses.push(WhereClause {
            field,
            operator,
            value,
        });
        self
    }
}
