use super::{OrderDirection, WhereClause, WhereOperator};
use crate::schema::Model;
use serde_json::Value;
use std::collections::HashMap;

pub struct SqlBuilder;

/// Quote a Postgres identifier — always, not just when it looks risky.
/// Unconditional quoting makes reserved words (`order`, `user`, `select`)
/// and mixed-case names safe at every emission site, and escapes stray `"`s.
pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

impl SqlBuilder {
    /// Build SELECT query. Returns (sql, params)
    pub fn select(
        model: &Model,
        where_clauses: &[WhereClause],
        order_by: &[(String, OrderDirection)],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> (String, Vec<Value>) {
        let mut sql = format!("SELECT * FROM {}", quote_ident(&table_name(model)));
        let (where_sql, params) = build_where(where_clauses, 0);
        if !where_sql.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_sql));
        }
        if !order_by.is_empty() {
            // Tolerant sort: only order by columns the model actually has, so an
            // `orderBy` on a column a given model lacks (e.g. the admin UI's default
            // `created_at` against a model that diverged) degrades to "no sort"
            // instead of a runtime `column does not exist` error (consumer feedback
            // #2). System columns are always present (see generate_migrations).
            let mut valid: std::collections::HashSet<String> = model
                .fields
                .values()
                .map(|f| to_snake_case(&f.name))
                .collect();
            for sys in ["created_at", "updated_at", "deleted_at", "tenant_id"] {
                valid.insert(sys.to_string());
            }
            let clauses: Vec<String> = order_by
                .iter()
                .filter(|(f, _)| valid.contains(&to_snake_case(f)))
                .map(|(f, d)| {
                    let dir = match d {
                        OrderDirection::Asc => "ASC",
                        OrderDirection::Desc => "DESC",
                    };
                    format!("{} {}", quote_ident(&to_snake_case(f)), dir)
                })
                .collect();
            if !clauses.is_empty() {
                sql.push_str(&format!(" ORDER BY {}", clauses.join(", ")));
            }
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

        for (key, val) in data {
            columns.push(quote_ident(&to_snake_case(key)));
            placeholders.push(write_value(model, key, val, &mut params));
        }

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({}) RETURNING *",
            quote_ident(&table_name(model)),
            columns.join(", "),
            placeholders.join(", ")
        );
        (sql, params)
    }

    /// Build a **multi-row** INSERT for a homogeneous batch — one statement, one round trip.
    /// Returns `None` (caller falls back to per-row inserts) when the batch is empty, has no
    /// columns, the records have differing key sets, or the batch would exceed Postgres' bind-param
    /// limit. Columns absent from every record get their DB default uniformly (`id`, `created_at`),
    /// because the homogeneity check guarantees the column set is identical across rows.
    pub fn insert_many(
        model: &Model,
        records: &[HashMap<String, Value>],
    ) -> Option<(String, Vec<Value>)> {
        let first = records.first()?;
        // Fixed, deterministic column order shared by every row (HashMap order is not stable).
        let mut cols: Vec<String> = first.keys().cloned().collect();
        cols.sort();
        let ncols = cols.len();
        if ncols == 0 {
            return None;
        }
        // Homogeneity: every record must have exactly these keys.
        for r in records {
            if r.len() != ncols || !cols.iter().all(|c| r.contains_key(c)) {
                return None;
            }
        }
        // Stay well under Postgres' 65535 bind-param ceiling; fall back to per-row beyond that.
        if records.len().saturating_mul(ncols) > 60_000 {
            return None;
        }

        let mut params = Vec::with_capacity(records.len() * ncols);
        let mut tuples = Vec::with_capacity(records.len());
        for record in records {
            let mut placeholders = Vec::with_capacity(ncols);
            for column in &cols {
                placeholders.push(write_value(
                    model,
                    column,
                    record.get(column).unwrap_or(&Value::Null),
                    &mut params,
                ));
            }
            tuples.push(format!("({})", placeholders.join(", ")));
        }

        let column_sql = cols
            .iter()
            .map(|c| quote_ident(&to_snake_case(c)))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO {} ({}) VALUES {} RETURNING *",
            quote_ident(&table_name(model)),
            column_sql,
            tuples.join(", ")
        );
        Some((sql, params))
    }

    /// Build UPDATE query. Returns (sql, params)
    pub fn update(
        model: &Model,
        where_clauses: &[WhereClause],
        data: &HashMap<String, Value>,
    ) -> (String, Vec<Value>) {
        let mut set_clauses = Vec::new();
        let mut params = Vec::new();

        for (key, val) in data {
            set_clauses.push(format!(
                "{} = {}",
                quote_ident(&to_snake_case(key)),
                write_value(model, key, val, &mut params)
            ));
        }

        let mut sql = format!(
            "UPDATE {} SET {}",
            quote_ident(&table_name(model)),
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
        let mut sql = format!("DELETE FROM {}", quote_ident(&table_name(model)));
        let (where_sql, params) = build_where(where_clauses, 0);
        if !where_sql.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_sql));
        }
        sql.push_str(" RETURNING \"id\"");
        (sql, params)
    }

    /// Build a soft-delete UPDATE (sets deleted_at = NOW())
    pub fn soft_delete(model: &Model, where_clauses: &[WhereClause]) -> (String, Vec<Value>) {
        let mut sql = format!(
            "UPDATE {} SET \"deleted_at\" = NOW()",
            quote_ident(&table_name(model))
        );
        let (where_sql, params) = build_where(where_clauses, 0);
        if !where_sql.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_sql));
        }
        // Return the affected ids so the Deleted event can carry them (projections/audit need
        // the id to remove the right row — previously the event had empty data).
        sql.push_str(" RETURNING \"id\"");
        (sql, params)
    }

    /// Build a restore UPDATE (clears deleted_at) for soft-deleted records.
    pub fn restore(model: &Model, where_clauses: &[WhereClause]) -> (String, Vec<Value>) {
        let mut sql = format!(
            "UPDATE {} SET \"deleted_at\" = NULL",
            quote_ident(&table_name(model))
        );
        let (where_sql, params) = build_where(where_clauses, 0);
        if !where_sql.is_empty() {
            sql.push_str(&format!(" WHERE {}", where_sql));
        }
        sql.push_str(" RETURNING \"id\"");
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

pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        if c.is_uppercase() && !result.is_empty() {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result
}

/// SQL NULL has an unknown type resolved by its destination column. Binding
/// Option<String>::None instead declares TEXT, which fails for numeric/bool/JSON columns.
fn write_value(model: &Model, key: &str, value: &Value, params: &mut Vec<Value>) -> String {
    if value.is_null() {
        return "NULL".into();
    }
    params.push(value.clone());
    let is_datetime = model
        .fields
        .get(key)
        .is_some_and(|field| matches!(field.field_type, crate::schema::FieldType::DateTime));
    format!(
        "${}{}",
        params.len(),
        if is_datetime { "::timestamptz" } else { "" }
    )
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
        let col = quote_ident(&to_snake_case(&clause.field));
        match &clause.operator {
            WhereOperator::Equals => {
                // For string values, cast the column to text so the comparison works
                // against both TEXT and UUID columns (e.g. id). Numbers/bools compare directly.
                let cast = if clause.value.is_string() {
                    "::text"
                } else {
                    ""
                };
                parts.push(format!("{}{} = ${}", col, cast, idx));
                params.push(clause.value.clone());
                idx += 1;
            }
            WhereOperator::NotEquals => {
                let cast = if clause.value.is_string() {
                    "::text"
                } else {
                    ""
                };
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
            WhereOperator::In | WhereOperator::NotIn => {
                // JSON arrays are native JSONB write parameters elsewhere. Expand only the
                // filter into bound scalars rather than treating that JSONB as a PG array.
                let Some(values) = clause.value.as_array() else {
                    parts.push("FALSE".to_string());
                    continue;
                };
                let negated = matches!(clause.operator, WhereOperator::NotIn);
                if values.is_empty() {
                    parts.push(if negated { "TRUE" } else { "FALSE" }.to_string());
                    continue;
                }
                let mut members = Vec::new();
                for value in values {
                    let operator = if negated { "!=" } else { "=" };
                    if value.is_null() {
                        members.push(format!("{col} {operator} NULL"));
                        continue;
                    }
                    let cast = if value.is_string() { "::text" } else { "" };
                    members.push(format!("{col}{cast} {operator} ${idx}"));
                    params.push(value.clone());
                    idx += 1;
                }
                parts.push(format!(
                    "({})",
                    members.join(if negated { " AND " } else { " OR " })
                ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Model;
    use serde_json::json;

    fn model(name: &str, table: Option<&str>) -> Model {
        Model {
            name: name.to_string(),
            fields: std::collections::HashMap::new(),
            access: None,
            hooks: None,
            validation: std::collections::HashMap::new(),
            table_name: table.map(|s| s.to_string()),
            relationships: std::collections::HashMap::new(),
            constraints: Vec::new(),
            events: Default::default(),
            ui: None,
        }
    }

    fn eq(field: &str, v: Value) -> WhereClause {
        WhereClause {
            field: field.to_string(),
            operator: WhereOperator::Equals,
            value: v,
        }
    }

    #[test]
    fn table_name_honors_explicit_else_pluralizes() {
        assert_eq!(table_name(&model("Contact", Some("contact"))), "contact");
        assert_eq!(table_name(&model("Contact", None)), "contacts");
    }

    #[test]
    fn insert_many_builds_one_multi_row_insert_for_homogeneous_batch() {
        let m = model("Note", Some("notes"));
        let mk = |t: &str, b: &str| {
            let mut h = std::collections::HashMap::new();
            h.insert("title".to_string(), json!(t));
            h.insert("body".to_string(), json!(b));
            h
        };
        let (sql, params) =
            SqlBuilder::insert_many(&m, &[mk("a", "x"), mk("b", "y")]).expect("homogeneous → Some");
        // Columns sorted for determinism; one statement with two value tuples.
        assert_eq!(
            sql,
            "INSERT INTO \"notes\" (\"body\", \"title\") VALUES ($1, $2), ($3, $4) RETURNING *"
        );
        assert_eq!(params, vec![json!("x"), json!("a"), json!("y"), json!("b")]);

        // Differing key sets → None (caller falls back to per-row).
        let mut odd = std::collections::HashMap::new();
        odd.insert("title".to_string(), json!("c"));
        assert!(SqlBuilder::insert_many(&m, &[mk("a", "x"), odd]).is_none());

        // Empty batch → None.
        assert!(SqlBuilder::insert_many(&m, &[]).is_none());
    }

    #[test]
    fn equals_casts_strings_to_text_but_not_numbers() {
        // String value → ::text cast (so it works against TEXT and UUID id columns).
        let (sql, params) = build_where(&[eq("id", json!("abc"))], 0);
        assert_eq!(sql, "\"id\"::text = $1");
        assert_eq!(params, vec![json!("abc")]);
        // Numeric value → no cast.
        let (sql, _) = build_where(&[eq("value", json!(50000))], 0);
        assert_eq!(sql, "\"value\" = $1");
    }

    #[test]
    fn build_where_honors_param_offset_and_field_snake_case() {
        // Offset 2 (e.g. after an UPDATE's SET params) → placeholders start at $3.
        let (sql, _) = build_where(&[eq("companyId", json!("c1"))], 2);
        assert_eq!(sql, "\"company_id\"::text = $3");
    }

    #[test]
    fn select_builds_where_order_limit_offset() {
        let (sql, _) = SqlBuilder::select(
            &model("Deal", Some("deal")),
            &[eq("stage", json!("won"))],
            &[("createdAt".into(), OrderDirection::Desc)],
            Some(20),
            Some(40),
        );
        assert_eq!(
            sql,
            "SELECT * FROM \"deal\" WHERE \"stage\"::text = $1 ORDER BY \"created_at\" DESC LIMIT 20 OFFSET 40"
        );
    }

    #[test]
    fn select_drops_unknown_order_columns() {
        // Tolerant sort (consumer feedback #2): an `orderBy` on a column the model
        // lacks is dropped rather than producing a `column does not exist` error.
        // System columns (created_at) are always valid; `bogus` is neither field nor
        // system, so it's filtered out — leaving only the valid column.
        let (sql, _) = SqlBuilder::select(
            &model("Deal", Some("deal")),
            &[],
            &[
                ("createdAt".into(), OrderDirection::Desc),
                ("bogus".into(), OrderDirection::Asc),
            ],
            None,
            None,
        );
        assert_eq!(sql, "SELECT * FROM \"deal\" ORDER BY \"created_at\" DESC");

        // All-unknown order → no ORDER BY clause at all (not a SQL error).
        let (sql2, _) = SqlBuilder::select(
            &model("Deal", Some("deal")),
            &[],
            &[("bogus".into(), OrderDirection::Asc)],
            None,
            None,
        );
        assert_eq!(sql2, "SELECT * FROM \"deal\"");
    }

    #[test]
    fn update_params_set_then_where() {
        let mut data = std::collections::HashMap::new();
        data.insert("stage".to_string(), json!("won"));
        let (sql, params) = SqlBuilder::update(
            &model("Deal", Some("deal")),
            &[eq("id", json!("d1"))],
            &data,
        );
        // SET uses $1, WHERE uses $2 (offset by set count); RETURNING * appended.
        assert_eq!(
            sql,
            "UPDATE \"deal\" SET \"stage\" = $1 WHERE \"id\"::text = $2 RETURNING *"
        );
        assert_eq!(params, vec![json!("won"), json!("d1")]);
    }

    #[test]
    fn soft_delete_returns_id() {
        let (sql, _) =
            SqlBuilder::soft_delete(&model("Deal", Some("deal")), &[eq("id", json!("d1"))]);
        assert_eq!(
            sql,
            "UPDATE \"deal\" SET \"deleted_at\" = NOW() WHERE \"id\"::text = $1 RETURNING \"id\""
        );
    }

    #[test]
    fn nullable_writes_use_sql_null_without_consuming_parameter_slots() {
        let model = model("Probe", Some("probes"));
        let (sql, params) = SqlBuilder::update(
            &model,
            &[eq("id", json!("kept"))],
            &HashMap::from([("sequence".into(), Value::Null)]),
        );
        assert_eq!(
            sql,
            "UPDATE \"probes\" SET \"sequence\" = NULL WHERE \"id\"::text = $1 RETURNING *"
        );
        assert_eq!(params, vec![json!("kept")]);
        let rows = vec![
            HashMap::from([
                ("id".into(), json!("null")),
                ("sequence".into(), Value::Null),
            ]),
            HashMap::from([("id".into(), json!("value")), ("sequence".into(), json!(7))]),
        ];
        let (sql, params) = SqlBuilder::insert_many(&model, &rows).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO \"probes\" (\"id\", \"sequence\") VALUES ($1, NULL), ($2, $3) RETURNING *"
        );
        assert_eq!(params, vec![json!("null"), json!("value"), json!(7)]);
    }

    #[test]
    fn reserved_word_identifiers_are_quoted() {
        // Postgres reserved words as field names (`order`, `user`) previously
        // produced syntax errors at migration and query time. Quoting is
        // unconditional so the whole reserved-word class is covered.
        let mut m = model("Ticket", Some("tickets"));
        for f in ["order", "user"] {
            m.fields.insert(
                f.to_string(),
                crate::schema::Field {
                    name: f.to_string(),
                    field_type: crate::schema::FieldType::String,
                    optional: true,
                    attributes: vec![],
                },
            );
        }
        let data = HashMap::from([("order".into(), json!(5))]);
        let (sql, _) = SqlBuilder::insert(&m, &data);
        assert_eq!(
            sql,
            "INSERT INTO \"tickets\" (\"order\") VALUES ($1) RETURNING *"
        );
        let (sql, _) = SqlBuilder::update(&m, &[eq("order", json!(1))], &data);
        assert_eq!(
            sql,
            "UPDATE \"tickets\" SET \"order\" = $1 WHERE \"order\" = $2 RETURNING *"
        );
        let (sql, _) = SqlBuilder::select(
            &m,
            &[eq("user", json!("u1"))],
            &[("order".into(), OrderDirection::Asc)],
            None,
            None,
        );
        assert_eq!(
            sql,
            "SELECT * FROM \"tickets\" WHERE \"user\"::text = $1 ORDER BY \"order\" ASC"
        );
    }

    #[test]
    fn set_filters_keep_scope_offsets_and_empty_semantics() {
        let clauses = vec![
            eq("worldId", json!("world-a")),
            WhereClause {
                field: "id".into(),
                operator: WhereOperator::In,
                value: json!(["one", "two"]),
            },
        ];
        let (sql, params) = build_where(&clauses, 2);
        assert_eq!(
            sql,
            "\"world_id\"::text = $3 AND (\"id\"::text = $4 OR \"id\"::text = $5)"
        );
        assert_eq!(params, vec![json!("world-a"), json!("one"), json!("two")]);
        for (operator, value, expected) in [
            (WhereOperator::In, json!([]), "FALSE"),
            (WhereOperator::NotIn, json!([]), "TRUE"),
            (WhereOperator::NotIn, json!("invalid"), "FALSE"),
        ] {
            assert_eq!(
                build_where(
                    &[WhereClause {
                        field: "id".into(),
                        operator,
                        value
                    }],
                    0
                )
                .0,
                expected
            );
        }
    }
}
