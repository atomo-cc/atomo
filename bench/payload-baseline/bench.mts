// Payload CMS baseline for the Atomo head-to-head — measures Payload's local-API data-layer latency
// on the same operations Atomo benchmarks, against the same co-located Postgres, same serial method.
//
// Run (co-located, inside WSL where Postgres is local):
//   docker run --rm --network host -v <repo>/bench/payload-baseline:/app -w /app \
//     -e DATABASE_URL=postgres://… node:20 \
//     bash -c "npm install && npx payload migrate && NODE_OPTIONS='--import tsx/esm' node bench.mts"
//
// This uses Payload's **local API** (payload.create / payload.find / etc.) — the same programmatic
// interface a Payload plugin or server-side code would use. It bypasses HTTP/REST/GraphQL, making it
// a fair data-layer comparison with Atomo's AtomoClient and Prisma Client.
//
// Payload 3 uses Drizzle ORM under the hood (via @payloadcms/db-postgres). It does more per
// operation than Prisma or raw SQL: access control hooks, field-level hooks, auto-versioning
// checks, and relation handling. That's the point — it's a full CMS data layer, and this
// measures what that costs.

import { getPayload } from "payload";
import pg from "pg";
import { readFileSync } from "fs";

const ITERS = parseInt(process.env.BENCH_ITERS || "2000", 10);

function stats(name: string, ds: number[]) {
  const s = [...ds].sort((a, b) => a - b);
  const n = s.length;
  const pct = (p: number) => s[Math.min(Math.floor(n * p), n - 1)];
  const totalUs = s.reduce((a, b) => a + b, 0);
  return {
    name,
    n,
    mean: totalUs / n,
    p50: pct(0.5),
    p95: pct(0.95),
    p99: pct(0.99),
    ops: (1e6 * n) / totalUs,
  };
}

const usSince = (t: bigint) => Number(process.hrtime.bigint() - t) / 1000;

// Create tables via raw SQL before Payload connects (avoids migration conflicts with
// existing shared tables like `users` from other Payload projects in the same DB).
const setupClient = new pg.Client({ connectionString: process.env.DATABASE_URL });
await setupClient.connect();
const setupSql = readFileSync(new URL("./setup.sql", import.meta.url), "utf8");
for (const stmt of setupSql.split(";").filter((s: string) => s.trim())) {
  await setupClient.query(stmt);
}
await setupClient.end();

// Initialize Payload (local API — no HTTP server)
const payload = await getPayload({
  config: (await import("./payload.config.ts")).default,
});

// Clean slate
await payload.delete({ collection: "bench-payload-tags", where: { id: { exists: true } } });
await payload.delete({ collection: "bench-payload-notes", where: { id: { exists: true } } });

const rows: ReturnType<typeof stats>[] = [];

// ----- Warmup -----
for (let i = 0; i < 50; i++) {
  await payload.create({ collection: "bench-payload-notes", data: { title: `warm${i}` } });
}

// ----- create (single record) -----
const cins: number[] = [];
for (let i = 0; i < ITERS; i++) {
  const t = process.hrtime.bigint();
  await payload.create({ collection: "bench-payload-notes", data: { title: `note${i}` } });
  cins.push(usSince(t));
}
rows.push(stats("payload: create", cins));

// ----- find (limit 20) -----
const sel: number[] = [];
for (let i = 0; i < ITERS; i++) {
  const t = process.hrtime.bigint();
  await payload.find({ collection: "bench-payload-notes", limit: 20 });
  sel.push(usSince(t));
}
rows.push(stats("payload: find (limit 20)", sel));

// ----- findByID -----
const firstDoc = await payload.find({ collection: "bench-payload-notes", limit: 1 });
const targetId = firstDoc.docs[0]?.id;
if (targetId) {
  const funi: number[] = [];
  for (let i = 0; i < ITERS; i++) {
    const t = process.hrtime.bigint();
    await payload.findByID({ collection: "bench-payload-notes", id: targetId });
    funi.push(usSince(t));
  }
  rows.push(stats("payload: findByID", funi));
}

