# Implementation Plan — consumer field-report fixes (multi-tenant adoption)

Source: a field report from a consumer that shipped per-user tenants on the
atomo backend (RLS + PerUser registration + worker tokens). Every item below was
**verified against this repo's code** before being scheduled — verdicts and code
anchors are listed per item. Three reported items were confirmed as real gaps;
two were partially stale; one is a likely misdiagnosis we answer with docs, not
code.

Verification date: 2026-09-12. Target: next minor release.

---

## F1 — GraphQL `update`/`updateMany` silently no-op on zero matched rows ✅ real

**Verified.** `Mutation::update` does
`results.into_iter().next().unwrap_or_default()` — a tenant-scoped update that
matches zero rows returns `update: {}` with no error and no count
(`crates/atomo/src/graphql.rs:491`). `updateMany` drops non-matching entries
silently (`graphql.rs:533`). Root cause: `scope_by_tenant` appends
`tenant_id = $tid` (`client.rs:19`), which is stricter than the RLS policy
(`tenant_id IS NULL OR …`, `rls.rs:78`) — a scoped write can never hit a
NULL-tenant row, and nothing tells the caller.

Contrast: `delete`/`restore`/`hardDelete` already return counts, and worker CRUD
`PATCH /api/worker/crud/{model}/{id}` already 404s on zero match
(`crud_routes.rs:260`). The gap is GraphQL-only.

**Design.** Make the zero-match case observable without breaking the success
shape:

- `update` → return type becomes `Option<HashMap>`; returns `null` when zero
  rows matched (field becomes nullable in the schema — minimal, honest signal;
  Prisma-style "record to update not found" semantics).
- `updateMany` → unchanged return type (list length is already the signal), but
  document that unmatched ids are omitted.
- Update the doc comments on both resolvers.

**Files:** `crates/atomo/src/graphql.rs` (resolvers), any SDK/typing surface that
assumes non-null `update` (`packages/atomo-client-sdk`), admin UI update flow if
it reads the mutation result.

**Verification:** unit test — scoped update on a non-matching row returns `null`;
pg-gated integration test in `tests/` for the tenant-scope no-op → null path.

## F2 — Read-cache "invalidation gap" ⚠️ misdiagnosis → docs, not code

**Not reproduced in code.** Every `AtomoClient` write calls
`cache.invalidate_model` **unconditionally** after commit — including zero-row
no-ops (`client.rs:489/594/692/777/837/897`). In the default `strong` mode,
invalidation bumps the generation *and* clears all entries; `set_if_generation`
rejects stale fills, including fills racing a cancelled invalidation. A poll
loop cannot outlive an update that went through the client — the reported
"stale `queued`" rows were the DB truth left behind by F1's no-op writes.

Real staleness is still possible — and undocumented — when a writer bypasses
`AtomoClient` entirely: hand-run SQL/backfills, a second server instance
(`multi_instance` exists for this), or `eventual` mode (TTL-bounded by design).

**Design (docs only):**

- `docs/guide/caching.md`: add an explicit "what does NOT invalidate" section —
  direct SQL, other processes/instances (`ATOMO_CACHE_MULTI_INSTANCE`), eventual
  mode semantics, and the poll-loop guidance (don't cache-serve job queues; use
  a per-model `enabled: false` override for queue-like models via
  `ATOMO_CACHE_MODELS`).
- Reply to the consumer (outside this repo): explain the F1↔F2 link and ask
  whether any out-of-band writes happened during the incident.

**Verification:** docs render; no code change.

## F3 — RLS silently bypassed under a superuser/BYPASSRLS connection ✅ real

**Verified.** `ensure_rls_policies` emits `FORCE ROW LEVEL SECURITY`
(`rls.rs:74`), which covers the *table-owner* bypass — but Postgres superusers
and `BYPASSRLS` roles bypass RLS regardless. Today that is only a comment; boot
succeeds and isolation silently does nothing.

**Design.** When `enable_rls` is on, after policies are applied, probe the
connection role once:

```sql
SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user
```

- If either flag is set → **refuse to boot** (fail closed — a security feature
  that silently does nothing is worse than a crash) with a message naming the
  role and pointing at `docs/guide/advanced/multi-tenant.md`.
- Escape hatch: `ATOMO_RLS_ALLOW_BYPASS_ROLE=true` downgrades to an `ERROR` log
  for deployments that intentionally connect as a privileged role behind other
  controls. Goes in `ServerConfig` (field + `Default` + `from_env`) and
  `.env.example`.

**Files:** `crates/atomo_server/src/rls.rs`, `server.rs` boot sequence,
`config.rs`, `.env.example`, `docs/guide/advanced/multi-tenant.md`.

**Verification:** pg-gated test asserting boot refusal under a superuser role
and success under a plain role (and under the escape hatch).

## F4 — Rate limiter: 429 has `Retry-After` but an empty body ✅ partial

**Verified.** Per-IP fixed-window bucket, all-at-once reset (no gradual refill)
— as reported. `Retry-After` already shipped (commit `11921dc`, ≥ v0.5.10), so
the remaining gap is only the empty body, which makes a 429 indistinguishable
from a transport error without header inspection
(`crates/atomo_server/src/rate_limit.rs:97-100`).

**Design.** Keep `Retry-After`; add a JSON body:

```json
{ "error": "rate_limited", "retryAfter": <secs> }
```

Also noticed while verifying: `x-forwarded-for` is trusted unconditionally —
spoofable bypass when not behind a trusted proxy. Add a note in docs (or gate
XFF parsing behind an env like `ATOMO_TRUST_X_FORWARDED_FOR`, default off →
falls back to connection IP); decide during implementation whether that is in
scope for this change or a follow-up.

