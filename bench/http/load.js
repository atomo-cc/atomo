// k6 load script — drives one target URL at a fixed concurrency, reports throughput + latency.
//   k6 run -e TARGET=http://127.0.0.1:3001/version -e VUS=50 -e DUR=15s load.js
import http from "k6/http";
import { check } from "k6";

export const options = {
  vus: parseInt(__ENV.VUS || "50", 10),
  duration: __ENV.DUR || "15s",
};

export default function () {
  const res = http.get(__ENV.TARGET);
  check(res, { "status 200": (r) => r.status === 200 });
}
