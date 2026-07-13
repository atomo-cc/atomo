//! User media uploads: metadata store + REST routes, event-sourced via the model event log.
//!
//! `POST /media` (auth) stores bytes through the configured `StorageBackend`, records metadata,
//! and emits a `Media` Created event (auto-audited). `GET /media/{id}` serves bytes by
//! unguessable uuid key (public). `DELETE /media/{id}` (auth) soft-deletes + emits Deleted.

use crate::auth::{optional_auth_middleware, AuthUser, HttpAuthService};
use crate::job_routes::optional_worker_auth_middleware;
use crate::jobs::{WorkerIdentity, WorkerTokenStore};
use crate::storage::StorageBackend;
use anyhow::Result;
use atomo::events::{EventType, ModelEvent};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Extension, Router,
};
use serde::Deserialize;
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
    /// Lets `POST /media` accept an `X-Worker-Token` in addition to a user JWT, so external workers
    /// can store generated artifacts. Verified lazily (only when the header is present).
    workers: Arc<WorkerTokenStore>,
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
        let workers = Arc::new(WorkerTokenStore::new(pool.clone()));
        Self {
            pool,
            storage,
            sender,
            max_size,
            private_reads,
            workers,
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
                checksum TEXT,
                uploaded_by TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                deleted_at TIMESTAMPTZ
            )",
        )
        .execute(&self.pool)
        .await?;
        // Pre-existing databases: add the content checksum column (idempotent).
        sqlx::query("ALTER TABLE media ADD COLUMN IF NOT EXISTS checksum TEXT")
            .execute(&self.pool)
            .await?;
        // Fast content-addressed dedup lookup (per tenant), live rows only.
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS media_checksum
             ON media (tenant_id, checksum) WHERE deleted_at IS NULL",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Store bytes + metadata + emit a `Media` Created event. Returns `(id, checksum)`.
    ///
    /// **Content-addressed dedup:** the sha256 of the bytes is recorded as `checksum`. If a live
    /// media with the same `(tenant, checksum)` already exists, its id is returned and nothing new
    /// is stored — identical re-uploads (e.g. the same reference image) cost nothing. Dedup is
    /// tenant-scoped, so it never shares bytes across tenants. Validation (size, content-type) is
    /// the caller's responsibility (done in the handler).
    pub async fn store_upload(
        &self,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
        user_id: &str,
        tenant_id: Option<&str>,
    ) -> Result<(String, String)> {
        let checksum = sha256_hex(bytes);

        // Dedup: return an existing live media with the same content for this tenant.
        // `IS NOT DISTINCT FROM` makes the tenant comparison NULL-safe (public uploads dedup too).
        if let Some(row) = sqlx::query(
            "SELECT id FROM media
             WHERE checksum = $1 AND tenant_id IS NOT DISTINCT FROM $2 AND deleted_at IS NULL
             LIMIT 1",
        )
        .bind(&checksum)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok((row.get::<String, _>("id"), checksum));
        }

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
            "INSERT INTO media (id, tenant_id, filename, content_type, size, storage_key, checksum, uploaded_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&id)
        .bind(tenant_id)
        .bind(filename)
        .bind(content_type)
        .bind(bytes.len() as i64)
        .bind(&key)
        .bind(&checksum)
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
        Ok((id, checksum))
    }

    /// Generated storage key: `{tenant}/{yyyy}/{mm}/{id}{ext}` — never the client filename
    /// (path-traversal defense), tenant-prefixed so a commit can validate ownership.
    fn storage_key(id: &str, filename: &str, tenant_id: Option<&str>) -> String {
        let tenant = tenant_id.unwrap_or("public");
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{e}"))
            .unwrap_or_default();
        let now = chrono::Utc::now();
        format!(
            "{}/{}/{}/{}{}",
            tenant,
            now.format("%Y"),
            now.format("%m"),
            id,
            ext
        )
    }

    /// Presign a direct **PUT** for a large/out-of-band upload: returns `(id, key, upload_url)`.
    /// The client PUTs bytes straight to `upload_url` (e.g. S3), then calls `commit_upload`. Returns
    /// `None` if the backend can't presign (e.g. local disk) — use the proxy `store_upload` instead.
    pub async fn presign_upload(
        &self,
        filename: &str,
        tenant_id: Option<&str>,
    ) -> Option<(String, String, String)> {
        let id = uuid::Uuid::new_v4().to_string();
        let key = Self::storage_key(&id, filename, tenant_id);
        let url = self
            .storage
            .presigned_put_url(&key, std::time::Duration::from_secs(300))
            .await?;
        Some((id, key, url))
    }

    /// Register an out-of-band (presigned) upload. Validates `key` belongs to the caller's tenant,
    /// confirms the object exists (and measures its size) via the backend, dedups on `checksum`,
    /// then records metadata + emits a `Media` Created event. Returns `(id, deduped)`.
    #[allow(clippy::too_many_arguments)]
    pub async fn commit_upload(
        &self,
        id: &str,
        key: &str,
        filename: &str,
        content_type: &str,
        checksum: Option<&str>,
        user_id: &str,
        tenant_id: Option<&str>,
    ) -> Result<(String, bool)> {
        // The key must live under the caller's tenant prefix (the presigned URL only allowed this
        // exact key; this stops a client committing someone else's object).
        let tenant = tenant_id.unwrap_or("public");
        if !key.starts_with(&format!("{tenant}/")) {
            anyhow::bail!("storage key does not belong to this tenant");
        }
        // Confirm the upload actually happened + get its true size (don't trust the client).
        let size = match self.storage.size(key).await? {
            Some(s) => s,
            None => anyhow::bail!("no object at key — upload not completed?"),
        };
        // Content-addressed dedup (when the client supplied a checksum).
        if let Some(sum) = checksum {
            if let Some(row) = sqlx::query(
                "SELECT id FROM media
                 WHERE checksum = $1 AND tenant_id IS NOT DISTINCT FROM $2 AND deleted_at IS NULL
                 LIMIT 1",
            )
            .bind(sum)
            .bind(tenant_id)
            .fetch_optional(&self.pool)
            .await?
            {
                // The freshly-uploaded duplicate object is left for GC.
                return Ok((row.get::<String, _>("id"), true));
            }
        }
        sqlx::query(
            "INSERT INTO media (id, tenant_id, filename, content_type, size, storage_key, checksum, uploaded_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(id)
        .bind(tenant_id)
        .bind(filename)
        .bind(content_type)
        .bind(size as i64)
        .bind(key)
        .bind(checksum)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        let mut extra = HashMap::new();
        extra.insert("contentType".to_string(), json!(content_type));
        extra.insert("size".to_string(), json!(size));
        self.emit(EventType::Created, id, user_id, extra);
        Ok((id.to_string(), false))
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
        // Cutoff on the DB clock: `deleted_at` is written by the database (NOW()),
        // so comparing it against an app-host timestamp is skew-sensitive — with a
        // small `older_than`, a DB clock ahead of the app host purges nothing.
        let secs = older_than.as_secs_f64();
        let rows = sqlx::query(
            "SELECT storage_key FROM media \
             WHERE deleted_at IS NOT NULL AND deleted_at < now() - make_interval(secs => $1)",
        )
        .bind(secs)
        .fetch_all(&self.pool)
        .await?;
        for r in &rows {
            self.storage
                .delete(&r.get::<String, _>("storage_key"))
                .await
                .ok();
        }
        let res = sqlx::query(
            "DELETE FROM media \
             WHERE deleted_at IS NOT NULL AND deleted_at < now() - make_interval(secs => $1)",
        )
        .bind(secs)
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
            origin: None,
        });
    }
}

