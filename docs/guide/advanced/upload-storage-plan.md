---
title: 'Plan: File Upload & Storage'
description: General-purpose media upload/storage capability for the Atomo core.
---

# Plan: File Upload & Storage

## 1. Goal & scope

Add a **general-purpose media upload/storage capability** to the platform core: a server
endpoint that accepts files, a pluggable storage backend, a `Media` entity in the
event-sourced model, a schema field type so any service can declare file fields, and the
SDK/Admin UI wiring to make the existing `MediaUploader` real.

**In scope:** multipart upload, local-disk + S3-compatible backends, `Media` metadata model,
download/serve, auth + validation, tenant scoping, codegen field type, SDK method, dogfood +
conformance tests.

**Non-goals (deferred, documented):** image transforms/thumbnails, virus scanning,
resumable/chunked uploads, CDN integration.

## 2. Architecture — mirror the registry pattern

Atomo already has a precedent: the worker token registry (`WorkerTokenStore` + `job_routes` +
read-only blob dir). The upload feature follows that shape plus a write path.

### 2a. Storage abstraction (`crates/atomo_server/src/storage.rs`)

```rust
trait StorageBackend: Send + Sync {
  async fn put(&self, key: &str, bytes: Bytes, content_type: &str) -> Result<()>;
  async fn get(&self, key: &str) -> Result<Option<(Bytes, String)>>;
  async fn delete(&self, key: &str) -> Result<()>;
  fn presign_get(&self, key: &str, ttl: Duration) -> Option<String>; // S3 only
}
```

- `LocalStorage` — files under a `blob_dir` (reuse `RegistryStore`'s `tokio::fs` +
  relative-path approach). Storage key = `{tenant}/{yyyy}/{mm}/{uuid}{ext}` (never the user's
  filename — kills path traversal).
- `S3Storage` — behind a Cargo feature `storage-s3` using `aws-sdk-s3` (new dep).
- Selected via env (see §6).

### 2b. Metadata model — `MediaStore`

Idempotent DDL like `RegistryStore::init()`:

```sql
CREATE TABLE media (
  id TEXT PRIMARY KEY, tenant_id TEXT, filename TEXT, content_type TEXT,
  size BIGINT, storage_key TEXT NOT NULL, checksum TEXT,
  uploaded_by TEXT, created_at TIMESTAMPTZ DEFAULT NOW(), deleted_at TIMESTAMPTZ
);
```

Emit `MediaUploaded` / `MediaDeleted` **events** through the existing event log so media
participates in audit/history/projections — this is what makes Atomo's upload event-sourced.

### 2c. Routes (`upload_routes.rs`, wired into `create_router`)

- `POST /media` — multipart upload → store bytes + insert metadata + emit event →
  `{id, url, contentType, size}`. **Behind `auth_middleware`**.
- `GET /media/{id}` — serve bytes (local) or 302 → presigned URL (S3). Gated by read access +
  tenant scope. The local proxy path honors HTTP **Range** requests (206 / `Content-Range`, 416 for
  an unsatisfiable range) so `video`/`audio` can seek, advertises `Accept-Ranges: bytes`, and emits
  a strong `ETag` (the immutable media id) for conditional GETs (`If-None-Match` → 304). On the S3
  redirect path, S3 serves Range natively against the presigned URL.
- `POST /media/presign` — (S3 only) get a presigned **PUT** URL for a large/out-of-band upload →
  `{ id, key, uploadUrl }`. `501` when the backend can't presign (local disk). The client PUTs bytes
  **directly** to `uploadUrl` (they never pass through the server), then calls commit.
- `POST /media/commit` — register a presigned upload: `{ id, key, filename?, contentType?, checksum? }`.
  Validates `key` belongs to the caller's tenant, confirms the object exists + measures it via S3
  `HEAD`, dedups on `checksum`, records metadata + emits `Media` Created → `{ id, url, deduped }`.
- `DELETE /media/{id}` — soft-delete + `MediaDeleted` event.
- Requires axum's **`multipart`** feature + `DefaultBodyLimit` size cap.

## 3. Schema-driven integration

- **New field type** in `atomo_schema` (`typescript_parser.rs`): a `File` type mapping to a
  `TEXT` column storing the media id (soft FK to `media.id`), surfaced in `/meta/schema`.
