//! Schema parsing and management
//!
//! This module handles parsing TypeScript schema files and converting them
//! into Rust types for the Atomo runtime.

use anyhow::Result;
use std::collections::HashMap;

// Re-export from atomo_schema for compatibility
pub use atomo_schema::{
    is_builder_dsl, parse_builder_dsl, ActionCondition, ActionDef, ActionInputDef,
    ActionInputField, ActionReturn, EventActionBinding, Field, FieldAttribute, FieldType, Model,
    ModelConstraint, ModelEvents, Schema, TypeScriptParser,
};

/// Parse a schema string into a Schema object. Auto-detects the builder DSL
/// (`@atomo/schema` imports) vs the legacy TypeScript-interface format.
pub fn parse_typescript_schema(content: &str) -> Result<Schema> {
    if is_builder_dsl(content) {
        parse_builder_dsl(content)
    } else {
        let parser = TypeScriptParser::new();
        parser.parse_schema(content)
    }
}

/// Generate database migrations from schema
pub fn generate_migrations(schema: &Schema) -> Result<Vec<String>> {
    let mut migrations = Vec::new();

    for model in schema.models.values() {
        let table = crate::query::sql_builder::table_name_for(model);
        let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (\n", table);

        let mut columns = Vec::new();
        let mut index_cols: Vec<String> = Vec::new();
        let mut unique_cols: Vec<String> = Vec::new();
        for field in model.fields.values() {
            let col = to_snake_case(&field.name);
            let column_type = field_type_to_sql(&field.field_type);
            // Primary key: id is TEXT (EntityId is a ULID string).
            // Default generates a value DB-side so inserts without an explicit id still work.
            let is_primary = field.name == "id"
                || field
                    .attributes
                    .iter()
                    .any(|a| matches!(a, FieldAttribute::Primary));
            if is_primary {
                columns.push(format!(
                    "  {} TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text",
                    col
                ));
                continue;
            }
            // Declared `.default(..)` plus the timestamp/JSON-array conventions (see
            // column_default_clause): a `status: text().default('draft')` or a required
            // `notes: ContentBlock[]` doesn't force every insert to pass it.
            let default = column_default_clause(field);
            let nullable = if field.optional { "" } else { " NOT NULL" };
            columns.push(format!("  {} {}{}{}", col, column_type, default, nullable));
            // `@unique`/`@index` annotations are reconciled as post-table indexes (idempotent and
            // applicable to tables created before the annotation existed), not inline constraints.
            if field
                .attributes
                .iter()
                .any(|a| matches!(a, FieldAttribute::Unique))
            {
                unique_cols.push(col.clone());
            }
            if field
                .attributes
                .iter()
                .any(|a| matches!(a, FieldAttribute::Index))
            {
                index_cols.push(col.clone());
            }
        }

        // Platform timestamp convention: every model gets created_at + updated_at so
        // the admin list view (which orders by created_at), audit, and replay behave
        // uniformly. Auto-added only when the model didn't declare them — a model that
        // declares created_at/updated_at already got those columns above (with DEFAULT
        // NOW()). Previously a model that declared only `updatedAt` silently lacked
        // created_at and 500'd the list view at query time (consumer feedback #2).
        let declared: std::collections::HashSet<String> = model
            .fields
            .values()
            .map(|f| to_snake_case(&f.name))
            .collect();
        if !declared.contains("created_at") {
            columns.push("  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()".to_string());
        }
        if !declared.contains("updated_at") {
            columns.push("  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()".to_string());
        }
        if !declared.contains("deleted_at") {
            columns.push("  deleted_at TIMESTAMPTZ".to_string());
        }
        if !declared.contains("tenant_id") {
            columns.push("  tenant_id TEXT".to_string());
        }
        sql.push_str(&columns.join(",\n"));
        sql.push_str("\n);");

        migrations.push(sql);

        // Forward-migrate declared fields for tables created from an older schema. Optional fields
        // and fields with a default add safely — a `DEFAULT` backfills existing rows, so even a
        // required-with-default column is safe on a populated table. A required field with *no*
        // default cannot be added to a populated table; instead of a raw `NOT NULL` add that fails
        // at startup with an opaque error, emit a guarded statement that adds the column when the
        // table is empty and otherwise raises an actionable message pointing at the fix.
        for field in model.fields.values() {
            let col = to_snake_case(&field.name);
            let is_primary = field.name == "id"
                || field
                    .attributes
                    .iter()
                    .any(|a| matches!(a, FieldAttribute::Primary));
            if is_primary {
                continue;
            }
            let column_type = field_type_to_sql(&field.field_type);
            let default = column_default_clause(field);
            if field.optional || !default.is_empty() {
                let nullable = if field.optional { "" } else { " NOT NULL" };
                migrations.push(format!(
                    "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS {col} {column_type}{default}{nullable};"
                ));
            } else {
                migrations.push(format!(
                    "DO $$ BEGIN \
                     IF NOT EXISTS (SELECT 1 FROM information_schema.columns \
                       WHERE table_schema = current_schema() AND table_name = '{table}' AND column_name = '{col}') THEN \
                       IF EXISTS (SELECT 1 FROM {table} LIMIT 1) THEN \
                         RAISE EXCEPTION 'atomo: cannot add required column {table}.{col} with no default to a populated table; declare a .default(...) on the field, or write an explicit backfill migration (add it nullable, backfill, then SET NOT NULL)'; \
                       ELSE \
                         ALTER TABLE {table} ADD COLUMN {col} {column_type} NOT NULL; \
                       END IF; \
                     END IF; END $$;"
                ));
            }
        }

        // Ensure auto-appended columns exist on tables that were created before
        // these columns were introduced (e.g. the platform `users` table).
        for col_def in [("deleted_at", "TIMESTAMPTZ"), ("tenant_id", "TEXT")] {
            migrations.push(format!(
                "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS {} {};",
                col_def.0, col_def.1
            ));
        }

        // Per-field uniqueness from `@unique` annotations, as a UNIQUE INDEX (idempotent). Emitted
        // here rather than as an inline column constraint so it reconciles on tables created before
        // the column was unique, mirroring the @index pass.
        for col in &unique_cols {
            migrations.push(format!(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_{table}_{col} ON {table} ({col});"
            ));
        }

        // Secondary indexes from `@index` annotations (idempotent).
        for col in &index_cols {
            migrations.push(format!(
                "CREATE INDEX IF NOT EXISTS idx_{table}_{col} ON {table} ({col});"
            ));
        }

        // Model-level constraints from @@unique([..]) / @@index([..]) / @@check(..).
        push_constraint_migrations(&table, &model.constraints, &mut migrations);
    }

    // Foreign-key pass (after all tables exist, so ordering doesn't matter): for each model's
    // `belongsTo` relationship with a foreignKey, add a FK to the target table's id. This is what
    // actually enforces referential integrity (the `exists:` validation rule is a no-op). Emitted
    // as guarded ALTERs (idempotent: skip if the constraint already exists). FKs are NOT VALID-free
    // here — they validate existing rows; tables are fresh at create time so that's fine.
    let table_of: HashMap<&str, String> = schema
        .models
        .values()
        .map(|m| {
            (
                m.name.as_str(),
                crate::query::sql_builder::table_name_for(m),
            )
        })
        .collect();
    for model in schema.models.values() {
        let table = crate::query::sql_builder::table_name_for(model);
        for (rel_name, rel) in &model.relationships {
            if rel.kind != "belongsTo" {
                continue;
            }
            let fk_raw = rel
                .foreign_key
                .clone()
                .unwrap_or_else(|| format!("{}Id", rel_name));
            let fk_col = to_snake_case(&fk_raw);
            let target = match table_of.get(rel.model.as_str()) {
                Some(t) => t,
                None => continue, // target model not in schema — skip
            };
            // Skip if the model doesn't actually have the FK column (defensive).
            if !model.fields.keys().any(|f| to_snake_case(f) == fk_col) {
                continue;
            }
            let cname = format!("fk_{}_{}", table, fk_col);
            migrations.push(format!(
                "DO $$ BEGIN \
                 IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = '{cname}') THEN \
                 ALTER TABLE {table} ADD CONSTRAINT {cname} FOREIGN KEY ({fk_col}) REFERENCES {target}(id); \
                 END IF; END $$;"
            ));
        }
    }

    // Validation-rule-to-CHECK pass: compile each model's `validation` entries into
    // SQL CHECK constraints so the database enforces the same invariants as the Rust
    // runtime validator (belt-and-suspenders). Emitted as guarded ALTERs (idempotent).
    for model in schema.models.values() {
        let table = crate::query::sql_builder::table_name_for(model);
        for (field_name, rules) in &model.validation {
            let field = match model.fields.get(field_name) {
                Some(f) => f,
                None => continue, // validation rule references unknown field — skip
            };
            let checks = validation_checks(&table, field_name, field, rules);
            migrations.extend(checks);
        }
    }

    Ok(migrations)
}

/// Emit idempotent DDL for model-level constraints (`@@unique` / `@@index` /
/// `@@check`, incl. partial `WHERE` variants) on `table`. Shared by the model
/// migration pass and built-in table extensions.
fn push_constraint_migrations(
    table: &str,
    constraints: &[ModelConstraint],
    migrations: &mut Vec<String>,
) {
    for (n, c) in constraints.iter().enumerate() {
        match c {
            // Composite uniqueness via a UNIQUE INDEX (idempotent with IF NOT EXISTS).
            ModelConstraint::Unique(cols) => {
                let snake: Vec<String> = cols.iter().map(|c| to_snake_case(c)).collect();
                migrations.push(format!(
                    "CREATE UNIQUE INDEX IF NOT EXISTS uq_{table}_{joined} ON {table} ({list});",
                    joined = snake.join("_"),
                    list = snake.join(", ")
                ));
            }
            ModelConstraint::Index(cols) => {
                let snake: Vec<String> = cols.iter().map(|c| to_snake_case(c)).collect();
                migrations.push(format!(
                    "CREATE INDEX IF NOT EXISTS idx_{table}_{joined} ON {table} ({list});",
                    joined = snake.join("_"),
                    list = snake.join(", ")
                ));
            }
            // PARTIAL unique/index: same as above plus a `WHERE <predicate>`. The
            // predicate is raw SQL over column names (snake_case), so a nullable
            // anti-abuse anchor like UNIQUE(store_account_id) WHERE store_account_id
            // IS NOT NULL is expressible in the schema instead of hand-written SQL.
            ModelConstraint::UniqueWhere(cols, predicate) => {
                let snake: Vec<String> = cols.iter().map(|c| to_snake_case(c)).collect();
                migrations.push(format!(
                    "CREATE UNIQUE INDEX IF NOT EXISTS uq_{table}_{joined} ON {table} ({list}) WHERE {predicate};",
                    joined = snake.join("_"),
                    list = snake.join(", ")
                ));
            }
            ModelConstraint::IndexWhere(cols, predicate) => {
                let snake: Vec<String> = cols.iter().map(|c| to_snake_case(c)).collect();
                migrations.push(format!(
                    "CREATE INDEX IF NOT EXISTS idx_{table}_{joined} ON {table} ({list}) WHERE {predicate};",
                    joined = snake.join("_"),
                    list = snake.join(", ")
                ));
            }
            // CHECK as a guarded ALTER (same idempotency pattern as the FK pass).
            ModelConstraint::Check(expr) => {
                let cname = format!("chk_{table}_{n}");
                migrations.push(format!(
                    "DO $$ BEGIN \
                     IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = '{cname}') THEN \
                     ALTER TABLE {table} ADD CONSTRAINT {cname} CHECK ({expr}); \
                     END IF; END $$;"
                ));
            }
        }
    }
}

/// Built-in platform tables consumers may extend via the schema `builtins` block.
/// Deliberately narrow — anything else fails loud at boot rather than silently
/// altering a table atomo doesn't expect to change.
const EXTENDABLE_BUILTINS: &[&str] = &["users"];

/// Generate idempotent DDL for built-in table extensions (`schema.builtins`):
/// append-only extra columns plus model-level constraints on platform tables.
///
/// Kept separate from [`generate_migrations`] because platform tables (e.g.
/// `users`) are created by the server AFTER schema migrations run — the server
/// executes these right after `ensure_platform_tables`, when the target exists.
pub fn generate_builtin_extension_migrations(schema: &Schema) -> Result<Vec<String>> {
    let mut migrations = Vec::new();
    let mut tables: Vec<_> = schema.builtins.iter().collect();
    tables.sort_by_key(|(t, _)| t.as_str());
    for (table, ext) in tables {
        if !EXTENDABLE_BUILTINS.contains(&table.as_str()) {
            anyhow::bail!(
                "builtins.{table}: not an extendable built-in table (allowed: {})",
                EXTENDABLE_BUILTINS.join(", ")
            );
        }
        let mut cols: Vec<_> = ext.columns.iter().collect();
        cols.sort_by_key(|(f, _)| f.as_str());
        for (field, sql_type) in cols {
            let ty = sql_type.trim();
            // Append-only: a NOT NULL column would break existing rows on adoption.
            if ty.to_uppercase().contains("NOT NULL") {
                anyhow::bail!(
                    "builtins.{table}.{field}: extension columns must be nullable (drop NOT NULL)"
                );
            }
            migrations.push(format!(
                "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS {col} {ty};",
                col = to_snake_case(field)
            ));
        }
        push_constraint_migrations(table, &ext.constraints, &mut migrations);
    }
    Ok(migrations)
}

/// Convert a pipe-separated validation rule string into SQL CHECK constraint DDL.
///
/// Rules that already have SQL-level equivalents (`required` -> NOT NULL, `unique` -> UNIQUE,
/// `exists:*` -> FK) are intentionally skipped — they are handled elsewhere.
fn validation_checks(table: &str, field_name: &str, field: &Field, rules: &str) -> Vec<String> {
    let col = to_snake_case(field_name);
    let is_number = matches!(field.field_type, FieldType::Number);
    let optional = field.optional;
    let mut stmts = Vec::new();

    for rule in rules.split('|') {
        let (rule_name, param) = if let Some(idx) = rule.find(':') {
            (&rule[..idx], Some(&rule[idx + 1..]))
        } else {
            (rule, None)
        };

        let expr = match rule_name {
            // `required` → NOT NULL (already on non-optional columns; no extra CHECK).
            // `unique` → column UNIQUE constraint (handled in column generation).
            // `exists:*` → FK (handled in the FK pass).
            // `numeric` / `in` / custom → not expressible as a simple CHECK; skip.
            "required" | "unique" | "numeric" | "in" => continue,
            _ if rule_name == "exists" => continue,

            "email" => format!("{col} ~ '^[^@]+@[^@]+\\.[^@]+$'"),

            "url" => format!("{col} ~ '^https?://'"),

            "min" => {
                let n = match param.and_then(|p| p.parse::<i64>().ok()) {
                    Some(v) => v,
                    None => continue,
                };
                if is_number {
                    format!("{col} >= {n}")
                } else {
                    format!("length({col}) >= {n}")
                }
            }

            "max" => {
                let n = match param.and_then(|p| p.parse::<i64>().ok()) {
                    Some(v) => v,
                    None => continue,
                };
                if is_number {
                    format!("{col} <= {n}")
                } else {
                    format!("length({col}) <= {n}")
                }
            }

            // Unknown/unsupported rules — skip silently.
            _ => continue,
        };

        // Optional fields must allow NULLs through the CHECK.
        let full_expr = if optional {
            format!("{col} IS NULL OR ({expr})")
        } else {
            expr
        };

        let cname = format!("chk_{table}_{col}_{rule_name}");
        stmts.push(format!(
            "DO $$ BEGIN \
             IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = '{cname}') THEN \
             ALTER TABLE {table} ADD CONSTRAINT {cname} CHECK ({full_expr}); \
             END IF; END $$;"
        ));
    }

    stmts
}

/// Convert FieldType to SQL type
fn field_type_to_sql(field_type: &FieldType) -> &'static str {
    match field_type {
        FieldType::String => "TEXT",
        FieldType::File => "TEXT",
        FieldType::Number => "BIGINT",
        FieldType::Boolean => "BOOLEAN",
        FieldType::Date => "DATE",
        FieldType::DateTime => "TIMESTAMPTZ",
        FieldType::EntityId => "TEXT",
        FieldType::Json => "JSONB",
        FieldType::Reference(_) => "TEXT",
        FieldType::Array(_) => "JSONB",
        FieldType::Blocks => "JSONB",
        FieldType::Custom(_) => "JSONB",
    }
}

