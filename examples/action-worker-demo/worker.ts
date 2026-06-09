/**
 * Example worker demonstrating the action/worker lifecycle.
 *
 * Flow:
 *   1. Client creates a Post via the REST/GraphQL API
 *   2. Atomo emits a Created event and enqueues the `processPost` action
 *   3. This worker leases the job, processes it, writes back via CRUD
 *   4. The write-back emits an Updated event — but origin prevents re-enqueuing
 *      `processPost` (loop prevention). `onStatusChange` still fires because
 *      it's a different action.
 */

import { createWorker } from "@atomo-cc/worker-sdk";
import type { ActionHandlers } from "./generated/client";

const worker = createWorker({
  url: process.env.ATOMO_URL ?? "http://localhost:3000",
  token: process.env.WORKER_TOKEN!,
  queues: ["actions"],
});

// -- Handler: processPost -----------------------------------------------------
// Fires on Post.created. Payload input is typed as Pick<Post, 'id' | 'title' | 'content'>.

worker.on("processPost", async ({ job, crud }) => {
  const input = (job.payload as { input: ActionHandlers["processPost"]["input"] }).input;
  const { id, title, content } = input;

  console.log(`[processPost] Processing "${title}" (${content.length} chars)`);

  // Write back via CRUD — origin prevents this update from re-triggering processPost.
  // The status change WILL trigger onStatusChange (different action).
  await crud.update("Post", id, { status: "processed" }, { origin: "processPost" });
});

// -- Handler: onStatusChange --------------------------------------------------
// Fires on Post.updated when `status` changed. Input is Pick<Post, 'id' | 'status'>.

worker.on("onStatusChange", async ({ job }) => {
  const input = (job.payload as { input: ActionHandlers["onStatusChange"]["input"] }).input;
  const { id, status } = input;

  console.log(`[onStatusChange] Post ${id} → ${status}`);
  // e.g., send a notification, update a search index, trigger a downstream pipeline
});

worker.start();
console.log("Worker listening on 'actions' queue...");
