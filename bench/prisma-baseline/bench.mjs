// Prisma baseline for the Atomo head-to-head — measures Prisma Client's data-layer latency on the
// same operations Atomo benchmarks, against the same co-located Postgres, same serial method.
//
// Run (co-located, inside WSL where Postgres is local):
//   docker run --rm --network host -v <repo>/bench/prisma-baseline:/app -w /app \
//     -e DATABASE_URL=postgres://… node:20 \
//     bash -c "npm i && npx prisma generate && npx prisma db push --accept-data-loss && node bench.mjs"
//
// Fairness note: Prisma Client is an ORM with a query engine (Rust binary under the hood). It does
// connection pooling, query building, result mapping — more than raw node-postgres, less than Atomo
// (no event sourcing, hooks, or built-in caching). The comparison shows where an ORM sits between
// the raw-SQL floor and Atomo's full engine.

import { PrismaClient } from "@prisma/client";
import { readFileSync } from "fs";

const ITERS = parseInt(process.env.BENCH_ITERS || "2000", 10);

function stats(name, ds) {
  const s = [...ds].sort((a, b) => a - b);
  const n = s.length;
  const pct = (p) => s[Math.min(Math.floor(n * p), n - 1)];
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

const prisma = new PrismaClient();
const usSince = (t) => Number(process.hrtime.bigint() - t) / 1000;
const uid = () => Math.random().toString(36).slice(2, 14);

// Create tables via raw SQL (avoids `prisma db push` which tries to drop unrelated tables)
const setupSql = readFileSync(new URL("./setup.sql", import.meta.url), "utf8");
for (const stmt of setupSql.split(";").filter((s) => s.trim())) {
  await prisma.$executeRawUnsafe(stmt);
}

// Clean slate
await prisma.$executeRawUnsafe("TRUNCATE bench_prisma_tags, bench_prisma_notes, bench_prisma_events CASCADE");

const rows = [];

// ----- Warmup -----
for (let i = 0; i < 50; i++) {
  await prisma.benchPrismaNote.create({ data: { id: `w${i}`, title: `warm${i}` } });
}

// ----- create (single record) -----
const cins = [];
for (let i = 0; i < ITERS; i++) {
  const id = `n${uid()}`;
  const t = process.hrtime.bigint();
  await prisma.benchPrismaNote.create({ data: { id, title: `note${i}` } });
  cins.push(usSince(t));
}
rows.push(stats("prisma: create", cins));

// ----- create + event in transaction (matches Atomo's create = record + event) -----
const ctx = [];
for (let i = 0; i < ITERS; i++) {
  const id = `e${uid()}`;
  const t = process.hrtime.bigint();
  await prisma.$transaction([
    prisma.benchPrismaNote.create({ data: { id, title: `txn${i}` } }),
    prisma.benchPrismaEvent.create({
      data: { id: `ev${uid()}`, model: "Note", data: { id } },
    }),
  ]);
  ctx.push(usSince(t));
}
rows.push(stats("prisma: create + event (txn)", ctx));

// ----- createMany (batch 100) -----
const batchSize = 100;
const batches = Math.max(Math.floor(ITERS / batchSize), 1);
const cbatch = [];
for (let b = 0; b < batches; b++) {
  const data = Array.from({ length: batchSize }, (_, i) => ({
    id: `b${uid()}${i}`,
    title: `batch${b}-${i}`,
  }));
  const t = process.hrtime.bigint();
  await prisma.benchPrismaNote.createMany({ data });
  const elapsed = usSince(t);
  cbatch.push(elapsed / batchSize); // per-row
}
rows.push(stats(`prisma: createMany (per row, batch=${batchSize})`, cbatch));

// ----- findMany (limit 20) -----
const sel = [];
for (let i = 0; i < ITERS; i++) {
  const t = process.hrtime.bigint();
  await prisma.benchPrismaNote.findMany({
    take: 20,
    orderBy: { createdAt: "desc" },
  });
  sel.push(usSince(t));
}
rows.push(stats("prisma: findMany (limit 20)", sel));

// ----- findUnique -----
const targetId = (
  await prisma.benchPrismaNote.findFirst({ select: { id: true } })
).id;
const funi = [];
for (let i = 0; i < ITERS; i++) {
  const t = process.hrtime.bigint();
  await prisma.benchPrismaNote.findUnique({ where: { id: targetId } });
  funi.push(usSince(t));
}
rows.push(stats("prisma: findUnique", funi));

// ----- count -----
const cnt = [];
for (let i = 0; i < ITERS; i++) {
  const t = process.hrtime.bigint();
  await prisma.benchPrismaNote.count();
  cnt.push(usSince(t));
}
rows.push(stats("prisma: count", cnt));

// ----- findMany with include (relation) -----
// Seed: 20 notes with 3 tags each
for (let i = 0; i < 20; i++) {
  const noteId = `rel${uid()}`;
  await prisma.benchPrismaNote.create({ data: { id: noteId, title: `rel${i}` } });
  for (let j = 0; j < 3; j++) {
    await prisma.benchPrismaTag.create({
      data: { id: `tag${uid()}`, label: `t${i}-${j}`, noteId },
    });
  }
}
const relIters = Math.min(ITERS, 500);
const incl = [];
for (let i = 0; i < relIters; i++) {
  const t = process.hrtime.bigint();
  await prisma.benchPrismaNote.findMany({
    take: 20,
    include: { tags: true },
  });
  incl.push(usSince(t));
}
rows.push(stats("prisma: findMany + include (20 notes × tags)", incl));

// ----- update (single by id) -----
const updIters = Math.min(ITERS, 500);
const upd = [];
for (let i = 0; i < updIters; i++) {
  const t = process.hrtime.bigint();
  await prisma.benchPrismaNote.update({
    where: { id: targetId },
    data: { title: `upd${i}` },
  });
  upd.push(usSince(t));
}
rows.push(stats("prisma: update (1 row by id)", upd));

// ----- delete (single by id, re-create between iterations) -----
const delIters = Math.min(ITERS, 500);
const del = [];
for (let i = 0; i < delIters; i++) {
  const id = `del${uid()}`;
  await prisma.benchPrismaNote.create({ data: { id, title: `del${i}` } });
  const t = process.hrtime.bigint();
  await prisma.benchPrismaNote.delete({ where: { id } });
  del.push(usSince(t));
}
rows.push(stats("prisma: delete (1 row by id)", del));

// ----- Cleanup + report -----
await prisma.$executeRawUnsafe("TRUNCATE bench_prisma_tags, bench_prisma_notes, bench_prisma_events CASCADE");
await prisma.$disconnect();

console.log(`\n## Prisma baseline (Prisma Client)\n`);
console.log(`iterations: ${ITERS} · serial, in-process latency\n`);
console.log(`| Benchmark | n | mean µs | p50 µs | p95 µs | p99 µs | ops/sec |`);
console.log(`|---|--:|--:|--:|--:|--:|--:|`);
for (const r of rows) {
  console.log(
    `| ${r.name} | ${r.n} | ${r.mean.toFixed(1)} | ${Math.round(r.p50)} | ${Math.round(r.p95)} | ${Math.round(r.p99)} | ${r.ops.toFixed(0)} |`,
  );
}
console.log();
