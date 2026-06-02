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

Atomo already has a precedent: the plugin registry (`RegistryStore` + `registry_routes` +
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
  tenant scope.
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

- **Multi-file fields** (`File[]`) — store as JSON array of media ids; reuse the array-column
  codegen.
- **GraphQL vs REST split** — uploads stay REST (multipart); GraphQL stores/returns only the
  media id + a resolved URL.
- **Orphan cleanup** — media referenced by deleted entities; defer a GC job.
- **Roadmap honesty** — keep the capability `[~]` in README/roadmap until Phase D+SEC land with
  green tests.

Smallest shippable slice with real value: **Phases A + C + D** (local storage, schema `File`
type, Admin UI wired) — uploads work end-to-end through the dogfood; S3 + full hardening as
fast-follows.

## Delivery status (honesty)

- **Phase A (local backend)** — ✅ done + tested (storage unit tests, HTTP lifecycle).
- **Phase B (event-sourcing/audit)** — ✅ done (Media Created/Deleted events; DB-gated test).
- **Phase C (schema File type)** — ✅ done (parser maps `File`/`File[]` to string-backed TEXT).
- **Phase D (Admin UI + SDK)** — 🟡 mostly: `apiClient.uploadMedia`/`getMediaUrl` + `MediaUploader`
  posts to real `/media` with auth + real retry (done). `FormField` renders `MediaUploader` for
  `file`-typed metadata (client done). **Remaining:** the server emits `string` (not `file`) for
  `File` fields because the parser collapses `File`→`String`; emitting a distinct `file` metadata
  type needs a `FieldType::File` variant across the codegen match-sites — deferred (fragile layer +
  rebuild). Until then a `File` field renders as a text input; the uploader is available via the
  existing `ui.component = 'media-uploader'` config and `apiClient.uploadMedia`.
- **Phase E (S3)** — ✅ implemented behind `storage-s3` feature; lib compiles with the feature;
  runtime test is `#[ignore]` (MinIO/S3, CI-only like pgvector).
- **Phase SEC** — ✅ magic-byte content sniffing + opt-in tenant read scoping
  (`STORAGE_PRIVATE_READS`); rate limiting is inherited from the app-level middleware.
