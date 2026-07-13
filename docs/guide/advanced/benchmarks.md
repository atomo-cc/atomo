---
title: Benchmarks
description: Reproducible engine-level benchmarks for Atomo's core paths — data layer and durable job lease engine.
---

# Benchmarks

These are **honest, reproducible, engine-level** numbers. They measure the cost of Atomo's core
operations **in-process** — the data layer and the durable job lease engine —
and deliberately **exclude** HTTP framing, the network, and GraphQL resolution. They answer *"what
does each core operation cost?"*, not *"requests/sec through the full stack."*

It also includes a **co-located head-to-head vs Node** — both at the data layer (`node-postgres`) and
at the **HTTP request layer** (Atomo's axum server vs Fastify under `k6`). Bottom line up front: the
roadmap's "3–5× faster than Node" line is **not supported on either layer** — Atomo is comparable-to-
slower on raw throughput because it does more out of the box. It stays a **target**. Atomo's edge is
footprint, hot-cache reads, and built-in capabilities, not raw speed (see Results).

## Running it

Atomo harness (release, gated on a real Postgres):

```bash
DATABASE_URL=postgres://… \
  cargo run --release -p atomo_server --example bench   # BENCH_ITERS=5000 to override (default 2000)
```

Action overhead harness (measures incremental cost of the event-to-action pipeline):

```bash
DATABASE_URL=postgres://… \
  cargo run --release -p atomo_server --example action_overhead   # BENCH_ITERS=500 to override
```

Node baseline (`node-postgres`, raw SQL):

```bash
DATABASE_URL=postgres://… BENCH_ITERS=2000 node bench/node-baseline.mjs   # needs `npm i pg`
```

Prisma baseline (Prisma Client v6.19, **no built-in cache**):

```bash
cd bench/prisma-baseline && npm install && npx prisma generate
DATABASE_URL=postgres://… BENCH_ITERS=2000 node bench.mjs
```

Payload CMS baseline (Payload v3.32 local API, **no built-in cache**):

```bash
cd bench/payload-baseline && npm install
DATABASE_URL=postgres://… PAYLOAD_CONFIG_PATH=$PWD/payload.config.ts \
  BENCH_ITERS=2000 npx tsx bench.mts
```

**Measure co-located** — both against a *local* Postgres on the same host. A remote DB inflates every
write with a network round trip (we learned this the hard way; see Results). One reproducible way to
do that on a dev box with Postgres in WSL2 is to run each in a container with `--network host`:

```bash
# Atomo (builds in a rust container; target on container fs to avoid slow /mnt I/O)
docker run --rm --network host -v "$PWD":/app -w /app -e CARGO_TARGET_DIR=/tmp/t \
  -e DATABASE_URL=… rust:latest bash -c "cargo build --release -p atomo_server --example bench && /tmp/t/release/examples/bench"
# Node baseline
docker run --rm --network host -v "$PWD":/app -w /app -e DATABASE_URL=… \
  node:20 bash -c "npm i pg --no-save --silent && node bench/node-baseline.mjs"
# Prisma baseline
docker run --rm --network host -v "$PWD"/bench/prisma-baseline:/app -w /app \
  -e DATABASE_URL=… node:20 \
  bash -c "npm install --silent && npx prisma generate && node bench.mjs"
# Payload CMS baseline
docker run --rm --network host -v "$PWD"/bench/payload-baseline:/app -w /app \
  -e DATABASE_URL=… -e PAYLOAD_CONFIG_PATH=/app/payload.config.ts node:20 \
  bash -c "npm install --silent && npx tsx bench.mts"
```

Both harnesses warm up, time each operation serially, and print a markdown table of mean +
p50/p95/p99 latency and ops/sec. **Release-only** (debug numbers are meaningless).

## What's measured

| Bench | What it isolates |
| --- | --- |
| **data layer: create** | one insert + model-event emission via `AtomoClient::create` |
| **data layer: create_many** | a 100-row batch via `AtomoClient::create_many` (one txn), reported per-row |
| **data layer: update_many / delete_many** | a single-row update / soft-delete by id (one txn, write + events) |
| **data layer: find_many hot / cold** | a bounded read (limit 20) via `AtomoClient::find_many` — hot = cache hit, cold = after a write invalidates the cache (DB round trip) |
| **data layer: find_unique hot / cold** | a point read by id via `AtomoClient::find_unique` — hot = cache hit, cold = cache miss |
| **data layer: count** | `AtomoClient::count` — `SELECT COUNT(*)` (not cached) |
| **data layer: find_many + include** | `find_many` with a hasMany relationship: 20 parent records, each resolving 3 children — measures the N+1 relation-resolution cost |
| **data layer: find_many eventual** | same as find_many hot but with `ATOMO_CACHE_MODE=eventual` — writes interleaved between reads do NOT invalidate the cache, showing cache staying hot under mixed load |
| **job lease: 1 worker** | `JobStore::lease` throughput draining a queue (cap 50/call) |
| **job lease: 8 workers** | concurrent `SELECT … FOR UPDATE SKIP LOCKED` dispatch — shows lock-free scaling |
| **HTTP request throughput** | a bare endpoint under `k6` concurrency — Atomo's axum server vs Fastify; isolates the request runtime (see Full-stack HTTP below) |
| **action overhead: baseline CRUD** | `create` with no event bindings — the floor (`action_overhead` example) |
| **action overhead: event emission** | `create` with events declared but no dispatcher — isolates event construction + broadcast cost |
| **action overhead: dispatch + enqueue** | `create` with a live action dispatcher — measures binding match + job INSERT overhead |
| **action overhead: worker CRUD callback** | HTTP round-trip through `/api/worker/crud/:model` via `oneshot` — measures auth + capability check + CRUD + JSON serialization |
| **crm: create Company / Contact** | `create` with a real-world CRM model (6–8 fields, FK relations, validations) — validates that field count doesn't change write cost |
| **crm: create_many Lead** | batch create with select/enum fields and two FK relations — per-row cost in a complex-model batch |
| **crm: find_many filtered** | `find_many` with WHERE on a select field (`status`) or FK field (`companyId`) — measures cache-hit read with non-trivial filters |
| **crm: find_unique by id** | point read on a CRM Contact — same cache-hit path as simple Note, validates no per-field overhead |
| **crm: mixed workload** | interleaved `create Deal` + `find_many Lead` — simulates a real CRM app pattern |
| **GraphQL: create mutation** | a full `create` mutation resolved in-process via `schema.execute()` — measures GraphQL resolution + casing conversion |
| **GraphQL: records query (hot)** | `records(model, limit: 20)` in-process — includes camelCase key conversion on cached results |
| **GraphQL: record by id (hot)** | `record(model, id)` point-read in-process — cache hit + camelCase conversion |
| **updateMany vs sequential** | N=50 sequential `update` mutations vs 1 `updateMany` bulk call — per-row latency comparison over 5 rounds |
| **rate limiter: serial** | `RateLimiter::check()` throughput on a single IP (token bucket, no contention) |
| **rate limiter: concurrent** | 8-worker concurrent `check()` with unique IPs — measures lock contention on the shared state |

## Results

All numbers below are **co-located** — Atomo (Linux, in a `rust` container) and the Node baseline
both run on the same host as Postgres, against the *same* local DB, so neither pays a network hop.
**Machine:** Intel i5-13400 (10C/16T), 64 GB · Postgres in WSL2 (local) · release builds ·
2000 iterations · **2026-07-14**, measured on a **quiet machine** (all other local containers and
workloads stopped — earlier runs shared the box with unrelated database containers, which inflated
both CPU-bound µs-scale numbers and fsync-bound writes by ~20%). Re-measured on post-v0.6.0 main
after the sqlx 0.7→0.8 upgrade and dependency refresh: no regressions; cold reads are ~2× faster
than the 2026-06-08 run. Head-to-head baseline sections below retain their original dates since
those stacks didn't change.

**Footprint:** the `atomo-server` release binary is **9.8 MB** (single static binary, stripped + thin
LTO) — the whole per-project runtime, vs a Node runtime (~50–90 MB) plus `node_modules`.

**Atomo engine (co-located):**

| Benchmark | mean µs | p50 | p95 | p99 | ops/sec |
|---|--:|--:|--:|--:|--:|
| data layer: create (insert + event, single txn) | 3010 | 2743 | 3727 | 7194 | 332 |
| data layer: **create_many** (per row, batch 100) | **65** | 63 | 73 | 73 | **15 435** |
| data layer: update_many (1 row by id, single txn) | 3002 | 2753 | 3731 | 4980 | 333 |
| data layer: delete_many (1 row by id, single txn) | 3044 | 2739 | 3694 | 5248 | 329 |
| data layer: **update_many** (per row, ~500 matched) | **46** | 44 | 55 | 55 | **21 775** |
| data layer: **delete_many** (per row, ~500 matched) | **40** | 42 | 42 | 42 | **25 308** |
| data layer: find_many hot (limit 20, cache hit) | 12.6 | 11 | 13 | 20 | 79 322 |
| data layer: find_many cold (limit 20, cache miss) | 259 | 238 | 384 | 754 | 3 857 |
| data layer: **find_unique** hot (cache hit) | **0.8** | — | — | — | **1 297 221** |
| data layer: find_unique cold (cache miss) | 251 | 232 | 385 | 617 | 3 990 |
| data layer: **count** hot (cache hit) | **0.1** | — | — | — | **6 962 673** |
| data layer: count cold (cache miss) | 723 | 687 | 957 | 1220 | 1 383 |
| data layer: find_many + include (20 notes × 3 tags) | 94 | 69 | 99 | 353 | 10 614 |
| data layer: find_many eventual (hot through writes) | 26.0 | 24 | 37 | 51 | 38 412 |
| job lease: 1 worker | 79 | — | — | — | 12 669 |
| job lease: 8 workers (`SKIP LOCKED`) | 32 | — | — | — | 30 866 |

**CRM-schema results (real-world model complexity):**

The simple-model bench above uses a trivial `Note {id, title}` — one field, no relations. To validate
that model complexity doesn't change the picture, the CRM scenarios use the full CRM schema shape: 5
models (Company, Contact, Lead, Deal, Activity), 6–8 fields each, FK relations, select/enum
constraints, and validation rules. Tables are `bench_crm_`-prefixed to isolate from the simple bench.

| Benchmark | mean µs | p50 | p95 | p99 | ops/sec |
|---|--:|--:|--:|--:|--:|
| crm: create Company (7 fields) | 3309 | 3515 | 3800 | 4546 | 302 |
| crm: create Contact (6 fields + FK) | 3290 | 3478 | 3835 | 5392 | 304 |
| crm: create_many Lead (per row, batch=50) | 139 | 138 | 177 | 177 | 7 183 |
| crm: find_many Lead WHERE status='qualified' hot | 20 | 19 | 22 | 34 | 49 367 |
| crm: find_many Contact WHERE companyId=X hot | 20 | 19 | 22 | 35 | 49 698 |
| crm: find_unique Contact by id hot | 1.3 | 1 | 1 | 2 | 794 405 |
| crm: mixed (create Deal + find_many Lead) | 3432 | 3547 | 3826 | 4657 | 291 |

**Machine:** Intel i5-13400 (10C/16T), 64 GB · Postgres in WSL2 (local) · release build (Docker
`rust:latest`, `--network host`) · 2000 iterations · quiet machine · **2026-07-14**.

Key takeaways:

- **Writes scale with transaction count, not field count.** CRM `create` (~3.3 ms) ≈ Note `create`
  (~3.0 ms, same run) — both are one txn = one `fsync`. Adding 6 more fields and a FK costs nothing
  measurable.
- **Batch creates stay efficient.** CRM `create_many` at 139 µs/row (batch=50) is proportional to the
  Note batch at 65 µs/row (batch=100) — the per-row cost is dominated by the multi-row INSERT, not
  per-field overhead. The difference is batch size (50 vs 100), not schema complexity.
- **Filtered reads are fast.** `find_many` with a WHERE clause on a select/enum field (20 µs) or FK
  field (20 µs) — cache-hit performance is consistent regardless of filter complexity.
- **Point reads stay sub-µs class.** CRM `find_unique` by id at 1.3 µs (794 k ops/s) vs Note at
  0.8 µs (1.3 M ops/s) — the slight difference is the larger cached record, not per-field overhead.
- **Mixed workloads** (interleaved create + read) cost ~3.4 ms — the write dominates, and the hot
  read adds negligible time.

**GraphQL full-stack (in-process, including camelCase conversion):**

These measure the full GraphQL resolution path — `schema.execute(Request)` — not just the data layer.
The cost includes parsing, validation, resolver dispatch, casing conversion, and result serialization.

| Benchmark | mean µs | p50 | p95 | p99 | ops/sec |
|---|--:|--:|--:|--:|--:|
| graphql: create mutation (full stack) | 2918 | 2738 | 3691 | 5242 | 343 |
| graphql: records query hot (limit 20, incl. camelCase) | 42 | 40 | 50 | 79 | 23 708 |
| graphql: record by id hot | 8.3 | 7 | 10 | 17 | 121 109 |

Key takeaways:

- **GraphQL create** (~2.9 ms) ≈ raw data layer create (~3.0 ms) — the resolution overhead is
  statistically invisible next to `fsync`.
- **GraphQL list read** (42 µs) is ~3.3× the raw data layer (12.6 µs) — the camelCase key conversion
  on 20 records + GraphQL resolution machinery accounts for the difference. Still **~24 k ops/s**.
- **GraphQL point read** (8.3 µs, 121 k ops/s) is ~10× the raw cache hit (0.8 µs) — GraphQL parsing
  and resolver dispatch dominate when the underlying lookup is sub-µs.

**`updateMany` vs sequential updates (GraphQL, in-process):**

Compares 50 individual `update` mutations vs 1 `updateMany` call with 50 items. Per-row latency over
5 rounds. Both in-process via `schema.execute()`.

| Benchmark | mean µs/row | p50 | p95 | p99 | ops/sec |
|---|--:|--:|--:|--:|--:|
| graphql: update ×50 sequential (per row) | 2923 | 2913 | 3007 | 3007 | 342 |
| graphql: updateMany ×50 bulk (per row) | 2931 | 2949 | 3035 | 3035 | 341 |

Key takeaway: **per-row engine cost is the same** — each row is still one update + event write in a
transaction (`fsync`-bound). `updateMany`'s value is **fewer HTTP/GraphQL round trips**, not faster
per-row throughput. Over HTTP, 1 request vs 50 requests is where the win shows.

**Rate limiter throughput (in-memory, no I/O):**

| Benchmark | mean µs | ops/sec |
|---|--:|--:|
| rate limiter: check (single IP, serial) | 0.1 | 11 917 985 |
| rate limiter: check (8 concurrent IPs, 5000 total) | 0.2 | 4 158 350 |

The token-bucket `check()` is **~11.9 M ops/s** serial — negligible per-request cost. Under 8-way
concurrency the `Mutex` contention drops throughput to ~4.2 M ops/s, still orders of magnitude above
any realistic request rate.

**Machine:** Intel i5-13400 (10C/16T), 64 GB · Postgres in WSL2 (local) · release build (Docker
`rust:latest`, `--network host`) · 2000 iterations · quiet machine · **2026-07-14**.

**Batch inserts:** `create_many` commits a 100-row batch via **two multi-row `INSERT`s** (the rows,
then their events) in **one** transaction — so the per-row cost drops from **~3.0 ms to ~65 µs —
roughly 46×** (≈15.4 k rows/sec). One `fsync` amortized across the batch *and* one round trip per
statement instead of per row. (An earlier per-row-in-one-transaction version was ~407 µs/row — the
multi-row `INSERT` cut another ~5× by eliminating the per-row round trips.) For very large loads a
`COPY`-based path could push further still — a follow-up.

**Single-row writes** (`create` / `update_many` / `delete_many`) all sit in the same **~3.0 ms**
band — each is **one transaction = one `fsync`** (the write + its events commit together). The number
is dominated by Postgres commit durability, not Atomo; relax it with co-located storage,
`synchronous_commit`, or batching (`create_many`). (Each was ~2× this before its event write was
folded into the same transaction.)

**Bulk `update_many` / `delete_many`:** a single call matching ~500 rows costs **~40–46 µs per row**
(≈22–25 k rows/sec) — **~65× cheaper per row** than a one-row call, because it's **one `UPDATE`**
for all matched rows (the SET/WHERE params don't scale with the match count) plus one chunked event
INSERT, all in one transaction. The event inserts are chunked under Postgres' bind-param limit, so
bulk operations are safe at any size (a 11 000-row batch is a regression test).

**Read paths — cold vs hot:** a *hot* `find_many` (cache hit) returns in **~12.6 µs** (79 k/s); a
*cold* read (cache miss, after a write invalidates the model cache) costs **~259 µs** — the actual
Postgres query time (~20× slower). `find_unique` (point read by id) is even faster when cached:
**~0.8 µs (1.3 M ops/s)**, because the cached value is a single record vs a list. Cold `find_unique`
is ~251 µs. `count` is now cached under the same per-model key space: hot count returns in **~0.1 µs
(7.0 M ops/s)**, cold (after a write invalidates) costs ~723 µs (the DB round trip).

**Relation resolution (`include`):** `find_many` with a `hasMany` include (20 parent notes, each
resolving 3 child tags) costs **~94 µs** — the child lookups hit the cache on repeated calls, so the
incremental cost of N+1 relation resolution is low when the cache is warm.

**Eventual mode:** with `ATOMO_CACHE_MODE=eventual`, writes do **not** invalidate the read cache (the
TTL alone bounds staleness). Under a mixed read/write load the cache stays hot — reads cost **~26 µs**
(38 k/s) even with a write between every read, vs ~259 µs cold in strong mode. The trade-off is
bounded staleness (up to the TTL) for ~10× faster reads under write pressure.

## Head-to-head: Atomo vs Node (node-postgres), co-located

The Node baseline (`bench/node-baseline.mjs`) does the equivalent raw SQL via `node-postgres`, same
machine, same DB, same serial-latency method.

| Operation | Atomo | Node (node-pg) | Read |
|---|--:|--:|---|
| **persist a record + event** | 3715 µs (269/s) | 3159 µs (317/s) | **~on par (~1.2×)** — Atomo commits the row + its event in one transaction (one `fsync`), same as the Node txn; both **fsync-bound**. *(Was ~1.9× before the single-transaction fix — see below.)* |
| raw insert (Node) / — | — | 2915 µs (343/s) | the DB write floor |
| **read 20 rows** | **12 µs** cache-hit (84 k/s) | 469 µs (2.1 k/s) | Atomo's **in-process read cache** wins ~40× on hot reads; a *cold* Atomo read (cache miss, ~625 µs) is ~the same as Node (both ≈ the PG query) |
| **point read (`find_unique`)** | **0.8 µs** cache-hit (1.2 M/s) | — | sub-microsecond point-read cache; cold = ~542 µs (the DB round trip) |
| **footprint** | **9.8 MB** binary | Node runtime + `node_modules` | — |
| **durable job lease** | **31 658/s** (8 workers) | — | **no Node equivalent** (Atomo-only) |

### Honest conclusions

- **Atomo's `create` is ~on par with raw `node-postgres`** for an equivalent record+event write
  (~1.2×), now that it commits the row and its event in **one transaction** (one `fsync`). It is
  still **not faster** — both are bounded by Postgres commit durability — so **the roadmap's "3–5×
  faster than Node" stays a *target*** (and the HTTP-layer test below shows the same: not faster
  there either). Atomo isn't a speed play; it's batteries + footprint.
  > **Optimization, this is the benchmark working:** the measurement caught that `create` was doing
  > **two** autocommit writes (the row, then `event_log`) = two `fsync`s ≈ 2× the latency. Collapsing
  > them into one transaction dropped create from **5998 µs → 3715 µs (−38%, +61% throughput)** and
  > took the Node gap from ~1.9× to ~1.2×. A benchmark that finds a real fix is worth more than one
  > that flatters.
- **Where Atomo wins:** hot reads (list cache ~40×, point-read cache **~1.2 M ops/s**), **footprint**
  (a 9.8 MB binary vs a Node install), and **capabilities Node has no built-in answer for** — the
  durable job lease engine (28 k leases/s, scaling **3×** from 1→8 workers via `SKIP LOCKED`), event
  sourcing, and the action dispatcher.
- **The trade, stated plainly:** Atomo costs ~2× a bare insert on writes (both Postgres-`fsync`-bound)
  in exchange for a **10 MB self-contained binary with event sourcing, hooks, durable jobs, sub-µs
  point-read caching (1.2 M ops/s), and a typed, schema-driven backend (API + admin) out of the
  box.** Not raw speed — built-ins + footprint. Most apps are read-heavy, where the cache wins and
  the write cost rarely bites.

> **Why co-located matters (a cautionary data point):** an earlier run with Postgres on a *separate*
> host (Windows → WSL2 over the LAN) showed Atomo `create` at ~9 ms — but that was the **network
> hop**, not Atomo. Always measure with a local DB; a remote DB inflates every write.

## Head-to-head: Atomo vs Prisma, co-located

The Prisma baseline (`bench/prisma-baseline/`) uses **Prisma Client v6.19** against the same co-located
Postgres, same serial-latency method. Prisma v6 does connection pooling, query building, and result
mapping via its Rust-based query engine — more than raw `node-postgres`, but **has no built-in
read cache** (every `find` hits the database; caching requires the paid Prisma Accelerate add-on or
an external layer). Atomo's in-process cache is what makes reads lopsided.

| Operation | Atomo | Prisma | Read |
|---|--:|--:|---|
| **create + event (txn)** | 4429 µs (226/s) | 4941 µs (202/s) | **comparable** — both fsync-bound; Atomo slightly faster (one binary, no IPC) |
| create (bare, no event) | — | 4076 µs (245/s) | Prisma's query-engine overhead vs raw node-pg (~2.9 ms) |
| **createMany** (per row, batch 100) | 75 µs (13 k/s) | 64 µs (16 k/s) | Prisma slightly faster (no event-log INSERT in the batch) |
| **findMany (limit 20)** | **12 µs** hot (84 k/s) | 1340 µs (746/s) | Atomo's cache = **~112×** Prisma; cold Atomo (~625 µs) is ~2× faster (one binary, no IPC) |
| **findUnique** | **0.8 µs** hot (1.2 M/s) | 607 µs (1.6 k/s) | Atomo's point-read cache = **~760×** Prisma |
| **count** | **0.2 µs** hot (6.6 M/s) | 828 µs (1.2 k/s) | Atomo's cached count = **~4 100×** Prisma; cold Atomo (~1111 µs) is comparable |
| **findMany + include** (20 × 3 tags) | **104 µs** (9.6 k/s) | 1177 µs (850/s) | cached relation resolution = **~11×** Prisma |
| **update (1 row)** | 3985 µs (251/s) | 4380 µs (228/s) | both fsync-bound; Atomo includes event emission |
| **delete (1 row)** | 4107 µs (244/s) | 4148 µs (241/s) | effectively identical — the fsync dominates |

### Honest conclusions

- **Writes are a wash.** Both Atomo and Prisma sit in the same ~4–5 ms band for single-row writes —
  Postgres `fsync` dominates, not the runtime. Atomo does *more* per write (event emission) and is
  still comparable. Batch writes are similar (~64–75 µs/row).
- **Reads are where Atomo pulls ahead.** Prisma has no built-in read cache — every `findMany` and
  `findUnique` hits the database. Atomo's in-process cache serves hot reads in **sub-µs to ~12 µs**
  vs Prisma's ~600–1300 µs. For read-heavy workloads (most apps), this is the decisive gap.
- **Prisma is the more direct comparison** than raw `node-postgres`. Most real backends use an ORM,
  not hand-written SQL. Atomo vs Prisma is the comparison someone evaluating both would actually make.
- **What Prisma has that Atomo doesn't (yet):** a mature ecosystem, broad ORM features (raw queries,
  nested writes, aggregations), Prisma Studio, Prisma Accelerate (hosted cache/pool). **What Atomo
  has that Prisma doesn't:** built-in event sourcing, hooks, durable job engine, admin
  UI, a 9.8 MB single binary, and the cache that makes this comparison lopsided on reads.

