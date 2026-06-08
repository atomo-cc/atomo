---
title: External Workers & Blob Storage (Design)
description: Two additive primitives that let Atomo own side-effect-heavy workloads (external API orchestration, browser automation, media pipelines) — durable event-sourced jobs driven by trusted out-of-process workers, plus a first-class blob/asset store.
---

# External Workers & Blob Storage — Design

> **Status:** Design proposal. Nothing here ships in the core server yet. It specifies two
> **additive** primitives — a durable **job + external-worker** system and a **blob/asset** store —
> that extend Atomo from "a schema-driven data/content core" to "a core that can also *own*
> side-effect-heavy workloads" (third-party API orchestration, browser automation, media
> generation) **without weakening the plugin sandbox**.

## Summary

Atomo's extension model today is a **sandbox**: fuel-metered WASM plugins and embedded JS
(Javy/QuickJS) with permission-gated effects. That sandbox is exactly right for *portable,
untrusted, deterministic, short* extension code. It is exactly wrong for the opposite shape of work:
**long-running, native-dependency, side-effect-heavy, first-party orchestration** — calling flaky
external AI providers, driving a headless browser, running an image/video pipeline, polling a job
for minutes, moving large binaries.

The instinct to make that work "fit" by widening the sandbox is a mistake — it would trade away the
sandbox's safety for the one workload that least needs to be sandboxed (it's *your own* trusted
code). The correct move is to **invert**:

1. **External workers** — trusted, out-of-process worker programs (any language, full native
   ecosystem) that **pull durable jobs** from Atomo, do the messy I/O, and report results back as
   events. Atomo becomes the **event-sourced brain**; the mess lives where mess belongs.
2. **Blob storage** — a first-class binary store. **This substantially already exists** as the
   `media` + `storage` subsystem (pluggable `StorageBackend` with local + S3 backends, a `media`
   metadata table, `POST/GET/DELETE /media` + GC, soft-delete, tenant scoping, event-sourced). It
   should be *extended* for media pipelines, not rebuilt — **HTTP Range serving is now done**;
   content checksum, presigned direct-upload, and optional dedup remain. See
   [Upload & Storage](/guide/advanced/upload-storage-plan).

> **Reality check (do not duplicate):** when this doc was first drafted it described the blob store
> as net-new. It is not — the `media`/`storage` subsystem already provides it. The "Primitive 2"
> sections below are kept as the *target shape*, but the work is to evolve `media`/`StorageBackend`,
> **not** to introduce a parallel `assets` table or `BlobStore` trait. The genuinely net-new
> capability in this design is **Primitive 1 (durable jobs + external workers)**.

Both are **purely additive**. Existing single-project servers, plugins, and schemas are unaffected
when the features are unused.

### The core bet: an event-sourced pipeline beats a status column

The reason this is worth building (rather than reaching for a Node backend per media app) is that a
side-effect pipeline expressed as **events** is structurally better than one expressed as a mutable
`status` column — and Atomo already *is* event-sourced. A generation job becomes an aggregate:

```
JobEnqueued → JobLeased → JobProgress×N → JobSucceeded(result)
                                        ↘ JobFailed(reason) → (retry policy) → JobEnqueued
```

