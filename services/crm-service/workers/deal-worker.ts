/**
 * Deal Worker — handles lifecycle events for the Deal model.
 *
 * Actions:
 *   onDealStatusChange — fired when a deal's `status` field changes
 *
 * Capabilities (least-privilege token):
 *   crud:Deal:read
 *   crud:Activity:create
 *   action:onDealStatusChange
 *
 * Key pattern: cross-model write with origin loop prevention.
 */

import { createWorker } from "@atomo-cc/worker-sdk";
import type { TypedJobContext } from "../generated/client";

const worker = createWorker({
  url: process.env.ATOMO_URL ?? "http://localhost:3000",
  token: process.env.DEAL_WORKER_TOKEN!,
  queues: ["actions"],
  concurrency: 2,
});

// ── onDealStatusChange ──────────────────────────────────────────────────────
// Triggered by: Deal.updated event (when `status` field changes)

worker.on("onDealStatusChange", async (ctx) => {
  const { input } = (ctx as TypedJobContext<"onDealStatusChange">).job.payload;

  console.log(
    `[deal-worker] Deal ${input.id} status → ${input.status} (value: ${input.value})`,
  );

  if (input.status === "won") {
    await ctx.crud.create("Activity", {
      contactId: input.contactId,
      type: "meeting",
      notes: `Deal closed-won (value: ${input.value}).`,
    }, { origin: "onDealStatusChange" });

    return { contactId: input.contactId, won: true, value: input.value };
  }

  if (input.status === "lost") {
    await ctx.crud.create("Activity", {
      contactId: input.contactId,
      type: "email",
      notes: `Deal was lost (value: ${input.value}).`,
    }, { origin: "onDealStatusChange" });

    return { contactId: input.contactId, lost: true, value: input.value };
  }

  return { status: input.status };
});

worker.start();
console.log("[deal-worker] Listening for onDealStatusChange");

const shutdown = async () => {
  console.log("[deal-worker] Shutting down...");
  await worker.stop();
  process.exit(0);
};
process.on("SIGTERM", shutdown);
process.on("SIGINT", shutdown);
