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
- Auth (JWT + RBAC): ✅ argon2id hashing, RBAC enforcement in resolvers, OAuth2/OIDC SSO
- Audit logs: ✅ REST endpoints + platform GraphQL
- TypeScript SDK: ✅ types, React hooks, offline queue with sync-on-reconnect
- WASM plugin runtime: ✅ fuel metering, permission-checked host functions, plugin lifecycle, CRUD hooks wired at boot
- Real-time: ✅ GraphQL subscriptions over WebSocket with model filtering
- Event sourcing: ✅ event_log persistence, replay, entity history, CQRS projections (listeners started at boot)
- AI: ✅ pgvector EmbeddingStore with similarity search
- Multi-tenant: ✅ TenantCtx with row-level isolation
- Workflow engine: ✅ triggers, conditions, retry policies, event-driven listener started at boot
- Caching: ✅ read cache with TTL and event-driven invalidation
- Rate limiting: ✅ per-IP token bucket middleware
- Observability: ✅ structured tracing with request ID propagation
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
- Performance: 3–5× faster than common Node.js stacks on core paths
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
  - Fulfill WASM host requests: execute the recorded DB/HTTP capability requests (currently captured, not yet acted on)
  - Optional reactflow drag-and-drop workflow canvas (list-based designer already shipped)
- Ecosystem
  - Plugin marketplace / registry (discovery, install, publish)
  - Atomo Cloud managed hosting
- Hardening
  - Centralize permission checks; expand integration test coverage across the server boot path

If you’re evaluating Atomo, the Guide covers the implemented developer workflows. For platform philosophy and architecture, see Vision & Architecture.
