# Architecture Overview

Atomo is a Content Core: a schema-driven, event-sourced platform.

- Core: Rust workspace in `crates/` — high performance, type-safe.
- Server: `atomo_server` (Axum + async-graphql) with subscriptions.
- CLI: `atomo_cli` orchestrates codegen, dev runtime, build, deploy.
- Schema: `atomo_schema` parses `schema.ts` (SWC) and generates code.
- Projectors: `atomo_projectors` build read models from the event log.
- Plugins: `atomo_wasm_runtime` executes sandboxed WASM extensions.

Pillars (from Atomo About & Paper):
- River of Events: all changes are immutable events (audit/time travel).
- Flowing Canvas: rich content blocks and flexible composition.
- Energy Hub: open integrations via events and WASM plugins.

Data flows: `schema.ts` → codegen → GraphQL API + Admin UI + SDK types.