/// SQL `DEFAULT <literal>` clause for a column, or an empty string when it has none.
///
/// A schema-declared `.default(..)` wins; otherwise the platform conventions apply
/// (`created_at`/`updated_at` -> `NOW()`, JSON array/block fields -> `'[]'`). Used by both the
/// CREATE TABLE path and the forward-migration ALTER path so a column's default is reconciled the
/// same way however the table came to exist — and so a forward-added column with a default backfills
/// existing rows instead of failing.
fn column_default_clause(field: &Field) -> String {
    if let Some(value) = field.attributes.iter().find_map(|a| match a {
        FieldAttribute::Default(v) => Some(v.as_str()),
        _ => None,
    }) {
        return format!(" DEFAULT {}", default_literal(value, &field.field_type));
    }
    let col = to_snake_case(&field.name);
    match (col.as_str(), &field.field_type) {
        ("created_at" | "updated_at", FieldType::DateTime) => " DEFAULT NOW()".to_string(),
        (_, FieldType::Array(_) | FieldType::Blocks) => " DEFAULT '[]'::jsonb".to_string(),
        _ => String::new(),
    }
}

/// Render a declared default value as a SQL literal. Numbers and booleans are emitted raw; every
/// other type becomes a single-quoted string literal (with `'` escaped), which Postgres casts to
/// the column type.
fn default_literal(value: &str, field_type: &FieldType) -> String {
    match field_type {
        FieldType::Number | FieldType::Boolean => value.to_string(),
        _ => format!("'{}'", value.replace('\'', "''")),
    }
}

