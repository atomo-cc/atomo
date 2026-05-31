//! Phase D2: AI / pgvector conformance — embed CRM Contact notes and retrieve by vector
//! similarity. Tests the EmbeddingStore (CREATE EXTENSION vector + store + cosine search) with
//! deterministic synthetic vectors, so it needs **pgvector** but NOT an embedding provider.
//! Infra-blocked locally (no pgvector); runs in CI (Postgres is `pgvector/pgvector`).
//! Run: cargo test -p atomo --test crm_ai -- --ignored

use atomo::ai::EmbeddingStore;

const DIM: usize = 1536;

/// A 1536-d vector that is `1.0` at `axis`, small elsewhere — distinct directions so cosine
/// similarity cleanly separates them (no OpenAI / embedding provider needed).
fn vec_on_axis(axis: usize) -> Vec<f32> {
    let mut v = vec![0.01f32; DIM];
    v[axis] = 1.0;
    v
}

#[tokio::test]
#[ignore]
async fn crm_contact_notes_semantic_search() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let store = EmbeddingStore::new(pool.clone());
    // Requires the pgvector extension; init creates it + the embeddings table.
    store.init().await.expect("pgvector init (needs the `vector` extension)");

    store.delete("Contact", "c-onboarding").await.ok();
    store.delete("Contact", "c-billing").await.ok();

    // Embed two Contact "notes" pointing in distinct directions.
    store.store("Contact", "c-onboarding", "notes", "wants a product onboarding call", &vec_on_axis(0)).await.unwrap();
    store.store("Contact", "c-billing", "notes", "asked about an invoice refund", &vec_on_axis(1)).await.unwrap();

    // Query close to the onboarding note → it must rank first.
    let results = store.search_similar("Contact", &vec_on_axis(0), 2).await.expect("similarity search");
    assert!(!results.is_empty(), "expected similarity results");
    assert_eq!(results[0].entity_id, "c-onboarding", "nearest note should be the onboarding contact");
    assert!(results[0].similarity >= results.get(1).map(|r| r.similarity).unwrap_or(0.0),
        "top result must be the most similar");

    store.delete("Contact", "c-onboarding").await.ok();
    store.delete("Contact", "c-billing").await.ok();
}
