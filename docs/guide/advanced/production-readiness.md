# Production Readiness Checklist

Use this checklist to take an Atomo service from local dev to production. It highlights actions and code tasks needed to harden the current implementation.

Quick start
- Require envs: `DATABASE_URL`, `JWT_SECRET`
- Build release binaries and static Admin UI
- Run migrations and seed minimal platform data (admin user)
- Put a reverse proxy (TLS, HSTS, gzip/brotli) in front of the service

Security & Auth
- JWT secret management
  - Set strong `JWT_SECRET` via secret manager (never in repo)
  - Rotate periodically; short token TTL + refresh flow (planned)
- Password hashing & policy
  - Bcrypt hashing (configurable cost); enforce minimum length and complexity
  - Env: `BCRYPT_COST`, `PASSWORD_MIN_LENGTH`, `PASSWORD_REQUIRE_COMPLEXITY`
  - Consider 2FA for Admin; add refresh-token flow
- RBAC enforcement
  - Centralize permission checks; review CRUD and audit routes
  - Deny-by-default for unrecognized actions/resources
- Transport security
  - Enforce HTTPS/TLS via proxy; add HSTS
  - Restrict CORS to allowed origins; disable permissive CORS in prod

Database
- Migrations
  - Audit the generated migrations; add missing indices (FKs, lookups)
  - Zero‑downtime patterns for schema changes (nullable, backfill, switch)
- Operations
  - Backups and PITR tested; document restore runbook
  - Connection pool sizing; `sqlx` pool timeouts and retry policy

Observability
- Logging & tracing
  - Structured logs with request IDs and user/session IDs
  - Add tracing spans around GraphQL, DB, codegen/hot reload
- Metrics
  - Expose Prometheus metrics endpoint; track latency, error rates, pool stats
- Alerting
  - Health checks integrated with uptime monitors

Performance & Limits
- GraphQL protections
  - Depth/complexity limits; pagination defaults and caps
  - N+1 mitigation; batch/resolver caching where applicable
- Throttling
  - Rate limiting per key/IP; burst window settings
- Caching
  - HTTP cache headers for static assets; CDN in front of Admin UI

Availability
- Health endpoints
  - `/health` (liveness), `/ready` (readiness) with DB probe
- Graceful shutdown
  - Ensure in‑flight requests drain; tune timeouts
- Startup checks
  - Config validation; fail fast when envs are missing/invalid

Admin UI & Proxy
- Build Admin UI for production and serve via static host/CDN
- Restrict Admin routes; IP allowlist or VPN for back‑office if needed
- Remove dev‑only Vite proxies in prod

Build & Deploy
- Build flags: `cargo build --release` with `RUST_LOG=info`
- Containerization: minimal base (distroless/alpine with MUSL) if applicable
- Migrations on deploy: run `atomo migrate` before rolling traffic
- Configuration: 12‑factor; per‑env config and secrets via env vars

Data Protection
- PII classification; restrict fields in logs/metadata
- Encryption at rest (DB/provider); field‑level crypto where necessary
- Compliance: data retention and deletion policies

Testing & QA
- Integration tests for auth flows, RBAC, audit trails
- Load testing baseline (p50/p95, concurrency, DB pool saturation)
- Chaos checks: DB failover/restart, network latency

Code tasks (prioritized)
- Auth hashing
  - Replace stubs in `crates/atomo_server/src/auth.rs` with bcrypt/argon2
  - Add migration for storing algorithm/params if changing format
- CORS & headers
  - Introduce prod CORS allowlist and security headers (CSP, HSTS)
- Rate limiting
  - Add a middleware (e.g., governor) for IP/key based limits
- GraphQL limits
  - Configure async‑graphql depth/complexity, disable introspection in prod if needed
- Observability
  - Add `tower_http::trace` and `tracing-subscriber` JSON logs; request IDs
  - Add `/metrics` (Prometheus) and basic counters/histograms
- Readiness
  - Implement `/ready` that checks DB connectivity and pending migrations
- Secrets/config
  - Validate required envs on startup; clear error messages

Deployment runbook (outline)
1) Build
   - `pnpm build:all` or `cargo build --workspace --release`
   - Build Admin UI static assets
2) Provision
   - Postgres, secrets manager, TLS certs, reverse proxy
3) Configure
   - Set envs: `DATABASE_URL`, `JWT_SECRET`, `RUST_LOG`
4) Database
   - `pnpm atomo migrate -- --service <name>` (CI step)
5) Release
   - Roll out service; verify `/health` and `/ready`
6) Verify
   - Run smoke tests; inspect logs/metrics; create admin user

Where to track this
- This checklist lives under Guide → Advanced. Keep the root `ROADMAP.md` for long‑term planning; use this page for production tasks and validation.
