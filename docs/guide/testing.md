# Testing

Atomo has unit, integration, and HTTP end-to-end tests across the Rust workspace.

## Running tests

```bash
# All unit tests, integration tests (non-DB), and doctests
cargo test --workspace
```

The unit/middleware/WASM/doctest suites need no database and run as part of `cargo test --workspace`.

## Database-backed tests

Some integration tests exercise real Postgres and are marked `#[ignore]`. Point `DATABASE_URL` at an empty test database and pass `--ignored`:

```bash
# Data layer: CRUD → event store → subscription, soft delete, count, relationships
DATABASE_URL=postgresql://localhost/atomo_test \
  cargo test -p atomo --test integration_test -- --ignored --test-threads=1

# HTTP layer: boots the axum router in-process and runs login → create → list
DATABASE_URL=postgresql://localhost/atomo_http_test \
  cargo test -p atomo_server --test http_e2e -- --ignored --test-threads=1
```

Use `--test-threads=1` for DB tests so they don't contend on shared tables.

## What's covered

- **Data layer** (`crates/atomo/tests/integration_test.rs`): create/find/update/delete, soft-delete filtering, event emission + persistence, event replay, count, find-by-id.
- **HTTP layer** (`crates/atomo_server/tests/http_e2e.rs`): `/health`, auth-gating (401 without token), and the full authenticated `login → create → list` cycle via `tower::oneshot`.
- **Middleware** (`crates/atomo_server/tests/middleware.rs`): per-IP rate limiting (allow/deny/isolation) and CORS headers — no DB required.
- **WASM runtime** (`crates/atomo_wasm_runtime/tests/host_api.rs`): loads a real compiled guest module and verifies fuel metering, `host_log`, permission-gated `host_emit_event`, and the `alloc`/`call_hook` ABI.
- **Workflow** (`crates/atomo/src/workflow.rs` unit tests): cron window logic, registration, event-trigger matching, step execution.

## Conventions

- New behavior and regressions should ship with a test.
- Prefer pure, DB-free unit tests where possible (e.g. `cron_should_fire`); reserve `#[ignore]` for tests that truly need Postgres.
