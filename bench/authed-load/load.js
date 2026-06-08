// k6 authed mixed-workload benchmark for Atomo.
//
// Exercises the full production path: JWT auth + GraphQL CRUD + read cache + event sourcing.
//
// Usage:
//   k6 run -e BASE=http://127.0.0.1:3099 load.js                        # default mixed
//   k6 run -e BASE=http://127.0.0.1:3099 -e SCENARIO=read-only load.js  # read-only
//   k6 run -e BASE=http://127.0.0.1:3099 -e SCENARIO=write-only load.js # create-only
//   k6 run -e BASE=http://127.0.0.1:3099 -e SCENARIO=mutate load.js     # update+delete
//   k6 run -e BASE=http://127.0.0.1:3099 -e SCENARIO=mixed load.js      # 80/10/5/5 (default)
//
// Environment variables:
//   BASE      — server origin      (default http://127.0.0.1:3000)
//   VUS       — virtual users      (default 50)
//   DUR       — steady-state dur   (default 60s)
//   EMAIL     — login email        (default admin@test.dev)
//   PASS      — login password     (default admin123)
//   SEED      — rows to pre-seed   (default 200)
//   SCENARIO  — workload profile   (default mixed)

import http from "k6/http";
import { check, sleep } from "k6";
import { Counter, Trend } from "k6/metrics";

// ---------------------------------------------------------------------------
// Options: 10s ramp → steady → 5s ramp-down
// ---------------------------------------------------------------------------
const vus = parseInt(__ENV.VUS || "50", 10);
const dur = __ENV.DUR || "60s";
const scenario = __ENV.SCENARIO || "mixed";

export const options = {
  stages: [
    { duration: "10s", target: vus },
    { duration: dur, target: vus },
    { duration: "5s", target: 0 },
  ],
  thresholds: {
    http_req_duration: ["p(95)<500"],
    http_req_failed: ["rate<0.01"],
  },
};

// ---------------------------------------------------------------------------
// Custom metrics
// ---------------------------------------------------------------------------
const readLatency = new Trend("atomo_read_ms", true);
const createLatency = new Trend("atomo_create_ms", true);
const updateLatency = new Trend("atomo_update_ms", true);
const deleteLatency = new Trend("atomo_delete_ms", true);
const gqlErrors = new Counter("atomo_gql_errors");

// ---------------------------------------------------------------------------
// Setup: login + seed
// ---------------------------------------------------------------------------
const BASE = __ENV.BASE || "http://127.0.0.1:3000";

export function setup() {
  const email = __ENV.EMAIL || "admin@test.dev";
  const pass = __ENV.PASS || "admin123";

  const res = http.post(
    `${BASE}/auth/login`,
    JSON.stringify({ email, password: pass }),
    { headers: { "Content-Type": "application/json" } }
  );

  check(res, { "login 200": (r) => r.status === 200 });
  if (res.status !== 200) {
    console.error(`Login failed: ${res.status} ${res.body}`);
    return {};
  }

  const token = res.json("token");

  const SEED = parseInt(__ENV.SEED || "200", 10);
  const hdrs = {
    "Content-Type": "application/json",
    Authorization: `Bearer ${token}`,
  };
  console.log(`Seeding ${SEED} rows...`);
  for (let i = 0; i < SEED; i++) {
    http.post(
      `${BASE}/graphql`,
      JSON.stringify({
        query: `mutation { create(model: "BenchNote", data: { title: "seed-${i}", body: "seed body ${i}" }) }`,
      }),
      { headers: hdrs }
    );
  }
  console.log(`Seed complete. Scenario: ${scenario}`);

  return { token };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function gql(token, query) {
  return http.post(`${BASE}/graphql`, JSON.stringify({ query }), {
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
  });
}

function checkGql(res, tag) {
  const ok = check(res, {
    [`${tag} 200`]: (r) => r.status === 200,
    [`${tag} no errors`]: (r) => {
      try {
        return !r.json("errors");
      } catch {
        return false;
      }
    },
  });
  if (!ok) gqlErrors.add(1);
  return res;
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------
const createdIds = [];

function doRead(token) {
  const res = gql(token, `{ records(model: "BenchNote", limit: 20) }`);
  readLatency.add(res.timings.duration);
  checkGql(res, "read");
}

function doCreate(token) {
  const title = `k6-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const res = gql(
    token,
    `mutation { create(model: "BenchNote", data: { title: "${title}", body: "load test" }) }`
  );
  createLatency.add(res.timings.duration);
  checkGql(res, "create");
  try {
    const rec = res.json("data.create");
    if (rec && rec.id) createdIds.push(rec.id);
  } catch (_) {}
}

function doUpdate(token) {
  const id =
    createdIds.length > 0
      ? createdIds[Math.floor(Math.random() * createdIds.length)]
      : null;
  const where = id
    ? `{ id: { equals: "${id}" } }`
    : `{ title: { equals: "seed-1" } }`;
  const res = gql(
    token,
    `mutation { update(model: "BenchNote", where: ${where}, data: { body: "updated-${Date.now()}" }) }`
  );
  updateLatency.add(res.timings.duration);
  checkGql(res, "update");
}

function doDelete(token) {
  const id = createdIds.pop();
  if (!id) return;
  const res = gql(
    token,
    `mutation { delete(model: "BenchNote", where: { id: { equals: "${id}" } }) }`
  );
  deleteLatency.add(res.timings.duration);
  checkGql(res, "delete");
}

// ---------------------------------------------------------------------------
// Scenario dispatch
// ---------------------------------------------------------------------------
export default function (data) {
  if (!data.token) return;

  switch (scenario) {
    case "read-only":
      doRead(data.token);
      break;

    case "write-only":
      doCreate(data.token);
      break;

    case "mutate":
      if (Math.random() < 0.5) {
        doUpdate(data.token);
      } else {
        doDelete(data.token);
      }
      break;

    case "mixed":
    default: {
      const roll = Math.random();
      if (roll < 0.8) doRead(data.token);
      else if (roll < 0.9) doCreate(data.token);
      else if (roll < 0.95) doUpdate(data.token);
      else doDelete(data.token);
      break;
    }
  }

  sleep(0.01);
}

// ---------------------------------------------------------------------------
// Teardown
// ---------------------------------------------------------------------------
export function teardown(data) {
  const profiles = {
    "read-only": "100% read",
    "write-only": "100% create",
    mutate: "50% update / 50% delete",
    mixed: "80% read / 10% create / 5% update / 5% delete",
  };
  console.log(`\n  Scenario: ${scenario} (${profiles[scenario] || "custom"})`);
  console.log(`  Auth: JWT (session-verified)`);
  console.log(`  VUs: ${vus}  Duration: ${dur}\n`);
}
