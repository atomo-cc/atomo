# TODO — consumer field-report fixes (multi-tenant adoption)

Plan and per-item evidence/verdicts: `IMPLEMENTATION_PLAN.md`.
Rule: mark `[x]` only after the item's verification passes. Work in plan order
(F7 → F6 → F4 → F3 → F1 → F5 → F2); one focused commit/PR per item.

## F7 — Version drift

- [x] `atomo_cli`: `#[command(version)]` → `CARGO_PKG_VERSION` (`src/main.rs:26`)
- [x] `init.rs`/`deploy.rs`: SDK pin + deploy manifest derive from `CARGO_PKG_VERSION`; `create-app` pins from its own package version (workspace_dev's `0.1.0` is the generated service package — correct as-is)
- [x] All `package.json`s aligned to 0.7.0; `create-app` added to publishable list + publish + verify steps in AGENTS.md release checklist
- [x] Verify: `atomo --version` → `atomo 0.7.0`; create-app scaffolds `@atomo-cc/client-sdk: ^0.7.0`
- [x] CHANGELOG `[Unreleased]` entry

## F6 — GraphQL charset

- [x] `graphql_handler` now returns `Response` and overwrites `Content-Type` → `application/graphql-response+json; charset=utf-8` (`crates/atomo_server/src/handlers.rs`)
- [x] Integration test `test_graphql_response_declares_utf8_charset` (http_e2e.rs, pg-gated): header + non-ASCII round-trip — passes
- [x] Docs note in `docs/api/graphql.md`
- [x] CHANGELOG `[Unreleased]` entry

## F4 — Rate limiter 429 body

- [x] 429 response gains JSON body `{"error":"rate_limited","retryAfter":<secs>}` (`crates/atomo_server/src/rate_limit.rs`); `Retry-After` header stays
- [x] Decision — fixed in scope: identity = trusted XFF (default, keeps proxied deployments correct) else real peer via `ConnectInfo` wired at serve; `ATOMO_TRUST_X_FORWARDED_FOR=false` opts out. Bonus fix: direct clients previously all shared the 127.0.0.1 fallback bucket
- [x] Unit tests: JSON body + `retry-after`, per-peer buckets, untrusted/trusted XFF — pass
- [x] Docs: `docs/api/index.md` + `docs/guide/configuration.md` + `.env.example`
- [x] CHANGELOG `[Unreleased]` entry

## F3 — RLS fail-closed boot check

- [x] `check_connection_role` probes `pg_roles` (`rolsuper`/`rolbypassrls` on `current_user`) at boot before `ensure_rls_policies`; bypassable role → boot refused (`rls.rs` + `server.rs`)
- [x] Escape hatch `ATOMO_RLS_ALLOW_BYPASS_ROLE` → ERROR log instead of refusal: `ServerConfig.rls_allow_bypass_role` (`#[serde(default)]` for old configs) + `Default` + `from_env` + `.env.example`
- [x] pg-gated test `check_connection_role_refuses_bypass_roles` (rls_enforcement.rs): asserts the check agrees with the role's real attributes — passes
- [x] Docs: `docs/guide/advanced/multi-tenant.md` ("Connect with a non-privileged role (enforced)" + least-privilege role recipe)
- [x] CHANGELOG `[Unreleased]` entry

## F1 — Observable zero-match update

- [x] GraphQL `update` → `Option<HashMap>`; `null` on zero matched rows (`crates/atomo/src/graphql.rs`)
- [x] `updateMany` doc: unmatched ids omitted (return type unchanged)
- [x] Callers checked: admin UI `updateEntity` now throws on null; worker-sdk uses REST CRUD (404 already); client-sdk's named mutations unaffected
- [x] pg-gated test `test_update_zero_match_returns_null` (http_e2e.rs): nonexistent id → null; tenant-scoped miss on NULL-tenant row → null; unscoped match → record — passes
- [x] Docs: `docs/api/graphql.md` update/updateMany semantics + tenant-scoping note
- [x] CHANGELOG `[Unreleased]` entry (mild schema change flagged)

## F5 — Schema parser footguns

- [x] `quote_ident()` applied to all table/column/index/constraint emissions (`sql_builder.rs`, `schema.rs`, `client.rs`, `rls.rs`, `projection.rs`, `migrate.rs`); stale unquoted-SQL assertions updated (80+55 tests green)
- [x] Regression test `reserved_identifiers.rs` (pg-gated): `order` column parses → migrates → inserts → filters → sorts → updates — passes
- [x] `Schema.warnings` collected by BOTH parsers (TS interfaces + builder DSL): entity-shaped interfaces missing from `schema.models`, models entries with no interface, unrecognized keys in `access`/`validation`/`relationships`/model options, dropped DSL fields/access rules/`on` kinds/constraints entries, spreads, reserved-word identifiers. Fixed in passing: `Note: {}` empty-metadata entries no longer drop the model; DSL `constraints:` is now parsed (was silently ignored — caught live on crm-service)
- [x] Surface: `warn!` per warning at server boot (`server.rs`) + new `atomo schema check [--schema]` command; verified on crm-service schema.ts
- [x] Tests: 7 TS-parser diagnostics + 3 DSL diagnostics tests incl. clean-schema no-warning guards
- [x] Docs: `docs/guide/modeling.md` "Schema diagnostics" + `docs/api/cli.md` command entry; `@atomo-cc/schema` gains `unique`/`index`/`check`/`ModelDef.constraints`
- [x] CHANGELOG `[Unreleased]` entry

## F2 — Cache boundary docs (no code change)

- [x] `docs/guide/caching.md`: "What does not invalidate the cache" — direct SQL, other instances (`ATOMO_CACHE_MULTI_INSTANCE`), eventual mode; poll-driven consumer guidance (no-op writes look identical to staleness; `enabled:false` for queue-like models)
- [ ] Reply to the reporting consumer (outside this repo): F1 explains the observed loop; ask whether out-of-band writes were involved

## Final gate

- [x] `cargo test --workspace` green (52 suites, 0 failures) + `cargo clippy --workspace --all-targets -- -D warnings` clean
- [x] `pnpm test` green (worker-sdk, client-sdk, admin-ui)
- [x] `ci.yml` dispatched on `fix/consumer-field-report` (run 34708183958): all five jobs green — Test Suite, Linting, Frontend Tests, Admin E2E Smoke, Build
