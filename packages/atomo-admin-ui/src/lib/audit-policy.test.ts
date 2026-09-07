import { describe, expect, it } from "vitest";
import { summarizeAuditPolicy } from "./audit-policy";

describe("runtime audit policy messaging", () => {
  it("does not claim enabled or empty history when effective policy is unavailable", () => {
    const summary = summarizeAuditPolicy();
    expect(summary.label).toBe("Unknown");
    expect(summary.empty).toContain("unknown");
  });
  it("distinguishes saving off from absence of existing history", () => {
    const summary = summarizeAuditPolicy({
      default: { mode: "off" },
      models: {},
    });
    expect(summary.label).toBe("Off");
    expect(summary.description).toContain("Existing entries may remain");
  });
  it("metadata does not promise full payload or identifiers beyond its contract", () => {
    const summary = summarizeAuditPolicy({
      default: { mode: "metadata" },
      models: {},
    });
    expect(summary.label).toBe("Metadata only");
    expect(summary.description).toContain("without payload, IP or user agent");
  });
  it("full default cannot hide disabled model overrides or retention", () => {
    const summary = summarizeAuditPolicy({
      default: { mode: "full" },
      models: { Counter: { mode: "off", max_bytes: 1000 } },
    });
    expect(summary.label).toBe("Per-model policy");
    expect(summary.empty).toContain("retention");
    expect(summary.description).not.toContain("Counter");
  });
});