- Added through the unified `parse_model_metadata` path (not a new brace-walk) with a parser
  unit test — the parse/codegen layer is the conformance plan's known fragile spot.
- A service declares `avatar: File` or `photos: File[]` and codegen + Admin UI handle it.

## 4. SDK + Admin UI

- **Admin UI:** `MediaUploader.tsx` already POSTs multipart to `uploadEndpoint` and expects
  `{ url }` — point it at `POST /media`, send the auth token, render the returned `url`. Wire
  the dynamic form renderer for `File`-typed fields. Fix the faked `retryUpload`.
- **TS SDK:** add `uploadMedia(file): Promise<{id,url}>` and `getMediaUrl(id)`.
- **No Dart SDK** — out of scope; document the raw `POST /media` contract.

## 5. Security (network-exposed write endpoint)

- **Auth required** on upload/delete; reads gated by model access + `tenant_id` scope.
- **Content-type allow-list + magic-byte sniff**; enforce `maxFileSize` server-side.
- **Generated storage keys only** — never use the client filename in the path.
- **Presigned, expiring URLs** for S3; private bucket.
- Rate-limit uploads (existing `middleware.rs`).
- A future "fetch-from-URL" variant = SSRF risk → host allow-list (deferred).

## 6. Config (`.env.example`)

```
STORAGE_BACKEND=local            # local | s3
STORAGE_LOCAL_DIR=./.atomo/media
STORAGE_MAX_FILE_SIZE=10485760
# S3 (when STORAGE_BACKEND=s3)
STORAGE_S3_BUCKET=...; STORAGE_S3_REGION=...; AWS_ACCESS_KEY_ID=...; AWS_SECRET_ACCESS_KEY=...
```

Constructed once at boot (like `OAuthManager::from_env()`), injected into `AppState`.

## 7. Phased delivery (each phase ends with a passing test)

- **Phase A — Local backend, happy path.** `storage.rs` trait + `LocalStorage`, `MediaStore` +
  migration, `POST /media` (auth + size + multipart) and `GET /media/{id}`. Test: upload a
  fixture, read it back, assert metadata. Enable axum `multipart`.
- **Phase B — Event-sourcing + audit.** Emit `MediaUploaded`/`MediaDeleted`; `DELETE /media/{id}`
  soft-delete. Test: upload→delete reconstructs via `entity_history`; audit records actor.
- **Phase C — Schema field type.** `File` type in the unified parser + `/meta/schema`. Test:
  parser unit test + a dogfood model field (`Contact.avatar: File`).
- **Phase D — Admin UI + SDK.** Repoint `MediaUploader`, wire `File` fields, add SDK
  `uploadMedia`. Test: extend Playwright e2e — upload on a CRM entity, assert url renders.
- **Phase E — S3 backend.** `S3Storage` behind `storage-s3` + presigned reads. Test: `#[ignore]`
  integration gated on S3 creds (MinIO in CI), mirroring the pgvector pattern.
- **Phase SEC — Hardening.** Allow-list + magic-byte check, tenant-scope read enforcement, rate
  limit. Tests: reject oversized, reject disallowed type, cross-tenant read denied.

## 8. Risks / open questions

- **Multi-file fields** (`File[]`) — ✅ parses to `Array(File)` (JSONB), renders the multi-file
  uploader.
- **GraphQL vs REST split** — uploads/serves stay **REST** (`/media`, multipart + bytes/redirect);
  GraphQL only stores/returns the media id (a TEXT `File` field), resolved to a URL by
  `apiClient.getMediaUrl`. Multipart over GraphQL is intentionally avoided.
- **Orphan cleanup** — `MediaState::purge_deleted(older_than)` GCs old **soft-deleted** rows
  (housekeeping; bytes are freed on soft-delete). Reference-based orphans (media whose referencing
  entity was deleted) are **intentionally not auto-GC'd** — needs per-schema reference tracking and
  would risk deleting in-use media. Wire `purge_deleted` to a scheduler under a retention policy.
- **Roadmap honesty** — capability marked delivered in README with scope; S3 verified via MinIO.

Smallest shippable slice with real value: **Phases A + C + D** (local storage, schema `File`
type, Admin UI wired) — uploads work end-to-end through the dogfood; S3 + full hardening as
fast-follows.

## Delivery status (honesty)

