# TODO — consumer field-report fixes (multi-tenant adoption)

## v0.8.0 released (2026-10-15)

- [x] Verify merged product code 084ac35: all five main CI jobs 38746219853 pass; clean Docker build and container admin smoke 9/9, exit 0. Image SHA and cleanup evidence: `.tmp/postmerge-pr82/merge-result.json`. PR #82 merged and its branch deleted.
- [x] Audit release preflight: registry client/worker 0.7.0, create-app 0.1.3; npm whoami fails 401. Choose 0.8.0 for additive APIs and nullable GraphQL update result.
- [x] Repair CLI workflow source ref/targets, existing-Release reuse and publish=false mode. actionlint 1.7.12 and actual workflow shell checks pass (reuse/create/error propagation/asset naming/rerun). Evidence: `.tmp/release-080/workflow-shell-results.json`.
- [x] Remote non-publishing CLI run 38751504410 passes on e128a81 for Linux/macOS/Windows. All three archives downloaded; binary headers confirm x86_64 ELF/Mach-O/PE. Native version checks pass, Release creation/uploads skipped. Evidence: `.tmp/release-080/{cli-result,cli-artifact-audit}.json`.
- [x] Cut changelog and bump all nine Rust crates, root and five npm manifests to 0.8.0. Regenerated both lockfiles; audit proves only nine workspace version changes in Cargo.lock and no pnpm-lock diff.
- [x] Local Rust check and all-target strict clippy pass; all three npm tarballs built and inspected (SDK exports and create-app templates present), real scaffolder tests 9/9. Evidence: `.tmp/release-080/{check,clippy}.log`, `npm-pack-results.json`.
- [x] Rebuilt real Rust CLI reports atomo 0.8.0; init generates client-sdk ^0.8.0. Docs build (13.31s), fmt/diff checks pass. Evidence: `.tmp/release-080/{cli-version,cli-init,docs}.log`.
- [x] Open release PR https://github.com/atomo-cc/atomo/pull/83; all five jobs pass on e128a81: https://github.com/atomo-cc/atomo/actions/runs/38751496488 (includes PostgreSQL and all admin modes). This final handoff only updates TODO/plan; tested code/dependencies/workflow remain identical.
- [x] Restore npm authentication using the user-supplied granular token (kept outside the repository). npm whoami succeeds as hixbuilder, listed maintainer for all three packages. Organization-level access listing returns 403; subsequent non-interactive publication of all three packages succeeds with the supplied token. Evidence: `.tmp/release-080/npm-auth-result.json`.
- [x] Final publication approved by the user; PR #83 squash-merged as 27d71bb. Tested release source e128a81 differs only in TODO/plan evidence.
- [x] All five post-merge main CI jobs 38756500055 pass on 27d71bb, including PostgreSQL and three admin modes. Evidence: `.tmp/release-080/main-ci-result.json`.
- [x] v0.8.0 tag points to 27d71bb and GitHub Release notes are public: https://github.com/atomo-cc/atomo/releases/tag/v0.8.0. Artifact publication/verification remains separately tracked.
- [x] Publish and verify client-sdk 0.8.0 using the validated tarball. Registry SHA512 and latest=0.8.0 verified; `.tmp/release-080/npm-publication.json`.
- [x] Publish and verify worker-sdk 0.8.0 using the validated tarball. Registry SHA512 and latest=0.8.0 verified; `.tmp/release-080/npm-publication.json`.
- [x] Publish and verify create-app 0.8.0 using the validated tarball. Registry SHA512 and latest=0.8.0 verified; `.tmp/release-080/npm-publication.json`.
- [x] Dispatch Docker 38757674572 (explicit tag=v0.8.0), docs 38757685942 and CLI 38757696375 (publish=true) from main; all run on 27d71bb.
- [x] Docker workflow 38757674572 succeeds, publishing v0.8.0 and latest. Public pull/digest/runtime checks are tracked below.
- [x] Documentation workflow 38757685942 deploys successfully; public HTTPS CLI/modeling/multi-tenant pages return 200 with release content markers, including publish=false. Historical DNS/TLS blocker is no longer observed. Evidence: `.tmp/release-080/docs-live-result.json`.
- [x] CLI workflow 38757696375 passes; GitHub Release contains Linux/macOS/Windows x86_64 binaries. Published-asset download verification is tracked separately below.
- [x] Public v0.8.0/latest images pull successfully with the same image ID; release version/SHA and nonroot runtime verified. Published-image admin smoke 9/9 (10.7s), exit 0; nine screenshots and owned-resource cleanup verified. Public docs checked above. Evidence: `.tmp/release-080/{published-image-result,docker-runtime-result,docs-live-result}.json`.
- [x] All three published CLI assets verified: Linux/macOS digests match reviewed x86_64 artifacts; downloaded Windows PE hash matches Release digest and --version reports atomo 0.8.0. Evidence: `.tmp/release-080/release-assets-result.json`.
- [x] Final release evidence recorded in `.tmp/release-080/final-release-result.json`; owned verification containers/network are verified absent. Consumer deployments are outside this release; no specific deployment target was authorized.

