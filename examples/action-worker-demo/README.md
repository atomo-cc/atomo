# Action Worker Demo

End-to-end example of the Atomo action/worker lifecycle.

## Architecture

```
schema.ts  ──→  Atomo parses events + actions
                     │
Client creates Post  │
         ↓           │
   Created event  ───┤
         ↓           │
  processPost job    │  (enqueued by action dispatcher)
         ↓           │
   worker.ts leases  │
         ↓           │
  crud.update(Post)  │  with origin: "processPost"
         ↓           │
   Updated event  ───┘
         ↓
  processPost SKIPPED   (origin == action → loop prevention)
  onStatusChange FIRES  (different action → not suppressed)
```

## Files

| File | Purpose |
|------|---------|
| `schema.ts` | Post model with events (`on.created → processPost`) and action definitions |
| `worker.ts` | TypeScript worker that handles `processPost` and `onStatusChange` |
| `generated/client.ts` | Typed client generated from the schema (models, CRUD, action types) |

## What the generated client provides

- **`Post` interface** — typed model fields
- **`processPostInput`** — `Pick<Post, 'id' | 'title' | 'content'>` (auto-derived from schema)
- **`TypedClient`** — `client.post.create(data)` with full autocomplete instead of `crud.create("Post", data)`
- **`ActionHandlers`** — maps action names to their input types for worker payload narrowing
- **`TypedWorker`** — typed `worker.on("processPost", ctx => ...)` where `ctx.job.payload.input` narrows automatically

## Running

```bash
# 1. Start the Atomo server with this schema
atomo serve --schema ./schema.ts

# 2. Run the worker
npx tsx worker.ts

# 3. Create a post (triggers the lifecycle)
curl -X POST http://localhost:3000/api/worker/crud/Post \
  -H "Content-Type: application/json" \
  -H "X-Worker-Token: <token>" \
  -d '{"data": {"title": "Hello", "content": "World", "status": "draft"}}'
```

## Loop prevention

When `processPost` updates the post's status to `"processed"`, the Updated event carries
`origin: "processPost"`. The action dispatcher sees that `processPost` matches the origin
and skips it. But `onStatusChange` is a different action, so it fires normally.

This is the same mechanism used in the integration test (`action_lifecycle.rs`).