Every transition is an immutable event. For a flaky, multi-provider, risk-controlled pipeline this
yields, for free: **replayable failure forensics** ("what exactly did we send, what came back, on
which attempt"), **resumable jobs** (re-drive from a mid-pipeline event), **provider A/B by routing
events**, and a complete **audit trail** — none of which a mutable status field on a CRUD backend
can give you. This is the one axis where Atomo can *beat* a batteries-included Node CMS for this
workload, not merely match it.

### Goals

- Let a first-party app run arbitrary native side-effects (provider APIs, browser automation,
  ffmpeg/`sharp`, long polling) **driven by** Atomo, without putting that code in the sandbox.
- Make the work **durable and observable**: every job is an event stream with at-least-once
  delivery, retries, and live progress.
- Make **binaries first-class**: store, reference, and stream-serve media without hand-rolled file
  routes.
- Keep the **trust boundary explicit**: a worker is trusted *relative to the sandbox* but still a
  least-privilege principal (scoped token), never an open door.
- Reuse what already exists — the **event store** (job lifecycle), the **workflow engine** (retry
  semantics), the **realtime hub** (progress fan-out), and the **SDK** (worker client).

### Non-goals

- **Not** a public, run-other-people's-code compute platform. Workers are *operator-owned, trusted*
  programs. Untrusted/portable extension code still belongs in the WASM/JS sandbox — this does not
  replace or relax it.
- **Not** an in-core media transform library. The core stores and serves bytes; transcoding/resizing
  happens in a worker (with `ffmpeg`/`sharp`) or an optional plugin. Atomo will not bundle native
  media tooling.
- **Not** a distribution lever. Like the multi-project work, this lowers build cost for a class of
  app; it does not acquire users. Evaluate on build-velocity and ownership.

## Why this is the right shape (and the wrong ones aren't)

| Approach | Verdict | Why |
| --- | --- | --- |
| **Widen the sandbox** (let JS/WASM plugins spawn processes, open sockets, touch the FS) | ✗ rejected | Destroys the sandbox's safety guarantee for *every* plugin to serve one trusted workload; still can't run a persistent browser profile or stream 50 MB. |
| **In-process native handlers** (trusted Rust compiled into a custom server build) | ✗ rejected for this | Possible, but couples messy I/O to the server's crash domain, loses hot-reload, and forces Rust for provider-glue/browser code that is far easier in TS. Blocks the request/boot path. |
| **Out-of-process trusted workers + durable jobs** | ✓ chosen | Decouples crash domains; workers scale independently; written in the right language with the full ecosystem; the event-sourced job stream is the payoff. |

This mirrors the multi-project decision: keep the core small and unmodified, add capability
*around* it. The worker is to *compute side-effects* what the control plane is to *deployment* — an
additive plane, not a core rewrite.

## Architecture overview

```
   GraphQL mutation ─┐
   Workflow step    ─┤ enqueue        ┌──────────────────────────────────────┐
   Plugin effect    ─┤───────────────▶│            atomo-server               │
   Control-plane API ┘                │  (the event-sourced brain)            │
                                      │                                        │
   ┌──────────────────────┐  lease    │  • event store  ← job lifecycle events │
   │   external worker     │◀──────────│  • jobs projection (queue working set) │
   │  (trusted, any lang)  │  heartbeat│  • realtime hub → live progress        │
   │  Playwright · ffmpeg  │──────────▶│  • blob store   ← bytes + metadata     │
   │  provider SDKs · HTTP │  complete │  • GraphQL / SDK / admin               │
   └───────┬──────────────┘  /fail     └─────────────┬──────────────────────────┘
           │  presigned PUT (large media)            │ GET /assets/:id (range)
           ▼                                         ▼
      ┌──────────────┐                          ┌──────────┐
      │ blob backend │  local FS  /  S3 · R2    │  clients │  (Admin UI, SDK, mobile)
      └──────────────┘                          └──────────┘
```

Three roles, deliberately separated by trust:

1. **Core (brain)** — owns the durable job log, the queue projection, blob metadata, and all
   data-model logic. Never runs the untrusted-shaped side-effects itself.
2. **Worker (hands)** — trusted, out-of-process, least-privilege. Pulls jobs, does native I/O,
   reports results. Holds a scoped **worker token**, not a user session.
3. **Sandbox (unchanged)** — WASM/JS plugins keep doing in-data-path hooks. A plugin may *enqueue* a
   job (an effect) but never *becomes* a worker.

---

## Primitive 1 — Durable jobs + external workers

### 1.1 The job as an event-sourced aggregate

Job state is **derived from events**, not stored as a single mutable row. Lifecycle events live in
the existing event store (audit, replay, history); a `jobs` **projection** table holds the queue's
working set for fast scheduling — the same CQRS split Atomo already uses for read models.

Lifecycle events:

| Event | Emitted by | Meaning |
| --- | --- | --- |
| `JobEnqueued` | any enqueue seam | job created with `queue`, `kind`, `payload`, `idempotency_key`, retry policy |
| `JobLeased` | core, on lease | a worker took it; carries `lease_id`, `worker_id`, `visible_at` (timeout) |
| `JobProgress` | worker | optional, repeatable; `{percent?, message?, data?}` → fan out to realtime |
| `JobSucceeded` | worker | terminal; carries the result payload (e.g. `{ assetId }`) |
| `JobFailed` | worker / lease-expiry | `{error, retryable}`; retry policy may emit a fresh `JobEnqueued` |
| `JobDeadLettered` | core | attempts exhausted; parked for inspection |

The `jobs` projection (working set):

```sql
CREATE TABLE jobs (
  id              TEXT PRIMARY KEY,            -- ULID
  queue           TEXT NOT NULL,               -- routing key, e.g. "media-gen"
  kind            TEXT NOT NULL,               -- handler selector, e.g. "video.generate"
  status          TEXT NOT NULL,               -- queued | leased | succeeded | failed | dead
  payload         JSONB NOT NULL,
  result          JSONB,
  idempotency_key TEXT,                        -- dedupe: at-least-once safe
  attempts        INT  NOT NULL DEFAULT 0,
  max_attempts    INT  NOT NULL DEFAULT 5,
  lease_id        TEXT,                        -- current lease (NULL if not leased)
  worker_id       TEXT,
  visible_at      TIMESTAMPTZ NOT NULL,        -- queued: when eligible; leased: lease deadline
  tenant_id       TEXT,                        -- RLS-compatible (see Multi-tenant)
  priority        INT NOT NULL DEFAULT 0,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (queue, idempotency_key)              -- enqueue is idempotent
);
CREATE INDEX jobs_dispatch ON jobs (queue, status, priority DESC, visible_at);
```

> The projection is rebuildable from the event log (consistent with Atomo's projector model), so the
> queue table is an optimization, not a second source of truth.

### 1.2 Delivery protocol — pull-based lease (at-least-once)

Workers **pull**; the core never pushes work to a worker socket. Pull is chosen deliberately:

- Workers can sit behind NAT, hold persistent browser profiles, and scale independently — no inbound
  port required on the worker.
- **Backpressure is free**: a worker leases up to its own concurrency limit; it can't be overrun.
- Crash recovery is trivial: an expired lease returns the job to `queued` (visibility-timeout
  pattern, the same idea behind SQS).

API (worker-token authenticated):

```
POST /jobs/lease       { queues:[...], capacity:n }  → leased job(s) + lease_id + visible_at
POST /jobs/:id/heartbeat { lease_id, progress? }     → extend lease deadline (+ optional JobProgress)
POST /jobs/:id/complete  { lease_id, result }        → JobSucceeded
POST /jobs/:id/fail      { lease_id, error, retryable } → JobFailed (retry policy decides re-enqueue)
```

- **Leasing** is an atomic claim: `UPDATE … SET status='leased', lease_id=…, visible_at=now()+timeout
  WHERE id = (SELECT … FOR UPDATE SKIP LOCKED …)` — `SKIP LOCKED` gives lock-free concurrent dispatch
  across many workers on one Postgres.
- **Long-poll or push-to-wake:** `/jobs/lease` can long-poll; additionally the **realtime hub**
  publishes a lightweight "queue X has work" nudge so idle workers wake instantly without tight
  polling. (The hub carries the *signal*; the lease still goes through the durable path.)
- **Idempotency:** at-least-once means a job can run twice (lease expiry + worker actually finished).
  `idempotency_key` makes enqueue idempotent; completing an already-terminal job is a no-op; worker
  handlers should be written to tolerate replays (and the blob store's content-addressing helps —
  see Primitive 2).

### 1.3 Retry, backoff, dead-letter

Per-queue (or per-job) policy, reusing the workflow engine's existing retry semantics:

- `max_attempts`, backoff strategy (fixed / exponential + jitter), and a **retryable** flag the
  worker sets (e.g. a provider rate-limit/risk-control error is retryable-after-cooldown; a malformed
  prompt is not).
- Exhausted attempts → `JobDeadLettered`; the job is parked, visible in the admin job view for
  inspection/replay, never silently dropped.
- **Domain-level reactions** ride the event stream: a `JobFailed{reason: "provider_risk_control"}`
  can trigger (via a workflow or projection) a separate "start account cooldown" event — keeping
  operational policy in data, not buried in worker code.

### 1.4 Enqueue seams (where jobs come from)

The data-model side stays in the core/sandbox; only the *dispatch* crosses the boundary:

| Seam | Shape | Use |
| --- | --- | --- |
| **GraphQL mutation** | `enqueueJob(queue, kind, payload, idempotencyKey)` | app/UI/mobile kicks off work |
| **Workflow step** | a new `Job` step type alongside HTTP/Mutation/Plugin steps | orchestrated pipelines |
| **Plugin effect** | `enqueueJob(...)` host effect (permission-gated, like `emit`/`http`) | a CRUD hook spawns async work |
| **Control-plane / SDK** | direct API | batch/backfill/admin |

A common pattern: a GraphQL mutation creates a domain record *and* enqueues the job in one
transaction (record + `JobEnqueued` committed atomically), so the work can never be "started but
unrecorded."

### 1.5 Worker trust & authentication

A worker is **trusted relative to the sandbox** — but still a scoped principal, not root:

- Authenticates with a **worker token** (distinct credential class from user JWTs), minted by the
  control plane / admin and stored in the secret store (AWS SSM, per the multi-project design).
- The token grants a **least-privilege capability set**: which `queues` it may lease, which job
  `kinds` it may complete, which **blob namespaces** it may write, which **event types** it may
  emit, which **GraphQL mutations** it may call. A worker that only generates video cannot read
  unrelated data or write unrelated blobs.
- Tokens are revocable and rot=able; a compromised worker is contained to its capability set.

This is the load-bearing security statement: "trusted" means *exempt from the sandbox*, **not**
*unrestricted*. The boundary moves from "sandboxed code" to "scoped credential," which is the right
model for first-party-but-still-isolated compute.

### 1.6 Worker SDK

The point is that you write **only the handler body** — the SDK owns lease/heartbeat/ack/retry:

```ts
// TypeScript worker (full Node ecosystem: Playwright, ffmpeg, provider SDKs)
const worker = createWorker({ url, token, queues: ["media-gen"], concurrency: 4 });

worker.on("video.generate", async (job, ctx) => {
  await ctx.progress({ message: "calling provider" });
  const mp4 = await runProviderPipeline(job.payload);          // your existing native code
  const asset = await ctx.assets.put(mp4, { contentType: "video/mp4" }); // → blob store
  return { assetId: asset.id };                                // → JobSucceeded
});
// crash/timeout → lease expires → another worker re-leases. Idempotency_key dedupes.
```

- **TS SDK** (Node) for the common case; a **Rust worker crate** for native/high-throughput workers.
- The SDK handles heartbeating during long handlers, surfaces `ctx.progress()` (→ realtime), and
  enforces the concurrency cap (= leases at most N).

> **Publishing note:** the npm SDK publish pipeline is intentionally deferred today; the worker SDK
> ships on the same timeline as that, or as a vendored package until then.

---

## Primitive 2 — Blob / asset store

> **Already shipped as `media`/`storage`.** Atomo is *not* data/GraphQL-only — the `media`
> subsystem (`crates/atomo_server/src/media.rs` + `storage.rs`) already provides the pluggable
> `StorageBackend` (local + feature-gated S3 with presigned GET), the `media` metadata table,
> `POST/GET/DELETE /media` + GC, soft-delete, tenant scoping, and event sourcing — and now **HTTP
> Range serving** (`206`/`Content-Range`/`416`, `Accept-Ranges`, `ETag` → conditional `304`) for
> video/audio seeking. The model below is the *target* shape; treat it as a list of **extensions to
> `media`** (checksum column, namespace, presigned PUT, dedup), **not** a new `assets` table. See
> [Upload & Storage](/guide/advanced/upload-storage-plan) for the shipped delivery status.

### 2.1 Model (target shape — extend `media`, don't replace it)

```sql
CREATE TABLE assets (
  id            TEXT PRIMARY KEY,              -- ULID (stable internal ID, decoupled from any CDN URL)
  namespace     TEXT NOT NULL,                 -- logical bucket, e.g. "reference" | "generation"
  filename      TEXT,
  content_type  TEXT NOT NULL,
  byte_size     BIGINT NOT NULL,
  checksum      TEXT NOT NULL,                 -- sha256 (ETag + optional content-addressing)
  backend       TEXT NOT NULL,                 -- local | s3 | r2 …
  storage_key   TEXT NOT NULL,                 -- key within the backend
  tenant_id     TEXT,                          -- RLS-compatible
  created_by    TEXT,                          -- user or worker principal
  metadata      JSONB NOT NULL DEFAULT '{}',
  deleted_at    TIMESTAMPTZ,                   -- soft-delete (matches Atomo's lifecycle)
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Bytes live in the backend; the row is metadata. Stable internal IDs mean app data references
`assetId`, never a provider CDN URL — which is exactly what media pipelines want (decouple from a
provider's expiring URLs).

### 2.2 Pluggable backends (mirrors the `Driver` pattern)

A `BlobStore` trait — `put` / `get` (with range) / `delete` / `presign_put` / `presign_get`:

| Backend | Use |
| --- | --- |
| `local` (filesystem volume) | dev + single-host default; zero external dependency |
| `s3` (S3 / Cloudflare R2 / MinIO) | production; offload bandwidth, durability, multi-host |
| `gcs`, … | later additions behind the same trait |

### 2.3 Serving — range-aware streaming

`GET /assets/:id`:

- Honors **HTTP Range requests** — essential for `video/mp4` seeking/scrubbing in a player.
- `ETag` = checksum, cache headers, `Content-Type` from metadata.
- **Authorization**: namespace/tenant-scoped; RLS applies because `assets.tenant_id` participates in
  the same policy as model tables.
- For large media on `s3`, optionally **302 to a presigned GET** so bytes never transit
  atomo-server.

### 2.4 Upload paths

- **Small/synchronous:** `POST /assets` multipart → store → return `{ id }`.
- **Large/worker-produced:** `POST /assets/presign` → presigned PUT URL → worker uploads the MP4
  **directly to S3** → `POST /assets/:id/commit` registers metadata (size, checksum). No large
  payload ever passes through the server.

### 2.5 Content-addressing & dedup (optional)

With sha256 as the storage key, identical bytes dedupe automatically — useful when the same
reference image is reused across many generations (store once, reference many). Stable IDs +
dedup together replace the "download the CDN image and re-upload it" anti-pattern with "reference
the existing asset ID."

### 2.6 Lifecycle

- **Soft-delete** consistent with Atomo's existing model; a GC pass reclaims backend bytes for
  assets with no live referrer after a retention window.
- Orphan detection: assets unreferenced by any model row past retention → eligible for hard delete.

---

## How they compose — a media-generation pipeline (reference workload)

End-to-end, the messy I/O stays in a worker; every state change is an event:

1. UI/mobile calls `enqueueJob("media-gen", "video.generate", {prompt, provider, refAssetIds})` —
   in the same transaction that creates the domain record. → `JobEnqueued`.
2. Projection updates; realtime nudges the `media-gen` queue.
3. A **worker** (full Node: browser automation + provider HTTP) leases the job, heartbeats.
4. Worker runs the provider pipeline, posting `ctx.progress()` → `JobProgress` → **admin sees live
   status**.
5. Worker downloads the result, **presigned-PUTs the MP4** to the blob store, commits metadata →
   gets `assetId`.
6. Worker returns `{ assetId }` → `JobSucceeded`.
7. A projection/workflow links the asset to the domain record; a GraphQL **subscription** pushes the
   finished media to the client.
8. On a provider risk-control error: worker fails with `retryable: true` → backoff; a
   `JobFailed{reason}` event drives a separate cooldown policy. **The whole run is replayable from
   the event log** — which prompt, which references, which attempt, what came back.

The app's **data model, auth, audit, admin, and API** are Atomo (schema-driven, type-safe,
event-sourced). The **side-effects** are an ordinary Node worker you can write with any library. You
keep your hard-won automation code; you swap the substrate (a mutable status column → an event
stream; ad-hoc file routes → a blob primitive).

## Where this wins vs a batteries-included Node CMS — and where it doesn't

**Wins (the reason to build it):**

- **Replayable forensics** for flaky pipelines — event stream vs. a lost mutable status.
- **Decoupled, scalable workers** — N workers `SKIP LOCKED`-dispatch; nothing blocks a request or a
  serverless invocation; crash recovery via lease expiry.
- **One audited, type-safe core** across every app in the portfolio; provider A/B and resume by
  routing events.
- **Trust boundary is explicit** — scoped worker tokens, not "trusted code can do anything."

**Honest losses (state them):**

- **Ecosystem & day-one velocity.** A mature Node CMS gives uploads, image processing, admin field
  types, and in-process hooks *today*; here the worker/blob primitives must be built first.
- **In-process simplicity.** A hook that calls a provider inline is fewer moving parts than a
  durable job + worker — until the pipeline gets flaky/long/large, which is exactly when the
  event-sourced model starts paying off.
- **No bundled media tooling.** `ffmpeg`/`sharp` live in your worker, not the core.

Build this when a real, side-effect-heavy app (or several) will dogfood it; otherwise a Node backend
per media app remains the rational default.

## Cross-cutting concerns

- **Multi-tenant:** `jobs.tenant_id` and `assets.tenant_id` participate in the same RLS policy as
  model tables (see [Multi-tenant](/guide/advanced/multi-tenant)) — tenant isolation for jobs and
  media comes for free when RLS is on.
- **Observability:** job events + `JobProgress` give a natural per-job timeline; fleet metrics
  (queue depth, lease age, failure rate, dead-letter count) scrape from the `jobs` projection. The
  `ATOMO_PROJECT_ID` label (multi-project design) tags worker traffic per project.
- **Backups:** the job event log and `assets` metadata back up with the project DB; blob *bytes*
  back up via the backend (S3 versioning / lifecycle, or `local` volume snapshots).
- **Secrets:** worker tokens and provider credentials live in AWS SSM (per the multi-project secrets
  model), injected into the worker's env — never in the registry or the core.
- **Security boundary:** the only new trusted principal is the worker, and it is capability-scoped.
  The plugin sandbox is untouched; a plugin can *enqueue* but never *execute* worker-class effects.

## Phased delivery plan

> Each phase is independently useful. The **blob half is largely already shipped** as `media`/
> `storage` (local + S3 + serving, now with Range); the genuinely new build is the **job + worker**
> system, which layers on top.

### Phase 0 — Foundations
- Blob: **already present** — `StorageBackend` trait, backend selection, `media` table + soft-delete.
- Job: define job event types + `jobs` projection schema; worker-token credential class in the
  secret model.
- **Deliverable:** job interfaces + schema merged; no behavior change when unused.

### Phase 1 — Blob store extensions (mostly done)
- **Done:** `local`+`s3` backends, `POST /media` (multipart), `GET /media/{id}` with **Range**
  support + `ETag`/conditional GET, soft-delete, GC.
- **Remaining:** content checksum column (strong content-addressed ETag), optional `namespace`.
- **Deliverable:** media upload/serve/stream on a single host — already useful today.

### Phase 2 — Durable jobs + lease API ✅ (done)
- **Lease engine** (`atomo_server::jobs::JobStore`): event-sourced job lifecycle (`Job` events) +
  `jobs` working-set table; idempotent enqueue; `lease`/`heartbeat`/`complete`/`fail` with
  `SELECT … FOR UPDATE SKIP LOCKED` dispatch, per-job lease tokens, visibility-timeout reclaim
  (at-least-once, crash-safe; a boot-time background sweep reclaims on `ATOMO_JOB_RECLAIM_INTERVAL`),
  and a retry/backoff/dead-letter policy.
- **HTTP lease API** (`job_routes::jobs_router`, mounted at `/jobs`): `POST /jobs/lease`,
  `POST /jobs/{id}/heartbeat|complete|fail` for the worker pull side; `POST /jobs/workers` (Admin)
  mints tokens.
- **Worker-token auth** (`WorkerTokenStore`): an `X-Worker-Token` credential class distinct from
  user JWTs, stored only as SHA-256, **capability-scoped to queues** (`WorkerIdentity::may_lease`) —
  trusted-vs-sandbox but least-privilege.
- **Tested:** pure-logic unit tests + Postgres `jobs_store` (lifecycle/idempotency/concurrency/
  reclaim/retry) and `jobs_http` (end-to-end lease/complete + 401/403 enforcement).

### Phase 3 — Worker SDK + enqueue seams (in progress)
- **Done — TS worker SDK** (`@atomo-cc/worker-sdk`, `packages/atomo-worker-sdk`): `createWorker` +
  per-`kind` handlers; the SDK owns the lease/heartbeat/complete/fail loop, concurrency, and
  auto-heartbeat. A thrown error fails the job (server retries); `NonRetryableError` dead-letters.
  vitest-tested; not yet npm-published.
- **Done — REST enqueue seam:** `POST /jobs` (any authenticated user; the job is stamped with the
  caller's tenant), so apps can put work on the queue over HTTP today (`jobs_http` covers it).
- **Remaining:** Rust worker crate; richer enqueue seams (GraphQL `enqueueJob` mutation, workflow
  `Job` step, plugin `enqueueJob` effect); `JobProgress` → realtime hub fan-out (live progress).
- **Deliverable:** write a handler body, get a production-grade worker; jobs kick off from data/UI.

### Phase 4 — Presigned upload + dedup (S3 backend already shipped)
- `s3`/R2 backend + presigned/302 **GET** already exist. Add presigned **PUT** (worker → S3 direct,
  then commit metadata) + optional sha256 content-addressing & dedup.
- **Deliverable:** large-media pipelines that never stream bytes through the server.

### Phase 5 — Operability & optional extensions (build on real need)
- Admin job views (list/inspect-stream/retry/dead-letter), blob GC/retention, queue metrics.
- **Scheduled jobs** (cron-enqueue reusing the queue), media **transform plugin/worker** recipes,
  multi-region blob.

## Sizing & risk

| Work | Size | Risk | Notes |
| --- | --- | --- | --- |
| Phase 0 (job schema/interfaces; blob already present) | S | Low | Additive; no behavior change |
| Phase 1 (blob extensions) | S | Low | **Mostly shipped** (Range done); only checksum/namespace remain |
| Phase 2 (job lease engine) | M | **Med** | Correctness-critical: leasing/visibility/idempotency under concurrency + pooling — the one piece to test hard |
| Phase 3 (SDK + enqueue seams) | M | Low–Med | Mostly assembly over existing event/workflow/realtime/SDK seams |
| Phase 4 (S3 + presign + dedup) | M | Low–Med | Standard object-store integration |
| Phase 5 (ops + extensions) | M–L | Low | Operational; build on demand |

The single highest-care item is **Phase 2's lease engine** — at-least-once delivery, visibility
timeouts, and idempotency must be correct under concurrent workers and connection pooling (mirror
the care taken for RLS under PgBouncer). Everything else is additive plumbing around an unchanged
server and sandbox.

## Decisions (confirmed for v1)

1. **Worker transport:** **HTTP pull-lease + realtime "wake" nudge.** Workers pull (lease) over
   HTTP; the realtime hub sends a lightweight "queue has work" nudge so idle workers wake without
   tight polling. Keeps workers behind NAT (no inbound port), gives free backpressure, and recovers
   from crashes via lease expiry. (Push/gRPC rejected: needs inbound worker connectivity and a
   hand-built backpressure path; worse fit for browser-automation boxes with persistent profiles.)
2. **Queue substrate:** **Postgres `SELECT … FOR UPDATE SKIP LOCKED`** on the `jobs` projection.
   Reuses the one datastore — no new infrastructure to deploy/secure/back up; sufficient to dozens
   of workers / moderate throughput. (A dedicated Redis/NATS broker is revisited only if a real
   high-throughput need appears.)
3. **Default blob backend:** **`local` for dev/single-host, `s3`/R2 for production**, selected per
   project like the deployment `Driver`.
4. **Worker languages:** **TypeScript SDK first** (matches existing provider/browser-automation
   code — Playwright, ffmpeg, provider SDKs), **Rust crate second** for native/high-throughput
   workers.
5. **Delivery semantics:** **at-least-once + idempotency keys.** A job may run twice (lease expiry
   after a worker actually finished); idempotency keys make enqueue safe and content-addressed
   blobs neutralize duplicate side-effects. (Exactly-once rejected: far more machinery, and it
   still can't make an *external provider call* exactly-once — the real side-effect lives outside
   the transaction, so the complexity wouldn't close the actual gap.)

## A standing caveat (from the portfolio thesis)

Like the multi-project control plane, this is a **cost-side** win — it lets Atomo *own* a class of
app (external-orchestration / media pipelines) it currently has to rent a Node backend for. It does
**not** acquire users or solve distribution. Build it when a real side-effect-heavy consumer will
dogfood it and Atomo's edge (event-sourced audit/replay, one owned core) justifies owning the stack
over a batteries-included alternative. The design doesn't expire; phase it in when a workload makes
it pay.

## See also
- [Multi-tenant](/guide/advanced/multi-tenant) — `tenant_id` + RLS that jobs/assets inherit
- [Multi-Project Platform (Design)](/guide/advanced/multi-project-design) — the deployment plane this composes with
- [Custom Event Stores](/guide/advanced/event-stores) — where job lifecycle events live
- [Architecture Overview](/guide/architecture) — the River of Events / Energy Hub pillars this realizes
