# Architecture Overview

Atomo is a Content Core: a schema-driven, event-sourced platform.

- Core: Rust workspace in `crates/` — high performance, type-safe.
- Server: `atomo_server` (Axum + async-graphql) with subscriptions.
- CLI: `atomo_cli` orchestrates codegen, dev runtime, build, deploy.
- Schema: `atomo_schema` parses `schema.ts` (SWC) and generates code.
- Projectors: `atomo_projectors` build read models from the event log.
- Realtime: `atomo_realtime` is a transport-agnostic, in-memory hub for the
  ephemeral, high-frequency tier (channels, presence, fan-out); `atomo_server`
  mounts its WebSocket transport at `/realtime/ws`. It never touches the event
  store — only durable *outcomes* flow back through the normal command path.
- Control plane: `atomo_control_plane` runs many isolated projects on shared
  infrastructure — a per-project database + `atomo-server` instance, managed by a
  registry, provisioner, and gateway. Purely additive: it sits *in front of*
  unmodified servers. See [Multi-Project Platform](/guide/advanced/multi-project-design).

Pillars (from Atomo About & Paper):
- River of Events: all changes are immutable events (audit/time travel).
- Flowing Canvas: rich content blocks and flexible composition.
- Energy Hub: open integrations via events and WASM plugins.

Data flows: `schema.ts` → codegen → GraphQL API + Admin UI + SDK types.
