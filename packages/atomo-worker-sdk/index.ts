/**
 * `@atomo-cc/worker-sdk` — write an Atomo job worker by supplying handlers; the SDK owns the
 * lease → heartbeat → complete/fail loop against the server's `/jobs` API.
 *
 * ```ts
 * const worker = createWorker({ url: "http://localhost:3000", token, queues: ["media-gen"] });
 * worker.on("video.generate", async ({ job }) => {
 *   const mp4 = await runProviderPipeline(job.payload); // your native code (Playwright, ffmpeg, …)
 *   return { assetId: await upload(mp4) };               // → JobSucceeded
 * });
 * worker.start();
 * ```
 *
 * Delivery is at-least-once: a job can run twice (lease expiry after the worker actually finished),
 * so handlers should be idempotent. A thrown error fails the job (server applies retry/backoff);
 * throw `NonRetryableError` to dead-letter immediately.
 */

export interface LeasedJob {
  id: string;
  queue: string;
  kind: string;
  payload: unknown;
  attempts: number;
  maxAttempts: number;
  leaseId: string;
}

/** A live progress update, published to the job's realtime channel (`job:{id}`). */
export interface ProgressUpdate {
  /** Fraction complete, 0..1 (your convention). */
  percent?: number;
  /** Human-readable status line. */
  message?: string;
  /** Arbitrary structured detail. */
  data?: unknown;
}

export interface JobContext {
  /** The leased job (with its `payload`). */
  job: LeasedJob;
  /** Aborted if the lease is lost (a heartbeat returned 409) or the worker is stopping. */
  signal: AbortSignal;
  /** Extend the lease now. The SDK also auto-heartbeats while the handler runs. */
  heartbeat(): Promise<void>;
  /** Publish a live progress update (ephemeral — fans out over realtime, not persisted). */
  progress(update: ProgressUpdate): Promise<void>;
  /** CRUD client scoped to the job's actor (role/tenant from the event that triggered the job). */
  crud: CrudClient;
}

export type JobHandler = (ctx: JobContext) => Promise<unknown> | unknown;

/** Throw from a handler to dead-letter the job immediately (no retry). */
export class NonRetryableError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "NonRetryableError";
  }
}

export interface WorkerOptions {
  /** Server base URL, e.g. `"http://localhost:3000"`. */
  url: string;
  /** Worker token (sent as `X-Worker-Token`). */
  token: string;
  /** Queues to lease from. */
  queues: string[];
  /** Max jobs in flight at once (default 1). */
  concurrency?: number;
  /** Lease visibility window in seconds (default 30). */
  visibilitySecs?: number;
  /** Auto-heartbeat interval in seconds (default `floor(visibilitySecs/3)`, min 1). */
  heartbeatSecs?: number;
  /** Idle poll interval in ms when no work is available (default 1000). */
  pollIntervalMs?: number;
  /** Injectable fetch (defaults to global `fetch`; Node ≥ 18). */
  fetch?: typeof globalThis.fetch;
  /** Background/transport error sink (default `console.error`). */
  onError?: (err: unknown) => void;
}

export interface Worker {
  /** Register a handler for a job `kind`. Chainable. */
  on(kind: string, handler: JobHandler): Worker;
  /** Begin the lease loop. Idempotent. */
  start(): void;
  /** Stop leasing and wait for in-flight handlers to drain. */
  stop(): Promise<void>;
}

/** Thin client over the `/jobs` worker endpoints. Exported for testing/advanced use. */
export class JobsClient {
  constructor(
    private readonly url: string,
    private readonly token: string,
    private readonly fetchImpl: typeof globalThis.fetch,
  ) {}

