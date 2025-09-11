---
title: Roadmap
description: Living implementation status and upcoming milestones for Atomo.
---

# Roadmap

This page is the single source of truth for delivery status and upcoming milestones. For the long‑term vision and architecture, see Vision & Architecture.

- Vision: /vision

## Status Overview

- CLI and dev runtime: implemented (init, migrate, codegen, dev, dev --workspace)
- Schema → Rust/GraphQL/codegen: implemented with hot reload
- GraphQL API: implemented and merged with platform queries
- Admin UI: dynamic rendering core implemented; proxied in workspace mode
- Auth (JWT + RBAC): implemented; hashing is dev-stub (see notes)
- Audit logs: implemented with REST endpoints and platform GraphQL
- TypeScript SDK: implemented (types and React hooks)
- WASM plugin runtime: manifest and permission types implemented; runtime execution pending
- Real-time collaboration: groundwork present; WebSocket/CRDT integration pending

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

## In Progress / Planned (Near‑Term)

- WASM runtime execution and sandboxing (wasmtime integration)
- Production‑grade password hashing (bcrypt/argon2) and policy
- Real‑time subscriptions and collaboration (WebSocket/CRDT wiring)
- AI integrations (pgvector, content understanding APIs)
- Security hardening, permission matrix, and multi‑tenant support

## Docs vs Code Notes (Ground Truth)

- Password hashing
  - Code: hashing/verification is stubbed for development
  - Docs: clearly marked in Auth docs; enable bcrypt/argon2 before production
- Rate limiting
  - Docs: example headers shown in API Overview
  - Code: no limiter in server code yet → future work
- WASM plugins
  - Code: `PluginManifest`/`Permission`/`PluginContext` present
  - Docs: manifest and permissions documented; execution runtime pending

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

- Security
  - Switch password hashing to bcrypt/argon2; add migration and config gates
  - Tighten auth middleware and permission patterns
- Runtime & DX
  - Land wasmtime sandbox with permission checks
  - Stabilize hot‑reload for wider crate changes; improve debounce
- Collaboration
  - Enable GraphQL subscriptions/WebSocket channels for UI updates
  - CRDT‑backed models where applicable
- Observability
  - Structured logging, tracing spans, request IDs
  - Optional rate limiting and metrics endpoints

If you’re evaluating Atomo, the Guide covers the implemented developer workflows. For platform philosophy and architecture, see Vision & Architecture.
