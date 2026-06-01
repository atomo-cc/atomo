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

/// Build the configured storage backend from env. Local-only for now (S3 = Phase E).
pub fn storage_from_env() -> Arc<dyn StorageBackend> {
    let dir = std::env::var("STORAGE_LOCAL_DIR").unwrap_or_else(|_| ".atomo/media".to_string());
    Arc::new(LocalStorage::new(dir))
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
