/**
 * Deal Worker — handles lifecycle events for the Deal model.
 *
 * Actions:
 *   onDealStageChange — fired when a deal's `stage` field changes
 *
 * Capabilities (least-privilege token):
 *   crud:Deal:read
 *   crud:Activity:create
 *   action:onDealStageChange
 *
 * Key pattern: cross-model write with origin loop prevention.
 * When a deal stage changes, this worker logs an Activity.
 * The origin tag ensures onDealStageChange is not re-enqueued
 * if the Activity model had events pointing back to it.
 */

import { createWorker } from "@atomo-cc/worker-sdk";
import type { TypedJobContext } from "../generated/client";

const worker = createWorker({
  url: process.env.ATOMO_URL ?? "http://localhost:3000",
  token: process.env.DEAL_WORKER_TOKEN!,
  queues: ["actions"],
  concurrency: 2,
});

// ── onDealStageChange ──────────────────────────────────────────────────────
// Triggered by: Deal.updated event (when `stage` field changes)
//
// Logs pipeline activity, sends notifications on key stages.

worker.on("onDealStageChange", async (ctx) => {
  const { input } = (ctx as TypedJobContext<"onDealStageChange">).job.payload;

  console.log(
    `[deal-worker] Deal ${input.id} stage → ${input.stage} (value: ${input.value})`,
  );

  // Look up the deal for the contact association.
  const deal = await ctx.crud.findById("Deal", input.id);
  if (!deal) {
    console.warn(`[deal-worker] Deal ${input.id} not found — skipping`);
    return { skipped: true };
  }

  const contactId = deal.contactId as string;

  if (input.stage === "won") {
    // Log a "won" activity on the contact timeline.
    await ctx.crud.create("Activity", {
      contactId,
      activityType: "note",
      title: "Deal won",
      content: `Deal "${deal.title}" closed-won for ${input.value}.`,
    }, { origin: "onDealStageChange" });

    console.log(
      `[deal-worker] Logged won activity for contact ${contactId}`,
    );

    // Example: notify sales manager, update forecasts
    // await notifySalesTeam({ dealId: input.id, stage: "won", value: input.value });

    return { contactId, won: true, value: input.value };
  }

  if (input.stage === "lost") {
    await ctx.crud.create("Activity", {
      contactId,
      activityType: "note",
      title: "Deal lost",
      content: `Deal "${deal.title}" was lost (value: ${input.value}).`,
    }, { origin: "onDealStageChange" });

    console.log(
      `[deal-worker] Logged lost activity for contact ${contactId}`,
    );

    return { contactId, lost: true, value: input.value };
  }

  // Any other stage transition — log progression
  await ctx.crud.create("Activity", {
    contactId,
    activityType: "note",
    title: `Deal moved to ${input.stage}`,
    content: `Deal "${deal.title}" progressed to ${input.stage}.`,
  }, { origin: "onDealStageChange" });

  return { stage: input.stage };
});

worker.start();
console.log("[deal-worker] Listening for onDealStageChange");

const shutdown = async () => {
  console.log("[deal-worker] Shutting down...");
  await worker.stop();
  process.exit(0);
};
process.on("SIGTERM", shutdown);
process.on("SIGINT", shutdown);
