---
title: Roadmap
description: Living implementation status and upcoming milestones for Atomo.
---

# Roadmap

This page is the single source of truth for delivery status and upcoming milestones. For the long‑term vision and architecture, see Vision & Architecture.

- Vision: /vision

## Status Overview

- CLI and dev runtime: ✅ implemented (init, migrate, codegen, dev, dev --workspace, test, deploy)
- Schema → Rust/GraphQL/codegen: ✅ implemented with hot reload
- GraphQL API: ✅ full CRUD with where/orderBy parsing, pagination, relationships
- Admin UI: ✅ dynamic rendering with aligned API client
- Auth (JWT + RBAC): ✅ argon2id hashing, RBAC enforced in GraphQL resolvers (parsed from schema + conformance-tested in S1; **data-layer callers not yet gated**), OAuth2/OIDC SSO
- Audit logs: ✅ REST endpoints + platform GraphQL; conformance-tested through CRM models (B4)
- TypeScript SDK: ✅ types, React hooks, offline queue with sync-on-reconnect (not yet integration-tested)
- WASM plugin runtime: ✅ fuel metering, permission-checked host functions, plugin lifecycle, CRUD hooks wired at boot
- Scripting plugins (JS): ✅ `.js` plugins via embedded Javy/QuickJS (no toolchain) — CRUD hooks, permission-gated effects (`emit`/`dbQuery`/`http`), typed `emit` onto the event stream
- Real-time (durable): ✅ GraphQL subscriptions over WebSocket with model filtering; **WS auth added in S2** (was unauthenticated)
- Real-time (ephemeral): ✅ `atomo_realtime` hub — channels, presence & fan-out over `/realtime/ws` (Phase 2); never event-sourced. CRM dogfood (live Kanban/presence) + coordinator sessions + rate-limit/metrics hardening still TODO
- Event sourcing: ✅ event_log persistence, replay, entity history (conformance-tested C3), CQRS projections (corruption fixed in B2; rebuild-replay still TODO)
- AI: 🟡 pgvector EmbeddingStore with similarity search — code exists, not yet conformance-tested (needs pgvector infra; runs in CI not locally)
- Multi-tenant: 🟢 `tenant_id` column generated + read/write scoping (S3+D1), subscription tenant-filter, per-user binding; **opt-in DB-enforced RLS** (`ATOMO_ENABLE_RLS`) wired through the query executor + Postgres-tested (`rls_enforcement`, `rls_executor`). Subscription/WS-path RLS + event-store tenant scoping still TODO
- Multi-project platform: 🟡 `atomo_control_plane` — silo-per-project (own DB + `atomo-server` instance) via registry / provisioner (Docker driver) / Caddy gateway / reconciler + `atomo project` CLI; compiles, RLS axis Postgres-tested. Provisioner/gateway/API integration tests + control-plane auth still TODO. See `/guide/advanced/multi-project-design`
- External workers & blob storage: 🟡 the **blob store already exists** (`media`/`storage`: pluggable local+S3 backend, upload/serve/delete/GC, soft-delete, tenant scoping, event-sourced) and now serves **HTTP Range** (video/audio seeking, `ETag`/conditional GET). The **durable-job system** is built: event-sourced lease engine (`atomo_server::jobs`: idempotent enqueue, `SELECT … FOR UPDATE SKIP LOCKED` lease/heartbeat/complete/fail, visibility-timeout reclaim, retry/backoff/dead-letter) **exposed over HTTP at `/jobs`** with **worker-token auth** (`X-Worker-Token`, capability-scoped to queues; admins mint via `POST /jobs/workers`) and app-side enqueue (`POST /jobs`, tenant-stamped). Postgres-tested (`jobs_store`, `jobs_http`). A **TypeScript worker SDK** (`@atomo-cc/worker-sdk`) wraps the lease/heartbeat/complete/fail loop (vitest-tested; not yet npm-published). Enqueue seams: `POST /jobs` (app) + a workflow **`Job` step** (no-code enqueue); apps poll via `GET /jobs/{id}`. Guide: `/guide/advanced/jobs-and-workers`; API: `/api/jobs`. Live **job progress** fans out over realtime (`POST /jobs/{id}/progress` → channel `job:{id}`; `ctx.progress` in the SDK). **Enqueue from every seam**: REST `POST /jobs` · GraphQL `enqueueJob` · workflow `Job` step · plugin `enqueueJob` effect · Rust `JobStore::enqueue`. Media now records a sha256 **checksum** with tenant-scoped **content dedup**, plus **presigned direct upload** to S3 (`POST /media/presign` → direct PUT → `POST /media/commit`; MinIO-verified). Only remaining (optional): a native **Rust worker crate** (the TS SDK is done). See `/guide/advanced/workers-and-blobs-design`
- Workflow engine: 🟢 triggers/conditions/retry + YAML loading + HTTP steps + **Mutation steps (GraphQL) and Plugin steps (WASM/JS)** wired via executor seams injected at server boot; advanced JS-authored workflows still TODO
- Caching: ✅ read cache with TTL and event-driven invalidation (conformance-tested C4; pagination cache-key bug fixed C2)
- Rate limiting: ✅ per-IP token bucket middleware