/// Convert camelCase to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let chars = s.chars().peekable();

    for c in chars {
        if c.is_uppercase() && !result.is_empty() {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }

    result
}

/// Check for declared schema fields whose columns are missing from the database.
/// Emits a `tracing::warn!` for each gap so consumers notice drift at boot instead
/// of hitting opaque runtime failures.
pub async fn check_column_drift(schema: &Schema, pool: &sqlx::PgPool) {
    for model in schema.models.values() {
        let table = crate::query::sql_builder::table_name_for(model);

        let existing: Vec<String> = match sqlx::query_scalar::<_, String>(
            "SELECT column_name FROM information_schema.columns \
             WHERE table_schema = current_schema() AND table_name = $1",
        )
        .bind(&table)
        .fetch_all(pool)
        .await
        {
            Ok(cols) => cols,
            Err(_) => continue,
        };

        if existing.is_empty() {
            continue;
        }

        let existing: std::collections::HashSet<String> = existing.into_iter().collect();

        for field in model.fields.values() {
            let col = to_snake_case(&field.name);
            if !existing.contains(&col) {
                tracing::warn!(
                    table = %table,
                    column = %col,
                    "table is missing declared column — schema/DB drift detected; \
                     enable migrations or run them manually"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomo_schema::{Field, FieldType, Model, Relationship, Schema};
    use std::collections::HashMap;

    fn field(name: &str, ty: FieldType, optional: bool) -> (String, Field) {
        (
            name.to_string(),
            Field {
                name: name.to_string(),
                field_type: ty,
                optional,
                attributes: vec![],
            },
        )
    }

    #[test]
    fn field_type_to_sql_mapping() {
        assert_eq!(field_type_to_sql(&FieldType::String), "TEXT");
        assert_eq!(field_type_to_sql(&FieldType::Number), "BIGINT");
        assert_eq!(field_type_to_sql(&FieldType::Boolean), "BOOLEAN");
        assert_eq!(field_type_to_sql(&FieldType::DateTime), "TIMESTAMPTZ");
        assert_eq!(field_type_to_sql(&FieldType::Json), "JSONB");
        assert_eq!(
            field_type_to_sql(&FieldType::Array(Box::new(FieldType::String))),
            "JSONB"
        );
    }

    fn model(
        name: &str,
        table: &str,
        fields: Vec<(String, Field)>,
        rels: Vec<(&str, Relationship)>,
    ) -> Model {
        Model {
            name: name.to_string(),
            fields: fields.into_iter().collect(),
            access: None,
            hooks: None,
            validation: HashMap::new(),
            table_name: Some(table.to_string()),
            relationships: rels.into_iter().map(|(n, r)| (n.to_string(), r)).collect(),
            constraints: Vec::new(),
            events: Default::default(),
            ui: None,
        }
    }

    #[test]
    fn generate_migrations_emits_softdelete_tenant_and_fk() {
        let contact = model(
            "Contact",
            "contact",
            vec![field("id", FieldType::EntityId, false)],
            vec![],
        );
        let deal = model(
            "Deal",
            "deal",
            vec![
                field("id", FieldType::EntityId, false),
                field("contactId", FieldType::String, false),
            ],
            vec![(
                "contact",
                Relationship {
                    kind: "belongsTo".into(),
                    model: "Contact".into(),
                    foreign_key: Some("contactId".into()),
                },
            )],
        );
        let mut models = HashMap::new();
        models.insert("Contact".into(), contact);
        models.insert("Deal".into(), deal);
        let sql = generate_migrations(&Schema {
            models,
            actions: HashMap::new(),
            builtins: HashMap::new(),
        })
        .unwrap()
        .join("\n");

        // Every table gets soft-delete + tenant columns.
        assert!(
            sql.contains("deleted_at TIMESTAMPTZ"),
            "deleted_at missing:\n{}",
            sql
        );
        assert!(
            sql.contains("tenant_id TEXT"),
            "tenant_id missing:\n{}",
            sql
        );
        // ...and auto-provisioned created_at/updated_at (consumer feedback #2): a
        // model that declares neither still gets both, so the list view's default
        // `ORDER BY created_at` never hits a missing column.
        assert!(
            sql.contains("created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()"),
            "auto created_at missing:\n{}",
            sql
        );
        assert!(
            sql.contains("updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()"),
            "auto updated_at missing:\n{}",
            sql
        );
        // belongsTo → FK constraint to the target table's id (referential integrity).
        assert!(
            sql.contains("ALTER TABLE deal ADD CONSTRAINT fk_deal_contact_id FOREIGN KEY (contact_id) REFERENCES contact(id)"),
            "FK not emitted:\n{}", sql
        );
    }

    #[test]
    fn generate_migrations_forward_migrates_declared_fields() {
        let schema = parse_typescript_schema(
            r#"
            import { model, text, datetime } from '@atomo/schema'
            export const Upload = model('uploads', {
              fields: {
                id: text().id(),
                sessionHash: text().required(),
                consumedAt: datetime().optional(),
              },
            })
            "#,
        )
        .unwrap();

        let sql = generate_migrations(&schema).unwrap().join("\n");
        // Required field with no default: a guarded add that raises an actionable message on a
        // populated table and adds the column only when the table is empty — never a bare NOT NULL
        // add that would fail at startup with an opaque error.
        assert!(
            sql.contains("RAISE EXCEPTION 'atomo: cannot add required column uploads.session_hash"),
            "guarded required-add missing:\n{sql}"
        );
        assert!(
            sql.contains("ALTER TABLE uploads ADD COLUMN session_hash TEXT NOT NULL;"),
            "empty-table branch missing:\n{sql}"
        );
        assert!(
            !sql.contains("ADD COLUMN IF NOT EXISTS session_hash TEXT NOT NULL;"),
            "must not emit an unguarded NOT NULL add:\n{sql}"
        );
        // Optional field: a plain nullable add.
        assert!(
            sql.contains("ALTER TABLE uploads ADD COLUMN IF NOT EXISTS consumed_at TIMESTAMPTZ;")
        );
        assert!(!sql.contains("ALTER TABLE uploads ADD COLUMN IF NOT EXISTS id"));
    }

    #[test]
    fn generate_migrations_reconciles_declared_string_default() {
        let schema = parse_typescript_schema(
            r#"
            import { model, text } from '@atomo/schema'
            export const Listing = model('listings', {
              fields: {
                id: text().id(),
                status: text().default('draft'),
              },
            })
            "#,
        )
        .unwrap();
        let sql = generate_migrations(&schema).unwrap().join("\n");
        // CREATE TABLE column carries the declared default...
        assert!(
            sql.contains("status TEXT DEFAULT 'draft'"),
            "create-table default missing:\n{sql}"
        );
        // ...and the forward-migration add carries it too, so it backfills existing rows.
        assert!(
            sql.contains(
                "ALTER TABLE listings ADD COLUMN IF NOT EXISTS status TEXT DEFAULT 'draft';"
            ),
            "forward-migration default missing:\n{sql}"
        );
    }

    #[test]
    fn generate_migrations_required_field_with_default_adds_safely() {
        let schema = parse_typescript_schema(
            r#"
            import { model, text } from '@atomo/schema'
            export const Listing = model('listings', {
              fields: {
                id: text().id(),
                status: text().required().default('draft'),
              },
            })
            "#,
        )
        .unwrap();
        let sql = generate_migrations(&schema).unwrap().join("\n");
        // A required field WITH a default is safe on a populated table (the default backfills),
        // so it is a plain idempotent add, not a guarded one.
        assert!(
            sql.contains(
                "ALTER TABLE listings ADD COLUMN IF NOT EXISTS status TEXT DEFAULT 'draft' NOT NULL;"
            ),
            "safe required-with-default add missing:\n{sql}"
        );
        assert!(
            !sql.contains("cannot add required column listings.status"),
            "must not guard a required field that has a default:\n{sql}"
        );
    }

    #[test]
    fn generate_migrations_emits_unique_and_index_from_attributes() {
        use atomo_schema::FieldAttribute;
        let attr_field = |name: &str, attrs: Vec<FieldAttribute>| {
            (
                name.to_string(),
                Field {
                    name: name.to_string(),
                    field_type: FieldType::String,
                    optional: false,
                    attributes: attrs,
                },
            )
        };
        let ledger = model(
            "CreditLedger",
            "credit_ledger",
            vec![
                field("id", FieldType::EntityId, false),
                attr_field("idempotencyKey", vec![FieldAttribute::Unique]),
                attr_field("accountId", vec![FieldAttribute::Index]),
            ],
            vec![],
        );
        let mut models = HashMap::new();
        models.insert("CreditLedger".into(), ledger);
        let sql = generate_migrations(&Schema {
            models,
            actions: HashMap::new(),
            builtins: HashMap::new(),
        })
        .unwrap()
        .join("\n");

        // @unique -> UNIQUE INDEX (reconcilable on existing tables, not an inline constraint).
        assert!(
            sql.contains(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_credit_ledger_idempotency_key ON credit_ledger (idempotency_key)"
            ),
            "unique index missing:\n{sql}"
        );
        // The column is still emitted (now without the inline UNIQUE keyword).
        assert!(
            sql.contains("idempotency_key TEXT NOT NULL")
                && !sql.contains("idempotency_key TEXT NOT NULL UNIQUE"),
            "unique column should be emitted without an inline UNIQUE:\n{sql}"
        );
        // @index -> CREATE INDEX.
        assert!(
            sql.contains(
                "CREATE INDEX IF NOT EXISTS idx_credit_ledger_account_id ON credit_ledger (account_id)"
            ),
            "index missing:\n{sql}"
        );
    }

    /// Helper: build a model with validation rules for testing the CHECK pass.
    fn model_with_validation(
        name: &str,
        table: &str,
        fields: Vec<(String, Field)>,
        validation: HashMap<String, String>,
    ) -> Model {
        Model {
            name: name.to_string(),
            fields: fields.into_iter().collect(),
            access: None,
            hooks: None,
            validation,
            table_name: Some(table.to_string()),
            relationships: HashMap::new(),
            constraints: Vec::new(),
            events: Default::default(),
            ui: None,
        }
    }

    #[test]
    fn validation_email_generates_regex_check() {
        let validation: HashMap<String, String> = [("email".to_string(), "email".to_string())]
            .into_iter()
            .collect();
        let m = model_with_validation(
            "User",
            "users",
            vec![
                field("id", FieldType::EntityId, false),
                field("email", FieldType::String, false),
            ],
            validation,
        );
        let mut models = HashMap::new();
        models.insert("User".into(), m);
        let sql = generate_migrations(&Schema {
            models,
            actions: HashMap::new(),
            builtins: HashMap::new(),
        })
        .unwrap()
        .join("\n");

        assert!(
            sql.contains("chk_users_email_email"),
            "email CHECK constraint name missing:\n{sql}"
        );
        assert!(
            sql.contains("email ~ '^[^@]+@[^@]+\\.[^@]+$'"),
            "email regex CHECK missing:\n{sql}"
        );
    }

    #[test]
    fn validation_min_max_string_generates_length_check() {
        let validation: HashMap<String, String> =
            [("title".to_string(), "min:1|max:100".to_string())]
                .into_iter()
                .collect();
        let m = model_with_validation(
            "Post",
            "posts",
            vec![
                field("id", FieldType::EntityId, false),
                field("title", FieldType::String, false),
            ],
            validation,
        );
        let mut models = HashMap::new();
        models.insert("Post".into(), m);
        let sql = generate_migrations(&Schema {
            models,
            actions: HashMap::new(),
            builtins: HashMap::new(),
        })
        .unwrap()
        .join("\n");

        assert!(
            sql.contains("chk_posts_title_min") && sql.contains("length(title) >= 1"),
            "min length CHECK missing:\n{sql}"
        );
        assert!(
            sql.contains("chk_posts_title_max") && sql.contains("length(title) <= 100"),
            "max length CHECK missing:\n{sql}"
        );
    }

    #[test]
    fn validation_min_max_number_generates_value_check() {
        let validation: HashMap<String, String> =
            [("amount".to_string(), "min:0|max:1000".to_string())]
                .into_iter()
                .collect();
        let m = model_with_validation(
            "Payment",
            "payments",
            vec![
                field("id", FieldType::EntityId, false),
                field("amount", FieldType::Number, false),
            ],
            validation,
        );
        let mut models = HashMap::new();
        models.insert("Payment".into(), m);
        let sql = generate_migrations(&Schema {
            models,
            actions: HashMap::new(),
            builtins: HashMap::new(),
        })
        .unwrap()
        .join("\n");

        assert!(
            sql.contains("chk_payments_amount_min") && sql.contains("amount >= 0"),
            "min value CHECK missing:\n{sql}"
        );
        assert!(
            sql.contains("chk_payments_amount_max") && sql.contains("amount <= 1000"),
            "max value CHECK missing:\n{sql}"
        );
        // Ensure it uses value comparison, not length.
        assert!(
            !sql.contains("length(amount)"),
            "number field should not use length():\n{sql}"
        );
    }

    #[test]
    fn validation_url_generates_regex_check() {
        let validation: HashMap<String, String> = [("website".to_string(), "url".to_string())]
            .into_iter()
            .collect();
        let m = model_with_validation(
            "Company",
            "companies",
            vec![
                field("id", FieldType::EntityId, false),
                field("website", FieldType::String, false),
            ],
            validation,
        );
        let mut models = HashMap::new();
        models.insert("Company".into(), m);
        let sql = generate_migrations(&Schema {
            models,
            actions: HashMap::new(),
            builtins: HashMap::new(),
        })
        .unwrap()
        .join("\n");

        assert!(
            sql.contains("chk_companies_website_url"),
            "url CHECK constraint name missing:\n{sql}"
        );
        assert!(
            sql.contains("website ~ '^https?://'"),
            "url regex CHECK missing:\n{sql}"
        );
    }

    #[test]
    fn validation_optional_field_wraps_with_is_null() {
        let validation: HashMap<String, String> = [("website".to_string(), "url".to_string())]
            .into_iter()
            .collect();
        let m = model_with_validation(
            "Company",
            "companies",
            vec![
                field("id", FieldType::EntityId, false),
                field("website", FieldType::String, true),
            ],
            validation,
        );
        let mut models = HashMap::new();
        models.insert("Company".into(), m);
        let sql = generate_migrations(&Schema {
            models,
            actions: HashMap::new(),
            builtins: HashMap::new(),
        })
        .unwrap()
        .join("\n");

        assert!(
            sql.contains("website IS NULL OR (website ~ '^https?://')"),
            "optional field should be wrapped with IS NULL OR:\n{sql}"
        );
    }

    #[test]
    fn validation_skips_required_unique_exists() {
        let validation: HashMap<String, String> = [
            ("email".to_string(), "required".to_string()),
            ("slug".to_string(), "unique".to_string()),
            ("authorId".to_string(), "exists:User".to_string()),
        ]
        .into_iter()
        .collect();
        let m = model_with_validation(
            "Post",
            "posts",
            vec![
                field("id", FieldType::EntityId, false),
                field("email", FieldType::String, false),
                field("slug", FieldType::String, false),
                field("authorId", FieldType::String, false),
            ],
            validation,
        );
        let mut models = HashMap::new();
        models.insert("Post".into(), m);
        let sql = generate_migrations(&Schema {
            models,
            actions: HashMap::new(),
            builtins: HashMap::new(),
        })
        .unwrap()
        .join("\n");

        // None of these rules should produce CHECK constraints.
        assert!(
            !sql.contains("chk_posts_email_required"),
            "required should not generate a CHECK:\n{sql}"
        );
        assert!(
            !sql.contains("chk_posts_slug_unique"),
            "unique should not generate a CHECK:\n{sql}"
        );
        assert!(
            !sql.contains("chk_posts_author_id_exists"),
            "exists should not generate a CHECK:\n{sql}"
        );
    }

    #[test]
    fn validation_combined_rules_generate_multiple_checks() {
        let validation: HashMap<String, String> = [(
            "email".to_string(),
            "required|email|min:5|max:255".to_string(),
        )]
        .into_iter()
        .collect();
        let m = model_with_validation(
            "User",
            "users",
            vec![
                field("id", FieldType::EntityId, false),
                field("email", FieldType::String, false),
            ],
            validation,
        );
        let mut models = HashMap::new();
        models.insert("User".into(), m);
        let sql = generate_migrations(&Schema {
            models,
            actions: HashMap::new(),
            builtins: HashMap::new(),
        })
        .unwrap()
        .join("\n");

        // `required` should be skipped, but the other three should each produce a CHECK.
        assert!(
            !sql.contains("chk_users_email_required"),
            "required should be skipped"
        );
        assert!(
            sql.contains("chk_users_email_email"),
            "email CHECK missing:\n{sql}"
        );
        assert!(
            sql.contains("chk_users_email_min"),
            "min CHECK missing:\n{sql}"
        );
        assert!(
            sql.contains("chk_users_email_max"),
            "max CHECK missing:\n{sql}"
        );
    }

    #[test]
    fn generate_migrations_emits_model_level_constraints() {
        let mut ledger = model(
            "CreditLedger",
            "credit_ledger",
            vec![
                field("id", FieldType::EntityId, false),
                field("accountId", FieldType::String, false),
                field("idempotencyKey", FieldType::String, false),
                field("amount", FieldType::Number, false),
            ],
            vec![],
        );
        ledger.constraints = vec![
            ModelConstraint::Unique(vec!["accountId".into(), "idempotencyKey".into()]),
            ModelConstraint::Check("amount <> 0".into()),
            ModelConstraint::UniqueWhere(vec!["accountId".into()], "account_id IS NOT NULL".into()),
        ];
        let mut models = HashMap::new();
        models.insert("CreditLedger".into(), ledger);
        let sql = generate_migrations(&Schema {
            models,
            actions: HashMap::new(),
            builtins: HashMap::new(),
        })
        .unwrap()
        .join("\n");

        // Composite unique -> UNIQUE INDEX (the idempotency key).
        assert!(
            sql.contains("CREATE UNIQUE INDEX IF NOT EXISTS uq_credit_ledger_account_id_idempotency_key ON credit_ledger (account_id, idempotency_key)"),
            "composite unique index missing:\n{sql}"
        );
        // CHECK -> guarded ADD CONSTRAINT.
        assert!(
            sql.contains("ADD CONSTRAINT chk_credit_ledger_1 CHECK (amount <> 0)"),
            "check constraint missing:\n{sql}"
        );
        // Partial unique (consumer feedback #6) -> UNIQUE INDEX ... WHERE <predicate>.
        assert!(
            sql.contains("CREATE UNIQUE INDEX IF NOT EXISTS uq_credit_ledger_account_id ON credit_ledger (account_id) WHERE account_id IS NOT NULL"),
            "partial unique index missing:\n{sql}"
        );
    }

    #[test]
    fn builtin_extensions_emit_column_and_partial_unique() {
        use atomo_schema::BuiltinExtension;
        // Consumer feedback #6: UNIQUE(store_account_id) WHERE ... on the built-in
        // `users` table, declared in the schema instead of hand-written SQL.
        let mut builtins = HashMap::new();
        builtins.insert(
            "users".to_string(),
            BuiltinExtension {
                columns: HashMap::from([("storeAccountId".to_string(), "TEXT".to_string())]),
                constraints: vec![ModelConstraint::UniqueWhere(
                    vec!["storeAccountId".into()],
                    "store_account_id IS NOT NULL".into(),
                )],
            },
        );
        let schema = Schema {
            models: HashMap::new(),
            actions: HashMap::new(),
            builtins,
        };

        // Not in generate_migrations: platform tables don't exist yet at that point.
        let base = generate_migrations(&schema).unwrap().join("\n");
        assert!(
            !base.contains("users"),
            "builtins leaked into schema migrations:\n{base}"
        );

        let sql = generate_builtin_extension_migrations(&schema)
            .unwrap()
            .join("\n");
        assert!(
            sql.contains("ALTER TABLE users ADD COLUMN IF NOT EXISTS store_account_id TEXT;"),
            "extension column missing:\n{sql}"
        );
        assert!(
            sql.contains("CREATE UNIQUE INDEX IF NOT EXISTS uq_users_store_account_id ON users (store_account_id) WHERE store_account_id IS NOT NULL"),
            "partial unique on built-in table missing:\n{sql}"
        );
        // Column ALTER must precede the index that depends on it.
        assert!(
            sql.find("ADD COLUMN").unwrap() < sql.find("CREATE UNIQUE INDEX").unwrap(),
            "column must be added before its index:\n{sql}"
        );
    }

    #[test]
    fn builtin_extensions_fail_loud_on_bad_declarations() {
        use atomo_schema::BuiltinExtension;
        // Non-whitelisted table → error, not a silent ALTER on an unexpected table.
        let mut builtins = HashMap::new();
        builtins.insert(
            "audit_log".to_string(),
            BuiltinExtension {
                columns: HashMap::from([("x".to_string(), "TEXT".to_string())]),
                constraints: vec![],
            },
        );
        let schema = Schema {
            models: HashMap::new(),
            actions: HashMap::new(),
            builtins,
        };
        let err = generate_builtin_extension_migrations(&schema)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("audit_log") && err.contains("users"),
            "bad error: {err}"
        );

        // NOT NULL column → error (would break existing rows on adoption).
        let mut builtins = HashMap::new();
        builtins.insert(
            "users".to_string(),
            BuiltinExtension {
                columns: HashMap::from([("x".to_string(), "TEXT NOT NULL".to_string())]),
                constraints: vec![],
            },
        );
        let schema = Schema {
            models: HashMap::new(),
            actions: HashMap::new(),
            builtins,
        };
        let err = generate_builtin_extension_migrations(&schema)
            .unwrap_err()
            .to_string();
        assert!(err.contains("nullable"), "bad error: {err}");
    }
}
