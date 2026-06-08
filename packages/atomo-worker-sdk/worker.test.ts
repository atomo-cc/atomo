import { describe, it, expect, vi } from "vitest";
import { handleJob, JobsClient, NonRetryableError, type JobLifecycle, type LeasedJob } from "./index";

const JOB: LeasedJob = {
  id: "j1",
  queue: "q",
  kind: "video.generate",
  payload: { prompt: "hi" },
  attempts: 1,
  maxAttempts: 5,
  leaseId: "L1",
};

// Heartbeat interval far in the future so it never fires during a test.
const OPTS = { visibilitySecs: 30, heartbeatSecs: 9999 };

function fakeClient(): JobLifecycle & {
  heartbeat: ReturnType<typeof vi.fn>;
  complete: ReturnType<typeof vi.fn>;
  fail: ReturnType<typeof vi.fn>;
} {
  return {
    heartbeat: vi.fn().mockResolvedValue(true),
    complete: vi.fn().mockResolvedValue(undefined),
    fail: vi.fn().mockResolvedValue(undefined),
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
