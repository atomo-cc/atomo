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
    assert_eq!(store.get(&key).await.unwrap().as_deref(), Some(&b"S3DATA"[..]));
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
