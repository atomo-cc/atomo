---
title: Caching
description: Built-in read cache with TTL and event-driven invalidation.
---

# Caching

Atomo wraps list reads (`find_many` / `records` / `paginatedRecords`) in an in-memory read cache
so repeated queries don't re-hit the database.

## How it works

- **Populate:** a list query caches its result keyed by model + where-clauses + orderBy + limit +
  offset (pagination is part of the key, so page 1 and page 2 never collide).
- **TTL:** entries expire after a fixed window (default 60s).
- **Invalidation:** in the default **strong** mode, any write to a model (create / update / delete /
  restore / hard-delete) invalidates that model's cached entries, so the next read reflects the
  change immediately.

No configuration is required — it's on by default. There's nothing to call; reads and writes go
through the cache automatically.

## Consistency mode (`ATOMO_CACHE_MODE`)

```bash
ATOMO_CACHE_MODE=strong     # default
ATOMO_CACHE_MODE=eventual   # opt-in
ATOMO_CACHE_TTL_SECS=60     # in eventual mode, the max read staleness
```

- **`strong`** (default): a write evicts the model's cached reads immediately — reads never reflect
  state older than the last write. Correct, but a write-heavy + read-heavy workload churns the cache,
  so reads keep falling back to the database.
- **`eventual`**: writes **do not** evict — cached reads are served until the TTL expires. Reads can
  be stale up to `ATOMO_CACHE_TTL_SECS`, but the cache stays **hot through writes**, so a mixed
  read/write workload hits the cache far more often (cache hits are ~µs vs a ~hundreds-of-µs database
  read). Choose this when your app can tolerate a few seconds of read staleness in exchange for much
  higher read throughput — and keep the TTL short to bound that staleness.

> Note: even in `strong` mode the cache is per-process. Across multiple instances it is already
> effectively eventual (an instance doesn't see another's writes until its own TTL lapses).

## What's not cached

- Single-record lookups (`find_unique` / `record`) are not cached today.
- The cache is per-process (no shared/distributed cache).

## See also
- [Database & Projections](/guide/database)
- [GraphQL API](/api/graphql)
