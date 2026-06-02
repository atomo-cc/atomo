//! Pluggable blob storage for user media uploads.
//!
//! Mirrors the registry's local-blob approach but behind a trait so the backend is swappable.
//! Local-disk is the only backend today; S3 is a documented next step (Phase E).

use anyhow::{bail, Result};
use std::path::PathBuf;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn delete(&self, key: &str) -> Result<()>;
    /// A presigned/redirectable GET URL when the backend supports it (S3); None = proxy bytes.
    async fn presigned_get_url(&self, _key: &str, _ttl: std::time::Duration) -> Option<String> {
        None
    }
}

/// Files under a local root directory. Keys are relative paths; `..`/absolute keys are rejected
/// (path-traversal defense) — callers always pass generated keys, never client filenames.
pub struct LocalStorage {
    root: PathBuf,
}

impl LocalStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, key: &str) -> Result<PathBuf> {
        if key.is_empty() || key.contains("..") || key.starts_with('/') {
            bail!("invalid storage key");
        }
        Ok(self.root.join(key))
    }
}

#[async_trait::async_trait]
impl StorageBackend for LocalStorage {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<()> {
        let path = self.resolve(key)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, bytes).await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let path = self.resolve(key)?;
        match tokio::fs::read(&path).await {
            Ok(b) => Ok(Some(b)),
            Err(_) => Ok(None),
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let path = self.resolve(key)?;
        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }
}

/// Build the configured storage backend from env. Local-disk by default; S3 when
/// STORAGE_BACKEND=s3 and the `storage-s3` feature is enabled.
pub async fn storage_from_env() -> Arc<dyn StorageBackend> {
    #[cfg(feature = "storage-s3")]
    {
        if std::env::var("STORAGE_BACKEND").as_deref() == Ok("s3") {
            return Arc::new(S3Storage::from_env().await);
        }
    }
    let dir = std::env::var("STORAGE_LOCAL_DIR").unwrap_or_else(|_| ".atomo/media".to_string());
    Arc::new(LocalStorage::new(dir))
}

/// S3-compatible backend (AWS S3, MinIO, etc.). Bytes are proxied through the server's
/// GET /media/{id}; presigned-redirect reads are a future optimization.
#[cfg(feature = "storage-s3")]
pub struct S3Storage {
    client: aws_sdk_s3::Client,
    bucket: String,
}

#[cfg(feature = "storage-s3")]
impl S3Storage {
    /// Reads credentials/region from the standard AWS env chain; bucket from STORAGE_S3_BUCKET.
    /// STORAGE_S3_ENDPOINT (optional) targets MinIO/other S3-compatible endpoints.
    pub async fn from_env() -> Self {
        let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        if let Ok(endpoint) = std::env::var("STORAGE_S3_ENDPOINT") {
            loader = loader.endpoint_url(endpoint);
        }
        let config = loader.load().await;
        let client = aws_sdk_s3::Client::new(&config);
        let bucket = std::env::var("STORAGE_S3_BUCKET").expect("STORAGE_S3_BUCKET required for s3");
        Self { client, bucket }
    }
}

#[cfg(feature = "storage-s3")]
#[async_trait::async_trait]
impl StorageBackend for S3Storage {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(aws_sdk_s3::primitives::ByteStream::from(bytes.to_vec()))
            .send()
            .await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match self.client.get_object().bucket(&self.bucket).key(key).send().await {
            Ok(out) => {
                let data = out.body.collect().await?.into_bytes().to_vec();
                Ok(Some(data))
            }
            Err(_) => Ok(None),
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        Ok(())
    }

    async fn presigned_get_url(&self, key: &str, ttl: std::time::Duration) -> Option<String> {
        let cfg = aws_sdk_s3::presigning::PresigningConfig::expires_in(ttl).ok()?;
        let req = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(cfg)
            .await
            .ok()?;
        Some(req.uri().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_get_delete_roundtrip() {
        let dir = std::env::temp_dir().join(format!("atomo-storage-{}", uuid::Uuid::new_v4()));
        let store = LocalStorage::new(&dir);
        let key = "t/2026/06/abc.bin";
        store.put(key, b"hello").await.unwrap();
        assert_eq!(store.get(key).await.unwrap().as_deref(), Some(&b"hello"[..]));
        store.delete(key).await.unwrap();
        assert!(store.get(key).await.unwrap().is_none());
        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn rejects_path_traversal() {
        let store = LocalStorage::new(std::env::temp_dir());
        assert!(store.put("../escape", b"x").await.is_err());
        assert!(store.get("/etc/passwd").await.is_err());
    }
}