**Files:** `rate_limit.rs`, `docs/api/` (rate-limit behaviour section).

**Verification:** unit test asserting status + `retry-after` header + JSON body
shape.

## F5 — Schema parser footguns ✅ real, broader than reported

**Verified.** The TypeScript schema parser is regex + brace-walking throughout
(`typescript_parser.rs`, `dsl_parser.rs`, `schema_dsl_parser.rs`); unrecognized
constructs are dropped with zero diagnostics — and `parse_schema` even filters
parsed models to those listed in `export const schema.models`
(`typescript_parser.rs:430`), so an exported `model(...)` with no `models`
entry vanishes silently.

The `order` crash is one instance of a wider bug: **no identifier is ever
quoted** — `sql_builder.rs` emits bare column names in `INSERT`/`SET`/`WHERE`
and `schema.rs` emits them in `CREATE TABLE`/`ALTER`. Any Postgres reserved word
as a field name (`order`, `user`, `group`, `select`, `default`, …) produces
invalid DDL at boot → `AtomoClient::new` errors → container crash-loops.

**Design (two workstreams):**

1. **Quote all identifiers.** Introduce one `quote_ident(&str)` helper
   (`"name"` with `"`→`""` escaping) and apply it to every column/table emission
   point in `sql_builder.rs` and `schema.rs` (migrations, where-builder,
   order-by, constraint generation). Regression test: a model with a field
   literally named `order` parses, migrates, inserts, and updates.
2. **Parser diagnostics.** Collect `Vec<SchemaWarning>` during parsing:
   exported `model()`/`interface` names not referenced by `schema.models`,
   `access`/`validation`/`relationships` blocks whose inner keys fail their
   regexes, reserved-word field names (warn even though quoting now fixes the
   crash — name still collides in generated APIs). Surface them as `warn!`s at
   server boot and in `atomo schema check` CLI output.

**Files:** `crates/atomo/src/query/sql_builder.rs`, `crates/atomo/src/schema.rs`,
`crates/atomo_schema/src/{typescript_parser,dsl_parser,schema_dsl_parser}.rs`,
`crates/atomo_cli` (`schema check`), `docs/guide/schema.md` (or the schema guide
page that documents the DSL).

**Verification:** unit tests per workstream; the reserved-word model e2e;
`atomo schema check` on a fixture with a dropped construct prints a warning.

## F6 — GraphQL responses lack `charset=utf-8` ✅ real (upstream; fixable here)

**Verified.** `async-graphql-axum` 7.2.1 hardcodes
`Content-Type: application/graphql-response+json` with no charset
(upstream `response.rs`). Generic HTTP clients then decode non-ASCII bodies as
latin1.

**Design.** `graphql_handler` already owns the response boundary — convert
`GraphQLResponse` into `axum::response::Response` and overwrite `Content-Type`
with `application/graphql-response+json; charset=utf-8` before returning. (No
fork, no upstream patch needed.)

**Files:** `crates/atomo_server/src/handlers.rs`.

**Verification:** integration test asserting the content-type header value on a
GraphQL response containing non-ASCII data.

## F7 — Version/docs drift ✅ partially stale; worse variant found

**Verified current state:** all 9 Rust crates and both publishable npm packages
are `0.7.0`, matching tag `v0.7.0`; the roadmap now marks RLS shipped. The
report's specifics are mostly resolved — **but verification found worse drift:**

- `atomo_cli` hardcodes `#[command(version = "0.1.0")]`
  (`crates/atomo_cli/src/main.rs:26`) — the CLI binary reports 0.1.0 against a
  v0.7.0 release.
- `init.rs` scaffolds new projects with `"@atomo-cc/client-sdk": "^0.1.0"`
  (`crates/atomo_cli/src/commands/init.rs:46`) — new consumers install a stale
  SDK by default.
- Non-publishable package.jsons lag: root `0.1.0`, `atomo-admin-ui` `0.1.0`,
  `atomo-schema` `0.1.0`, `create-app` `0.1.3`. Cosmetic but it is what a
  browsing consumer sees.

**Design.**

- `atomo_cli`: `#[command(version = env!("CARGO_PKG_VERSION"))]` so the CLI can
  never drift again.
- `init.rs` (+ `deploy.rs`, `workspace_dev.rs` templates): derive the SDK pin
  from the CLI's own version instead of a literal.
- Align non-publishable package.json versions to the release version as part of
  the release checklist (or document why they stay 0.1.x); update the Release
  Checklist in `AGENTS.md` if a step is missing.

**Files:** `crates/atomo_cli/src/main.rs`, `commands/{init,deploy,workspace_dev}.rs`,
root + non-publishable `package.json`s, `AGENTS.md` release checklist.

**Verification:** `atomo --version` prints the crate version; a scaffolded
project's package.json pins the current SDK line.

---

## Order of work

Grouped by risk, lowest first; each lands as its own focused commit/PR per the
Feature Change Checklist:

1. F7 — mechanical, no behaviour change.
2. F6 — one-line header patch + test.
3. F4 — 429 JSON body (+ XFF decision).
4. F3 — boot-time role probe + escape hatch.
5. F1 — nullable `update` (mild schema change; call out in changelog).
6. F5 — identifier quoting + parser diagnostics (largest; splits into two commits).
7. F2 — docs paragraph, ships with whichever PR lands last.

Every item: unit tests co-located + integration test where wired, `cargo test`,
`cargo clippy -- -D warnings`, docs page, `CHANGELOG.md [Unreleased]` entry.
Final gate: dispatch `ci.yml` on the merged result and confirm all five jobs.
