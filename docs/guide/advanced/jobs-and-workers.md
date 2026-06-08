---
title: Durable Jobs & External Workers
description: Enqueue durable, event-sourced jobs and run them in trusted out-of-process workers — the right way to do provider calls, browser automation, and media pipelines on Atomo.
---

# Durable Jobs & External Workers

Atomo's plugin sandbox (WASM / JS) is deliberately restrictive — great for short, safe, in-data-path
hooks, wrong for **long-running, side-effect-heavy** work (calling flaky external APIs, browser
automation, `ffmpeg`/image pipelines, minute-long polling, large binaries).

For that work Atomo provides a **durable job queue** plus **external workers**: the server owns an
event-sourced job lifecycle; a trusted worker process (any language, full native ecosystem) **pulls**
jobs, does the messy I/O, and reports results back. The server is the brain; the worker is the hands.

> For the *why* and the full design, see
> [External Workers & Blob Storage](/guide/advanced/workers-and-blobs-design).

## The shape

```
 enqueue                          lease (X-Worker-Token)
 ───────▶  POST /jobs   ──┐    ┌──────────────────────────┐
 workflow Job step ───────┤    │   your worker process    │
 JobStore::enqueue ───────┘    │  @atomo-cc/worker-sdk     │
                               │  Playwright · ffmpeg · …  │
        ┌──────────────────┐   └───────────┬──────────────┘
 poll   │   atomo-server   │◀── complete / fail / heartbeat
 ──────▶│  jobs (events +  │
 GET /jobs/{id}  projection)│
        └──────────────────┘
```

## 1. Mint a worker token (admin)

Workers authenticate with a **worker token** — a credential distinct from user logins, scoped to
specific queues. An admin mints one (the plaintext is shown **once**):

```bash
curl -X POST http://localhost:3000/jobs/workers \
  -H "authorization: Bearer $ADMIN_JWT" -H 'content-type: application/json' \
  -d '{"name":"media-worker","queues":["media-gen"]}'
# → 201 { "id": "...", "token": "wkr_…", "queues": ["media-gen"] }
```

The token can only lease the queues it was minted for (`["*"]` allows any). Store it as a secret for
the worker; only its SHA-256 is kept server-side. Admins can `GET /jobs/workers` to list tokens
(metadata only) and `DELETE /jobs/workers/{id}` to revoke one — a revoked token's requests
immediately become `401`.

## 2. Enqueue work

Three ways, all idempotent when you pass an `idempotencyKey`:

**From an app/UI (HTTP).** Any authenticated user; the job is stamped with the caller's tenant.

```bash
curl -X POST http://localhost:3000/jobs \
  -H "authorization: Bearer $JWT" -H 'content-type: application/json' \
  -d '{"queue":"media-gen","kind":"video.generate","payload":{"prompt":"a calm sea"}}'
# → 201 { "id": "..." }
```

**From a workflow (no code).** Add a `Job` step — the new job id lands in the workflow context as
`job_id`:

```json
{ "name": "kick-off-render",
  "action": { "Job": { "queue": "media-gen", "kind": "video.generate",
                       "payload": { "prompt": "…" }, "idempotency_key": "deal-42" } } }
```

**From Rust** (in-process): `JobStore::enqueue(queue, kind, payload, idempotency_key, max_attempts, priority, tenant_id)`.

## 3. Run a worker

Use [`@atomo-cc/worker-sdk`](https://www.npmjs.com/package/@atomo-cc/worker-sdk) — you write a
handler per `kind`; the SDK owns the lease → heartbeat → complete/fail loop.

```ts
import { createWorker, NonRetryableError } from "@atomo-cc/worker-sdk";

const worker = createWorker({
  url: "http://localhost:3000",
  token: process.env.ATOMO_WORKER_TOKEN!,
  queues: ["media-gen"],
  concurrency: 4,
});

worker.on("video.generate", async ({ job, signal }) => {
  if (typeof job.payload !== "object") throw new NonRetryableError("bad payload");
  const mp4 = await runProviderPipeline(job.payload, { signal }); // your native code
  return { assetId: await upload(mp4) };                          // → stored as the job result
});

worker.start();
process.on("SIGTERM", () => worker.stop());
```

## 4. Poll the result

```bash
curl http://localhost:3000/jobs/<id> -H "authorization: Bearer $JWT"
# → { "id", "queue", "kind", "status": "succeeded", "attempts": 1,
#     "maxAttempts": 5, "result": { "assetId": "…" }, "error": null }
```

`status` is one of `queued` · `leased` · `succeeded` · `dead`. A tenant-bound user only sees their
own tenant's jobs.

## Semantics you can rely on

- **At-least-once delivery.** A job can run twice (e.g. a lease expired *after* the handler finished).
  Make handlers idempotent; `idempotencyKey` makes *enqueue* idempotent.
- **Leases & crash recovery.** A leased job has a visibility window; if the worker dies, the lease
  expires and a background sweep (every `ATOMO_JOB_RECLAIM_INTERVAL` seconds, default 30) returns the
  job to the queue. Each lease carries a unique token, so a revived zombie worker can't complete a
  job that was already re-leased (its `heartbeat`/`complete`/`fail` get `409`).
- **Retry / backoff / dead-letter.** A failed attempt is retried with exponential backoff until
  `maxAttempts`, then **dead-lettered** (`status: "dead"`, never silently dropped). Throw
  `NonRetryableError` (or report `retryable: false`) to dead-letter immediately.
- **Concurrent workers.** Dispatch uses Postgres `SELECT … FOR UPDATE SKIP LOCKED`, so many workers
  lease disjoint jobs from one database without contention.
- **Event-sourced.** Every transition is emitted as a `Job` model event (audit / history / replay) —
  the operational advantage over a mutable status column for flaky pipelines.

## Security boundary

A worker is **trusted relative to the sandbox** but still least-privilege: its token is scoped to
specific queues, distinct from user JWTs, and revocable. The plugin sandbox is untouched — a plugin
can *enqueue* a job but never *becomes* a worker.

## See also
- [Jobs API (REST)](/api/jobs) — endpoint reference
- [Workflows API](/api/workflows) — the `Job` step
- [External Workers & Blob Storage](/guide/advanced/workers-and-blobs-design) — the design
- [Multi-tenant](/guide/advanced/multi-tenant) — jobs carry `tenant_id`
