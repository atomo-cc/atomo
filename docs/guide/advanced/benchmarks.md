---
title: Benchmarks
description: Reproducible engine-level benchmarks for Atomo's core paths — data layer, durable job lease engine, and the plugin (WASM/JS) hook tax.
---

# Benchmarks

These are **honest, reproducible, engine-level** numbers. They measure the cost of Atomo's core
operations **in-process** — the data layer, the durable job lease engine, and the plugin hook path —
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
| **plugin hook tax: JS/Javy** | a `before_create` hook through `WasmHookRunner` (the real CRUD-hook path): the per-operation cost of crossing into the **JS (Javy/QuickJS)** sandbox + JSON marshalling |
| **HTTP request throughput** | a bare endpoint under `k6` concurrency — Atomo's axum server vs Fastify; isolates the request runtime (see Full-stack HTTP below) |

## Results

All numbers below are **co-located** — Atomo (Linux, in a `rust` container) and the Node baseline
both run on the same host as Postgres, against the *same* local DB, so neither pays a network hop.
**Machine:** Intel i5-13400 (10C/16T), 64 GB · Postgres in WSL2 (local) · release builds ·
2000 iterations · **2026-06-08**.

**Footprint:** the `atomo-server` release binary is **9.8 MB** (single static binary, stripped + thin
LTO) — the whole per-project runtime, vs a Node runtime (~50–90 MB) plus `node_modules`.

**Atomo engine (co-located):**

| Benchmark | mean µs | p50 | p95 | p99 | ops/sec |
|---|--:|--:|--:|--:|--:|
| data layer: create (insert + event, single txn) | 3715 | 3664 | 5508 | 7429 | 269 |
| data layer: **create_many** (per row, batch 100) | **77** | 76 | 107 | 107 | **13 006** |
| data layer: update_many (1 row by id, single txn) | 3709 | 3607 | 4645 | 6835 | 270 |
| data layer: delete_many (1 row by id, single txn) | 3786 | 3617 | 5260 | 7165 | 264 |
| data layer: **update_many** (per row, ~500 matched) | **36** | 34 | 37 | 37 | **28 028** |
| data layer: **delete_many** (per row, ~500 matched) | **33** | 32 | 37 | 37 | **30 114** |
| data layer: find_many hot (limit 20, cache hit) | 11.9 | 11 | 12 | 22 | 83 721 |
| data layer: find_many cold (limit 20, cache miss) | 625 | 570 | 1012 | 1285 | 1 600 |
| data layer: **find_unique** hot (cache hit) | **0.8** | — | 1 | 1 | **1 219 298** |
| data layer: find_unique cold (cache miss) | 542 | 500 | 848 | 1191 | 1 845 |
| data layer: **count** hot (cache hit) | **0.2** | — | — | — | **6 571 295** |
| data layer: count cold (cache miss) | 1111 | 1025 | 1648 | 2238 | 900 |
| data layer: find_many + include (20 notes × 3 tags) | 104 | 57 | 75 | 176 | 9 582 |
| data layer: find_many eventual (hot through writes) | 42.5 | 37 | 73 | 112 | 23 528 |
| job lease: 1 worker | 104 | — | — | — | 9 634 |
| job lease: 8 workers (`SKIP LOCKED`) | 32 | — | — | — | 31 658 |
| plugin hook tax: JS/Javy `before_create` (load 413 ms once) | 178 | 166 | 237 | 331 | 5 630 |

**Batch inserts:** `create_many` commits a 100-row batch via **two multi-row `INSERT`s** (the rows,
then their events) in **one** transaction — so the per-row cost drops from **~3.9 ms to ~77 µs —
roughly 50×** (≈13 k rows/sec). One `fsync` amortized across the batch *and* one round trip per
statement instead of per row. (An earlier per-row-in-one-transaction version was ~407 µs/row — the
multi-row `INSERT` cut another ~5× by eliminating the per-row round trips.) For very large loads a
`COPY`-based path could push further still — a follow-up.

**Single-row writes** (`create` / `update_many` / `delete_many`) all sit in the same **~3.7–3.9 ms**
band — each is **one transaction = one `fsync`** (the write + its events commit together). The number
is dominated by Postgres commit durability, not Atomo; relax it with co-located storage,
`synchronous_commit`, or batching (`create_many`). (Each was ~2× this before its event write was
folded into the same transaction.)

