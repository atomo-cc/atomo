---
title: Caching
description: Bounded process-local reads, expiration and write invalidation.
---

# Caching

Atomo caches ordinary list reads (`find_many`), point lookups (`find_unique`) and counts. Keys include the model, operation, query shape, pagination and active tenant scope. Relational `include` reads bypass the outer cache; their nested ordinary reads may cache independently. Trash reads are not cached.

## Defaults and limits

Caching is enabled by default with a 60-second absolute TTL, 10,000 entries, a 64 MiB estimated-byte budget and a 1 MiB maximum entry. Entries are reclaimed every 30 seconds and on reads/insertion. Capacity pressure evicts least-recently-accessed entries. Oversized results still return normally without being cached.

Byte accounting estimates the serialized JSON payload, key and fixed entry overhead. It is not process RSS or an exact allocator measurement. The entry count and estimated-byte budget are enforced on insertion; both global limits remain in force when a model has overrides.

| Environment variable | Default | Meaning |
| --- | --- | --- |
| `ATOMO_CACHE_ENABLED` | `true` | Bypass caching when false |
| `ATOMO_CACHE_MODE` | `strong` | `strong` or `eventual` |
| `ATOMO_CACHE_TTL_SECS` | `60` | Absolute lifetime after insertion |
| `ATOMO_CACHE_TTI_SECS` | unset | Optional maximum idle lifetime |
| `ATOMO_CACHE_CLEANUP_INTERVAL_SECS` | `30` | Reclamation interval, 1?86400 seconds |
| `ATOMO_CACHE_MAX_ENTRIES` | `10000` | Maximum global entry count |
| `ATOMO_CACHE_MAX_BYTES` | `67108864` | Maximum global estimated bytes |
| `ATOMO_CACHE_MAX_ENTRY_BYTES` | `1048576` | Maximum estimated bytes per entry |
| `ATOMO_CACHE_REFRESH_AFTER_SECS` | unset | Optional demand-driven refresh age |
| `ATOMO_CACHE_MULTI_INSTANCE` | `false` | Explicitly bypass process-local caching when true |
| `ATOMO_CACHE_MODELS` | `{}` | Model-specific policy overrides as JSON |

Booleans accept `true`/`false`. Zero TTL/TTI makes an entry immediately unusable; zero entry/byte budgets prevent admission. Invalid environment values or unknown model override names fail client/server startup.

## Model policies

```bash
# Replace these example names with models in your schema.
ATOMO_CACHE_MODELS='{"Counter":{"enabled":false},"Article":{"ttl_secs":30,"tti_secs":10,"refresh_after_secs":20,"max_entries":500,"max_bytes":4194304,"max_entry_bytes":262144}}'
```

Unspecified model fields inherit the global policy. Model budgets cannot relax global limits. A global disabled switch or multi-instance bypass cannot be overridden by a model. Omit optional idle/refresh fields to inherit them; this version has no separate per-model ?clear inherited optional value? token.

## Expiration and refresh

TTL is absolute: reading an entry updates its idle timestamp but never extends its TTL. The cleanup interval controls memory reclamation, not permission to serve expired data. Maintenance keeps weak ownership and stops retaining cache state when clients disappear; constructing a cache before entering a Tokio runtime starts maintenance on first use.

Identical plain reads coalesce using bounded lock stripes. Optional refresh is synchronous and demand-driven: a read after the refresh age waits for a fresh database result, and concurrent requests reuse the completed fill. There is no periodic database polling or stale-on-error fallback. Refresh does not extend the old value's expiration; a successful fresh database read creates a new entry.

## Consistency

- `strong` means **process-local write invalidation**. A committed client mutation advances an atomic generation before event delivery and after-hooks. Existing reads are conservatively invalidated across models, and a query started before that generation cannot refill the cache with an older result. Cancellation while waiting for the cache lock still fences old entries.
- `eventual` retains existing entries through writes until expiration, but also rejects fills that began before a write. TTL bounds the age of a cached database result, not replication lag or the age of the underlying business data.
- Direct SQL, other processes and other server instances do not participate in local invalidation. Set `ATOMO_CACHE_MULTI_INSTANCE=true` (or disable the cache) when reads must observe those writers. There is no distributed invalidation service or cross-instance strong-consistency claim.
- Cache keys partition tenants and query filters; the cache does not grant authorization. Trusted core APIs remain distinct from request-boundary RBAC.

## What does not invalidate the cache

A cached entry is only evicted by TTL/TTI expiry, capacity pressure, or — in
`strong` mode — a committed write through the **same serving process**. Writes
through this instance's REST/GraphQL/worker APIs invalidate normally. Everything
below is invisible until expiry:

- **Direct SQL** — `psql` sessions, backfill scripts, hand-edited migrations.
- **Other server instances** — a second `atomo-server` on the same database has
  its own cache (set `ATOMO_CACHE_MULTI_INSTANCE=true` to bypass).
- **`eventual` mode writes** — entries survive even local writes until TTL.

Two practical consequences for poll-driven consumers (e.g. a worker polling a
job row for status changes):

- A stale-looking read is often **not** a cache gap: a tenant-scoped write that
  matched zero rows is a no-op — the database itself is unchanged, so every
  poll returns the same value from cache *and* from the database. Check that
  the write actually landed first (the GraphQL `update` mutation returns `null`
  on zero matches).
- Queue/status models are poor cache candidates even without out-of-band
  writes: every poll is a cache miss in disguise. Prefer
  `ATOMO_CACHE_MODELS='{"Job":{"enabled":false}}'` for those models, keeping
  the cache for read-heavy reference data.

## Diagnostics and Rust configuration

Administrators can inspect counters and effective policy using [`GET /storage/diagnostics`](/api/storage). Counters include hits, misses, expiration, eviction, skipped admissions, rejected stale fills, invalidations and relational bypasses. Entries and estimated bytes reflect the current process only.

Rust applications configure `atomo::cache::CacheConfig` through `Atomo::builder().cache_config(config)` or `AtomoClient::builder().cache_config(config)`. The standalone server uses `ServerConfig.cache`. Cache lifetime is independent from [durable history and audit retention](/guide/storage-lifecycle).
