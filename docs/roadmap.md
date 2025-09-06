# Roadmap

This page summarizes the current implementation status and aligns it with the vision in Atomo-about.md and Atomo-paper.md. For the long-form plan, also see the repository root `ROADMAP.md`.

- Root roadmap: https://github.com/atomo-org/atomo/blob/main/ROADMAP.md

## Status Overview

- CLI and dev runtime: implemented (init, migrate, codegen, dev, workspace-dev)
- Schema → Rust/GraphQL/codegen: implemented with hot reload
- GraphQL API: implemented and merged with platform queries
- Admin UI: dynamic rendering core implemented; proxied in workspace-dev
- Auth (JWT + RBAC): implemented; hashing is dev-stub (see notes)
- Audit logs: implemented with REST endpoints and platform GraphQL
- TypeScript SDK: implemented (types and React hooks)
- WASM plugin runtime: manifest and permission types implemented; runtime integration pending
- Real-time collaboration: groundwork present; WebSocket/CRDT integration pending

## Implemented Highlights

- Development runtime
  - `.atomo/runtime` generation, incremental compilation, hot reload
  - `workspace-dev` watches core crates and service schema; proxies Admin UI under `/admin`
- GraphQL and metadata
  - Service + platform schema merge (users, sessions, audit)
  - `/meta/schema` JSON metadata and `GET /schema.ts` in dev
  - GraphQL IDE: `/graphql` (dev server) or `/playground` (workspace-dev)
- Auth and sessions
  - JWT issuance/verification and session storage in Postgres
  - Role model (`Admin|Manager|Sales|Support|Viewer`) with basic RBAC checks
  - REST: `/auth/login`, `/auth/logout`, `/auth/me`
- Audit
  - REST: `/audit/logs`, `/audit/user/:id/activity`, `/audit/entity/:type/:id/audit`, `/audit/statistics`
  - Platform GraphQL queries for users and sessions
- SDK
  - Type generation and React hooks scaffolding in `packages/atomo-client-sdk`

## In Progress / Planned

- WASM runtime execution and sandboxing (wasmtime integration)
- Production-grade password hashing (bcrypt/argon2) and policy
- Real-time subscriptions and collaboration (WebSocket/CRDT wiring)
- AI integrations (pgvector, content understanding APIs)
- Security hardening, permission matrix, and multi-tenant support

## Docs vs Code Notes

- Password hashing
  - Code: hashing/verification is stubbed for development
  - Docs: clearly marked in Auth docs; enable bcrypt before production
- Rate limiting
  - Docs: example headers shown in API Overview
  - Code: no limiter in server code yet → future work
- WASM plugins
  - Code: `PluginManifest`/`Permission`/`PluginContext` present
  - Docs: manifest and permissions documented; execution runtime pending

## Mapping to Vision (About/Paper)

- “Instantly compiled service runtime” (paper §2.2): implemented via dev runtime and workspace-dev
- “Schema-driven platform” (about/paper): implemented end-to-end (TS → GraphQL/UI/SDK)
- “Hydra UI” (paper §2.4): Admin UI dynamic renderer exists; cross-platform renderers are future work
- “Local-first & collaboration” (about/paper): architectural groundwork; CRDT/sync SDK planned
- “Extensibility via WASM” (about/paper): types in place; runtime execution/sandboxing in progress

## Next Milestones

- Security
  - Switch password hashing to bcrypt/argon2; add migration and config gates
  - Tighten auth middleware and permission patterns
- Runtime & DX
  - Land wasmtime sandbox with permission checks
  - Stabilize hot-reload for wider crate changes, improve debounce
- Collaboration
  - Enable GraphQL subscriptions/WebSocket channels for UI updates
  - CRDT-backed models where applicable
- Observability
  - Structured logging, tracing spans, request IDs
  - Optional rate limiting and metrics endpoints

If you’re evaluating Atomo, the Guide covers the implemented developer workflows. For deep technical context, consult Atomo-about.md and Atomo-paper.md; this page keeps a living snapshot of what’s shipped versus envisioned.