  private post(path: string, body: unknown): Promise<Response> {
    return this.fetchImpl(`${this.url}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json", "x-worker-token": this.token },
      body: JSON.stringify(body),
    });
  }

  async lease(queues: string[], capacity: number, visibilitySecs: number): Promise<LeasedJob[]> {
    const res = await this.post("/jobs/lease", { queues, capacity, visibilitySecs });
    if (!res.ok) throw new Error(`lease failed: ${res.status}`);
    const data = (await res.json()) as { jobs?: LeasedJob[] };
    return data.jobs ?? [];
  }

  /** Returns `false` if the lease was lost (409). */
  async heartbeat(id: string, leaseId: string, visibilitySecs: number): Promise<boolean> {
    const res = await this.post(`/jobs/${id}/heartbeat`, { leaseId, visibilitySecs });
    if (res.status === 409) return false;
    if (!res.ok) throw new Error(`heartbeat failed: ${res.status}`);
    return true;
  }

  async complete(id: string, leaseId: string, result: unknown): Promise<void> {
    const res = await this.post(`/jobs/${id}/complete`, { leaseId, result });
    // 409 = lease already lost; nothing to do.
    if (!res.ok && res.status !== 409) throw new Error(`complete failed: ${res.status}`);
  }

  async fail(id: string, leaseId: string, error: string, retryable: boolean): Promise<void> {
    const res = await this.post(`/jobs/${id}/fail`, { leaseId, error, retryable });
    if (!res.ok && res.status !== 409) throw new Error(`fail failed: ${res.status}`);
  }

  /** Publish a progress update. Returns `false` if the lease was lost (409). */
  async progress(id: string, leaseId: string, update: ProgressUpdate): Promise<boolean> {
    const res = await this.post(`/jobs/${id}/progress`, { leaseId, ...update });
    if (res.status === 409) return false;
    if (!res.ok) throw new Error(`progress failed: ${res.status}`);
    return true;
  }
}

// ── CRUD Client ──────────────────────────────────────────────────────────────

export interface ActorSnapshot {
  userId?: string;
  role?: string;
  tenantId?: string;
}

/** Thrown by `CrudClient` on non-OK responses. */
export class CrudError extends Error {
  constructor(
    public readonly status: number,
    public readonly body: string,
  ) {
    super(`CRUD ${status}: ${body}`);
    this.name = "CrudError";
  }
}

export interface CrudOptions {
  /** The action that caused this mutation. The server uses it for loop prevention:
   *  the action dispatcher won't re-enqueue the same action for events it caused. */
  origin?: string;
}

/** REST client for `/api/worker/crud/:model` — available as `ctx.crud` in handlers. */
export class CrudClient {
  constructor(
    private readonly url: string,
    private readonly token: string,
    private readonly fetchImpl: typeof globalThis.fetch,
    private readonly actor?: ActorSnapshot,
  ) {}

  private headers(): Record<string, string> {
    return { "content-type": "application/json", "x-worker-token": this.token };
  }

  async create(
    model: string,
    data: Record<string, unknown>,
    opts?: CrudOptions,
  ): Promise<Record<string, unknown>> {
    const res = await this.fetchImpl(`${this.url}/api/worker/crud/${model}`, {
      method: "POST",
      headers: this.headers(),
      body: JSON.stringify({ data, actor: this.actor, origin: opts?.origin }),
    });
    if (!res.ok) throw new CrudError(res.status, await res.text());
    return (await res.json()) as Record<string, unknown>;
  }

  async findById(model: string, id: string): Promise<Record<string, unknown> | null> {
    const params = this.actor ? `?actor=${encodeURIComponent(JSON.stringify(this.actor))}` : "";
    const res = await this.fetchImpl(`${this.url}/api/worker/crud/${model}/${id}${params}`, {
      headers: this.headers(),
    });
    if (res.status === 404) return null;
    if (!res.ok) throw new CrudError(res.status, await res.text());
    return (await res.json()) as Record<string, unknown>;
  }

  async update(
    model: string,
    id: string,
    data: Record<string, unknown>,
    opts?: CrudOptions,
  ): Promise<Record<string, unknown>> {
    const res = await this.fetchImpl(`${this.url}/api/worker/crud/${model}/${id}`, {
      method: "PATCH",
      headers: this.headers(),
      body: JSON.stringify({ data, actor: this.actor, origin: opts?.origin }),
    });
    if (!res.ok) throw new CrudError(res.status, await res.text());
    return (await res.json()) as Record<string, unknown>;
  }

  async delete(model: string, id: string, opts?: CrudOptions): Promise<void> {
    const res = await this.fetchImpl(`${this.url}/api/worker/crud/${model}/${id}`, {
      method: "DELETE",
      headers: this.headers(),
      body: JSON.stringify({ actor: this.actor, origin: opts?.origin }),
    });
    if (!res.ok) throw new CrudError(res.status, await res.text());
  }

  async findMany(
    model: string,
    opts?: { where?: unknown; limit?: number; offset?: number },
  ): Promise<Record<string, unknown>[]> {
    const qs = new URLSearchParams();
    if (opts?.where) qs.set("where", JSON.stringify(opts.where));
    if (opts?.limit != null) qs.set("limit", String(opts.limit));
    if (opts?.offset != null) qs.set("offset", String(opts.offset));
    if (this.actor) qs.set("actor", JSON.stringify(this.actor));
    const q = qs.toString() ? `?${qs}` : "";
    const res = await this.fetchImpl(`${this.url}/api/worker/crud/${model}${q}`, {
      headers: this.headers(),
    });
    if (!res.ok) throw new CrudError(res.status, await res.text());
    return (await res.json()) as Record<string, unknown>[];
  }
}

/** Extract an ActorSnapshot from a job payload (as written by the action dispatcher). */
function actorFromPayload(payload: unknown): ActorSnapshot | undefined {
  if (typeof payload !== "object" || payload === null) return undefined;
  const p = payload as Record<string, unknown>;
  const actor = typeof p.actor === "string" ? p.actor : undefined;
  const tenantId = typeof p.tenantId === "string" ? p.tenantId : undefined;
  const role = typeof p.role === "string" ? p.role : undefined;
  if (!actor && !tenantId && !role) return undefined;
  return { userId: actor, tenantId, role };
}

// ── handleJob ────────────────────────────────────────────────────────────────

/** Minimal surface `handleJob` needs — lets tests pass a fake client. */
export type JobLifecycle = Pick<JobsClient, "heartbeat" | "complete" | "fail" | "progress">;

/**
 * Run a single leased job to completion: auto-heartbeat while the handler runs, then `complete`
 * with its result or `fail` on error (non-retryable for `NonRetryableError` or a missing handler).
 * Never throws — reports outcome via the client.
 */
export async function handleJob(
  client: JobLifecycle,
  job: LeasedJob,
  handler: JobHandler | undefined,
  opts: { visibilitySecs: number; heartbeatSecs: number; crud: CrudClient },
): Promise<void> {
  if (!handler) {
    await client.fail(job.id, job.leaseId, `no handler for kind '${job.kind}'`, false);
    return;
  }

  const controller = new AbortController();
  const beat = async (): Promise<void> => {
    try {
      const alive = await client.heartbeat(job.id, job.leaseId, opts.visibilitySecs);
      if (!alive) controller.abort();
    } catch {
      // Transient heartbeat failure — the next tick retries; don't kill the handler.
    }
  };
  const timer = setInterval(() => void beat(), Math.max(1, opts.heartbeatSecs) * 1000);

  const progress = async (update: ProgressUpdate): Promise<void> => {
    const alive = await client.progress(job.id, job.leaseId, update);
    if (!alive) controller.abort();
  };

  try {
    const result = await handler({
      job,
      signal: controller.signal,
      heartbeat: beat,
      progress,
      crud: opts.crud,
    });
    await client.complete(job.id, job.leaseId, result ?? null);
  } catch (err) {
    const retryable = !(err instanceof NonRetryableError);
    const message = err instanceof Error ? err.message : String(err);
    await client.fail(job.id, job.leaseId, message, retryable);
  } finally {
    clearInterval(timer);
  }
}

export function createWorker(opts: WorkerOptions): Worker {
  const fetchImpl = opts.fetch ?? globalThis.fetch;
  if (!fetchImpl) {
    throw new Error("global fetch is unavailable; pass opts.fetch (requires Node >= 18)");
  }
  const client = new JobsClient(opts.url, opts.token, fetchImpl);
  const handlers = new Map<string, JobHandler>();

  const concurrency = Math.max(1, opts.concurrency ?? 1);
  const visibilitySecs = Math.max(1, opts.visibilitySecs ?? 30);
  const heartbeatSecs = Math.max(1, opts.heartbeatSecs ?? (Math.floor(visibilitySecs / 3) || 1));
  const pollIntervalMs = Math.max(1, opts.pollIntervalMs ?? 1000);
  const onError = opts.onError ?? ((e: unknown) => console.error("[atomo-worker]", e));
  const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

  let running = false;
  let inFlight = 0;
  const active = new Set<Promise<void>>();
  let loopDone: Promise<void> | undefined;

  async function loop(): Promise<void> {
    while (running) {
      const capacity = concurrency - inFlight;
      if (capacity <= 0) {
        await sleep(pollIntervalMs);
        continue;
      }
      let jobs: LeasedJob[];
      try {
        jobs = await client.lease(opts.queues, capacity, visibilitySecs);
      } catch (e) {
        onError(e);
        await sleep(pollIntervalMs);
        continue;
      }
      if (jobs.length === 0) {
        await sleep(pollIntervalMs);
        continue;
      }
      for (const job of jobs) {
        inFlight++;
        const actor = actorFromPayload(job.payload);
        const crud = new CrudClient(opts.url, opts.token, fetchImpl, actor);
        const p = handleJob(client, job, handlers.get(job.kind), {
          visibilitySecs,
          heartbeatSecs,
          crud,
        })
          .catch(onError)
          .finally(() => {
            inFlight--;
            active.delete(p);
          });
        active.add(p);
      }
    }
  }

  return {
    on(kind, handler) {
      handlers.set(kind, handler);
      return this;
    },
    start() {
      if (running) return;
      running = true;
      loopDone = loop();
    },
    async stop() {
      running = false;
      await loopDone?.catch(() => {});
      await Promise.allSettled([...active]);
    },
  };
}
