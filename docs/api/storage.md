# Storage diagnostics (REST)

## `GET /storage/diagnostics`

Read-only, authenticated **Admin** endpoint. It reports the local cache and database history/audit usage without returning record payloads, tokens or credentials.

```http
GET /storage/diagnostics
Authorization: Bearer <admin-jwt>
```

| Field | Contents |
| --- | --- |
| `cache.config` | Global cache policy and model overrides |
| `cache.effective_enabled` | False for disabled or multi-instance bypass |
| `cache.consistency` | `bypass`, `process-local-write-invalidation`, or `ttl-bounded-eventual` |
| `cache.metrics` | Hits/misses, expiration/eviction, skipped entries, stale-fill rejection, invalidations, relational bypasses, current entries and estimated bytes |
| `history.config` | Full/off/retained configuration |
| `history.events`, `history.estimated_bytes` | Current model-event count and estimated row bytes |
| `history.coverage` | Per-model `model`, `mode`, `complete`, `first_gap_at`, `retained_after`, `removed_events` |
| `audit.config` | Full/metadata/off and retention configuration |
| `audit.rows`, `audit.estimated_bytes` | Current audit row count and estimated row bytes |
| `capacity_semantics` | Reminder that estimates exclude separate database/allocator costs |

Successful responses return 200. Missing/invalid authentication returns 401; an authenticated non-admin returns 403; unavailable history/audit diagnostics return 503. Database usage aggregation can scan history/audit tables; poll deliberately, not per application request. Cache statistics apply only to the serving process and reset when it restarts.

The endpoint has no mutation, deletion, retention-trigger or coverage-reset operation. See [Storage lifecycle](/guide/storage-lifecycle), [Caching](/guide/caching) and [Audit API](/api/audit).


### Trusted tenant upload context and private metadata

`POST /media` accepts `x-tenant-id` from an authenticated unbound administrator.
A tenant-bound user can supply only their own tenant; a conflicting header returns
403. An unbound non-administrator cannot select a tenant. A missing header keeps
existing user binding (including legacy NULL uploads). Worker tokens cannot select
an arbitrary tenant. Explicit tenant IDs must be 1?128 ASCII letters, digits,
underscores or hyphens; malformed or repeated headers return 400. Deduplication
continues to use tenant plus byte checksum: uploading the same bytes in a selected
tenant does not adopt or reassign an existing NULL/other-tenant media row.

`GET /media/{id}/metadata` requires user authentication and a resolved tenant using
the same rules. Returns `{id, tenantId, checksum, size, contentType}` directly from
live metadata. `checksum` is SHA-256 hex, or null for older unverified records.
Missing, deleted, NULL-owned and foreign-tenant media return 404 under a selected
tenant. Missing authentication returns 401; no resolved tenant returns 403.
Storage paths and uploader identifiers are not exposed. Public byte serving is
unchanged; knowing a public media URL does not grant access to metadata.

This change covers multipart upload; presigned upload/commit retain their existing
user-bound context. Existing NULL media must be reuploaded through authenticated
multipart with explicit context to obtain a tenant-owned copy, never reassigned.
