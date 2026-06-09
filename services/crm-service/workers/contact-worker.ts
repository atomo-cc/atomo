/**
 * Contact Worker — handles lifecycle events for the Contact model.
 *
 * Actions:
 *   onNewContact — fired when a contact is created
 *
 * Capabilities (least-privilege token):
 *   crud:Contact:read,update
 *   crud:Activity:create
 *   action:onNewContact
 */

import { createWorker } from "@atomo-cc/worker-sdk";
import type { TypedJobContext } from "../generated/client";

const worker = createWorker({
  url: process.env.ATOMO_URL ?? "http://localhost:3000",
  token: process.env.CONTACT_WORKER_TOKEN!,
  queues: ["actions"],
  concurrency: 2,
});

// ── onNewContact ────────────────────────────────────────────────────────────
// Triggered by: Contact.created event
// Logs an Activity entry for the new contact.

worker.on("onNewContact", async (ctx) => {
  const { input } = (ctx as TypedJobContext<"onNewContact">).job.payload;

  console.log(
    `[contact-worker] New contact: ${input.name} <${input.email}>`,
  );

  await ctx.crud.create("Activity", {
    contactId: input.id,
    type: "email",
    notes: `${input.name} was added to the CRM.`,
  }, { origin: "onNewContact" });

  return { processed: true };
});

worker.start();
console.log("[contact-worker] Listening for onNewContact");

const shutdown = async () => {
  console.log("[contact-worker] Shutting down...");
  await worker.stop();
  process.exit(0);
};
process.on("SIGTERM", shutdown);
process.on("SIGINT", shutdown);
