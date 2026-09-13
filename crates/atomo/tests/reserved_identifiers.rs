//! Postgres reserved words as field names must survive the full path:
//! migration (CREATE TABLE / ALTER), INSERT, UPDATE, SELECT-WHERE, ORDER BY.
//! Previously every identifier was emitted bare, so a field named `order`
//! syntax-errored at boot and crash-looped the server.
use atomo::query::{WhereClause, WhereOperator};
use serde_json::json;
use std::collections::HashMap;

const TABLE: &str = "reserved_ident_probes";
const SCHEMA: &str = "export interface ReservedIdentProbe { id: string; order: number; user?: string; select?: string; } export const schema = { models: { ReservedIdentProbe: { tableName: 'reserved_ident_probes' } } }; export default schema;";

#[tokio::test]
#[ignore]
async fn reserved_word_columns_migrate_and_round_trip() {
    let url = std::env::var("DATABASE_URL").unwrap();
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    // Owns exactly one base table; remove only an interrupted run of itself.
    sqlx::query(&format!("DROP TABLE IF EXISTS {TABLE}"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM event_log WHERE model_name='ReservedIdentProbe'")
        .execute(&pool)
        .await
        .unwrap();

    // Boot migration: CREATE TABLE with "order"/"user"/"select" columns.
    let app = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url(url)
        .build()
        .await
        .expect("migration must accept reserved-word columns");
    let client = app.client();

    // INSERT with reserved columns.
    let mut rec = HashMap::from([
        ("id".into(), json!("r1")),
        ("order".into(), json!(5)),
        ("user".into(), json!("u1")),
        ("select".into(), json!("s1")),
    ]);
    client
        .create("ReservedIdentProbe", &rec, &[], None)
        .await
        .unwrap();

    // SELECT WHERE on a reserved column.
    let found = client
        .find_many(
            "ReservedIdentProbe",
            &[WhereClause {
                field: "order".into(),
                operator: WhereOperator::Equals,
                value: json!(5),
            }],
            // ORDER BY on the reserved column too.
            &[("order".to_string(), atomo::query::OrderDirection::Desc)],
            None,
            None,
            &[],
        )
        .await
        .unwrap();
    assert_eq!(found.len(), 1, "WHERE \"order\" must match");

    // UPDATE SET on a reserved column.
    rec.insert("order".into(), json!(9));
    let updated = client
        .update_many(
            "ReservedIdentProbe",
            &[WhereClause {
                field: "id".into(),
                operator: WhereOperator::Equals,
                value: json!("r1"),
            }],
            &rec,
            &[],
            None,
        )
        .await
        .unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0]["order"], json!(9));

    // Cleanup.
    sqlx::query(&format!("DROP TABLE IF EXISTS {TABLE}"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM event_log WHERE model_name='ReservedIdentProbe'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM model_history_coverage WHERE model_name='ReservedIdentProbe'")
        .execute(&pool)
        .await
        .unwrap();
}
