//! Real-Postgres schema-evolution tests for forward migration (DB-gated). Run with a Postgres:
//!   DATABASE_URL=postgres:///atomo_test cargo test -p atomo --test schema_evolution -- --ignored --test-threads=1
//!
//! Proves the forward-migration semantics against a live database: an optional field is added as
//! nullable; a field with a declared default is added AND backfills existing rows; re-applying the
//! same migrations is idempotent; a reconciled `@unique` field's UNIQUE INDEX actually enforces
//! uniqueness; and adding a required field with no default to a POPULATED table fails with the
//! actionable message instead of an opaque NOT NULL violation (while succeeding on an empty table).

use atomo::schema::{
    generate_migrations, parse_typescript_schema, Field, FieldAttribute, FieldType, Model, Schema,
};
use std::collections::HashMap;

async fn pool() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    sqlx::PgPool::connect(&url).await.unwrap()
}

async fn apply(pool: &sqlx::PgPool, statements: Vec<String>) -> Result<(), sqlx::Error> {
    for stmt in statements {
        sqlx::query(&stmt).execute(pool).await?;
    }
    Ok(())
}

async fn drop_table(pool: &sqlx::PgPool, table: &str) {
    sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
        .execute(pool)
        .await
        .unwrap();
}

/// A `Listing` model on `table` with `id` + optional `name`, plus whatever `extra_fields` (raw DSL
/// field lines) the caller appends — used to model an old vs. evolved schema for the same table.
fn listing_schema(table: &str, extra_fields: &str) -> Schema {
    let src = format!(
        r#"
        import {{ model, text }} from '@atomo/schema'
        export const Listing = model('{table}', {{
          fields: {{
            id: text().id(),
            name: text().optional(),
            {extra_fields}
          }},
        }})
        "#
    );
    parse_typescript_schema(&src).unwrap()
}

/// A model with a `@unique` field, built programmatically: the builder DSL does not parse
/// field-level `.unique()`, so the attribute is set directly (as a typescript-interface schema or a
/// programmatic consumer would).
fn unique_schema(table: &str) -> Schema {
    let mk = |name: &str, ty: FieldType, optional: bool, attrs: Vec<FieldAttribute>| {
        (
            name.to_string(),
            Field {
                name: name.to_string(),
                field_type: ty,
                optional,
                attributes: attrs,
            },
        )
    };
    let mut fields = HashMap::new();
    fields.extend([
        mk(
            "id",
            FieldType::EntityId,
            false,
            vec![FieldAttribute::Primary],
        ),
        mk(
            "code",
            FieldType::String,
            false,
            vec![FieldAttribute::Unique],
        ),
    ]);
    let model = Model {
        name: "Uniq".to_string(),
        fields,
        access: None,
        hooks: None,
        validation: HashMap::new(),
        table_name: Some(table.to_string()),
        relationships: HashMap::new(),
        constraints: Vec::new(),
        events: Default::default(),
    };
    let mut models = HashMap::new();
    models.insert("Uniq".to_string(), model);
    Schema {
        models,
        actions: HashMap::new(),
    }
}

#[tokio::test]
#[ignore]
async fn forward_migration_adds_optional_and_defaulted_columns_and_backfills() {
    let p = pool().await;
    let table = "at4_evo";
    drop_table(&p, table).await;

    // Old schema: id + name. Create it and insert a row.
    apply(&p, generate_migrations(&listing_schema(table, "")).unwrap())
        .await
        .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {table} (id, name) VALUES ('r1', 'x')"
    ))
    .execute(&p)
    .await
    .unwrap();

    // Evolved schema: + status (declared default) + note (optional, no default).
    let new = listing_schema(
        table,
        "status: text().default('active'),\n            note: text().optional(),",
    );
    apply(&p, generate_migrations(&new).unwrap()).await.unwrap();

    // The defaulted column backfilled the existing row...
    let status: String = sqlx::query_scalar(&format!("SELECT status FROM {table} WHERE id = 'r1'"))
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(status, "active");
    // ...and the optional column is NULL for the existing row.
    let note: Option<String> =
        sqlx::query_scalar(&format!("SELECT note FROM {table} WHERE id = 'r1'"))
            .fetch_one(&p)
            .await
            .unwrap();
    assert!(note.is_none());

    // Re-applying the same migrations is idempotent.
    apply(&p, generate_migrations(&new).unwrap())
        .await
        .expect("re-applying the same migrations must be idempotent");

    drop_table(&p, table).await;
}

#[tokio::test]
#[ignore]
async fn forward_migration_required_no_default_fails_with_actionable_message_on_populated_table() {
    let p = pool().await;
    let table = "at4_evo_req";
    drop_table(&p, table).await;

    apply(&p, generate_migrations(&listing_schema(table, "")).unwrap())
        .await
        .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {table} (id, name) VALUES ('r1', 'x')"
    ))
    .execute(&p)
    .await
    .unwrap();

    // Add a required field with no default to the populated table.
    let newer = listing_schema(table, "owner: text().required(),");
    let mut err = None;
    for stmt in generate_migrations(&newer).unwrap() {
        if let Err(e) = sqlx::query(&stmt).execute(&p).await {
            err = Some(e.to_string());
        }
    }
    let err = err.expect("adding a required no-default column to a populated table must fail");
    assert!(
        err.contains("cannot add required column") && err.contains("owner"),
        "expected an actionable message, got: {err}"
    );
    // The column was NOT added (the guard prevented it).
    let exists: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT 1::bigint FROM information_schema.columns \
         WHERE table_name = '{table}' AND column_name = 'owner'"
    ))
    .fetch_optional(&p)
    .await
    .unwrap();
    assert!(exists.is_none(), "owner column must not have been added");

    drop_table(&p, table).await;
}

#[tokio::test]
#[ignore]
async fn forward_migration_required_no_default_succeeds_on_empty_table() {
    let p = pool().await;
    let table = "at4_evo_empty";
    drop_table(&p, table).await;

    // Empty table: the same required-no-default add succeeds (guard takes the empty branch).
    apply(&p, generate_migrations(&listing_schema(table, "")).unwrap())
        .await
        .unwrap();
    let newer = listing_schema(table, "owner: text().required(),");
    apply(&p, generate_migrations(&newer).unwrap())
        .await
        .expect("a required no-default add on an empty table succeeds");
    let exists: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT 1::bigint FROM information_schema.columns \
         WHERE table_name = '{table}' AND column_name = 'owner'"
    ))
    .fetch_optional(&p)
    .await
    .unwrap();
    assert!(
        exists.is_some(),
        "owner column should be added on an empty table"
    );

    drop_table(&p, table).await;
}

#[tokio::test]
#[ignore]
async fn reconciled_unique_index_enforces_uniqueness() {
    let p = pool().await;
    let table = "at4_uniq";
    drop_table(&p, table).await;

    apply(&p, generate_migrations(&unique_schema(table)).unwrap())
        .await
        .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {table} (id, code) VALUES ('r1', 'C1')"
    ))
    .execute(&p)
    .await
    .unwrap();
    let dup = sqlx::query(&format!(
        "INSERT INTO {table} (id, code) VALUES ('r2', 'C1')"
    ))
    .execute(&p)
    .await;
    assert!(
        dup.is_err(),
        "the reconciled UNIQUE INDEX must reject a duplicate code"
    );

    drop_table(&p, table).await;
}
