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
```

Both harnesses warm up, time each operation serially, and print a markdown table of mean +
p50/p95/p99 latency and ops/sec. **Release-only** (debug numbers are meaningless).

## What's measured

| Bench | What it isolates |
| --- | --- |
| **data layer: create** | one insert + model-event emission via `AtomoClient::create` |
| **data layer: create_many** | a 100-row batch via `AtomoClient::create_many` (one txn), reported per-row |
| **data layer: find_many** | a bounded read (limit 20) via `AtomoClient::find_many` |
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
| data layer: **create_many** (per row, batch 100) | **407** | 405 | 544 | 544 | **2 460** |
| data layer: find_many (limit 20, cache hit) | 14.4 | 12 | 24 | 30 | 69 462 |
| job lease: 1 worker | 104 | — | — | — | 9 634 |
| job lease: 8 workers (`SKIP LOCKED`) | 32 | — | — | — | 31 658 |
| plugin hook tax: JS/Javy `before_create` (load 413 ms once) | 178 | 166 | 237 | 331 | 5 630 |

**Batch inserts:** `create_many` commits a 100-row batch in **one** transaction, so the per-row cost
drops from **~3.7–4.1 ms to ~0.4 ms — roughly 10×** (one `fsync` amortized across the batch instead
of one per row). For bulk imports/seeding this is the difference between N `fsync`s and one. (A
multi-row `INSERT` / `COPY` would push this further by also cutting per-row round trips — a follow-up.)

## Head-to-head: Atomo vs Node (node-postgres), co-located

The Node baseline (`bench/node-baseline.mjs`) does the equivalent raw SQL via `node-postgres`, same
machine, same DB, same serial-latency method.

| Operation | Atomo | Node (node-pg) | Read |
|---|--:|--:|---|
| **persist a record + event** | 3715 µs (269/s) | 3159 µs (317/s) | **~on par (~1.2×)** — Atomo commits the row + its event in one transaction (one `fsync`), same as the Node txn; both **fsync-bound**. *(Was ~1.9× before the single-transaction fix — see below.)* |
| raw insert (Node) / — | — | 2915 µs (343/s) | the DB write floor |
| **read 20 rows** | **14 µs** cache-hit (69 k/s) | 469 µs (2.1 k/s) | Atomo's **in-process read cache** wins ~30× on hot reads; a *cold* Atomo read (cache miss) is ~the same as Node (both = the PG query) |
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
- **Where Atomo wins:** hot reads (its cache, ~30×), **footprint** (a 9.8 MB binary vs a Node
  install), and **capabilities Node has no built-in answer for** — the durable job lease engine
  (31 k leases/s, scaling **3.3×** from 1→8 workers via `SKIP LOCKED`), event sourcing, and the
  plugin sandbox.
- **The trade, stated plainly:** Atomo costs ~2× a bare insert on writes (both Postgres-`fsync`-bound)
  in exchange for a **10 MB self-contained binary with event sourcing, hooks, durable jobs, hot-path
  read caching, and a typed, schema-driven backend (API + admin) out of the box.** Not raw speed —
  built-ins + footprint. Most apps are read-heavy, where the cache wins and the write cost rarely
  bites.

> **Why co-located matters (a cautionary data point):** an earlier run with Postgres on a *separate*
> host (Windows → WSL2 over the LAN) showed Atomo `create` at ~9 ms — but that was the **network
> hop**, not Atomo. Always measure with a local DB; a remote DB inflates every write.

Other portable takeaways: the **JS plugin load** (Javy/QuickJS instantiate) is a one-time ~400 ms at
boot, not per-call; the **JS hook tax** (~178 µs/call here on Linux; was ~424 µs on Windows — wasmtime
is slower there) is CPU-only and DB-independent.

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