## Head-to-head: Atomo vs Payload CMS, co-located

The Payload baseline (`bench/payload-baseline/`) uses **Payload CMS v3.32** (local API —
`payload.create` / `payload.find` / etc., no HTTP server) against the same co-located Postgres, same
serial-latency method. Payload v3 uses Drizzle ORM under the hood and does more per operation than
Prisma or raw SQL: field-level hooks, access control evaluation, locked-document tracking, and
preference cleanup. Like Prisma, **Payload v3 has no built-in read cache** — every `find` / `findByID`
hits the database.

| Operation | Atomo | Payload CMS | Read |
|---|--:|--:|---|
| **create** | 4429 µs (226/s) | 6186 µs (162/s) | Atomo **~1.4× faster** (includes event emission; Payload runs hooks + locked-doc tracking) |
| **find (limit 20)** | **12 µs** hot (84 k/s) | 1495 µs (669/s) | Atomo's cache = **~125×** Payload; cold Atomo (~625 µs) is ~2.4× faster |
| **findByID / findUnique** | **0.8 µs** hot (1.2 M/s) | 689 µs (1.5 k/s) | Atomo's point-read cache = **~860×** Payload |
| **count** | **0.2 µs** hot (6.6 M/s) | 393 µs (2.5 k/s) | Atomo's cached count = **~2 000×** Payload; cold Atomo (~1111 µs) is slower (RLS scope overhead) |
| **find + depth/include** | **104 µs** (9.6 k/s) | 3417 µs (293/s) | Atomo cached = **~33×**; Payload's depth resolution is heavier (each related doc goes through its full find pipeline) |
| **update (1 row)** | 3985 µs (251/s) | 7271 µs (138/s) | Atomo **~1.8× faster** (includes event emission; Payload runs before/after hooks + locked-doc update) |
| **delete (1 row)** | 4107 µs (244/s) | 7616 µs (131/s) | Atomo **~1.9× faster** (soft-delete + event; Payload does hard-delete + preference cleanup + locked-doc cleanup) |
| **bulk create** (per row, ×100) | **75 µs** (13 k/s) | 5434 µs (184/s) | Atomo's multi-row INSERT = **~72× per row**; Payload has no native bulk create (sequential) |