/// Lowercase hex SHA-256 of the bytes — the content checksum used for dedup + integrity.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Content types we will store + serve. Excludes inline-XSS-risky types (svg, html).
pub(crate) fn content_type_allowed(ct: &str) -> bool {
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
pub(crate) fn content_matches(ct: &str, bytes: &[u8]) -> bool {
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
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresignReq {
    filename: Option<String>,
}

/// `POST /media/presign` (auth): get a presigned PUT URL for a direct upload to the backend (S3).
/// `501` when the backend can't presign (e.g. local disk) — use `POST /media` instead.
async fn presign(
    State(state): State<Arc<MediaState>>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<PresignReq>,
) -> Response {
    let user = match user {
        Some(Extension(u)) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "authentication required"})),
            )
                .into_response()
        }
    };
    let filename = req.filename.unwrap_or_else(|| "upload".to_string());
    match state
        .presign_upload(&filename, user.tenant_id.as_deref())
        .await
    {
        Some((id, key, url)) => Json(json!({
            "id": id,
            "key": key,
            "uploadUrl": url,
            "method": "PUT",
        }))
        .into_response(),
        None => (
            StatusCode::NOT_IMPLEMENTED,
            "storage backend does not support presigned uploads; use POST /media",
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitReq {
    id: String,
    key: String,
    filename: Option<String>,
    content_type: Option<String>,
    checksum: Option<String>,
}

/// `POST /media/commit` (auth): register an out-of-band (presigned) upload. The bytes were uploaded
/// directly to the backend, so content can't be magic-byte sniffed here — the declared content-type
/// allowlist still applies. Returns `{ id, url, deduped }`.
async fn commit(
    State(state): State<Arc<MediaState>>,
    user: Option<Extension<AuthUser>>,
    Json(req): Json<CommitReq>,
) -> Response {
    let user = match user {
        Some(Extension(u)) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "authentication required"})),
            )
                .into_response()
        }
    };
    let content_type = req
        .content_type
        .unwrap_or_else(|| "application/octet-stream".to_string());
    if !content_type_allowed(&content_type) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "content-type not allowed",
        )
            .into_response();
    }
    let filename = req.filename.unwrap_or_else(|| "upload".to_string());
    match state
        .commit_upload(
            &req.id,
            &req.key,
            &filename,
            &content_type,
            req.checksum.as_deref(),
            &user.id,
            user.tenant_id.as_deref(),
        )
        .await
    {
        Ok((id, deduped)) => Json(json!({
            "id": id,
            "url": format!("/media/{}", id),
            "deduped": deduped,
        }))
        .into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

pub fn media_router(state: Arc<MediaState>, auth: HttpAuthService) -> Router {
    let max = state.max_size;
    Router::new()
        .route("/media", post(upload))
        .route("/media/presign", post(presign))
        .route("/media/commit", post(commit))
        .route("/media/gc", post(gc))
        .route("/media/{id}", get(serve_media).delete(delete_media))
        // Hard backstop with headroom for multipart framing; the precise per-file limit is the
        // in-handler `bytes.len() > max_size` check, which returns a clean 413.
        .layer(DefaultBodyLimit::max(max.saturating_add(1024 * 1024)))
        .route_layer(middleware::from_fn_with_state(
            auth,
            optional_auth_middleware,
        ))
        // Also accept a worker token (non-fatal: only verifies when X-Worker-Token is present), so
        // external workers can store generated artifacts without a user session.
        .route_layer(middleware::from_fn_with_state(
            state.workers.clone(),
            optional_worker_auth_middleware,
        ))
        .with_state(state)
}

async fn upload(
    State(state): State<Arc<MediaState>>,
    user: Option<Extension<AuthUser>>,
    worker: Option<Extension<WorkerIdentity>>,
    mut multipart: Multipart,
) -> Response {
    // Accept either a user JWT (owner = that user/tenant) or a worker token (owner = the worker, no
    // tenant). One of the two must be present.
    let (owner_id, tenant_id): (String, Option<String>) = match (user, worker) {
        (Some(Extension(u)), _) => (u.id, u.tenant_id),
        (None, Some(Extension(w))) => (format!("worker:{}", w.id), None),
        (None, None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "authentication required"})),
            )
                .into_response()
        }
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
            &owner_id,
            tenant_id.as_deref(),
        )
        .await
    {
        Ok((id, checksum)) => (
            StatusCode::OK,
            Json(json!({
                "id": id,
                "url": format!("/media/{}", id),
                "contentType": content_type,
                "size": bytes.len(),
                "checksum": checksum,
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
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "authentication required"})),
            )
                .into_response()
        }
    };
    if !matches!(user.role, crate::platform_models::UserRole::Admin) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "admin access required"})),
        )
            .into_response();
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
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn serve_media(
    State(state): State<Arc<MediaState>>,
    user: Option<Extension<AuthUser>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (key, ct, tenant) = match state.lookup(&id).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    if state.private_reads {
        match &user {
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "authentication required"})),
                )
                    .into_response()
            }
            Some(Extension(u)) if u.tenant_id != tenant => {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "access denied"})),
                )
                    .into_response()
            }
            Some(_) => {}
        }
    }
    // Backends that support it (S3) redirect to a short-lived presigned URL; the client then makes
    // its `Range` request directly against S3, which honors byte ranges natively — so range
    // handling below only needs to cover the proxied-bytes path (local disk).
    if let Some(url) = state
        .storage
        .presigned_get_url(&key, std::time::Duration::from_secs(300))
        .await
    {
        return axum::response::Redirect::temporary(&url).into_response();
    }
    match state.storage.get(&key).await {
        Ok(Some(bytes)) => serve_bytes(&headers, &id, &ct, bytes, state.private_reads),
        _ => (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response(),
    }
}