> **Conformance status**: capability claims above are validated against the flagship CRM by the
> CRM Conformance Suite (`/guide/advanced/crm-conformance-plan`), which fixed 8 silent gaps and
> 2 security holes. 🟡 marks capabilities that work in part with documented follow-ups.
- Observability: ✅ structured tracing with request ID propagation
- Benchmarks: 🟡 reproducible engine harness (`examples/bench`) + a **co-located `node-postgres` head-to-head** (`bench/node-baseline.mjs`) — data layer, job-lease engine (`SKIP LOCKED` scaling), JS/Javy hook tax, footprint; results recorded honestly (Atomo is ~2× slower than raw node-pg on writes but wins on cached reads/footprint/engine features — the "3–5× vs Node" line stays a **target**). Full-stack HTTP load test + precise compiled-WASM hook number still TODO. See `/guide/advanced/benchmarks`
- Validation: ✅ rules parsed from schema.ts and enforced (required, email, min, max, numeric)
- Soft deletes: ✅ full lifecycle — delete / restore / hardDelete / trash (deletedRecords) with query filtering
- Audit: ✅ mutations auto-logged with the acting user; admin/manager-gated audit REST
- Workflow designer: ✅ admin-UI editor (list-based, typed action forms, graph preview) on the tested serde layer
- Verification: ✅ CRUD → event store → subscription → audit → projection tested against PostgreSQL (9 data-layer + 8 HTTP E2E + workflow serde unit tests)

## Implemented Highlights

- Development runtime
  - `.atomo/runtime` generation, incremental compilation, hot reload
  - Workspace mode watches core crates and service schema; proxies Admin UI under `/admin`
- GraphQL and metadata
  - Service + platform schema merge (users, sessions, audit)
  - `/meta/schema` JSON metadata and `GET /schema.ts` in dev
- GraphQL IDE: `/graphql` (dev server) or `/playground` (workspace mode)
- Auth and sessions
  - JWT issuance/verification and session storage in Postgres
  - Role model (`Admin|Manager|Sales|Support|Viewer`) with RBAC checks
  - REST: `/auth/login`, `/auth/logout`, `/auth/me`
- Audit
  - REST: `/audit/logs`, `/audit/user/:id/activity`, `/audit/entity/:type/:id/audit`, `/audit/statistics`
  - Platform GraphQL queries for users and sessions
- SDK
  - Type generation and React hooks scaffolding in `packages/atomo-client-sdk`

## Recently Completed

- WASM runtime with fuel metering, permission-checked host functions, plugin lifecycle
- Production password hashing (argon2id with bcrypt fallback)
- Real-time GraphQL subscriptions over WebSocket with model filtering
- AI integrations (pgvector EmbeddingStore with similarity search)
- Multi-tenant isolation (TenantCtx with row-level scoping)
- RBAC enforcement in GraphQL resolvers from schema access rules
- Event sourcing with event_log persistence and replay
- CQRS read projections (TableProjection with event-driven updates)
- Workflow engine with triggers, conditions, and retry policies
- OAuth2/OIDC SSO (Google, GitHub, Microsoft, Okta)
- Rate limiting middleware (per-IP token bucket)
- Structured tracing with request ID propagation
- Input validation (required, email, min, max, numeric)
- Soft deletes with automatic query filtering
- Pagination metadata (total count, has_next/prev)
- Relationship resolution (belongsTo/hasMany)
- Read cache with TTL and event-driven invalidation
- WASM plugin hooks in CRUD lifecycle (before/after)
- Scripting plugins: JS via embedded Javy/QuickJS — runtime selection (`wasm`|`js`), CRUD hooks, permission-gated effects (emit/dbQuery/http) with live fulfillment, typed emit onto the event stream
- Local-first offline queue with sync-on-reconnect (SDK)
- Blog and ecommerce project templates

