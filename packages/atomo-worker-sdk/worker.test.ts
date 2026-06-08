import { describe, it, expect, vi } from "vitest";
import {
  handleJob,
  JobsClient,
  CrudClient,
  CrudError,
  NonRetryableError,
  type JobLifecycle,
  type LeasedJob,
} from "./index";

const JOB: LeasedJob = {
  id: "j1",
  queue: "q",
  kind: "video.generate",
  payload: { prompt: "hi" },
  attempts: 1,
  maxAttempts: 5,
  leaseId: "L1",
};

const nopFetch = vi.fn().mockResolvedValue(new Response("{}"));
const nopCrud = new CrudClient("http://h", "tok", nopFetch as unknown as typeof fetch);

// Heartbeat interval far in the future so it never fires during a test.
const OPTS = { visibilitySecs: 30, heartbeatSecs: 9999, crud: nopCrud };

function fakeClient(): JobLifecycle & {
  heartbeat: ReturnType<typeof vi.fn>;
  complete: ReturnType<typeof vi.fn>;
  fail: ReturnType<typeof vi.fn>;
  progress: ReturnType<typeof vi.fn>;
} {
  return {
    heartbeat: vi.fn().mockResolvedValue(true),
    complete: vi.fn().mockResolvedValue(undefined),
    fail: vi.fn().mockResolvedValue(undefined),
    progress: vi.fn().mockResolvedValue(true),
  };
}

describe("handleJob", () => {
  it("completes with the handler result on success", async () => {
    const c = fakeClient();
    await handleJob(c, JOB, async () => ({ assetId: "a1" }), OPTS);
    expect(c.complete).toHaveBeenCalledWith("j1", "L1", { assetId: "a1" });
    expect(c.fail).not.toHaveBeenCalled();
  });

  it("normalizes an undefined handler result to null", async () => {
    const c = fakeClient();
    await handleJob(c, JOB, async () => undefined, OPTS);
    expect(c.complete).toHaveBeenCalledWith("j1", "L1", null);
  });

  it("fails retryable when the handler throws a plain error", async () => {
    const c = fakeClient();
    await handleJob(c, JOB, async () => {
      throw new Error("provider rate limited");
    }, OPTS);
    expect(c.fail).toHaveBeenCalledWith("j1", "L1", "provider rate limited", true);
    expect(c.complete).not.toHaveBeenCalled();
  });

  it("fails non-retryable on NonRetryableError", async () => {
    const c = fakeClient();
    await handleJob(c, JOB, async () => {
      throw new NonRetryableError("malformed prompt");
    }, OPTS);
    expect(c.fail).toHaveBeenCalledWith("j1", "L1", "malformed prompt", false);
  });

  it("dead-letters (non-retryable) when no handler is registered for the kind", async () => {
    const c = fakeClient();
    await handleJob(c, JOB, undefined, OPTS);
    expect(c.fail).toHaveBeenCalledWith("j1", "L1", expect.stringContaining("no handler"), false);
  });

  it("exposes the job payload to the handler via ctx.job", async () => {
    const c = fakeClient();
    let seen: unknown;
    await handleJob(c, JOB, async ({ job }) => {
      seen = job.payload;
      return null;
    }, OPTS);
    expect(seen).toEqual({ prompt: "hi" });
  });

  it("ctx.progress publishes via the client", async () => {
    const c = fakeClient();
    await handleJob(c, JOB, async ({ progress }) => {
      await progress({ percent: 0.5, message: "halfway" });
      return null;
    }, OPTS);
    expect(c.progress).toHaveBeenCalledWith("j1", "L1", { percent: 0.5, message: "halfway" });
  });
});

describe("JobsClient.progress", () => {
  it("posts to /jobs/{id}/progress and returns false on a lost lease (409)", async () => {
    const ok = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    let client = new JobsClient("http://h", "tok", ok as unknown as typeof fetch);
    expect(await client.progress("j1", "L1", { percent: 0.1 })).toBe(true);
    const [url, init] = ok.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("http://h/jobs/j1/progress");
    expect(JSON.parse(init.body as string)).toEqual({ leaseId: "L1", percent: 0.1 });

    const lost = vi.fn().mockResolvedValue(new Response(null, { status: 409 }));
    client = new JobsClient("http://h", "tok", lost as unknown as typeof fetch);
    expect(await client.progress("j1", "L1", {})).toBe(false);
  });
});

describe("JobsClient", () => {
  it("lease posts the worker token + body and parses the jobs array", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(new Response(JSON.stringify({ jobs: [JOB] }), { status: 200 }));
    const client = new JobsClient("http://h", "tok", fetchMock as unknown as typeof fetch);

    const got = await client.lease(["q"], 3, 30);
    expect(got).toEqual([JOB]);

    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("http://h/jobs/lease");
    expect((init.headers as Record<string, string>)["x-worker-token"]).toBe("tok");
    expect(JSON.parse(init.body as string)).toEqual({ queues: ["q"], capacity: 3, visibilitySecs: 30 });
  });

  it("heartbeat returns false when the lease was lost (409)", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 409 }));
    const client = new JobsClient("http://h", "tok", fetchMock as unknown as typeof fetch);
    expect(await client.heartbeat("j1", "L1", 30)).toBe(false);
  });

  it("complete tolerates a 409 (lease already lost) without throwing", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 409 }));
    const client = new JobsClient("http://h", "tok", fetchMock as unknown as typeof fetch);
    await expect(client.complete("j1", "L1", {})).resolves.toBeUndefined();
  });
});