/// A parsed single-range request against a body of `total` bytes.
enum RangeSpec {
    /// A satisfiable range, inclusive `[start, end]`.
    Satisfiable(u64, u64),
    /// Syntactically valid but cannot be served (e.g. start past EOF) → 416.
    Unsatisfiable,
    /// Absent or malformed `Range` header → ignore it and serve the full body (RFC 7233 §3.1).
    Ignore,
}

/// Parse a single `Range: bytes=…` value. Only one range is supported; multi-range
/// (`a-b,c-d`) and malformed specs are ignored (full body served), matching RFC 7233 guidance to
/// disregard an unparseable Range rather than fail the request.
fn parse_range(raw: &str, total: u64) -> RangeSpec {
    let spec = match raw.trim().strip_prefix("bytes=") {
        Some(s) => s.trim(),
        None => return RangeSpec::Ignore,
    };
    if spec.is_empty() || spec.contains(',') {
        return RangeSpec::Ignore; // multi-range unsupported → serve full
    }
    let (lo, hi) = match spec.split_once('-') {
        Some(p) => p,
        None => return RangeSpec::Ignore,
    };
    if total == 0 {
        return RangeSpec::Unsatisfiable;
    }
    let last = total - 1;
    match (lo.trim(), hi.trim()) {
        ("", "") => RangeSpec::Ignore,
        // Suffix range: last N bytes (`bytes=-N`).
        ("", n) => match n.parse::<u64>() {
            Ok(0) => RangeSpec::Unsatisfiable,
            Ok(n) => RangeSpec::Satisfiable(total.saturating_sub(n), last),
            Err(_) => RangeSpec::Ignore,
        },
        // Open-ended: `bytes=start-`.
        (start, "") => match start.parse::<u64>() {
            Ok(s) if s <= last => RangeSpec::Satisfiable(s, last),
            Ok(_) => RangeSpec::Unsatisfiable,
            Err(_) => RangeSpec::Ignore,
        },
        // Closed: `bytes=start-end`.
        (start, end) => match (start.parse::<u64>(), end.parse::<u64>()) {
            (Ok(s), Ok(e)) if s > e => RangeSpec::Ignore, // invalid spec
            (Ok(s), Ok(_)) if s > last => RangeSpec::Unsatisfiable,
            (Ok(s), Ok(e)) => RangeSpec::Satisfiable(s, e.min(last)),
            _ => RangeSpec::Ignore,
        },
    }
}