## Docs vs Code Notes (Ground Truth)

- Password hashing: argon2id in production, bcrypt fallback for existing hashes
- Rate limiting: token-bucket middleware, configurable via RATE_LIMIT_RPS env
- WASM plugins: full lifecycle with fuel metering, permission checks, host functions
- Subscriptions: working over WebSocket at /graphql/ws with model filtering

## Phases (High‑Level)

### Phase 1 — Developer Experience Core (4–6 months)
- P0 Core Infrastructure (mostly complete)
  - CLI toolchain (`init`, `generate`, `migrate`, `dev`, `codegen`, `dev --workspace`)
  - Dual‑mode definitions (TS → Rust/GraphQL) with hot reload
  - Event‑friendly data layer and audit log
- P1 Dynamic API & Admin UI (largely complete)
  - Dynamic GraphQL API (schema merge, CRUD resolvers)
  - Authn/Z (JWT + RBAC), metadata API
  - Dynamic Admin UI engine (schema‑driven rendering)
- P2 Extensibility & AI Foundation (partial)
  - Hook/Access DSL and plugin interfaces
  - WASM runtime scaffolding
  - Extend-without-forking seams: declarable schema constraints
    (`@unique`/`@index`/`@@unique`/`@@index`/`@@check`, incl. partial `WHERE`) and
    plugin-served custom HTTP routes (`/ext/<plugin>`); transactional route handlers
    designed (phase 3)
  - AI groundwork (pgvector, content APIs)

### Phase 2 — Cognition & Edge (6–9 months)
- Event sourcing + CQRS maturation (replay, observability, ops playbooks)
- Local‑first sync foundations (SDK alpha), real‑time subscriptions
- WASM runtime (backend) with permissions and sandboxing
- Edge projections (Workers/Vercel KV), similarity search

### Phase 3 — Ecosystem & Solutions (8–12 months)
- Visual workflow designer with native AI nodes
- “Solutions as code” marketplace and official templates
- Enterprise features (RBAC/ABAC, SSO, multi‑tenant) and Atomo Cloud launch

## Success Metrics & Quality Gates
- Tests: > 85% coverage overall; 100% on critical paths
- Performance (**target, not yet measured against a Node baseline**): 3–5× faster than common
  Node.js stacks on core paths. Engine-level numbers we *do* measure live in
  [Benchmarks](/guide/advanced/benchmarks); a fair head-to-head Node baseline is a follow-up.
- Security: independent security audit
- Reliability: ≥ 99.9% service availability
- Community: GitHub traction and template adoption

## Milestone Timeline (Indicative)
- 2024 Q4 — Phase 1 kickoff; first complete CRM demo
- 2025 Q1–Q2 — Phase 1 completion; open‑source release and community building
- 2025 Q3–Q4 — Phase 2 execution; Atomo Cloud Private Beta
- 2026 Q1+ — Phase 3 expansion; solutions marketplace maturity

## Next Milestones

- Collaboration
  - CRDT‑backed models for conflict-free real-time editing
- Plugins & Workflows
  - ✅ Scripting plugins (JS via Javy/QuickJS): drop in a `.js`, no toolchain — CRUD hooks,
    permission-gated effects (`emit`/`dbQuery`/`http`), typed `emit` onto the event stream
  - Fulfill **compiled** WASM host requests in the live hook path (the JS effect path is wired;
    the compiled-plugin `fulfill_requests` helper exists but isn't yet called from the live flow)
  - Optional reactflow drag-and-drop workflow canvas (list-based designer already shipped)
- Ecosystem
  - Plugin marketplace / registry: read API shipped (M1); discovery → install → publish remain
  - Atomo Cloud managed hosting
- Hardening
  - **CRM Conformance Suite** (active approach): use the flagship CRM as an executable spec
    that exercises the full backend; see `/guide/advanced/crm-conformance-plan`. Already
    surfaced + fixed real codegen/validation gaps (enum→JSONB, array NOT NULL, validation
    regex + data-layer enforcement) that synthetic schemas never reached.
  - Centralize permission checks (compiled host-fn gating + JS effect gating share one seam);
    expand integration test coverage across the server boot path

If you’re evaluating Atomo, the Guide covers the implemented developer workflows. For platform philosophy and architecture, see Vision & Architecture.
