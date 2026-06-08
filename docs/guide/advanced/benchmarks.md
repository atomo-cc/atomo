---
title: Benchmarks
description: Reproducible engine-level benchmarks for Atomo's core paths — data layer, durable job lease engine, and the plugin (WASM/JS) hook tax.
---

# Benchmarks

These are **honest, reproducible, engine-level** numbers. They measure the cost of Atomo's core
operations **in-process** — the data layer, the durable job lease engine, and the plugin hook path —
and deliberately **exclude** HTTP framing, the network, and GraphQL resolution. They answer *"what
does each core operation cost?"*, not *"requests/sec through the full stack."*

> **What this is not (yet):** a full-stack HTTP load test, and **not** a head-to-head vs Node. The
> roadmap's "3–5× faster than Node" line is a **target**, not a measured result — a fair Node
> baseline is a documented follow-up. We publish what we actually measure.

## Running it

```bash
DATABASE_URL=postgres:///atomo_bench \
  cargo run --release -p atomo_server --example bench   # BENCH_ITERS=5000 to override (default 2000)
```

The harness (`crates/atomo_server/examples/bench.rs`) warms up, times each operation, and prints a
markdown table of mean + p50/p95/p99 latency and ops/sec. It is **release-only** (debug numbers are
meaningless) and **gated on a real Postgres**.

## What's measured

| Bench | What it isolates |
| --- | --- |
| **data layer: create** | one insert + model-event emission via `AtomoClient::create` |
| **data layer: find_many** | a bounded read (limit 20) via `AtomoClient::find_many` |
| **job lease: 1 worker** | `JobStore::lease` throughput draining a queue (cap 50/call) |
| **job lease: 8 workers** | concurrent `SELECT … FOR UPDATE SKIP LOCKED` dispatch — shows lock-free scaling |
| **plugin hook tax: JS/Javy** | a `before_create` hook through `WasmHookRunner` (the real CRUD-hook path): the per-operation cost of crossing into the **JS (Javy/QuickJS)** sandbox + JSON marshalling |

## Results

**Footprint:** the `atomo-server` release binary is **9.8 MB** (single static binary, stripped + thin
LTO) — the whole per-project runtime, vs hundreds of MB of `node_modules` or multi-GB container
stacks.

**Engine benchmarks** (2000 iterations each):

| Benchmark | n | mean µs | p50 µs | p95 µs | p99 µs | ops/sec |
|---|--:|--:|--:|--:|--:|--:|
| data layer: create (insert + event) † | 2000 | 9074 | 8772 | 12114 | 14310 | 110 |
| data layer: find_many (limit 20, cache hit) | 2000 | 24.5 | 22 | 30 | 53 | 40825 |
| job lease: 1 worker | 2000 | 146 | — | — | — | 6825 |
| job lease: 8 workers (SKIP LOCKED) | 2000 | 47 | — | — | — | 21220 |
| plugin hook tax: JS/Javy `before_create` ‡ | 2000 | 424 | 388 | 698 | 943 | 2357 |

**Machine:** Intel i5-13400 (10C/16T), 64 GB, Windows 11 · Postgres **remote in WSL2 over the LAN** ·
release build · **Date:** 2026-06-08.

> **† The create number is network-bound, not engine cost.** This run's Postgres is on a *separate
> host* (Windows → WSL2 over the LAN), so each write pays a full round trip — ~9 ms is dominated by
> the network, not Atomo. **Co-located Postgres makes writes dramatically faster** (typically
> sub-millisecond). Re-run with a local DB before quoting write throughput; treat create and the
> job-lease figures as **upper-bound latencies for this setup**. (`find_many` here is a read-cache
> hit, hence the 24 µs.)

Takeaways that *are* portable from this run:

- **Footprint** (9.8 MB) and the **JS hook tax** (‡ ~424 µs, CPU-only — no DB round trip) don't
  depend on DB locality, so they're representative.
- **`SELECT … FOR UPDATE SKIP LOCKED` scales**: 8 concurrent workers drained the queue at **3.1×**
  the single-worker rate (21 k vs 6.8 k leases/sec) — lock-free concurrent dispatch, as designed.
- The **JS plugin load** (Javy/QuickJS instantiate) is a **one-time ~700 ms** at boot, not per-call.

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
- A wrong or gamed benchmark is worse than none — keep these reproducible and conservative.

## See also
- [Performance Tuning](/guide/advanced/performance) — knobs that move these numbers
- [Durable Jobs & Workers](/guide/advanced/jobs-and-workers) — the worker model the lease bench backs
