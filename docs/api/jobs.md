# Jobs (REST)

The durable job queue lets apps enqueue work and trusted external workers pull it. Jobs are
event-sourced (each transition emits a `Job` model event) and crash-safe (leases with a
visibility-timeout reclaim). For the concepts and a worker walkthrough, see
[Durable Jobs & External Workers](/guide/advanced/jobs-and-workers).

## Endpoints

```http
# App plane (user JWT)
POST   /jobs                  # enqueue a job (any authenticated user)
GET    /jobs/{id}             # poll a job's status + result

# Worker plane (X-Worker-Token)
POST   /jobs/lease            # lease up to N eligible jobs
POST   /jobs/{id}/heartbeat   # extend a held lease
POST   /jobs/{id}/complete    # mark a leased job succeeded (+ result)
POST   /jobs/{id}/fail        # report a failed attempt

# Admin plane (user JWT, Admin role)
POST   /jobs/workers          # mint a worker token
```

## App plane

### `POST /jobs` — enqueue

Requires an authenticated user; the job is stamped with the caller's tenant. Idempotent when
`idempotencyKey` is supplied (a repeat returns the existing job id).

```json
// request
{ "queue": "media-gen", "kind": "video.generate",
  "payload": { "prompt": "…" },
  "idempotencyKey": "deal-42",   // optional
  "maxAttempts": 5,              // optional, default 5
  "priority": 0 }               // optional, default 0
// 201 → { "id": "01J…" }
```

`400` if `queue`/`kind` is blank; `401` without auth.

### `GET /jobs/{id}` — poll status

```json
// 200
{ "id": "01J…", "queue": "media-gen", "kind": "video.generate",
  "status": "succeeded",          // queued | leased | succeeded | dead
  "attempts": 1, "maxAttempts": 5,
  "result": { "assetId": "…" },   // present once succeeded
  "error": null }
```

`401` without auth; `404` if unknown — or if a tenant-bound user requests another tenant's job.

## Worker plane

All worker-plane requests send `X-Worker-Token: wkr_…`. A token may only lease the queues it was
minted for (`403` otherwise); a missing/invalid/revoked token is `401`.

### `POST /jobs/lease`

```json
// request
{ "queues": ["media-gen"], "capacity": 4, "visibilitySecs": 30 }
// 200
{ "jobs": [ { "id", "queue", "kind", "payload", "attempts", "maxAttempts", "leaseId" } ] }
```

Each returned job carries a unique `leaseId` that must be echoed back to heartbeat/complete/fail.
`capacity` is clamped to 1–100, `visibilitySecs` to 1–86400. An empty `queues` is `400`.

### `POST /jobs/{id}/heartbeat`

```json
{ "leaseId": "…", "visibilitySecs": 30 }   // → 204; 409 if the lease was lost
```

### `POST /jobs/{id}/complete`

```json
{ "leaseId": "…", "result": { "assetId": "…" } }   // → 204; 409 if the lease is stale
```

### `POST /jobs/{id}/fail`

```json
// request
{ "leaseId": "…", "error": "provider rate limited", "retryable": true }
// 200 → retry scheduled with backoff
{ "outcome": "retry", "delaySecs": 5 }
// 200 → attempts exhausted / non-retryable
{ "outcome": "dead" }
// 409 → the lease is stale (no-op)
```

## Admin plane

### `POST /jobs/workers` — mint a worker token

Requires the **Admin** role. The plaintext token is returned **once** (only its SHA-256 is stored).

```json
// request
{ "name": "media-worker", "queues": ["media-gen"] }   // ["*"] = any queue
// 201
{ "id": "…", "token": "wkr_…", "queues": ["media-gen"] }
```

`401` without auth; `403` for a non-admin user.

## Configuration

| Env | Default | Meaning |
| --- | --- | --- |
| `ATOMO_JOB_RECLAIM_INTERVAL` | `30` | Seconds between sweeps that return expired leases to the queue |

## See also
- [Durable Jobs & External Workers](/guide/advanced/jobs-and-workers) — concepts + worker SDK
- [Workflows API](/api/workflows) — the `Job` step enqueues onto this queue
