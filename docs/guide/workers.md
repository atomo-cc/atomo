# Workers

Workers are external TypeScript processes that handle side effects triggered by data changes in Atomo. When a record is created, updated, or deleted, your schema can declare that an **action** should fire. Atomo enqueues a durable job, and your worker leases it, runs your logic, and reports the result.

Workers run outside the Atomo server. They can live anywhere that runs Node.js (or Bun/Deno) -- a separate container, a serverless function, your laptop during development. They communicate with Atomo over HTTP using the `@atomo-cc/worker-sdk` package.

**When to use workers vs. plain CRUD:**

| Use case | Approach |
|----------|----------|
| Read/write data, validate, enforce RBAC | CRUD API (built-in) |
| Send an email after signup | Worker |
| Generate a thumbnail when an image is uploaded | Worker |
| Update a search index when content changes | Worker |
| Run an AI pipeline over submitted text | Worker |

The rule of thumb: if it is a side effect, needs external I/O, or takes more than a few milliseconds, put it in a worker.

## Quick start

### 1. Declare events and actions in your schema

```ts
// schema.ts
export interface Post {
  id: string;
  title: string;
  content: string;
  status: string;
  createdAt: Date;
}

export const schema = {
  models: {
    Post: {
      tableName: "posts",
      events: {
        created: [{ action: "processPost" }],
      },
    },
  },
  actions: {
    processPost: {
      input: { pick: { model: "Post", fields: ["id", "title", "content"] } },
    },
  },
};

export default schema;
```

### 2. Generate types

```bash
atomo codegen
```

This produces `generated/client.ts` with typed interfaces for your models, actions, and worker context.

### 3. Write a worker

```ts
// worker.ts
import { createWorker } from "@atomo-cc/worker-sdk";
import type { ActionHandlers } from "./generated/client";

const worker = createWorker({
  url: process.env.ATOMO_URL ?? "http://localhost:3000",
  token: process.env.WORKER_TOKEN!,
  queues: ["actions"],
});

worker.on("processPost", async ({ job, crud }) => {
  const input = (job.payload as { input: ActionHandlers["processPost"]["input"] }).input;

  console.log(`Processing "${input.title}"`);

  // Write back via CRUD -- origin prevents this from re-triggering processPost
  await crud.update("Post", input.id, { status: "processed" }, { origin: "processPost" });
});

worker.start();
```

### 4. Run it

```bash
# Terminal 1: start the Atomo server
atomo serve --schema ./schema.ts

# Terminal 2: start the worker
npx tsx worker.ts
```

Create a post and watch the worker pick it up:

```bash
curl -X POST http://localhost:3000/api/worker/crud/Post \
  -H "Content-Type: application/json" \
  -H "X-Worker-Token: <token>" \
  -d '{"data": {"title": "Hello", "content": "World", "status": "draft"}}'
```

The canonical end-to-end example lives in `examples/action-worker-demo/`.

## Schema events

Events are declared on each model under the `events` key. Three lifecycle hooks are available: `created`, `updated`, and `deleted`.

```ts
events: {
  created: [{ action: "processPost" }],
  updated: [
    { action: "onStatusChange", condition: { changedAny: ["status"] } },
  ],
  deleted: [{ action: "cleanupPost" }],
}
```

Each entry in the array is an object with:

| Field | Required | Description |
|-------|----------|-------------|
| `action` | yes | Name of the action to enqueue (must exist in `schema.actions`) |
| `condition` | no | Only fire when the condition is met |

### Conditions

Conditions narrow when an action fires. Available conditions for `updated` events:

- **`changedAny`** -- fires if any of the listed fields changed:
  ```ts
  { action: "reindex", condition: { changedAny: ["title", "content"] } }
  ```
- **`fieldEquals`** -- fires when a field has a specific value after the update:
  ```ts
  { action: "onPublish", condition: { fieldEquals: { status: "published" } } }
  ```

Conditions can be combined. When both are present, all must be satisfied (AND logic).