// ----- count -----
const cnt: number[] = [];
for (let i = 0; i < ITERS; i++) {
  const t = process.hrtime.bigint();
  await payload.count({ collection: "bench-payload-notes" });
  cnt.push(usSince(t));
}
rows.push(stats("payload: count", cnt));

// ----- find with depth (relation) -----
// Seed: 20 notes with 3 tags each
for (let i = 0; i < 20; i++) {
  const note = await payload.create({
    collection: "bench-payload-notes",
    data: { title: `rel${i}` },
  });
  for (let j = 0; j < 3; j++) {
    await payload.create({
      collection: "bench-payload-tags",
      data: { label: `t${i}-${j}`, note: note.id },
    });
  }
}
// Payload resolves relations via `depth` — depth:1 populates one level of relationships.
// We query tags and let Payload resolve the `note` relationship (belongsTo equivalent).
const relIters = Math.min(ITERS, 500);
const incl: number[] = [];
for (let i = 0; i < relIters; i++) {
  const t = process.hrtime.bigint();
  await payload.find({ collection: "bench-payload-tags", limit: 60, depth: 1 });
  incl.push(usSince(t));
}
rows.push(stats("payload: find + depth:1 (60 tags → notes)", incl));

// ----- update (single by id) -----
const updIters = Math.min(ITERS, 500);
const upd: number[] = [];
for (let i = 0; i < updIters; i++) {
  const t = process.hrtime.bigint();
  await payload.update({
    collection: "bench-payload-notes",
    id: targetId,
    data: { title: `upd${i}` },
  });
  upd.push(usSince(t));
}
rows.push(stats("payload: update (1 row by id)", upd));

// ----- delete (single by id, re-create between iterations) -----
const delIters = Math.min(ITERS, 500);
const del: number[] = [];
for (let i = 0; i < delIters; i++) {
  const doc = await payload.create({
    collection: "bench-payload-notes",
    data: { title: `del${i}` },
  });
  const t = process.hrtime.bigint();
  await payload.delete({ collection: "bench-payload-notes", id: doc.id });
  del.push(usSince(t));
}
rows.push(stats("payload: delete (1 row by id)", del));

// ----- Bulk create (batch via Promise.all — Payload has no native createMany) -----
const batchSize = 100;
const batches = Math.max(Math.floor(ITERS / batchSize), 1);
const cbatch: number[] = [];
for (let b = 0; b < batches; b++) {
  const data = Array.from({ length: batchSize }, (_, i) => ({
    title: `batch${b}-${i}`,
  }));
  const t = process.hrtime.bigint();
  // Payload 3 has no native bulk create — best available is sequential creates.
  // This measures the per-operation overhead × N, which is the real cost a Payload user pays.
  for (const d of data) {
    await payload.create({ collection: "bench-payload-notes", data: d });
  }
  const elapsed = usSince(t);
  cbatch.push(elapsed / batchSize); // per-row
}
rows.push(stats(`payload: create ×${batchSize} sequential (per row)`, cbatch));

// ----- Cleanup + report -----
await payload.delete({ collection: "bench-payload-tags", where: { id: { exists: true } } });
await payload.delete({ collection: "bench-payload-notes", where: { id: { exists: true } } });

console.log(`\n## Payload CMS baseline (local API)\n`);
console.log(`iterations: ${ITERS} · serial, in-process latency\n`);
console.log(`| Benchmark | n | mean µs | p50 µs | p95 µs | p99 µs | ops/sec |`);
console.log(`|---|--:|--:|--:|--:|--:|--:|`);
for (const r of rows) {
  console.log(
    `| ${r.name} | ${r.n} | ${r.mean.toFixed(1)} | ${Math.round(r.p50)} | ${Math.round(r.p95)} | ${Math.round(r.p99)} | ${r.ops.toFixed(0)} |`,
  );
}
console.log();

process.exit(0);
