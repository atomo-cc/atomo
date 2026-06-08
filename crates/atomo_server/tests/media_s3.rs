//! S3 storage backend roundtrip (feature- + infra-gated, like the pgvector/AI tests).
//! Needs the `storage-s3` feature AND a reachable S3/MinIO with STORAGE_S3_BUCKET (+ creds/region).
//! Run: STORAGE_S3_BUCKET=test STORAGE_S3_ENDPOINT=http://localhost:9000 AWS_ACCESS_KEY_ID=... \
//!   AWS_SECRET_ACCESS_KEY=... AWS_REGION=us-east-1 \
//!   cargo test -p atomo_server --features storage-s3 --test media_s3 -- --ignored
#![cfg(feature = "storage-s3")]

use atomo_server::storage::{S3Storage, StorageBackend};

#[tokio::test]
#[ignore]
async fn s3_put_get_delete_roundtrip() {
    let store = S3Storage::from_env().await;
    let key = format!("test/{}.bin", uuid::Uuid::new_v4());
    store.put(&key, b"S3DATA").await.unwrap();
    assert_eq!(
        store.get(&key).await.unwrap().as_deref(),
        Some(&b"S3DATA"[..])
    );
    store.delete(&key).await.unwrap();
    assert!(store.get(&key).await.unwrap().is_none());
}

#[tokio::test]
#[ignore]
async fn s3_presigned_url_is_fetchable() {
    use std::time::Duration;
    let store = S3Storage::from_env().await;
    let key = format!("test/{}.bin", uuid::Uuid::new_v4());
    store.put(&key, b"PRESIGNED").await.unwrap();
    let url = store
        .presigned_get_url(&key, Duration::from_secs(60))
        .await
        .expect("S3 backend returns a presigned URL");
    // The URL is directly fetchable without credentials (the signature authorizes it).
    let body = reqwest::get(&url).await.unwrap().bytes().await.unwrap();
    assert_eq!(body.as_ref(), b"PRESIGNED");
    store.delete(&key).await.unwrap();
}

#[tokio::test]
#[ignore]
async fn s3_presigned_put_is_uploadable() {
    use std::time::Duration;
    let store = S3Storage::from_env().await;
    let key = format!("test/{}.bin", uuid::Uuid::new_v4());
    let url = store
        .presigned_put_url(&key, Duration::from_secs(60))
        .await
        .expect("S3 backend returns a presigned PUT URL");
    // A client uploads directly to the URL — no credentials, the signature authorizes the PUT.
    let body = b"DIRECT-UPLOAD".to_vec();
    let resp = reqwest::Client::new()
        .put(&url)
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "PUT status {}", resp.status());
    assert_eq!(store.get(&key).await.unwrap().as_deref(), Some(&body[..]));
    assert_eq!(store.size(&key).await.unwrap(), Some(body.len() as u64));
    store.delete(&key).await.unwrap();
}

// Full presigned-upload flow through MediaState: presign → direct PUT → commit → read back.
// Needs both DATABASE_URL and the S3/MinIO env.
#[tokio::test]
#[ignore]
async fn media_presign_commit_roundtrip() {
    use atomo_server::media::MediaState;
    use std::sync::Arc;

    let db = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let pool = sqlx::PgPool::connect(&db).await.unwrap();
    let (tx, _rx) = tokio::sync::broadcast::channel(8);
    let state = Arc::new(MediaState::new(
        pool.clone(),
        Arc::new(S3Storage::from_env().await),
        tx,
    ));
    state.init().await.unwrap();

    let (id, key, upload_url) = state
        .presign_upload("clip.mp4", Some("t-presign"))
        .await
        .expect("S3 backend presigns uploads");

    // The worker PUTs bytes straight to S3 — they never pass through the server.
    let body = format!("mp4-bytes-{}", uuid::Uuid::new_v4()).into_bytes();
    let resp = reqwest::Client::new()
        .put(&upload_url)
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "presigned PUT: {}",
        resp.status()
    );

    // Commit registers metadata (size measured via S3 HEAD).
    let (committed, deduped) = state
        .commit_upload(
            &id,
            &key,
            "clip.mp4",
            "video/mp4",
            None,
            "user-1",
            Some("t-presign"),
        )
        .await
        .unwrap();
    assert_eq!(committed, id);
    assert!(!deduped);

    // The committed media reads back the uploaded bytes.
    let (got, ct, tenant) = state.read(&id).await.unwrap().unwrap();
    assert_eq!(got, body);
    assert_eq!(ct, "video/mp4");
    assert_eq!(tenant.as_deref(), Some("t-presign"));

    // Committing a key outside the caller's tenant prefix is rejected.
    assert!(state
        .commit_upload(
            &id,
            "other-tenant/x",
            "f",
            "video/mp4",
            None,
            "u",
            Some("t-presign")
        )
        .await
        .is_err());

    state.soft_delete(&id, "user-1").await.ok();
}
