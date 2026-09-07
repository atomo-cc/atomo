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