**Bulk `update_many` / `delete_many`:** a single call matching ~500 rows costs **~33–36 µs per row**
(≈28–30 k rows/sec) — **~110× cheaper per row** than a one-row call, because it's **one `UPDATE`**
for all matched rows (the SET/WHERE params don't scale with the match count) plus one chunked event
INSERT, all in one transaction. The event inserts are chunked under Postgres' bind-param limit, so
bulk operations are safe at any size (a 11 000-row batch is a regression test).

**Read paths — cold vs hot:** a *hot* `find_many` (cache hit) returns in **~12 µs** (84 k/s); a *cold*
read (cache miss, after a write invalidates the model cache) costs **~625 µs** — the actual Postgres
query time (~52× slower). `find_unique` (point read by id) is even faster when cached: **~0.8 µs
(1.2 M ops/s)**, because the cached value is a single record vs a list. Cold `find_unique` is ~542 µs.
`count` is now cached under the same per-model key space: hot count returns in **~0.2 µs (6.6 M
ops/s)**, cold (after a write invalidates) costs ~1111 µs (the DB round trip).

**Relation resolution (`include`):** `find_many` with a `hasMany` include (20 parent notes, each
resolving 3 child tags) costs **~104 µs** — the child lookups hit the cache on repeated calls, so the
incremental cost of N+1 relation resolution is low when the cache is warm.

**Eventual mode:** with `ATOMO_CACHE_MODE=eventual`, writes do **not** invalidate the read cache (the
TTL alone bounds staleness). Under a mixed read/write load the cache stays hot — reads cost **~43 µs**
(23 k/s) even with a write between every read, vs ~625 µs cold in strong mode. The trade-off is
bounded staleness (up to the TTL) for ~15× faster reads under write pressure.

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
  sourcing, and the plugin sandbox.
- **The trade, stated plainly:** Atomo costs ~2× a bare insert on writes (both Postgres-`fsync`-bound)
  in exchange for a **10 MB self-contained binary with event sourcing, hooks, durable jobs, sub-µs
  point-read caching (1.2 M ops/s), and a typed, schema-driven backend (API + admin) out of the
  box.** Not raw speed — built-ins + footprint. Most apps are read-heavy, where the cache wins and
  the write cost rarely bites.

> **Why co-located matters (a cautionary data point):** an earlier run with Postgres on a *separate*
> host (Windows → WSL2 over the LAN) showed Atomo `create` at ~9 ms — but that was the **network
> hop**, not Atomo. Always measure with a local DB; a remote DB inflates every write.

Other portable takeaways: the **JS plugin load** (Javy/QuickJS instantiate) is a one-time ~400 ms at
boot, not per-call; the **JS hook tax** (~178 µs/call here on Linux; was ~424 µs on Windows — wasmtime
is slower there) is CPU-only and DB-independent.

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
  has that Prisma doesn't:** built-in event sourcing, WASM plugin hooks, durable job engine, admin
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
  binary with an in-process cache; Payload is a Node.js CMS with a richer plugin/admin ecosystem.
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

> **Follow-up:** this is bare-endpoint throughput (no DB). DB-bound endpoints are comparable for both
> (see the data layer); a full *authenticated CRUD under load* comparison needs JWT plumbing on both
> sides to stay fair (Atomo gates reads behind auth by default) — not yet done.

Harness: `bench/http/` (`schema.ts`, `node-server.mjs`, `load.js`, `seed.sql`) — boot each server
co-located, then `k6 run` against it.

## Reading the plugin hook tax (for migrators)

If you're evaluating Atomo for a **custom-logic-heavy** project, the hook tax is the number that
matters: it's what each plugin-touched operation costs on top of the bare data layer.

- The measured figure is for a **JS (Javy/QuickJS)** hook — the easy "drop in a `.js`, no toolchain"
  path, and the **upper bound** of the tax. **Compiled-WASM** plugins execute faster than interpreted
  JS (precise compiled-WASM numbers are a follow-up).
- **This does not change the [external-workers](/guide/advanced/jobs-and-workers) guidance.** The
  sandbox tax is about logic that *fits* the sandbox (validation, transforms, domain rules). Work
  that touches the **outside world** — provider APIs, browser automation, `ffmpeg`, long polling,
  native deps — **can't run in the sandbox at all**, regardless of speed, and belongs in a worker.
  The hook tax just tells you the cost of the in-sandbox logic that *does* fit.

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
