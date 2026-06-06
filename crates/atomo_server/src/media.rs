//! User media uploads: metadata store + REST routes, event-sourced via the model event log.
//!
//! `POST /media` (auth) stores bytes through the configured `StorageBackend`, records metadata,
//! and emits a `Media` Created event (auto-audited). `GET /media/{id}` serves bytes by
//! unguessable uuid key (public). `DELETE /media/{id}` (auth) soft-deletes + emits Deleted.

use crate::auth::{optional_auth_middleware, AuthUser, HttpAuthService};
use crate::storage::StorageBackend;
use anyhow::Result;
use atomo::events::{EventType, ModelEvent};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, StatusCode},
    middleware,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Extension, Router,
};
use serde_json::json;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

const DEFAULT_MAX_SIZE: usize = 10 * 1024 * 1024; // 10 MiB

pub struct MediaState {
    pool: PgPool,
    storage: Arc<dyn StorageBackend>,
    sender: broadcast::Sender<ModelEvent>,
    max_size: usize,
    /// When true, GET /media/{id} requires auth and the caller's tenant must match the media's
    /// (opt-in via STORAGE_PRIVATE_READS). Default false = public-by-unguessable-key.
    private_reads: bool,
}

impl MediaState {
    pub fn new(
        pool: PgPool,
        storage: Arc<dyn StorageBackend>,
        sender: broadcast::Sender<ModelEvent>,
    ) -> Self {
        let max_size = std::env::var("STORAGE_MAX_FILE_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_SIZE);
        let private_reads = std::env::var("STORAGE_PRIVATE_READS")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        Self {
            pool,
            storage,
            sender,
            max_size,
            private_reads,
        }
    }

    /// Idempotent metadata table.
    pub async fn init(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS media (
                id TEXT PRIMARY KEY,
                tenant_id TEXT,
                filename TEXT NOT NULL DEFAULT '',
                content_type TEXT NOT NULL DEFAULT 'application/octet-stream',
                size BIGINT NOT NULL DEFAULT 0,
                storage_key TEXT NOT NULL,
                uploaded_by TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                deleted_at TIMESTAMPTZ
            )",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Store bytes + metadata + emit a `Media` Created event. Returns the new media id.
    /// Validation (size, content-type) is the caller's responsibility (done in the handler).
    pub async fn store_upload(
        &self,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
        user_id: &str,
        tenant_id: Option<&str>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let tenant = tenant_id.unwrap_or("public");
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e))
            .unwrap_or_default();
        let now = chrono::Utc::now();
        // Generated key — never the client filename (path-traversal defense).
        let key = format!(
            "{}/{}/{}/{}{}",
            tenant,
            now.format("%Y"),
            now.format("%m"),
            id,
            ext
        );
        self.storage.put(&key, bytes).await?;
        let insert = sqlx::query(
            "INSERT INTO media (id, tenant_id, filename, content_type, size, storage_key, uploaded_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(&id)
        .bind(tenant_id)
        .bind(filename)
        .bind(content_type)
        .bind(bytes.len() as i64)
        .bind(&key)
        .bind(user_id)
        .execute(&self.pool)
        .await;
        if let Err(e) = insert {
            self.storage.delete(&key).await.ok();
            return Err(e.into());
        }
        let mut extra = HashMap::new();
        extra.insert("contentType".to_string(), json!(content_type));
        extra.insert("size".to_string(), json!(bytes.len()));
        self.emit(EventType::Created, &id, user_id, extra);
        Ok(id)
    }

    /// Look up a non-deleted media's storage key + content type + tenant (no bytes).
    pub async fn lookup(&self, id: &str) -> Result<Option<(String, String, Option<String>)>> {
        let row = sqlx::query(
            "SELECT storage_key, content_type, tenant_id FROM media WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            (
                r.get::<String, _>("storage_key"),
                r.get::<String, _>("content_type"),
                r.get::<Option<String>, _>("tenant_id"),
            )
        }))
    }

    /// Read a non-deleted media's bytes + content type + owning tenant, or None.
    pub async fn read(&self, id: &str) -> Result<Option<(Vec<u8>, String, Option<String>)>> {
        let (key, ct, tenant) = match self.lookup(id).await? {
            Some(m) => m,
            None => return Ok(None),
        };
        match self.storage.get(&key).await? {
            Some(bytes) => Ok(Some((bytes, ct, tenant))),
            None => Ok(None),
        }
    }

    /// Soft-delete + emit a `Media` Deleted event. Returns false if unknown/already deleted.
    pub async fn soft_delete(&self, id: &str, actor: &str) -> Result<bool> {
        let row = sqlx::query(
            "UPDATE media SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL RETURNING storage_key",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(r) => {
                let key: String = r.get("storage_key");
                self.storage.delete(&key).await.ok();
                self.emit(EventType::Deleted, id, actor, HashMap::new());
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// GC: hard-delete soft-deleted media older than `older_than` (best-effort byte delete too).
    /// Housekeeping only — does NOT detect media orphaned by a deleted referencing entity (that
    /// needs per-schema reference tracking and is intentionally out of scope to avoid data loss).
    pub async fn purge_deleted(&self, older_than: std::time::Duration) -> Result<u64> {
        let cutoff = chrono::Utc::now()
            - chrono::Duration::from_std(older_than).unwrap_or_else(|_| chrono::Duration::zero());
        let rows = sqlx::query(
            "SELECT storage_key FROM media WHERE deleted_at IS NOT NULL AND deleted_at < $1",
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await?;
        for r in &rows {
            self.storage
                .delete(&r.get::<String, _>("storage_key"))
                .await
                .ok();
        }
        let res = sqlx::query("DELETE FROM media WHERE deleted_at IS NOT NULL AND deleted_at < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    fn emit(
        &self,
        event_type: EventType,
        id: &str,
        actor: &str,
        extra: HashMap<String, serde_json::Value>,
    ) {
        let mut data = extra;
        data.insert("id".to_string(), json!(id));
        let _ = self.sender.send(ModelEvent {
            event_type,
            model_name: "Media".to_string(),
            data,
            previous_data: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            event_id: uuid::Uuid::new_v4().to_string(),
            actor: Some(actor.to_string()),
        });
    }
}

/// Content types we will store + serve. Excludes inline-XSS-risky types (svg, html).
fn content_type_allowed(ct: &str) -> bool {
    if ct == "image/svg+xml" || ct.starts_with("text/html") {
        return false;
    }
    const ALLOWED: &[&str] = &[
        "image/",
        "video/",
        "audio/",
        "application/pdf",
        "text/plain",
        "application/json",
        "application/zip",
        "application/octet-stream",
    ];
    ALLOWED.iter().any(|a| ct.starts_with(a))
}

/// Defense-in-depth: for types with a well-known file signature, the declared content-type must
/// match the actual leading bytes (blocks e.g. an executable/HTML declared as image/png). Types
/// without a reliable signature (text/*, json, octet-stream, most audio) are not sniffed.
fn content_matches(ct: &str, bytes: &[u8]) -> bool {
    let starts = |sig: &[u8]| bytes.len() >= sig.len() && &bytes[..sig.len()] == sig;
    match ct {
        "image/png" => starts(&[0x89, 0x50, 0x4E, 0x47]),
        "image/jpeg" => starts(&[0xFF, 0xD8, 0xFF]),
        "image/gif" => starts(b"GIF8"),
        "image/webp" => bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        "application/pdf" => starts(b"%PDF"),
        "application/zip" => starts(&[0x50, 0x4B, 0x03, 0x04]),
        "video/mp4" => bytes.len() >= 8 && &bytes[4..8] == b"ftyp",
        _ => true, // no known signature for this type — accept
    }
}

/// POST /media (auth) and GET/DELETE /media/{id}. Upload/delete require a token (enforced in
/// the handler after optional_auth injects the user); GET is public via unguessable key.
pub fn media_router(state: Arc<MediaState>, auth: HttpAuthService) -> Router {
    let max = state.max_size;
    Router::new()
        .route("/media", post(upload))
        .route("/media/gc", post(gc))
        .route("/media/{id}", get(serve_media).delete(delete_media))
        // Hard backstop with headroom for multipart framing; the precise per-file limit is the
        // in-handler `bytes.len() > max_size` check, which returns a clean 413.
        .layer(DefaultBodyLimit::max(max.saturating_add(1024 * 1024)))
        .route_layer(middleware::from_fn_with_state(
            auth,
            optional_auth_middleware,
        ))
        .with_state(state)
}

async fn upload(
    State(state): State<Arc<MediaState>>,
    user: Option<Extension<AuthUser>>,
    mut multipart: Multipart,
) -> Response {
    let user = match user {
        Some(Extension(u)) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let field = match multipart.next_field().await {
        Ok(Some(f)) => f,
        _ => return (StatusCode::BAD_REQUEST, "missing file field").into_response(),
    };
    let filename = field.file_name().unwrap_or("upload").to_string();
    let content_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();
    let bytes = match field.bytes().await {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, "could not read file").into_response(),
    };
    if bytes.len() > state.max_size {
        return (StatusCode::PAYLOAD_TOO_LARGE, "file too large").into_response();
    }
    if !content_type_allowed(&content_type) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "content-type not allowed",
        )
            .into_response();
    }
    if !content_matches(&content_type, bytes.as_ref()) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "content does not match declared type",
        )
            .into_response();
    }

    match state
        .store_upload(
            &filename,
            &content_type,
            bytes.as_ref(),
            &user.id,
            user.tenant_id.as_deref(),
        )
        .await
    {
        Ok(id) => (
            StatusCode::OK,
            Json(json!({
                "id": id,
                "url": format!("/media/{}", id),
                "contentType": content_type,
                "size": bytes.len(),
            })),
        )
            .into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "upload failed").into_response(),
    }
}

