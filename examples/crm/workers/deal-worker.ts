/**
 * Deal Worker — handles lifecycle events for the Deal model.
 *
 * Actions:
 *   onDealStatusChange — fired when a deal's `status` field changes
 *
 * Capabilities (least-privilege token):
 *   crud:Deal:read
 *   crud:Contact:update
 *   action:onDealStatusChange
 *
 * Key pattern: cross-model write with origin loop prevention.
 * When a deal is won, this worker updates the associated contact's stage
 * to "customer". That update triggers Contact.onStageChange, but the
 * origin tag prevents onDealStatusChange from being re-enqueued.
 */

import { createWorker } from "@atomo-cc/worker-sdk";
import type { ActionHandlers, TypedJobContext } from "../generated/client";

const worker = createWorker({
  url: process.env.ATOMO_URL ?? "http://localhost:3000",
  token: process.env.DEAL_WORKER_TOKEN!,
  queues: ["actions"],
  concurrency: 2,
});

// ── onDealStatusChange ──────────────────────────────────────────────────────
// Triggered by: Deal.updated event (when `status` field changes)
//
// deal won  → update contact stage to "customer" (cross-model write)
// deal lost → log for analytics, no cross-model mutation

worker.on("onDealStatusChange", async (ctx) => {
  const { input } = (ctx as TypedJobContext<"onDealStatusChange">).job.payload;

  console.log(`[deal-worker] Deal ${input.id} status changed to: ${input.status}`);

  if (input.status === "won") {
    // Look up the deal to get the associated contact.
    const deal = await ctx.crud.findById("Deal", input.id);
    if (!deal) {
      console.warn(`[deal-worker] Deal ${input.id} not found — skipping`);
      return { skipped: true };
    }

    const contactId = deal.contactId as string;

    // Cross-model write: promote the contact to "customer".
    //
    // origin: "onDealStatusChange" tells the server's action dispatcher:
    //   "this mutation was caused by onDealStatusChange — do NOT re-enqueue
    //    onDealStatusChange if a Contact.updated event fires."
    //
    // The Contact.updated event WILL still fire onStageChange (different action),
    // which is correct — the contact-worker should know the stage changed.
    await ctx.crud.update(
      "Contact",
      contactId,
      { stage: "customer" },
      { origin: "onDealStatusChange" },
    );

    console.log(
      `[deal-worker] Contact ${contactId} promoted to "customer" (deal ${input.id} won, value: ${input.value})`,
    );

    return { contactId, promoted: true, value: input.value };
  }

  if (input.status === "lost") {
    console.log(
      `[deal-worker] Deal ${input.id} lost (value: ${input.value}) — logged for analytics`,
    );

    // Example: record loss reason, notify manager, update forecasts
    // await analytics.trackDealLost({ dealId: input.id, value: input.value });

    return { lost: true, value: input.value };
  }

  return { status: input.status };
});

worker.start();
console.log("[deal-worker] Listening for onDealStatusChange");

process.on("SIGTERM", () => {
  console.log("[deal-worker] Shutting down...");
  worker.stop();
});
