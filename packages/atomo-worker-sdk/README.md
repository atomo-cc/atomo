# @atomo-cc/worker-sdk

Write an [Atomo](https://docs.atomo.cc) **external job worker** by supplying handlers. The SDK owns
the `lease → heartbeat → complete/fail` loop against the server's `/jobs` API, so your code only
does the actual work — calling external APIs, browser automation, media pipelines, anything Node can
do.

This is the *worker* (pull) side of Atomo's external-worker model. The server is the event-sourced
brain; the worker is the hands. See
[External Workers & Blob Storage](https://docs.atomo.cc/guide/advanced/workers-and-blobs-design).

## Install

```bash
npm install @atomo-cc/worker-sdk
```

Requires Node ≥ 18 (uses the global `fetch`).

## Usage

```ts
import { createWorker, NonRetryableError } from "@atomo-cc/worker-sdk";

const worker = createWorker({
  url: "http://localhost:3000",
  token: process.env.ATOMO_WORKER_TOKEN!, // minted by an admin via POST /jobs/workers
  queues: ["media-gen"],
  concurrency: 4,
});

worker.on("video.generate", async ({ job, signal }) => {
  if (typeof job.payload !== "object") throw new NonRetryableError("bad payload"); // dead-letter
  const mp4 = await runProviderPipeline(job.payload, { signal }); // your native code
  return { assetId: await upload(mp4) };                          // → JobSucceeded
});

worker.start();
process.on("SIGTERM", () => worker.stop());
```

## Semantics

- **At-least-once delivery.** A job can run twice (e.g. a lease expired *after* the handler actually
  finished). Make handlers idempotent; the server makes *enqueue* idempotent via an idempotency key.
- **Failure → retry.** A thrown error fails the job; the **server** applies the retry/backoff policy
  and dead-letters once attempts are exhausted. Throw `NonRetryableError` to dead-letter immediately.
- **Heartbeats are automatic.** While a handler runs, the SDK extends the lease. If the lease is lost
  (a heartbeat returns `409`), `ctx.signal` aborts so a cooperating handler can bail early.
- **Live progress.** Call `ctx.progress({ percent?, message?, data? })` to publish an ephemeral
  update; it fans out over realtime on channel `job:{id}` (not persisted) for a UI to show progress.
- **Capability-scoped.** The worker token only lets you lease the queues it was minted for; leasing
  any other queue is rejected by the server.

## Handling Lifecycle Actions

When a schema declares lifecycle event bindings (e.g. `on.created: [processPost]`), the server's
**action dispatcher** enqueues jobs on the `"actions"` queue with `kind` set to the action name.
Register handlers by action name:

```ts
const worker = createWorker({
  url: "http://localhost:3000",
  token: process.env.ATOMO_WORKER_TOKEN!,
  queues: ["actions"],
});

worker.on("processPost", async ({ job }) => {
  const { input, model, event } = job.payload as any;
  // input contains the fields declared in ActionDef.input (e.g. { id, title })
  // model is the source model name, event is "created"/"updated"/"deleted"
  await callExternalApi(input);
});

worker.start();
```

## API

- `createWorker(options)` → `Worker` with `.on(kind, handler)`, `.start()`, `.stop()`.
- `NonRetryableError` — throw to skip retries.
- `handleJob`, `JobsClient` — lower-level building blocks (exported for testing/advanced use).