### Honest conclusions

- **Payload does more per operation** — access control hooks, locked-document tracking, preference
  cleanup on delete — so it's expected to be slower on writes. Atomo does event sourcing per write
  (which Payload doesn't) and is still ~1.4–1.9× faster on mutations.
- **Reads are the biggest gap** — same story as Prisma but wider, because Payload's find pipeline
  (access control evaluation, hook execution, field transforms) adds overhead on top of the DB query.
  Atomo's cache bypasses all of that on hot reads. Even cold Atomo reads (pure DB) are ~2× faster
  than Payload's find, because Atomo's query path is thinner (one compiled binary, no JS hook
  pipeline for a no-hook model).
- **Count is now cached too.** Atomo's count was previously slower than Payload's (RLS scope
  overhead); now it caches under the same model key space — hot count is **0.2 µs** (~2 000× Payload).
  Cold count (~1111 µs) is still heavier than Payload's 393 µs due to RLS scoping.
- **Payload is the closest "apples-to-apples" competitor** — both are schema-driven backend
  frameworks with admin UI, hooks, and relation resolution. The difference: Atomo is a compiled Rust
  binary with an in-process cache; Payload is a Node.js CMS with a richer extension/admin ecosystem.
  For read-heavy workloads the gap is decisive; for write-heavy workloads with heavy hook logic,
  Payload's JS ecosystem may be the right trade.

## Full-stack HTTP: request throughput

The numbers above are in-process. This measures the **HTTP request runtime under concurrency** —
Atomo's axum/tokio server vs a fast Node framework — on a bare endpoint (small JSON, **no DB**), so it
isolates request handling (the layer the data-layer bench excludes). Both co-located; **k6**, 50 VUs,
15 s. Atomo `/version` vs Fastify `/health`.

| Server | req/sec | p95 | max |
| --- | --: | --: | --: |
| Node **Fastify** (bare routing) | 43 210 | 2.0 ms | 195 ms |
| Atomo **lean** (`RUST_LOG=warn`, security headers off) | 30 169 | 2.3 ms | 9 ms |
| Atomo **default** (full production middleware) | 16 996 | 3.7 ms | 15 ms |

### Honest conclusions

- **Atomo is *not* faster than a fast Node framework at the HTTP layer** — comparable when lean,
  ~2.5× slower by default. So **"3–5× faster than Node" is not supported at the HTTP layer either**
  (nor the data layer). Across everything measured, Atomo's value is **not raw speed.**
- **Atomo does more per request out of the box.** Its default path runs tracing/logging, security
  headers, CORS, a rate-limit token bucket, and request-id propagation; the bare Fastify does routing
  only. A Fastify with equivalent middleware would close the gap — the honest framing is
  *batteries-included request path vs bare router*.
- **Now fixed in the default.** The default→lean jump (17 k → 30 k req/s) was almost all
  **per-request `INFO` logging**. The per-request *completion* log is now emitted at **DEBUG**, so a
  default deployment performs like the "lean" row (~30 k req/s), not the old ~17 k — the benchmark's
  clearest finding, banked as the default. (Boot/error logs stay at INFO+; set `RUST_LOG=debug` to
  restore per-request logs; ship as `LOG_FORMAT=json` to a collector.) *The ~17 k "default" row above
  reflects pre-change behavior.*
- A single-IP flood also trips Atomo's **rate limiter** (fast 429s — real protection, a load-test
  artifact); raise `RATE_LIMIT_RPS` when benchmarking, as we did.

