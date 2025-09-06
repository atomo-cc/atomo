# Rust APIs

Rust crates live in `crates/` and expose Atomo’s core capabilities.

## Workspace Crates
- `crates/atomo` — Unified library and binary
- `crates/atomo_core` — Domain models, errors, traits
- `crates/atomo_server` — Axum-based server + GraphQL
- `crates/atomo_cli` — Developer CLI
- `crates/atomo_schema` — TS schema parsing and codegen
- `crates/atomo_wasm_runtime` — WASM plugin runtime

## Build & Docs
```bash
cargo build --workspace
cargo doc --workspace --no-deps
```

API docs can be published into the site via `pnpm -w docs:api`.