/// Serve `bytes` with HTTP Range support (RFC 7233) so players can seek — essential for scrubbing
/// `video/*` and `audio/*`. Media bytes are immutable per id, so a strong `ETag` (the id) plus
/// `Cache-Control` enable conditional GETs (`If-None-Match` → 304) and long client caching.
fn serve_bytes(
    headers: &HeaderMap,
    id: &str,
    content_type: &str,
    bytes: Vec<u8>,
    private: bool,
) -> Response {
    let total = bytes.len() as u64;
    let etag = format!("\"{id}\"");
    let cache_control = if private {
        "private, max-age=3600"
    } else {
        "public, max-age=86400, immutable"
    };

    // Conditional GET: client already has this (immutable) content.
    if let Some(inm) = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    {
        if inm == etag || inm.trim() == "*" {
            return Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header(header::ETAG, &etag)
                .header(header::CACHE_CONTROL, cache_control)
                .body(Body::empty())
                .unwrap();
        }
    }

    if let Some(raw) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        match parse_range(raw, total) {
            RangeSpec::Satisfiable(start, end) => {
                let slice = bytes[start as usize..=end as usize].to_vec();
                return Response::builder()
                    .status(StatusCode::PARTIAL_CONTENT)
                    .header(header::CONTENT_TYPE, content_type)
                    .header("X-Content-Type-Options", "nosniff")
                    .header(header::ACCEPT_RANGES, "bytes")
                    .header(
                        header::CONTENT_RANGE,
                        format!("bytes {start}-{end}/{total}"),
                    )
                    .header(header::ETAG, &etag)
                    .header(header::CACHE_CONTROL, cache_control)
                    .body(Body::from(slice))
                    .unwrap();
            }
            RangeSpec::Unsatisfiable => {
                return Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{total}"))
                    .header(header::ACCEPT_RANGES, "bytes")
                    .body(Body::empty())
                    .unwrap();
            }
            RangeSpec::Ignore => {} // fall through to full body
        }
    }

    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header("X-Content-Type-Options", "nosniff")
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ETAG, &etag)
        .header(header::CACHE_CONTROL, cache_control)
        .body(Body::from(bytes))
        .unwrap()
}

