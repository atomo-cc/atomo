# TODO — consumer field-report fixes (multi-tenant adoption)

## 2026-10-15 constraint parser panic

- [x] Reproduce CHECK/partial-index string parsing failures: new parser regression fails with the same slice panic; real `atomo-cli schema check` probe exits 101.
- [x] Limit constraint scanning to unconsumed entries and preserve SQL literal quotes; schema suite 106/106 and real CLI integration regression passed.
- [x] Document quoted expression support and Unreleased fix; workspace tests/check, all-target workspace clippy (`-D warnings`), dependent server build, fmt and diff checks passed. Logs: `.tmp/review-constraint-{workspace-test,check,clippy,server-build}.log`.

Schema fix committed as `c9b23b8`. Next: address the separate RLS installation finding; remote CI and Docker-image verification remain open.

## 2026-10-07 Admin scaffold ownership

Plan: docs/implementation/admin-scaffold/IMPLEMENTATION_PLAN.md.

- [x] Inventory: create-app already provides schema + Compose + packaged /admin; reuse it.
- [x] Add fullstack-atomo compatibility alias, real CLI tests, package-content verification (9/9 Node 20 tests, npm pack dry-run included).
- [x] Add actual admin production preview mode to existing E2E; fail browser page errors (production TypeScript + Vite build passed; runtime verification below).
- [x] Verify production build + existing E2E against isolated local backend/database: final same-origin dev 9/9 (21.64s), preview 9/9 twice (21.11s/22.33s), native `/admin/` 9/9 twice (19.06s/18.47s); every run exited 0. Evidence under `.tmp/e2e-review-20261015/`.
- [ ] Investigate final same-origin production login timeout (case 8, Axios 10s); no retirement based on reruns alone.
  - [x] Capture safe login timing on disposable backend: 5 direct + 5 proxy requests all 200, 344–406ms; per-browser login lifecycle logged without bodies/headers/tokens. Evidence: `.tmp/e2e-review-20261015/login-timing.json`.
  - [x] Observe bounded browser/preview cleanup: browser close 97/191ms and preview ~1.6s; both complete processes exit 0. Historical delay not reproduced or attributed; timeouts/retries unchanged.
  - [x] Two consecutive isolated production-preview runs: 9/9 each, exit 0, 29.50s/26.39s; no overlapping builds. Evidence: `.tmp/e2e-review-20261015/production-{1,2}{.log,-exit.json}`. These passes do not establish the old timeout's cause.
- [x] Support `/admin/` navigation in the same smoke and verify native server static assets/deep links: two full exit-0 passes. Existing CI job extended; YAML and each e2e shell step pass syntax checks. Documentation build also passed.
- [x] Decouple test proxy selection from the client URL using existing `E2E_API_URL`; final same-origin dev and two production runs pass with empty `VITE_API_URL`. The earlier absolute-API dev diagnostic hit CORS and was stopped (not a pass); no CORS rules changed.
- [x] Documentation build (9.14s), workflow YAML/e2e shell syntax and diff checks passed. Owned native backend stopped and labeled disposable database/container volume removed; logs retained. Historical timeout attribution, remote CI and Docker-image verification remain open; no duplicate-template retirement is authorized by local passes alone.
- [x] Diagnose browser fixture shutdown delay (pw:test log); bounded unchanged-shell run exited 0, 9/9 passed (3.1m). First interrupted run is not counted as a pass. Verify new headless channel and managed preview before retirement.
- [x] Update README/CHANGELOG/existing public guides/roadmap; VitePress build passed (30.75s, rerun 26.75s; nonblocking large-chunk warning).
- [x] Historical 2026-10-07 handoff: final 8 passed / 1 failed plus preview teardown timeout; owned backend/database cleaned up. Changes were uncommitted at that checkpoint. Later verified results are recorded above.


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
