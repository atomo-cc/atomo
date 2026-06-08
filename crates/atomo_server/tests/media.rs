//! Media store integration test (DB-gated). Run with a Postgres:
//!   DATABASE_URL=postgres:///atomo_test cargo test -p atomo_server --test media -- --ignored
//! Exercises the upload core (store -> event -> read -> soft-delete) against a real pool.

use atomo_server::media::MediaState;
use atomo_server::storage::LocalStorage;
use std::sync::Arc;

#[tokio::test]
#[ignore]
async fn upload_read_delete_roundtrip_and_events() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for --ignored test");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let dir = std::env::temp_dir().join(format!("atomo-media-it-{}", uuid::Uuid::new_v4()));
    let (tx, mut rx) = tokio::sync::broadcast::channel(16);
    let state = Arc::new(MediaState::new(
        pool.clone(),
        Arc::new(LocalStorage::new(&dir)),
        tx,
    ));
    state.init().await.unwrap();

    // upload — unique bytes per run so content-addressed dedup never reuses a stale row.
    let data = format!("PNGDATA-{}", uuid::Uuid::new_v4()).into_bytes();
    let (id, checksum) = state
        .store_upload("photo.png", "image/png", &data, "user-1", Some("t1"))
        .await
        .unwrap();
    assert_eq!(checksum.len(), 64, "sha256 content checksum returned");

    // a Media Created event was emitted (auto-audited by the boot listener in the running server)
    let ev = rx.recv().await.unwrap();
    assert_eq!(ev.model_name, "Media");
    assert_eq!(ev.actor.as_deref(), Some("user-1"));

    // read back
    let (bytes, ct, tenant) = state.read(&id).await.unwrap().unwrap();
    assert_eq!(bytes, data);
    assert_eq!(ct, "image/png");
    assert_eq!(tenant.as_deref(), Some("t1"));

    // soft-delete removes it from reads
    assert!(state.soft_delete(&id, "user-1").await.unwrap());
    assert!(state.read(&id).await.unwrap().is_none());
    assert!(!state.soft_delete(&id, "user-1").await.unwrap()); // idempotent

    // GC purges the soft-deleted row; second pass is a no-op
    assert!(
        state
            .purge_deleted(std::time::Duration::ZERO)
            .await
            .unwrap()
            >= 1
    );
    assert_eq!(
        state
            .purge_deleted(std::time::Duration::ZERO)
            .await
            .unwrap(),
        0
    );

    tokio::fs::remove_dir_all(&dir).await.ok();
}
