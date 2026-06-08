/**
 * Contact Worker — handles lifecycle events for the Contact model.
 *
 * Actions:
 *   onNewContact  — fired when a contact is created
 *   onStageChange — fired when a contact's `stage` field changes
 *
 * Capabilities (least-privilege token):
 *   crud:Contact:read,update
 *   action:onNewContact
 *   action:onStageChange
 */

import { createWorker } from "@atomo-cc/worker-sdk";
import type { ActionHandlers, TypedJobContext } from "../generated/client";

const worker = createWorker({
  url: process.env.ATOMO_URL ?? "http://localhost:3000",
  token: process.env.CONTACT_WORKER_TOKEN!,
  queues: ["actions"],
  concurrency: 2,
});

// ── onNewContact ────────────────────────────────────────────────────────────
// Triggered by: Contact.created event
// Could send a welcome email, enrich data from a third-party API, etc.

worker.on("onNewContact", async (ctx) => {
  const { input } = (ctx as TypedJobContext<"onNewContact">).job.payload;

  console.log(`[contact-worker] New contact created: ${input.name} <${input.email}>`);

  // Example: send welcome email via external service
  // await sendWelcomeEmail({ to: input.email, name: input.name });

  // Example: enrich from Clearbit/Apollo
  // const enriched = await enrichContact(input.email);
  // if (enriched) {
  //   await ctx.crud.update("Contact", input.id, { phone: enriched.phone }, { origin: "onNewContact" });
  // }

  return { processed: true };
});

// ── onStageChange ───────────────────────────────────────────────────────────
// Triggered by: Contact.updated event (when `stage` field changes)
//
// IMPORTANT: When the deal-worker updates a contact's stage to "customer"
// (after a deal is won), this handler fires. But because the deal-worker
// passes `origin: "onDealStatusChange"`, the server's action dispatcher
// will NOT re-enqueue onDealStatusChange — breaking the potential loop.
//
// Flow: Deal won → deal-worker updates Contact.stage → onStageChange fires
//       → this handler runs, but does NOT trigger another deal event.

worker.on("onStageChange", async (ctx) => {
  const { input } = (ctx as TypedJobContext<"onStageChange">).job.payload;

  console.log(`[contact-worker] Contact ${input.id} stage changed to: ${input.stage}`);

  // When a contact becomes a customer, update open deals for visibility.
  if (input.stage === "customer") {
    const openDeals = await ctx.crud.findMany("Deal", {
      where: { contactId: input.id, status: "open" },
    });

    console.log(
      `[contact-worker] Contact ${input.id} became a customer — ${openDeals.length} open deal(s) found`,
    );

    // Log for CRM analytics / notifications
    // await notifySalesTeam({ contactId: input.id, stage: input.stage, openDeals: openDeals.length });
  }

  return { stage: input.stage };
});

worker.start();
console.log("[contact-worker] Listening for onNewContact, onStageChange");

process.on("SIGTERM", () => {
  console.log("[contact-worker] Shutting down...");
  worker.stop();
});