> **Follow-up:** bare-endpoint throughput (no DB). For authenticated CRUD under load, see the
> **Authed Load** section below.

Harness: `bench/http/` (`schema.ts`, `node-server.mjs`, `load.js`, `seed.sql`) — boot each server
co-located, then `k6 run` against it.

## Authed load: full-stack CRUD under concurrency

The engine-level and bare-HTTP benches above isolate individual layers. This measures the **full
production path under realistic concurrency** — JWT auth, session verification, GraphQL resolution,
read cache, write + event sourcing — all together. **k6**, 50 VUs, 60 s steady state, server in Docker
(`--network host`), k6 on the host. Co-located Postgres. Pool size 30.

Four scenarios, each isolating a workload shape:

| Scenario | Workload | Req/s | p50 | p95 | Errors |
|---|---|--:|--:|--:|--:|
| **mixed** | 80% read, 10% create, 5% update, 5% delete | **2 384** | 5.2 ms | 13.8 ms | 0% |
| **read-only** | 100% reads (limit 20) | **2 397** | 5.0 ms | 10.7 ms | 0% |
| **write-only** | 100% creates | **1 361** | 17.8 ms | 30.7 ms | 0% |
| **mutate** | 50% update, 50% delete | **201** | 131 ms | 482 ms | 0% |