`created` and `deleted` events do not support `changedAny` (there is no "previous value"), but `fieldEquals` works on the created/deleted snapshot.

## Action definitions

Actions live under `schema.actions` and define the payload shape that the worker receives.

### Pick inputs

The most common pattern: pick fields from an existing model.

```ts
actions: {
  processPost: {
    input: { pick: { model: "Post", fields: ["id", "title", "content"] } },
  },
}
```

The codegen produces a Pick type:

```ts
// generated/client.ts
export type processPostInput = Pick<Post, 'id' | 'title' | 'content'>;
```

### Object inputs

For actions not tied to a single model, define the shape directly:

```ts
actions: {
  sendEmail: {
    input: {
      object: {
        to: "string",
        subject: "string",
        body: "string",
      },
    },
  },
}
```

## Writing a worker

### createWorker

`createWorker(opts)` returns a `Worker` instance. The options:

| Option | Default | Description |
|--------|---------|-------------|
| `url` | -- | Atomo server base URL (e.g. `"http://localhost:3000"`) |
| `token` | -- | Worker authentication token (sent as `X-Worker-Token`) |
| `queues` | -- | Array of queue names to lease from |
| `concurrency` | `1` | Max jobs in flight at once |
| `visibilitySecs` | `30` | How long a leased job stays invisible to other workers |
| `heartbeatSecs` | `visibilitySecs / 3` | Auto-heartbeat interval |
| `pollIntervalMs` | `1000` | How often to poll when idle |
| `fetch` | `globalThis.fetch` | Injectable fetch (Node 18+) |
| `onError` | `console.error` | Error sink for transport/background errors |

### Registering handlers

Use `.on(kind, handler)` to register a handler for a job kind. Handlers are chainable:

```ts
worker
  .on("processPost", handleProcessPost)
  .on("onStatusChange", handleStatusChange)
  .on("generateThumbnail", handleThumbnail);
```

If a job arrives with no matching handler, it is failed immediately as non-retryable.

### The handler context

Every handler receives a `JobContext` with these members:

```ts
worker.on("processPost", async (ctx) => {
  ctx.job;       // the leased job (id, kind, payload, attempts, maxAttempts, leaseId)
  ctx.signal;    // AbortSignal -- aborted if the lease is lost or the worker stops
  ctx.crud;      // CrudClient for calling back into Atomo
  ctx.heartbeat; // manually extend the lease (the SDK also auto-heartbeats)
  ctx.progress;  // publish a live progress update
});
```

### ctx.job

The `job` object contains:

| Field | Description |
|-------|-------------|
| `id` | Unique job ID |
| `queue` | Queue the job was leased from |
| `kind` | Action name (e.g. `"processPost"`) |
| `payload` | The action payload, including `input` (typed by the schema) |
| `attempts` | Current attempt number (starts at 1) |
| `maxAttempts` | Maximum attempts before dead-lettering |
| `leaseId` | Opaque lease identifier (used internally) |

Access the typed input:

```ts
const input = (job.payload as { input: ActionHandlers["processPost"]["input"] }).input;
const { id, title, content } = input;
```

### ctx.crud

The CRUD client lets your worker read and write data through Atomo with full validation and RBAC. The client is automatically scoped to the actor (user/role/tenant) that triggered the original event.

```ts
// Create
const comment = await ctx.crud.create("Comment", {
  postId: input.id,
  body: "Auto-generated summary",
});

// Read
const post = await ctx.crud.findById("Post", input.id);
const drafts = await ctx.crud.findMany("Post", {
  where: { status: "draft" },
  limit: 10,
});

// Update
await ctx.crud.update("Post", input.id, { status: "processed" }, { origin: "processPost" });

// Delete
await ctx.crud.delete("Comment", commentId, { origin: "processPost" });
```

The CRUD methods:

| Method | Signature |
|--------|-----------|
| `create` | `create(model, data, opts?) -> record` |
| `findById` | `findById(model, id) -> record \| null` |
| `findMany` | `findMany(model, { where?, limit?, offset? }) -> record[]` |
| `update` | `update(model, id, data, opts?) -> record` |
| `delete` | `delete(model, id, opts?) -> void` |