async fn delete_media(
    State(state): State<Arc<MediaState>>,
    user: Option<Extension<AuthUser>>,
    Path(id): Path<String>,
) -> Response {
    let user = match user {
        Some(Extension(u)) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "authentication required"})),
            )
                .into_response()
        }
    };
    match state.soft_delete(&id, &user.id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
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

    #[test]
    fn range_parsing_covers_rfc_cases() {
        use super::{parse_range, RangeSpec};
        let sat = |r: RangeSpec| match r {
            RangeSpec::Satisfiable(s, e) => Some((s, e)),
            _ => None,
        };
        // Closed range, clamped to EOF.
        assert_eq!(sat(parse_range("bytes=0-499", 1000)), Some((0, 499)));
        assert_eq!(sat(parse_range("bytes=500-100000", 1000)), Some((500, 999)));
        // Open-ended.
        assert_eq!(sat(parse_range("bytes=900-", 1000)), Some((900, 999)));
        // Suffix (last N bytes).
        assert_eq!(sat(parse_range("bytes=-200", 1000)), Some((800, 999)));
        assert_eq!(sat(parse_range("bytes=-5000", 1000)), Some((0, 999))); // suffix > total
                                                                           // Whitespace tolerated.
        assert_eq!(sat(parse_range(" bytes=0-0 ", 1000)), Some((0, 0)));

        // Unsatisfiable: start past EOF, or zero-length suffix.
        assert!(matches!(
            parse_range("bytes=1000-1001", 1000),
            RangeSpec::Unsatisfiable
        ));
        assert!(matches!(
            parse_range("bytes=-0", 1000),
            RangeSpec::Unsatisfiable
        ));
        assert!(matches!(
            parse_range("bytes=0-0", 0),
            RangeSpec::Unsatisfiable
        ));

        // Ignored (serve full): missing unit, multi-range, reversed, garbage, empty.
        assert!(matches!(parse_range("0-100", 1000), RangeSpec::Ignore));
        assert!(matches!(
            parse_range("bytes=0-10,20-30", 1000),
            RangeSpec::Ignore
        ));
        assert!(matches!(
            parse_range("bytes=500-100", 1000),
            RangeSpec::Ignore
        ));
        assert!(matches!(
            parse_range("bytes=abc-def", 1000),
            RangeSpec::Ignore
        ));
        assert!(matches!(parse_range("bytes=-", 1000), RangeSpec::Ignore));
    }
}
