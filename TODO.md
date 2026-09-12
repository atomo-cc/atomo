# TODO — consumer field-report fixes (multi-tenant adoption)

Plan and per-item evidence/verdicts: `IMPLEMENTATION_PLAN.md`.
Rule: mark `[x]` only after the item's verification passes. Work in plan order
(F7 → F6 → F4 → F3 → F1 → F5 → F2); one focused commit/PR per item.

## F7 — Version drift

- [ ] `atomo_cli`: `#[command(version = env!("CARGO_PKG_VERSION"))]` (`src/main.rs:26`)
- [ ] `init.rs`/`deploy.rs`/`workspace_dev.rs`: scaffold SDK pin derives from CLI version (no `^0.1.0` literal)
- [ ] Align non-publishable `package.json` versions (root, admin-ui, atomo-schema, create-app) or document the policy; update AGENTS.md release checklist if a step is missing
- [ ] Verify: `atomo --version` matches crate version; scaffolded package.json pins current SDK
- [ ] CHANGELOG `[Unreleased]` entry

## F6 — GraphQL charset

- [ ] `graphql_handler`: overwrite `Content-Type` → `application/graphql-response+json; charset=utf-8` (`crates/atomo_server/src/handlers.rs`)
- [ ] Integration test: non-ASCII GraphQL response carries the charset
- [ ] CHANGELOG `[Unreleased]` entry

## F4 — Rate limiter 429 body

- [ ] 429 response gains JSON body `{"error":"rate_limited","retryAfter":<secs>}` (`crates/atomo_server/src/rate_limit.rs`); `Retry-After` header stays
- [ ] Decide: gate `x-forwarded-for` trust behind env (default off) or defer as follow-up — record decision here
- [ ] Unit test: status + `retry-after` + body shape
- [ ] Docs: rate-limit behaviour (`docs/api/`)
- [ ] CHANGELOG `[Unreleased]` entry

## F3 — RLS fail-closed boot check

- [ ] When `ATOMO_ENABLE_RLS` on: after `ensure_rls_policies`, probe `pg_roles` for `rolsuper`/`rolbypassrls` on `current_user`; refuse boot when bypassable (`crates/atomo_server/src/rls.rs` + `server.rs`)
- [ ] Escape hatch `ATOMO_RLS_ALLOW_BYPASS_ROLE` → ERROR log instead of refusal: `ServerConfig` field + `Default` + `from_env` + `.env.example`
- [ ] pg-gated test: superuser role → boot refused; plain role → ok; escape hatch → ok
- [ ] Docs: `docs/guide/advanced/multi-tenant.md` (connect-as-role requirement)
- [ ] CHANGELOG `[Unreleased]` entry

## F1 — Observable zero-match update

- [ ] GraphQL `update` → `Option<HashMap>`; `null` on zero matched rows (`crates/atomo/src/graphql.rs`)
- [ ] `updateMany` doc: unmatched ids omitted (return type unchanged)
- [ ] Check `packages/atomo-client-sdk` + admin UI for non-null `update` assumptions; update typings/callers
- [ ] Unit test: scoped no-match update → `null`; pg-gated integration test for the tenant-scope no-op
- [ ] Docs: mutation semantics (`docs/api/`)
- [ ] CHANGELOG `[Unreleased]` entry (flag mild schema change)

## F5 — Schema parser footguns

- [ ] `quote_ident()` helper; apply to all column/table emissions in `sql_builder.rs` + `schema.rs` (migrations, where, order-by, constraints)
- [ ] Regression test: field named `order` (and one more reserved word) parses → migrates → inserts → updates
- [ ] Parser collects `SchemaWarning`s: exported models not in `schema.models`, `access`/`validation`/`relationships` blocks with unrecognized inner keys, reserved-word field names
- [ ] Surface warnings: `warn!` at server boot + `atomo schema check` output
- [ ] Test: `schema check` on fixture with a dropped construct prints the warning
- [ ] Docs: schema DSL page notes diagnostics + reserved words
- [ ] CHANGELOG `[Unreleased]` entry

## F2 — Cache boundary docs (no code change)

- [ ] `docs/guide/caching.md`: "what does NOT invalidate" — direct SQL, other instances (`ATOMO_CACHE_MULTI_INSTANCE`), eventual mode; recommend per-model `enabled:false` for queue-like models
- [ ] Reply to the reporting consumer (outside this repo): F1 explains the observed loop; ask whether out-of-band writes were involved

## Final gate

- [ ] `cargo test --workspace` + `cargo clippy -- -D warnings` green
- [ ] `pnpm test` (packages) green if SDK/admin touched
- [ ] Dispatch `ci.yml`; all five jobs green
