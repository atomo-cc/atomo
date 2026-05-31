use super::{OrderDirection, WhereClause, WhereOperator};
use crate::schema::Model;
use serde_json::Value;
use std::collections::HashMap;

pub struct SqlBuilder;

impl SqlBuilder {
    /// Build SELECT query. Returns (sql, params)
    pub fn select(
        model: &Model,
        where_clauses: &[WhereClause],
        order_by: &[(String, OrderDirection)],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> (String, Vec<Value>) {
        let mut sql = format!("SELECT * FROM {}", table_name(model));
        let (where_sql, params) = build_where(where_clauses, 0);
        if !where_sql.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_sql));
        }
        if !order_by.is_empty() {
            let clauses: Vec<String> = order_by
                .iter()
                .map(|(f, d)| {
                    let dir = match d {
                        OrderDirection::Asc => "ASC",
                        OrderDirection::Desc => "DESC",
                    };
                    format!("{} {}", to_snake_case(f), dir)
                })
                .collect();
            sql.push_str(&format!(" ORDER BY {}", clauses.join(", ")));
        }
        if let Some(l) = limit {
            sql.push_str(&format!(" LIMIT {}", l));
        }
        if let Some(o) = offset {
            sql.push_str(&format!(" OFFSET {}", o));
        }
        (sql, params)
    }

    /// Build SELECT ... LIMIT 1 for find_unique
    pub fn select_one(model: &Model, where_clauses: &[WhereClause]) -> (String, Vec<Value>) {
        let (mut sql, params) = Self::select(model, where_clauses, &[], Some(1), None);
        // select already adds LIMIT 1
        let _ = &mut sql;
        (sql, params)
    }

    /// Build INSERT query. Returns (sql, params)
    pub fn insert(model: &Model, data: &HashMap<String, Value>) -> (String, Vec<Value>) {
        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut params = Vec::new();

        for (i, (key, val)) in data.iter().enumerate() {
            columns.push(to_snake_case(key));
            placeholders.push(format!("${}", i + 1));
            params.push(val.clone());
        }

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({}) RETURNING *",
            table_name(model),
            columns.join(", "),
            placeholders.join(", ")
        );
        (sql, params)
    }

    /// Build UPDATE query. Returns (sql, params)
    pub fn update(
        model: &Model,
        where_clauses: &[WhereClause],
        data: &HashMap<String, Value>,
    ) -> (String, Vec<Value>) {
        let mut set_clauses = Vec::new();
        let mut params = Vec::new();

        for (i, (key, val)) in data.iter().enumerate() {
            set_clauses.push(format!("{} = ${}", to_snake_case(key), i + 1));
            params.push(val.clone());
        }

        let mut sql = format!(
            "UPDATE {} SET {}",
            table_name(model),
            set_clauses.join(", ")
        );

        let (where_sql, where_params) = build_where(where_clauses, params.len());
        if !where_sql.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_sql));
        }
        params.extend(where_params);
        sql.push_str(" RETURNING *");
        (sql, params)
    }

    /// Build DELETE query. Returns (sql, params)
    pub fn delete(model: &Model, where_clauses: &[WhereClause]) -> (String, Vec<Value>) {
        let mut sql = format!("DELETE FROM {}", table_name(model));
        let (where_sql, params) = build_where(where_clauses, 0);
        if !where_sql.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_sql));
        }
        (sql, params)
    }

    /// Build a soft-delete UPDATE (sets deleted_at = NOW())
    pub fn soft_delete(model: &Model, where_clauses: &[WhereClause]) -> (String, Vec<Value>) {
        let mut sql = format!("UPDATE {} SET deleted_at = NOW()", table_name(model));
        let (where_sql, params) = build_where(where_clauses, 0);
        if !where_sql.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_sql));
        }
        // Return the affected ids so the Deleted event can carry them (projections/audit need
        // the id to remove the right row — previously the event had empty data).
        sql.push_str(" RETURNING id");
        (sql, params)
    }

    /// Build a restore UPDATE (clears deleted_at) for soft-deleted records.
    pub fn restore(model: &Model, where_clauses: &[WhereClause]) -> (String, Vec<Value>) {
        let mut sql = format!("UPDATE {} SET deleted_at = NULL", table_name(model));
        let (where_sql, params) = build_where(where_clauses, 0);
        if !where_sql.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_sql));
        }
        (sql, params)
    }

    /// Build SELECT with soft-delete filter (excludes deleted records)
    pub fn select_active(
        model: &Model,
        where_clauses: &[WhereClause],
        order_by: &[(String, OrderDirection)],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> (String, Vec<Value>) {
        let mut clauses = where_clauses.to_vec();
        clauses.push(WhereClause {
            field: "deleted_at".to_string(),
            operator: WhereOperator::IsNull,
            value: Value::Null,
        });
        Self::select(model, &clauses, order_by, limit, offset)
    }

    /// Build SELECT for only soft-deleted records (deleted_at IS NOT NULL).
    pub fn select_deleted(
        model: &Model,
        where_clauses: &[WhereClause],
        order_by: &[(String, OrderDirection)],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> (String, Vec<Value>) {
        let mut clauses = where_clauses.to_vec();
        clauses.push(WhereClause {
            field: "deleted_at".to_string(),
            operator: WhereOperator::IsNotNull,
            value: Value::Null,
        });
        Self::select(model, &clauses, order_by, limit, offset)
    }
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        if c.is_uppercase() && !result.is_empty() {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result
}

pub fn table_name_for(model: &Model) -> String {
    table_name(model)
}
pub fn build_where_pub(where_clauses: &[WhereClause], param_offset: usize) -> (String, Vec<Value>) {
    build_where(where_clauses, param_offset)
}

fn table_name(model: &Model) -> String {
    // Honor an explicit `tableName` from the schema; otherwise pluralize the model name.
    model
        .table_name
        .clone()
        .unwrap_or_else(|| to_snake_case(&model.name) + "s")
}

fn build_where(where_clauses: &[WhereClause], param_offset: usize) -> (String, Vec<Value>) {
    let mut parts = Vec::new();
    let mut params = Vec::new();
    let mut idx = param_offset + 1;

    for clause in where_clauses {
        let col = to_snake_case(&clause.field);
        match &clause.operator {
            WhereOperator::Equals => {
                // For string values, cast the column to text so the comparison works
                // against both TEXT and UUID columns (e.g. id). Numbers/bools compare directly.
                let cast = if clause.value.is_string() { "::text" } else { "" };
                parts.push(format!("{}{} = ${}", col, cast, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::NotEquals => {
                let cast = if clause.value.is_string() { "::text" } else { "" };
                parts.push(format!("{}{} != ${}", col, cast, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::Contains => {
                parts.push(format!("{} ILIKE '%' || ${} || '%'", col, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::StartsWith => {
                parts.push(format!("{} ILIKE ${} || '%'", col, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::EndsWith => {
                parts.push(format!("{} ILIKE '%' || ${}", col, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::GreaterThan => {
                parts.push(format!("{} > ${}", col, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::GreaterThanOrEqual => {
                parts.push(format!("{} >= ${}", col, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::LessThan => {
                parts.push(format!("{} < ${}", col, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::LessThanOrEqual => {
                parts.push(format!("{} <= ${}", col, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::In => {
                parts.push(format!("{} = ANY(${})", col, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::NotIn => {
                parts.push(format!("{} != ALL(${})", col, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::IsNull => {
                parts.push(format!("{} IS NULL", col));
            }
            WhereOperator::IsNotNull => {
                parts.push(format!("{} IS NOT NULL", col));
            }
        }
    }

    (parts.join(" AND "), params)
}
