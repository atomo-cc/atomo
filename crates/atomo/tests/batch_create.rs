//! `create_many` inserts a batch in one transaction (one fsync), atomically.
//! Requires Postgres via DATABASE_URL. Run: cargo test -p atomo --test batch_create -- --ignored

use serde_json::{json, Value};
use std::collections::HashMap;

const SCHEMA: &str = "export interface Note { id: string; title: string; }\n\
     export const schema = { models: { Note: { tableName: 'batch_create_notes' } } };\n\
     export default schema;";

fn rec(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

#[tokio::test]
#[ignore]
async fn create_many_inserts_all_atomically() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let atomo = atomo::Atomo::builder()
        .schema_content(SCHEMA)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .expect("schema build + migrate");
    let c = atomo.client();
    sqlx::query("TRUNCATE batch_create_notes")
        .execute(c.db_pool())
        .await
        .ok();

    // Happy path: a batch of 5 commits together and all are returned + persisted.
    let batch: Vec<_> = (0..5)
        .map(|i| rec(&[("title", json!(format!("n{i}")))]))
        .collect();
    let out = c.create_many("Note", &batch, Some("test")).await.unwrap();
    assert_eq!(out.len(), 5, "returns all created records");
    assert!(out.iter().all(|r| r.get("id").is_some()), "each got an id");
    let all = c
        .find_many("Note", &[], &[], None, None, &[])
        .await
        .unwrap();
    assert_eq!(all.len(), 5, "all 5 persisted");

    // Atomicity: a duplicate primary key within the batch fails the whole transaction — nothing
    // from the bad batch is persisted.
    let bad = vec![
        rec(&[("id", json!("dup")), ("title", json!("a"))]),
        rec(&[("id", json!("dup")), ("title", json!("b"))]),
    ];
    assert!(
        c.create_many("Note", &bad, Some("test")).await.is_err(),
        "duplicate id in a batch must error"
    );
    let after = c
        .find_many("Note", &[], &[], None, None, &[])
        .await
        .unwrap();
    assert_eq!(after.len(), 5, "failed batch rolled back — still 5 rows");

    // Empty batch is a no-op.
    assert!(c.create_many("Note", &[], None).await.unwrap().is_empty());

    sqlx::query("TRUNCATE batch_create_notes")
        .execute(c.db_pool())
        .await
        .ok();
}

/// A batch large enough that its events exceed Postgres' 65535 bind-param ceiling (6 params/event
/// → >10 922 events). Pre-chunking this failed the event INSERT; `persist_many_in` now chunks.
#[tokio::test]
#[ignore]
async fn create_many_large_batch_chunks_event_inserts() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    // Own table — these tests share the dev DB and run in parallel, so they must not collide.
    let schema = "export interface Note { id: string; title: string; }\n\
         export const schema = { models: { Note: { tableName: 'batch_create_large_notes' } } };\n\
         export default schema;";
    let atomo = atomo::Atomo::builder()
        .schema_content(schema)
        .database_url(&url)
        .enable_migrations(true)
        .build()
        .await
        .expect("schema build + migrate");
    let c = atomo.client();
    sqlx::query("TRUNCATE batch_create_large_notes")
        .execute(c.db_pool())
        .await
        .ok();

    let n = 11_000; // 11 000 × 6 = 66 000 event params — over the single-statement limit
    let batch: Vec<_> = (0..n)
        .map(|i| rec(&[("title", json!(format!("n{i}")))]))
        .collect();
    let out = c.create_many("Note", &batch, Some("bulk")).await.unwrap();
    assert_eq!(out.len(), n, "all rows returned");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM batch_create_large_notes")
        .fetch_one(c.db_pool())
        .await
        .unwrap();
    assert_eq!(
        count, n as i64,
        "all rows persisted (event chunking worked)"
    );

    sqlx::query("TRUNCATE batch_create_large_notes")
        .execute(c.db_pool())
        .await
        .ok();
}