- **Phase A (local backend)** — ✅ done + tested (storage unit tests, HTTP lifecycle).
- **Phase B (event-sourcing/audit)** — ✅ done (Media Created/Deleted events; DB-gated test).
- **Phase C (schema File type)** — ✅ done (parser maps `File`/`File[]` to string-backed TEXT).
- **Phase D (Admin UI + SDK)** — ✅ done: `apiClient.uploadMedia`/`getMediaUrl` + `MediaUploader`
  posts to real `/media` with auth + real retry. A `FieldType::File` variant (TEXT-backed in all
  codegen/DB paths) makes the server emit a distinct `file` metadata type, and `FormField`
  auto-renders `MediaUploader` for it. Test: `field_type_str(File) == "file"`.
- **Phase E (S3)** — ✅ implemented behind `storage-s3` feature **and verified against MinIO**:
  `put/get/delete` roundtrip + presigned-URL read both pass. `GET /media/{id}` 302-redirects to a
  short-lived presigned URL when the backend provides one (S3); local proxies bytes.
- **Phase SEC** — ✅ magic-byte content sniffing + opt-in tenant read scoping
  (`STORAGE_PRIVATE_READS`); rate limiting is inherited from the app-level middleware.
- **Range / streaming serve** — ✅ `GET /media/{id}` supports HTTP Range (RFC 7233) on the local
  proxy path: single-range `206` with `Content-Range`, `416` for unsatisfiable ranges, `Accept-Ranges`
  on every response, and a strong `ETag` (immutable media id) enabling `If-None-Match` → `304`. This
  makes `video`/`audio` seekable. Tested by `media_http_supports_range_requests` + `range_parsing_covers_rfc_cases`.
- **Content checksum + dedup** — ✅ every upload records a sha256 `checksum` (returned in the upload
  response). Identical content for the **same tenant** dedups to the existing media id — nothing is
  re-stored (e.g. re-uploading the same reference image is free). Dedup is tenant-scoped (never
  shares bytes across tenants) and ignores soft-deleted rows. Tested by
  `media_http_dedups_identical_content_per_tenant`.
- **Presigned direct upload (S3)** — ✅ `POST /media/presign` returns a presigned **PUT** URL; the
  client uploads bytes **directly to S3** (never through the server), then `POST /media/commit`
  validates the tenant-prefixed key, confirms + measures the object via S3 `HEAD`, dedups, and
  records metadata. `presigned_put_url` + `size` on the `StorageBackend` trait (S3 = presign/HEAD,
  local = None/stat). **Verified against MinIO** (`s3_presigned_put_is_uploadable`,
  `media_presign_commit_roundtrip`). For large media a worker bypasses the server entirely. *(The
  `storage-s3` feature needs rustc ≥ 1.91 — the latest aws-sdk MSRV.)*

## Verifying the S3 backend locally (MinIO)

MinIO is a single static binary — no Docker. To run the `storage-s3` tests:

```bash
# 1. Get + launch MinIO (localhost, ephemeral creds)
curl -sSL -o /tmp/minio https://dl.min.io/server/minio/release/linux-amd64/minio && chmod +x /tmp/minio
curl -sSL -o /tmp/mc    https://dl.min.io/client/mc/release/linux-amd64/mc    && chmod +x /tmp/mc
MINIO_ROOT_USER=atomotest MINIO_ROOT_PASSWORD=atomotest123 \
  setsid bash -c '/tmp/minio server /tmp/minio-data --address 127.0.0.1:9000' &

# 2. Create the bucket
/tmp/mc alias set local http://127.0.0.1:9000 atomotest atomotest123
/tmp/mc mb --ignore-existing local/atomo-media

# 3. Run the feature-gated, #[ignore]d S3 tests
STORAGE_S3_BUCKET=atomo-media STORAGE_S3_ENDPOINT=http://127.0.0.1:9000 \
  AWS_REGION=us-east-1 AWS_ACCESS_KEY_ID=atomotest AWS_SECRET_ACCESS_KEY=atomotest123 \
  cargo test -p atomo_server --features storage-s3 --test media_s3 -- --ignored
```

CI runs the same way (MinIO service + these env vars), mirroring how the pgvector/AI tests are
gated. Tip: stop MinIO with `pkill -x minio` (not `pkill -f 'minio …'` — `-f` matches your own
command line).
