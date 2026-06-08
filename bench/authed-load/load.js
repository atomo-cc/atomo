// k6 authed mixed-workload benchmark for Atomo.
//
// Exercises the full production path: JWT auth + GraphQL CRUD + read cache + event sourcing.
//
// Usage (co-located, same host as Atomo server):
//   k6 run -e BASE=http://127.0.0.1:3000 -e VUS=50 -e DUR=60s load.js
//
// Environment variables:
//   BASE  — server origin (default http://127.0.0.1:3000)
//   VUS   — virtual users  (default 50)
//   DUR   — steady-state duration (default 60s)
//   EMAIL — login email    (default admin@test.dev)
//   PASS  — login password (default admin123)

import http from "k6/http";
import { check, sleep } from "k6";
import { Counter, Trend } from "k6/metrics";

// ---------------------------------------------------------------------------
// Options: 10s ramp → steady → 5s ramp-down
// ---------------------------------------------------------------------------
const vus = parseInt(__ENV.VUS || "50", 10);
const dur = __ENV.DUR || "60s";

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
// Setup: login once, return a shared JWT
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

  // Seed 200 rows so reads have data from the start
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
  console.log("Seed complete.");

  return { token };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function gql(token, query, variables) {
  const payload = variables
    ? JSON.stringify({ query, variables })
    : JSON.stringify({ query });

  return http.post(`${BASE}/graphql`, payload, {
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
// Workload: 80% read / 10% create / 5% update / 5% delete
// ---------------------------------------------------------------------------
const createdIds = [];

export default function (data) {
  if (!data.token) return;

  const roll = Math.random();

  if (roll < 0.8) {
    doRead(data.token);
  } else if (roll < 0.9) {
    doCreate(data.token);
  } else if (roll < 0.95) {
    doUpdate(data.token);
  } else {
    doDelete(data.token);
  }

  sleep(0.01);
}

// --- Read (find_many, limit 20) -------------------------------------------
function doRead(token) {
  const q = `{ records(model: "BenchNote", limit: 20) }`;
  const res = gql(token, q);
  readLatency.add(res.timings.duration);
  checkGql(res, "read");
}

// --- Create ---------------------------------------------------------------
function doCreate(token) {
  const title = `k6-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const q = `mutation {
    create(model: "BenchNote", data: { title: "${title}", body: "load test" })
  }`;
  const res = gql(token, q);
  createLatency.add(res.timings.duration);
  checkGql(res, "create");

  try {
    const rec = res.json("data.create");
    if (rec && rec.id) {
      createdIds.push(rec.id);
    }
  } catch (_) {
    // best-effort id collection
  }
}

// --- Update (pick a recently-created id or a random title filter) ---------
function doUpdate(token) {
  const id =
    createdIds.length > 0
      ? createdIds[Math.floor(Math.random() * createdIds.length)]
      : null;

  let q;
  if (id) {
    q = `mutation {
      update(model: "BenchNote",
             where: { id: { equals: "${id}" } },
             data:  { body: "updated-${Date.now()}" })
    }`;
  } else {
    q = `mutation {
      update(model: "BenchNote",
             where: { title: { equals: "note-1" } },
             data:  { body: "updated-${Date.now()}" })
    }`;
  }
  const res = gql(token, q);
  updateLatency.add(res.timings.duration);
  checkGql(res, "update");
}

// --- Delete (soft delete a recently-created row) --------------------------
function doDelete(token) {
  const id = createdIds.pop();
  if (!id) return;

  const q = `mutation {
    delete(model: "BenchNote",
           where: { id: { equals: "${id}" } })
  }`;
  const res = gql(token, q);
  deleteLatency.add(res.timings.duration);
  checkGql(res, "delete");
}

// ---------------------------------------------------------------------------
// Teardown: report summary context
// ---------------------------------------------------------------------------
export function teardown(data) {
  console.log(
    `\n  Workload: 80% read / 10% create / 5% update / 5% delete`
  );
  console.log(`  Auth: JWT (session-verified)`);
  console.log(`  VUs: ${vus}  Duration: ${dur}\n`);
}