describe("CrudClient", () => {
  it("create POSTs to /api/worker/crud/:model with actor snapshot", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(new Response(JSON.stringify({ id: "r1", title: "hi" }), { status: 201 }));
    const crud = new CrudClient("http://h", "tok", fetchMock as unknown as typeof fetch, {
      userId: "u1",
      role: "editor",
    });
    const result = await crud.create("Post", { title: "hi" });
    expect(result).toEqual({ id: "r1", title: "hi" });
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("http://h/api/worker/crud/Post");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body as string)).toEqual({
      data: { title: "hi" },
      actor: { userId: "u1", role: "editor" },
    });
    expect((init.headers as Record<string, string>)["x-worker-token"]).toBe("tok");
  });

  it("findById returns null on 404", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 404 }));
    const crud = new CrudClient("http://h", "tok", fetchMock as unknown as typeof fetch);
    expect(await crud.findById("Post", "x")).toBeNull();
  });

  it("findById parses JSON on 200", async () => {
    const body = JSON.stringify({ id: "p1", title: "found" });
    const fetchMock = vi.fn().mockResolvedValue(new Response(body, { status: 200 }));
    const crud = new CrudClient("http://h", "tok", fetchMock as unknown as typeof fetch);
    expect(await crud.findById("Post", "p1")).toEqual({ id: "p1", title: "found" });
  });

  it("update PATCHes the correct URL", async () => {
    const body = JSON.stringify({ id: "p1", title: "new" });
    const fetchMock = vi.fn().mockResolvedValue(new Response(body, { status: 200 }));
    const crud = new CrudClient("http://h", "tok", fetchMock as unknown as typeof fetch);
    await crud.update("Post", "p1", { title: "new" });
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("http://h/api/worker/crud/Post/p1");
    expect(init.method).toBe("PATCH");
  });

  it("delete DELETEs the correct URL", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ deleted: 1 })));
    const crud = new CrudClient("http://h", "tok", fetchMock as unknown as typeof fetch);
    await crud.delete("Post", "p1");
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("http://h/api/worker/crud/Post/p1");
    expect(init.method).toBe("DELETE");
  });

  it("throws CrudError on non-OK response", async () => {
    const fetchMock = vi
      .fn()
      .mockImplementation(() => Promise.resolve(new Response("forbidden", { status: 403 })));
    const crud = new CrudClient("http://h", "tok", fetchMock as unknown as typeof fetch);
    await expect(crud.create("Post", {})).rejects.toThrow(CrudError);
    await expect(crud.create("Post", {})).rejects.toThrow(/403/);
  });

  it("findMany sends where/limit/offset as query params", async () => {
    const body = JSON.stringify([{ id: "p1" }]);
    const fetchMock = vi.fn().mockResolvedValue(new Response(body, { status: 200 }));
    const crud = new CrudClient("http://h", "tok", fetchMock as unknown as typeof fetch);
    const result = await crud.findMany("Post", { where: { status: "published" }, limit: 10 });
    expect(result).toEqual([{ id: "p1" }]);
    const [url] = fetchMock.mock.calls[0] as [string];
    expect(url).toContain("/api/worker/crud/Post?");
    expect(url).toContain("limit=10");
    expect(url).toContain("where=");
  });

  it("update sends origin in the request body", async () => {
    const body = JSON.stringify({ id: "p1", title: "new" });
    const fetchMock = vi.fn().mockResolvedValue(new Response(body, { status: 200 }));
    const crud = new CrudClient("http://h", "tok", fetchMock as unknown as typeof fetch);
    await crud.update("Post", "p1", { title: "new" }, { origin: "processPost" });
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const parsed = JSON.parse(init.body as string);
    expect(parsed.origin).toBe("processPost");
  });

  it("create sends origin in the request body", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(new Response(JSON.stringify({ id: "r1" }), { status: 201 }));
    const crud = new CrudClient("http://h", "tok", fetchMock as unknown as typeof fetch);
    await crud.create("Post", { title: "hi" }, { origin: "initPost" });
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const parsed = JSON.parse(init.body as string);
    expect(parsed.origin).toBe("initPost");
  });

  it("ctx.crud is passed to the handler by handleJob", async () => {
    const c = fakeClient();
    const actor = { actor: "user-42", tenantId: "t1", role: "admin" };
    const jobWithActor: LeasedJob = { ...JOB, payload: actor };
    let seenCrud: CrudClient | undefined;
    await handleJob(
      c,
      jobWithActor,
      async ({ crud }) => {
        seenCrud = crud;
        return null;
      },
      OPTS,
    );
    expect(seenCrud).toBeInstanceOf(CrudClient);
  });
});