**Machine:** Intel i5-13400 (10C/16T), 64 GB · Postgres in WSL2 (local) · release build ·
pool size 30 · `RATE_LIMIT_RPS=999999` · **2026-06-08**.

### Per-operation latency (from the mixed scenario)

| Operation | avg | p50 | p95 |
|---|--:|--:|--:|
| read (limit 20) | 5.8 ms | 4.5 ms | 9.6 ms |
| create | 14.1 ms | 11.4 ms | 17.8 ms |
| update | 13.1 ms | 11.4 ms | 17.9 ms |
| delete | 14.0 ms | 11.3 ms | 17.7 ms |

### Honest conclusions

- **Reads dominate throughput.** Under the 80/10/5/5 mixed workload, the read cache keeps p95 at
  **13.8 ms** — most iterations hit the cache and return without a DB round trip.
- **Writes are ~3× slower than reads** (~14 ms vs ~5 ms) — each write commits a row mutation + event
  in one transaction (one `fsync`), which is the expected cost.
- **The mutate scenario is a worst-case stress test**, not a realistic workload. All 50 VUs hit the
  same fallback row (the seeded data has no `createdIds` for the delete path to consume), causing
  heavy **row-level lock contention** on Postgres. The p95 of ~482 ms reflects lock wait time, not
  Atomo overhead. Real workloads spread updates across many rows.
