//! Phase B2: CQRS projection correctness. Proves the two silent-corruption fixes:
//! (1) numeric/non-string fields are stored as their real value (was "" via as_str()),
//! (2) a Deleted event actually removes the projection row (delete events now carry the id).
//! Requires Postgres via DATABASE_URL.
//! Run: cargo test -p atomo_projectors --test projection_correctness -- --ignored

use std::collections::HashMap;

use atomo_projectors::{Projection, TableProjection};
use serde_json::{json, Value};

fn rec(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

#[tokio::test]
#[ignore]
async fn projection_stores_numeric_and_removes_on_delete() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS deals_projection").execute(&pool).await.ok();
    sqlx::query("CREATE TABLE deals_projection (id TEXT PRIMARY KEY, title TEXT, value TEXT)")
        .execute(&pool).await.unwrap();

    let proj = TableProjection::new(
        "Deal",
        "deals_projection",
        vec!["id".into(), "title".into(), "value".into()],
    );

    // Created with a NUMERIC value — must be stored as "50000", not "" (the old as_str() bug).
    proj.handle_event(
        "Created",
        &rec(&[("id", json!("d1")), ("title", json!("Acme")), ("value", json!(50000))]),
        &pool,
    ).await.unwrap();

    let stored: (String,) = sqlx::query_as("SELECT value FROM deals_projection WHERE id = 'd1'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(stored.0, "50000", "numeric field must be stored, not empty string");

    // Deleted carrying the id — must remove the projection row.
    proj.handle_event("Deleted", &rec(&[("id", json!("d1"))]), &pool).await.unwrap();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM deals_projection WHERE id = 'd1'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(count.0, 0, "Deleted event must remove the projection row");

    sqlx::query("DROP TABLE IF EXISTS deals_projection").execute(&pool).await.ok();
}