All write methods (`create`, `update`, `delete`) accept an optional `opts` parameter with an `origin` field for loop prevention (see below).

On non-OK responses, the client throws a `CrudError` with `status` and `body` properties.

### ctx.signal

The `AbortSignal` is aborted when:

- A heartbeat returns 409 (the lease was taken by another worker)
- `worker.stop()` is called

Use it to cancel long-running work:

```ts
worker.on("generateVideo", async ({ job, signal }) => {
  const result = await renderVideo(job.payload.input, { signal });
  return { videoUrl: result.url };
});
```

### ctx.progress

Publish live progress updates to clients listening on the job's realtime channel (`job:{id}`). Progress updates are ephemeral -- they fan out over realtime but are not persisted.

```ts
worker.on("processPost", async ({ job, crud, progress }) => {
  await progress({ percent: 0, message: "Starting..." });

  const analysis = await analyzeContent(job.payload.input.content);
  await progress({ percent: 0.5, message: "Analysis complete" });

  await crud.update("Post", job.payload.input.id, { analysis }, { origin: "processPost" });
  await progress({ percent: 1, message: "Done" });
});
```

The `ProgressUpdate` shape:

| Field | Type | Description |
|-------|------|-------------|
| `percent` | `number` (0..1) | Fraction complete |
| `message` | `string` | Human-readable status |
| `data` | `unknown` | Arbitrary structured detail |

If the lease has been lost, `progress()` aborts the signal silently.

### Lifecycle: start and stop

```ts
worker.start();  // begins the lease loop (idempotent)

// Graceful shutdown
process.on("SIGTERM", async () => {
  await worker.stop();  // stops leasing, drains in-flight handlers
  process.exit(0);
});
```

`stop()` returns a promise that resolves once all in-flight handlers have completed. No new jobs are leased after `stop()` is called.

## Loop prevention

When a worker writes back into Atomo via `ctx.crud`, that mutation can itself trigger events. Without safeguards, this creates infinite loops: `processPost` updates a Post, which fires `processPost` again, forever.

The solution is the **origin** field.

### How it works

```
Client creates Post
       |
  Created event fires
       |
  processPost job enqueued
       |
  Worker leases and processes
       |
  crud.update("Post", id, data, { origin: "processPost" })
       |
  Updated event fires
       |
  Action dispatcher checks: origin == "processPost"?
       |
  processPost: SKIPPED (same action as origin)
  onStatusChange: FIRES (different action, not suppressed)
```

The origin is compared against each action name on the event. Only the action that matches the origin string is suppressed. All other actions fire normally.

### When to use it

Pass `origin` on every write-back that could trigger the same action:

```ts
// Always pass origin when updating the model that triggered this action
await ctx.crud.update("Post", id, { status: "processed" }, { origin: "processPost" });

// Also pass origin on create/delete if they trigger events on the same action
await ctx.crud.delete("Post", id, { origin: "cleanupPost" });
```

If your worker writes to a different model that has no events pointing back to the same action, you can omit `origin`. But when in doubt, include it -- it is a no-op if there is no matching action to suppress.

## Generated types

Running `atomo codegen` produces `generated/client.ts` with types that give you full autocomplete and compile-time safety. The key exports:

### ActionHandlers

Maps action names to their input types:

```ts
export interface ActionHandlers {
  processPost: { input: processPostInput };
  onStatusChange: { input: onStatusChangeInput };
}
```

Use it to narrow `job.payload`:

```ts
worker.on("processPost", async ({ job }) => {
  const input = (job.payload as { input: ActionHandlers["processPost"]["input"] }).input;
  // input.id, input.title, input.content -- all typed
});
```

### TypedJobContext\<K\>

A fully typed version of `JobContext` where `job.payload.input` is narrowed to the correct action's input type, and `crud` is a `TypedClient` instead of the untyped `CrudClient`:

```ts
export interface TypedJobContext<K extends keyof ActionHandlers> {
  job: {
    id: string;
    queue: string;
    kind: K;
    payload: { input: ActionHandlers[K]['input']; [key: string]: unknown };
    attempts: number;
    maxAttempts: number;
    leaseId: string;
  };
  signal: AbortSignal;
  heartbeat(): Promise<void>;
  progress(update: { percent?: number; message?: string; data?: unknown }): Promise<void>;
  crud: TypedClient;
}
```

### TypedWorker

A typed version of `Worker` that constrains `.on()` to known action names:

```ts
export interface TypedWorker {
  on<K extends keyof ActionHandlers>(
    kind: K,
    handler: (ctx: TypedJobContext<K>) => Promise<unknown> | unknown,
  ): TypedWorker;
  start(): void;
  stop(): Promise<void>;
}
```

Cast your worker to `TypedWorker` for full autocomplete:

```ts
import { createWorker } from "@atomo-cc/worker-sdk";
import type { TypedWorker } from "./generated/client";

const worker = createWorker({ /* ... */ }) as unknown as TypedWorker;

// Now "processPost" autocompletes, and ctx.job.payload.input is typed
worker.on("processPost", async (ctx) => {
  const { id, title } = ctx.job.payload.input; // typed as Pick<Post, 'id' | 'title' | 'content'>
});
```

### TypedClient

A model-aware CRUD client with per-model methods:

```ts
export class TypedClient {
  post: {
    create(data: Partial<Post>): Promise<Post>;
    findById(id: string): Promise<Post | null>;
    update(id: string, data: Partial<Post>): Promise<Post>;
    delete(id: string): Promise<void>;
    findMany(opts?: Record<string, unknown>): Promise<Post[]>;
  };
}
```

Inside a `TypedJobContext`, `ctx.crud` is a `TypedClient`, so you get:

```ts
const post = await ctx.crud.post.findById(input.id);  // returns Post | null
```

### ModelEventMap

Documents which events map to which actions:

```ts
export interface ModelEventMap {
  'Post.created': 'processPost';
  'Post.updated': 'onStatusChange';
}
```

This is primarily for documentation and tooling -- you do not need to use it at runtime.

## Worker tokens and capabilities

Workers authenticate with Atomo using a **worker token**, passed as the `X-Worker-Token` header on every request. Tokens are registered with the server and scoped to specific capabilities.

### Capability format

Capabilities use a colon-separated syntax:

| Capability | Scope |
|------------|-------|
| `crud:*` | Full CRUD access to all models |
| `crud:Post` | All operations on the Post model |
| `crud:Post:read,update` | Only read and update on Post |
| `action:*` | Handle any action (lease jobs of any kind) |
| `action:processPost` | Handle only the `processPost` action |

### Registering a token

Register a worker token via the Atomo CLI or API:

```bash
atomo worker-token create \
  --name "media-worker" \
  --capabilities "action:processPost,action:generateThumbnail,crud:Post:read,update,crud:Media"
```

### Least-privilege principle

Scope tokens as tightly as possible. A worker that only processes posts and writes back status changes needs:

```
action:processPost
crud:Post:read,update
```

It does not need `crud:*` or `action:*`. If the token lacks a required capability, the server returns 403 and the CRUD call throws a `CrudError`.

## Error handling

### Retryable errors

By default, any thrown error is **retryable**. The server re-enqueues the job with exponential backoff up to `maxAttempts`:

```ts
worker.on("processPost", async ({ job }) => {
  const res = await fetch("https://api.example.com/analyze", { body: job.payload.input.content });
  if (!res.ok) {
    throw new Error(`API returned ${res.status}`);  // retryable -- will be retried
  }
});
```

### Non-retryable errors

Throw `NonRetryableError` to dead-letter the job immediately with no further retries. Use this for permanent failures:

```ts
import { NonRetryableError } from "@atomo-cc/worker-sdk";

worker.on("processPost", async ({ job, crud }) => {
  const post = await crud.findById("Post", job.payload.input.id);
  if (!post) {
    throw new NonRetryableError(`Post ${job.payload.input.id} not found -- cannot process`);
  }
});
```