- **Pool size matters.** An earlier run with pool=10 (default) under the same mixed workload showed
  309 req/s and p95 446 ms — **7.7× lower throughput and 32× higher tail latency** than pool=30.
  The pool bottleneck (5:1 VU-to-connection contention) dominated. `DATABASE_POOL_MAX` is now
  configurable (default 20).

Harness: `bench/authed-load/` (`load.js`, `schema.ts`, `Dockerfile`, `docker-run.sh`).

```bash
# Quick run (from WSL, with Docker):
DATABASE_URL=postgres://… ./bench/authed-load/docker-run.sh

# Individual scenario:
k6 run -e BASE=http://127.0.0.1:3099 -e SCENARIO=read-only bench/authed-load/load.js
```

## When to use what

The benchmarks above measure cost. This section maps cost to product decisions — when do those
numbers matter, and when don't they?

| Workload | Better fit | Why (data-backed) |
|---|---|---|
| **Read-heavy API** (dashboards, feeds, listings) | **Atomo** | Hot-cache reads: **12 µs** list / **0.8 µs** point-read vs Payload's ~700–1500 µs and Prisma's ~600–1300 µs. Most apps are 80%+ reads — the cache is the decisive gap. |
| **Bulk data ingestion** (imports, migrations, ETL) | **Atomo** | `create_many`: **75 µs/row** (13 k/s) vs Payload's 5434 µs/row (184/s) — **~72× per row**. Bulk `update_many`/`delete_many`: ~33 µs/row (28–30 k/s). Payload has no native bulk path. |
| **Lightweight self-hosted backend** (SaaS API, internal tools) | **Atomo** | A **9.8 MB** static binary with auth, event sourcing, durable jobs, schema-driven API, and admin UI. No Node runtime, no `node_modules`, no process manager. Deploy a single binary + Postgres. |
| **High-concurrency mixed workload** | **Atomo** | The authed load test shows **2 384 req/s at p95 13.8 ms** under 50 VUs (JWT + GraphQL + cache + events). The in-process cache means reads don't contend for DB connections. |
| **Content team CMS / editorial workflow** | **Payload** | Payload's admin UI is a polished React app with live preview, rich text (Lexical/Slate), media library, draft/publish workflow, and localization — built for content editors, not developers. Atomo's admin is functional but developer-oriented. |
| **Plugin ecosystem / custom admin extensions** | **Payload** | Payload has a mature plugin ecosystem (SEO, form builder, nested docs, redirects, search) and a React-based admin that supports custom field components, views, and providers. Atomo's extension model is actions & workers — powerful but earlier-stage. |
| **Write-heavy with complex hooks** | **Either** | Single-row writes are fsync-bound in both (~4–8 ms). Atomo is ~1.4–1.9× faster per write (includes event sourcing), but if your hooks need npm packages or native deps, Payload's JS runtime is the pragmatic choice. |
| **Durable background jobs** | **Atomo** | Built-in job engine with `SKIP LOCKED` lease dispatch: **31 658 leases/s** (8 workers). Payload has no built-in job queue — you'd add BullMQ, pg-boss, or similar. |