async fn gc(
    State(state): State<Arc<MediaState>>,
    user: Option<Extension<AuthUser>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let user = match user {
        Some(Extension(u)) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    if !matches!(user.role, crate::platform_models::UserRole::Admin) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let secs = params
        .get("older_than_secs")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(7 * 24 * 3600); // default: 7-day retention
    match state
        .purge_deleted(std::time::Duration::from_secs(secs))
        .await
    {
        Ok(n) => (StatusCode::OK, Json(json!({ "purged": n }))).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn serve_media(
    State(state): State<Arc<MediaState>>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<String>,
) -> Response {
    let (key, ct, tenant) = match state.lookup(&id).await {
        Ok(Some(m)) => m,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if state.private_reads {
        match &user {
            None => return StatusCode::UNAUTHORIZED.into_response(),
            Some(Extension(u)) if u.tenant_id != tenant => {
                return StatusCode::FORBIDDEN.into_response()
            }
            Some(_) => {}
        }
    }
    // Backends that support it (S3) redirect to a short-lived presigned URL; others proxy bytes.
    if let Some(url) = state
        .storage
        .presigned_get_url(&key, std::time::Duration::from_secs(300))
        .await
    {
        return axum::response::Redirect::temporary(&url).into_response();
    }
    match state.storage.get(&key).await {
        Ok(Some(bytes)) => Response::builder()
            .header(header::CONTENT_TYPE, ct)
            .header("X-Content-Type-Options", "nosniff")
            .body(Body::from(bytes))
            .unwrap(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn delete_media(
    State(state): State<Arc<MediaState>>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<String>,
) -> Response {
    let user = match user {
        Some(Extension(u)) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    match state.soft_delete(&id, &user.id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::content_type_allowed;

    #[test]
    fn allowlist_blocks_xss_risky_and_allows_common() {
        assert!(content_type_allowed("image/png"));
        assert!(content_type_allowed("application/pdf"));
        assert!(!content_type_allowed("image/svg+xml"));
        assert!(!content_type_allowed("text/html"));
        assert!(!content_type_allowed("application/x-msdownload"));
    }

    #[test]
    fn magic_bytes_must_match_declared_type() {
        use super::content_matches;
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert!(content_matches("image/png", &png));
        assert!(!content_matches("image/png", b"<html>")); // spoofed
        assert!(content_matches("application/pdf", b"%PDF-1.7"));
        assert!(!content_matches("application/pdf", b"nope"));
        // types without a known signature are accepted
        assert!(content_matches("text/plain", b"anything"));
        assert!(content_matches("application/octet-stream", b"\x00\x01"));
    }
}