### Dead-lettering

When a job exhausts all retries (or is marked non-retryable), it moves to the dead-letter queue. Dead-lettered jobs retain their full payload and error history for debugging.

### CrudError

CRUD calls that receive a non-OK HTTP response throw `CrudError` with `status` and `body`:

```ts
import { CrudError } from "@atomo-cc/worker-sdk";

worker.on("processPost", async ({ job, crud }) => {
  try {
    await crud.update("Post", job.payload.input.id, { status: "processed" }, { origin: "processPost" });
  } catch (err) {
    if (err instanceof CrudError && err.status === 404) {
      // Post was deleted between enqueue and processing -- skip
      return;
    }
    throw err;  // anything else is retryable
  }
});
```

### Lease loss

If a heartbeat returns 409, the lease was taken by another worker instance (usually because processing took longer than `visibilitySecs`). The SDK aborts `ctx.signal` and the handler should exit. The other instance will process the job.

## Best practices

### Make handlers idempotent

Delivery is **at-least-once**: a job can run twice if the lease expires after the handler finishes but before `complete` is acknowledged. Design handlers so that running them twice produces the same result:

```ts
// Good: upsert-style -- safe to run twice
await crud.update("Post", id, { status: "processed" }, { origin: "processPost" });

// Bad: append-style -- creates duplicates on retry
await crud.create("AuditLog", { postId: id, event: "processed" });

// Better: use an idempotency key
const existing = await crud.findMany("AuditLog", {
  where: { postId: id, event: "processed" },
  limit: 1,
});
if (existing.length === 0) {
  await crud.create("AuditLog", { postId: id, event: "processed" });
}
```

### Tune visibility and heartbeats

The default `visibilitySecs: 30` works for fast handlers. For long-running jobs (video encoding, AI pipelines), increase it:

```ts
const worker = createWorker({
  url: "http://localhost:3000",
  token: process.env.WORKER_TOKEN!,
  queues: ["actions"],
  visibilitySecs: 300,   // 5-minute lease
  heartbeatSecs: 60,     // heartbeat every minute
});
```

The SDK auto-heartbeats at the configured interval. You can also call `ctx.heartbeat()` manually before a particularly long operation.

### Use concurrency for throughput

For I/O-bound handlers, increase `concurrency` to process multiple jobs in parallel:

```ts
const worker = createWorker({
  // ...
  concurrency: 5,  // up to 5 jobs in flight
});
```

For CPU-bound work, keep concurrency at 1 and scale horizontally (run multiple worker processes).

### Respect the abort signal

Long-running handlers should check `ctx.signal` and bail out if aborted:

```ts
worker.on("processBatch", async ({ job, signal }) => {
  for (const item of job.payload.input.items) {
    if (signal.aborted) return;  // lease was lost or worker is stopping
    await processItem(item);
  }
});
```

### Handle graceful shutdown

Always wire up `SIGTERM`/`SIGINT` so in-flight jobs complete before exit:

```ts
const shutdown = async () => {
  console.log("Shutting down...");
  await worker.stop();
  process.exit(0);
};

process.on("SIGTERM", shutdown);
process.on("SIGINT", shutdown);
```

### Always pass origin on write-backs

When in doubt, pass `origin`. It costs nothing if there is no matching action to suppress, and it prevents accidental infinite loops:

```ts
await crud.update("Post", id, data, { origin: "processPost" });
await crud.create("Notification", data, { origin: "processPost" });
await crud.delete("TempFile", id, { origin: "processPost" });
```

### Keep handlers focused

Each handler should do one thing. If an action needs multiple steps, break them into separate actions chained through events rather than building a monolithic handler.

## Further reading

- `examples/action-worker-demo/` -- minimal end-to-end example with schema, worker, and generated types
- `services/crm-service/workers/` -- multi-model CRM workers demonstrating cross-model writes, Activity logging, and least-privilege capabilities
- [Architecture overview](/guide/architecture) -- how workers fit into the broader Atomo platform