### What this table is not

This is not "Atomo replaces Payload." They overlap on schema-driven CRUD + admin but diverge on
audience: Atomo is a **backend runtime** (API-first, event-sourced, small footprint); Payload is a
**headless CMS** (content-first, editor-friendly, extension-rich). The benchmark numbers inform which
trade-offs matter for a given project — they don't make the choice for you.

- If your team is **content editors** who need rich text, media management, and a polished admin
  experience — Payload is the right tool regardless of the latency gap.
- If your team is **developers** shipping an API backend where reads dominate, footprint matters, or
  you need event sourcing and durable jobs out of the box — that's where the numbers above translate
  to a real product advantage.

## Caveats (read before quoting a number)

- **In-process, not HTTP.** Add network + HTTP + GraphQL overhead for end-to-end figures.
- **Environment-sensitive.** Postgres locality dominates write latency; a remote DB inflates every
  number. Always record the machine + DB location alongside results.
- **No CI perf-gate.** Perf on shared CI runners is too noisy to gate on; track the trend manually.
- **Run-to-run variance.** `fsync`-bound write latency wobbles (±~20% between runs); compare medians
  and re-run before drawing a conclusion. The *relative* picture (Atomo vs Node, reads vs writes) is
  stable; the absolute µs are not.
- A wrong or gamed benchmark is worse than none — keep these reproducible and conservative.

## See also
- [Performance Tuning](/guide/advanced/performance) — knobs that move these numbers
- [Durable Jobs & Workers](/guide/advanced/jobs-and-workers) — the worker model the lease bench backs
