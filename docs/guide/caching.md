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
- **Invalidation:** any write to a model (create / update / delete / restore / hard-delete)
  invalidates that model's cached entries, so the next read reflects the change immediately.

No configuration is required — it's on by default. There's nothing to call; reads and writes go
through the cache automatically.

## What's not cached

- Single-record lookups (`find_unique` / `record`) are not cached today.
- The cache is per-process (no shared/distributed cache).

## See also
- [Database & Projections](/guide/database)
- [GraphQL API](/api/graphql)
