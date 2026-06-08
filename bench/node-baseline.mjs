// Node baseline for the Atomo head-to-head — the *data-layer floor*: raw INSERT + SELECT via
// node-postgres against the same Postgres, measured the same way (serial, in-process, latency).
//
// Run (co-located, inside WSL where Postgres is local), e.g. via Docker:
//   docker run --rm --network host -v <repo>:/app -w /app node:20 \
//     bash -c "npm i pg --no-save --silent && DATABASE_URL=postgres://… node bench/node-baseline.mjs"
//
// Fairness note: this measures *raw* SQL. Atomo's `create` does more per call (event emission,
// CRUD hooks, validation, typed codegen) — so a raw Node insert is expected to be at or below
// Atomo's create. The honest story is "the data-layer floor + what Atomo's conveniences cost on
// top," not "Atomo inserts rows faster than node-postgres." Reads (SELECT 20) are the closest
// like-for-like.

import pg from "pg";

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

const url = process.env.DATABASE_URL;
if (!url) throw new Error("DATABASE_URL required");

const client = new pg.Client({ connectionString: url });
await client.connect();

await client.query(
  "CREATE TABLE IF NOT EXISTS bench_node_notes (id TEXT PRIMARY KEY, title TEXT, created_at TIMESTAMPTZ DEFAULT NOW())",
);
await client.query(
  "CREATE TABLE IF NOT EXISTS bench_node_events (id TEXT PRIMARY KEY, model TEXT, data JSONB, at TIMESTAMPTZ DEFAULT NOW())",
);
await client.query("TRUNCATE bench_node_notes");
await client.query("TRUNCATE bench_node_events");

const rows = [];
const usSince = (t) => Number(process.hrtime.bigint() - t) / 1000;

// warmup
for (let i = 0; i < 50; i++) {
  await client.query("INSERT INTO bench_node_notes (id, title) VALUES ($1,$2)", [
    `w${i}`,
    `warm${i}`,
  ]);
}

// create (raw INSERT)
const cins = [];
for (let i = 0; i < ITERS; i++) {
  const t = process.hrtime.bigint();
  await client.query("INSERT INTO bench_node_notes (id, title) VALUES ($1,$2)", [
    `n${i}`,
    `n${i}`,
  ]);
  cins.push(usSince(t));
}
rows.push(stats("node-pg: raw INSERT", cins));

// create-equivalent (INSERT row + INSERT event in one txn) — matches Atomo's create, which
// persists the record AND an event. The fair "persist a record" head-to-head.
const ctx = [];
for (let i = 0; i < ITERS; i++) {
  const t = process.hrtime.bigint();
  await client.query("BEGIN");
  await client.query("INSERT INTO bench_node_notes (id, title) VALUES ($1,$2)", [`e${i}`, `e${i}`]);
  await client.query("INSERT INTO bench_node_events (id, model, data) VALUES ($1,$2,$3)", [
    `ev${i}`,
    "Note",
    JSON.stringify({ id: `e${i}` }),
  ]);
  await client.query("COMMIT");
  ctx.push(usSince(t));
}
rows.push(stats("node-pg: INSERT + event (txn)", ctx));

// find_many (SELECT 20)
const sel = [];
for (let i = 0; i < ITERS; i++) {
  const t = process.hrtime.bigint();
  await client.query(
    "SELECT id, title FROM bench_node_notes ORDER BY created_at DESC LIMIT 20",
  );
  sel.push(usSince(t));
}
rows.push(stats("node-pg: SELECT limit 20", sel));

await client.query("TRUNCATE bench_node_notes");
await client.end();

console.log(`\n## Node baseline (node-postgres, raw SQL)\n`);
console.log(`iterations: ${ITERS} · serial, in-process latency\n`);
console.log(`| Benchmark | n | mean µs | p50 µs | p95 µs | p99 µs | ops/sec |`);
console.log(`|---|--:|--:|--:|--:|--:|--:|`);
for (const r of rows) {
  console.log(
    `| ${r.name} | ${r.n} | ${r.mean.toFixed(1)} | ${r.p50} | ${r.p95} | ${r.p99} | ${r.ops.toFixed(0)} |`,
  );
}
console.log();
