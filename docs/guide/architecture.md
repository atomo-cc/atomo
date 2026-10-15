# Architecture Overview

Fullstack schema/Compose scaffolding is owned by `packages/create-app`; the real
admin frontend is `packages/atomo-admin-ui`, served at `/admin` in the server
image. The Admin CI job runs the same real-backend smoke suite in dev, production
preview, and native-server `/admin/` modes. Docker packaging is verified separately. Generic dashboard UI and adapter
contracts remain in their framework packages.

Automatic table projections use `atomo_projectors::CurrentStateProjection`: startup performs additive column migration and transactional synchronization from current base rows, and notifications refresh individual rows. This read-model maintenance is independent of the guarded historical replay API and does not change event-history coverage.

Atomo is a Content Core: a schema-driven, event-sourced platform.

- Core: Rust workspace in `crates/` — high performance, type-safe.
- Server: `atomo_server` (Axum + async-graphql) with subscriptions.
  Opt-in RLS policy installation is transactional across model tables; any setup
  error rolls back the batch and aborts startup before the listener binds.
- CLI: `atomo_cli` orchestrates codegen, dev runtime, build, deploy.
- Schema: `atomo_schema` parses builder and interface `schema.ts` declarations and generates code.
- Projectors: `atomo_projectors` build read models from the event log.
- Realtime: `atomo_realtime` is a transport-agnostic, in-memory hub for the
  ephemeral, high-frequency tier (channels, presence, fan-out); `atomo_server`
  mounts its WebSocket transport at `/realtime/ws`. It never touches the event
  store — only durable *outcomes* flow back through the normal command path.
- Control plane: `atomo_control_plane` runs many isolated projects on shared
  infrastructure — a per-project database + `atomo-server` instance, managed by a
  registry, provisioner, and gateway. Purely additive: it sits *in front of*
  unmodified servers. See [Multi-Project Platform](/guide/advanced/multi-project-design).
- Metered commands: `atomo_server::metered` — generic primitives (expiring single-use token store,
  integer-unit budget ledger) that compose transactionally with `atomo_server::jobs`
  (`JobStore::enqueue_tx`) so a consumer's metered command commits or rolls back as one unit. No
  business policy; library-only. See [Metered Command Primitives](/guide/advanced/metered-command-primitives).

Pillars (from Atomo About & Paper):
- River of Events: full mutation history by default, optional declared history gaps, and independent durable aggregate event streams.
- Flowing Canvas: rich content blocks and flexible composition.
- Energy Hub: open integrations via events and external workers.

Data flows: `schema.ts` → codegen → GraphQL API + Admin UI + SDK types.

## Storage lifecycle components

`atomo::cache` owns bounded process-local read caching and weakly-owned expiration maintenance. `atomo::history` defines generic history/audit policy; `atomo::event_store::EventStore` applies model-history policy transactionally and persists coverage metadata. `atomo_server::audit` applies independent audit payload/retention policy. Server-owned tasks perform bounded maintenance; the aggregate store in `atomo_core` is unaffected.

`atomo_projectors` checks all source-model replay capabilities before destructive rebuild, coordinating with retention through database locks. `atomo_server::storage_lifecycle_routes` exposes administrator-only, read-only usage and capability diagnostics. See [Storage lifecycle](/guide/storage-lifecycle) and [Storage API](/api/storage).

## Request security

`atomo::security` carries verified request identity and compiles row policies into SQL predicates used by shared CRUD, counts and relation reads. Shared write transactions validate resulting ownership and visible parent tenants before events commit. `atomo_server::auth::request_principal` resolves HTTP tenant selection; Worker CRUD verifies active job delegation stored separately from job payload. No new crate or public transaction API is introduced.
