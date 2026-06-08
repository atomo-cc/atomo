// Fast Node baseline (Fastify — a performant framework, not strawman Express) for the HTTP
// request-throughput head-to-head vs Atomo's axum server. `/health` returns small JSON with no DB,
// isolating the request runtime (Node event loop vs Rust/tokio).
import Fastify from "fastify";

const app = Fastify({ logger: false });

app.get("/health", async () => ({
  ok: true,
  name: "node-fastify",
  version: "baseline",
}));

await app.listen({ port: 3002, host: "0.0.0.0" });