Release complete: npm client-sdk/worker-sdk/create-app 0.8.0, Docker v0.8.0/latest,
three CLI assets and public HTTPS docs are verified. Same token completed the
three npm submissions without interactive challenges; async registry visibility
was polled, with no duplicate upload. crates.io remains disabled. No downstream
deployment was selected. Historical local timeout attribution is still open
and does not negate the passing release gates. Published artifacts stay pinned
to 27d71bb; the final handoff changes only maintainer documentation and records.

## PR #82 — latest-stable CI follow-up

- [x] Push three focused commits and open https://github.com/atomo-cc/atomo/pull/82; dispatch CI 38736776136 on f2173e5. Frontend passed; lint exposed async-trait 0.1.89 generated-code double_must_use under Rust 1.99.
- [x] Align local stable with CI (Rust 1.99.0) and update async-trait to upstream 0.1.92. All-target workspace clippy passes with -D warnings; lock audit confirms only async-trait replacement and its new syn 3.0.9 dependency. Evidence: `.tmp/pr82-clippy-rust199.log`, `.tmp/pr82-lock-audit.json`.
- [x] Latest-stable workspace tests/check and all-target clippy pass; fmt/diff checks pass. Existing PostgreSQL/admin verification is supplemented by the initial remote green jobs. Logs: `.tmp/pr82-{tests,check,clippy}-rust199.log`.
- [x] Server build on Rust 1.99 passes: `.tmp/pr82-build-rust199.log`.
- [x] Resolve smoke output directories against the admin package root; actual Playwright configuration checks pass for default, relative and absolute paths.
- [x] Remote run 38738387183 retains artifact 11975095386 (1,938,847 bytes); downloaded archive contains 9 screenshots per dev/production/server mode. Smoke passes 9/9 each (21.7s/16.3s/15.6s). Evidence: `.tmp/pr82-e2e-second.log`, `.tmp/pr82-artifact-audit.json`.
- [x] Push CI correction 349e3aa and verify all five jobs pass on run https://github.com/atomo-cc/atomo/actions/runs/38738387183 (Test Suite including PostgreSQL, Frontend Tests, Admin E2E Smoke, Linting, Build). This handoff only records results in TODO/plan; tested code remains 349e3aa.

PR #82 was subsequently merged as 084ac35. Main CI 38746219853 and clean Docker packaging/runtime smoke pass (9/9, exit 0); owned containers/network removed. Historical timeout attribution remains unresolved.

## 2026-10-15 RLS policy installation

- [x] PostgreSQL regressions reproduce both failures on a non-superuser owner: missing table and missing tenant_id return success instead of errors (2 failed / 1 success-idempotence passed). Snapshots cover flags and old policies; disabled/no-connection unit passed in workspace tests. Evidence: `.tmp/rls-review-20261015/regressions-before.log`.
- [x] Install all requested policies in one transaction, with stable table lock order and contextual errors; final PostgreSQL tests prove full-batch rollback of flags and old policies, successful commit and idempotence.
- [x] Real server.run() rejects damaged policy installation even with the role-bypass flag enabled. Non-superuser/no-BYPASSRLS owner: RLS integration 6/6 + query-executor tenant isolation 1/1 passed. Evidence: `.tmp/rls-review-20261015/{nonprivileged-final,executor-final}.log`.
- [x] Updated multi-tenant/config guides, architecture, both roadmaps, README/CHANGELOG and existing env comments. Workspace tests/check, all-target clippy (`-D warnings`), server build, fmt/diff and docs build (17.69s) passed. Evidence: `.tmp/rls-review-20261015/{workspace-tests,workspace-check,workspace-clippy-final,server-build,docs-build}.log`.
- [x] Verified the real startup failure under both nonprivileged and explicitly allowed superuser roles. Repaired the existing executor fixture to drop widgets (including its superuser skip path); checked zero leftover RLS model tables, then removed the owned PostgreSQL container/volume. Evidence: `.tmp/rls-review-20261015/{bypass-startup,executor-super-skip}.log` and `cleanup.json`.

PR #82 is merged; main CI and Docker verification are recorded above.
Historical E2E timeout attribution remains open. The new release preparation
is tracked at the top; no public v0.8.0 publication has occurred.

## 2026-10-15 constraint parser panic

- [x] Reproduce CHECK/partial-index string parsing failures: new parser regression fails with the same slice panic; real `atomo-cli schema check` probe exits 101.
- [x] Limit constraint scanning to unconsumed entries and preserve SQL literal quotes; schema suite 106/106 and real CLI integration regression passed.
- [x] Document quoted expression support and Unreleased fix; workspace tests/check, all-target workspace clippy (`-D warnings`), dependent server build, fmt and diff checks passed. Logs: `.tmp/review-constraint-{workspace-test,check,clippy,server-build}.log`.

Schema fix: `c9b23b8`. Admin scaffold/runtime coverage: `f9a4cca`.
The verified RLS installation fix is f2173e5; final remote CI and
Docker-image verification remain open.

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
