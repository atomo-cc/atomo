---
title: 'Plan: CRM Conformance Suite'
description: Use the flagship CRM as an executable specification that exercises Atomo's full backend surface.
---

# Plan: CRM Conformance Suite

## Why

Atomo's README calls the CRM the flagship app that "drives platform evolution," but until
recently nothing enforced that — every Rust integration test used a synthetic 2–3 field
schema. The first test that ran the **real** `services/crm-service/schema.ts` through the
platform (`crates/atomo/tests/crm_dogfood.rs`) immediately surfaced four silent bugs that
toy schemas could never reach (enum→JSONB, array `NOT NULL`, validation regex only matching
single quotes, and validation never enforced in the data layer).

This plan turns the CRM from a *demo* into an **executable specification**: a conformance
suite of integration tests, all driven by the real `schema.ts`, that systematically walk
Atomo's capability surface. If a platform change breaks the flagship, a test goes red.

**Framing (honest):** the CRM can validate *most* of the backend, but not all. A few
capabilities (multi-tenant, OAuth, CLI, SDK offline sync) need their own harnesses because
the CRM can't naturally express them. So: **CRM as the primary conformance driver, plus
targeted supplementary harnesses for what it structurally can't reach.**

## Status legend

- ✅ **conformance-tested via CRM** — proven against the real schema
- 🟡 **synthetic-only** — an integration test exists, but on a toy schema, not the CRM
- 🔴 **GAP** — investigated and found broken/silently dropping a real-schema declaration
- 🔬 **read-only** — code read/exists, but no integration test (treat as "unverified")
- ❌ **no test**

## Coverage map

| Capability | CRM can drive? | Status | Notes |
|---|---|---|---|
| Schema→codegen→migrations | yes | 🟡→partial | dogfood fixed enum/array; **`tableName` still ignored** |
| CRUD | yes | ✅ | `crm_dogfood` + `integration_test` |
| Validation rules | yes | ✅ | data-layer enforced on create + update (update-aware via `validate_partial`); `exists:` deferred to FKs |
| Relationships (belongsTo/hasMany) | yes | ✅ CRM | C1: `include` resolves contact.company + contact.deals (nested), proven in dogfood. Latent: convention-based, ignores the declared `relationships` block (works only when rel name == model name) |
| Soft delete / restore / hard delete | yes | ✅ | C2: full delete→trash→restore lifecycle via CRM Deals |
| Pagination + where/orderBy | yes | ✅ fixed | C2: orderBy+limit+offset via CRM; **fixed cache-key collision** (page 2 returned page 1) |
| Event sourcing + replay | yes | ✅ | C3: Deal Created→Updated→Updated→Deleted reconstructs via `entity_history` (`crm_deal_event_history_replays`); confirms B2 delete-event id fix |
| GraphQL resolvers | yes | 🟡 | `http_e2e`, synthetic |
| Subscriptions (WebSocket) | yes | ✅ auth | S2: `/graphql/ws` now auth'd via connection_init JWT + `model_changes` gated by read access (`test_subscription_requires_auth_role`). SDK `SubscriptionBuilder` filter args still dead code (separate, LOW) |
| RBAC enforcement | yes | ✅ GraphQL | S1: rules now parsed from export-const-schema; `check_access` via shared `decide()` seam. **Data-layer callers still bypass** (no role ctx) — follow-up |
| Audit logging | yes | ✅ | B4: model-agnostic listener works through CRM models (`test_crm_mutation_audited_with_actor`) — already worked, no fix |
| Workflows | yes | 🟡 partial | B1: YAML loads now; `Http` step really executes; trigger wiring tested. **CRM's `sales-pipeline.yml` still can't run** — its steps are inline JS (no execution model); `Mutation`/`Plugin` steps still no-op |
| WASM/JS plugins | yes | ✅ | `host_api`, `js_*`, `boot_wiring`, `example_plugin` |
| Caching (TTL + invalidation) | yes | ✅ | C4: populate + invalidate-on-create confirmed via CRM (dogfood 7b). LOW polish deferred (find_unique uncached, no eviction) |
| CQRS projections / aggregate | yes | 🟡 fixed | B2: Deleted removes rows (RETURNING id + per-id events); non-string columns stored via `value_to_text`. `projection_correctness` test. **Rebuild still truncate-no-replay** (deferred, operator action) |
| AI / pgvector | partial | ❌ | semantic search over notes; AI path not wired in a test |
| Multi-tenant (RLS) | yes | 🟡 core | S3: `tenant_id` column now generated → read+write scoping works (`test_two_tenant_isolation`); header honored only when authed. **Deferred**: subscription tenant-filter (leaks), per-user tenant binding, event-store/PG-RLS |
| OAuth/OIDC | no (needs mock IdP) | ❌ | supplementary harness |
| Rate limiting | infra | ✅ | `middleware.rs` |
| CLI (init/dev/migrate/codegen) | no (process-level) | ❌ | largest untested surface (`dev.rs`) |
| SDK offline queue/sync | no (client harness) | ❌ | types only |
| Admin UI | via E2E | 🟡 | Playwright (timeline, kanban) — may use demo fallback |

## Investigation findings (2026-05-31, parallel discovery + spot-verified)

Read-only investigation of the 5 biggest unknowns. **Every capability probed has at least one
HIGH-risk silent gap** — same pattern as the dogfood bugs: the platform parses/accepts a
schema declaration, then silently drops/skips/mismaps it. Two findings spot-verified by direct
read (RBAC parse gap, tenant_id column gap); the rest are subagent reports with file:line and
should be reconfirmed by the conformance test that targets them.

Ranked by risk × correctness/security impact:

| # | Capability | Worst gap (file:line) | Class | Risk |
|---|---|---|---|---|
| 1 | **RBAC** | access rules never parsed from `export const schema`; `Model.access` always `None`; `check_access` defaults allow-all (`graphql.rs:49-53`) | SECURITY | 🔴 HIGH |
| 2 | **Subscriptions auth** | `/graphql/ws` mounted with no auth middleware (`handlers.rs:253`) | SECURITY | 🔴 HIGH |
| 3 | **Multi-tenant** | no `tenant_id` column generated (`schema.rs:29-75`); reads/writes fail or leak; no RLS; no header→user check | SECURITY | 🔴 HIGH |
| 4 | **Workflows** | YAML never loaded (`lib.rs:148`), schema mismatch, steps are no-ops (`workflow.rs:230-260`) | CORRECTNESS (facade) | 🔴 HIGH |
| 5 | **Projections** | Deleted never removes rows; numeric→`""`; rebuild = truncate-no-replay | CORRECTNESS (data) | 🔴 HIGH |
| 6 | Cache | find_unique uncached, no eviction, Debug-format keys | PERF | 🟢 LOW |

**Three are SECURITY holes** (#1 RBAC bypass, #2 unauth WebSocket, #3 tenant bypass/leak) — any
authenticated (or for #2, unauthenticated) client can read/modify all data. These jump the queue.
**Two are CORRECTNESS holes** (#4 workflows are facade, #5 projections silently corrupt the read
model). The shared root cause of #1 (and the earlier validation bug) is the **same parser gap**:
only the `defineModel` DSL format is parsed for access/validation, not the `export const schema`
format the real CRM (and the docs' own examples) use.

## Phases

Each phase grows `crm_dogfood` (or sibling CRM-driven tests) and ends with the platform
demonstrably running its flagship for that capability. **These are bug-fix phases, not just
test-add phases** — the investigation proved the features don't work, so fixing what the test
targets is the bulk of the work.

### Phase A — Unblocker (must go first, solo)
- [x] A1. Honor explicit `tableName` (✅ done — `Model.table_name`, parsed via `parse_table_names`,
  used in `sql_builder::table_name` + migrations; falls back to pluralized name). `crm_dogfood`
  now creates `company`/`contact`/`deal` (not `companys`). Parser test `parses_explicit_table_name`.
  - **Migration-drift assessment**: the 7 hand-written CRM migrations are **stale artifacts of an
    older, buggier codegen** — they predate this session's fixes. With `tableName` honored the
    *table names* now match (`contact`/`company`/`deal`), but the hand-written SQL also has:
    `version INTEGER` + per-table indexes (platform generates neither), **no `deleted_at`** (so
    soft-delete would break on them), enum-as-table junk (`companysize` with `_enum_value_0..5`),
    and `UUID` ids on block tables vs `TEXT` on core. **Recommendation: retire the hand-written
    migrations in favor of platform-generated ones** rather than reconcile column-by-column — but
    that's a separate, riskier change to the CRM service's migration history (deferred, not done).

### Phase SEC — Security holes (jumped the queue; do right after A1)
- [x] S1. **RBAC** (✅ GraphQL path done): `parse_access_rules` extracts `access` from the
  export-const-schema format (the bug — only `defineModel` was parsed); attached in a sixth parse
  pass. New shared seam `AccessControl::decide(action, role) -> AccessDecision` (in atomo_schema)
  handles `public`/`authenticated`/pipe-OR; `graphql.rs::check_access` refactored onto it. Tests:
  `parses_and_enforces_access_rules` (unit) + `test_rbac_viewer_denied_create_admin_allowed`
  (e2e: viewer denied, admin allowed). **CAVEAT: data-layer `client.create/update/delete` does
  NOT yet enforce** — it has no role context (only `actor` user_id); the decide() seam is shared
  and ready, but plumbing role through the data-layer API is a follow-up. GraphQL is the external
  boundary, so the API-level bypass is closed; direct SDK/internal/plugin callers still bypass.
- [x] S2. **WebSocket auth** (✅ done): `/graphql/ws` now routes to an authenticated handler
  (`graphql_ws_handler`) that verifies a JWT from the `connection_init` payload
  (`{"authorization":"Bearer <jwt>"}` / bare `token`) and injects `UserRoleCtx`/`UserIdCtx` —
  rejects missing/invalid tokens. Second layer: `model_changes` resolver now takes `ctx` and gates
  by the model's `read` rule via `AccessControl::decide` (errors on Forbidden/NeedsAuth). Test:
  `test_subscription_requires_auth_role` (no role → rejected; role → stream stays open).
- [x] S3. **Multi-tenant — core done** (✅): `generate_migrations` now emits a nullable
  `tenant_id TEXT` column on every table, so the pre-existing `scope_by_tenant` (reads) +
  create-resolver injection (writes) finally work — they failed before because the column never
  existed. Nullable = backward-compatible for single-tenant (no TenantCtx → NULL, no scoping).
  `x-tenant-id` is now only honored for **authenticated** requests (was: anyone could claim any
  tenant). Test: `test_two_tenant_isolation` (A and B each see only their own rows).
  **Deferred (documented, not done):**
  - [ ] S3a. **Subscription tenant-filtering** — `model_changes` filters by model only; events
    still leak across tenants on the WS stream. Needs TenantCtx threaded into the subscription
    filter (the WS handler can inject it from a tenant in connection_init).
  - [ ] S3b. **Per-user tenant binding** — there is no `tenant_id` on users to validate the header
    against, so a user can still claim *any* tenant (just not anonymously). Real validation needs
    a user→tenant data model (users.tenant_id + JWT claim). Substantial; separate feature.
  - [ ] S3c. **Event-store + PG RLS** — events carry no tenant; no row-level-security policies
    generated (defense-in-depth beyond app-layer WHERE).

### Phase B — Correctness holes
- [~] B1. **Workflows — engine fixed, CRM yml deferred** (partial): YAML loading added
  (`load_workflows` now parses `.json`/`.yaml`/`.yml` into the `Workflow` struct via serde); the
  `Http` step now **actually performs the request** (was a no-op log) and records `http_status`.
  Tests: `deal_update_event_finds_workflow` (trigger wiring) + `http_step_actually_sends_request`
  (real HTTP to a local listener). **DEFERRED — the CRM's own `sales-pipeline.yml` still cannot
  run**: its steps are *inline JavaScript* (`await sendNotification(...)`, `throw new Error(...)`)
  with `type: validation|action|data_transformation` — a shape the engine has no execution model
  for. Making it run needs a JS step runtime; the Javy plugin system (Phase-2 scripting) is the
  natural foundation for that, but it's a large separate feature, not a B1 fix.
  - [ ] B1a. `Mutation`/`Plugin` step actions are still no-op logs (need client/plugin-manager wired into the engine).
  - [ ] B1b. A JS-step execution model so the CRM's literal `sales-pipeline.yml` runs.
- [~] B2. **Projections — corruption fixed, rebuild deferred**: (1) ✅ Deleted now removes the
  projection row — `soft_delete` gained `RETURNING id` and `delete_many` emits a Deleted event
  per affected id (was empty data → row never removed). (2) ✅ non-string columns stored correctly
  — projection binds via `value_to_text` (was `as_str().unwrap_or_default()` → numerics became `""`).
  Test: `projection_correctness` (numeric `value` stored as "50000"; delete removes the row).
  - [ ] B2a. **Rebuild still truncate-only (no replay)** — `TableProjection::rebuild` only
    `TRUNCATE`s; true replay needs the event store fed into the projection (signature only has
    `pool`). Operator action, not silent corruption — deferred.
- [x] B3. **Update-aware validation** (✅ done): `validate_partial` only checks rules for fields
  present in the patch, enforced in `update_many` after `before_update`. A stage-only update no
  longer trips `title: required`, but setting `title: ""` is still rejected. Tests: 3 unit
  (`partial_update_*`, `full_validate_still_requires_absent_field`) + dogfood partial-update
  assertion. `exists:<table>,<col>` stays a **documented no-op** — referential integrity is the
  DB's job (FK constraints); a sync validator can't query, and an async pass would duplicate the FK.
- [x] B4. **Audit-on-CRM-mutation** (✅ done — already worked): the boot audit listener is
  model-agnostic (subscribes to the event stream, records any `model_name` with the actor), so it
  handles CRM models correctly with no fix needed. `test_crm_mutation_audited_with_actor` proves a
  Contact create + update are both audited with op + actor `sales-7`. **First capability that was
  not silently broken** — only needed CRM-driven proof.

### Phase C — Data-pipeline polish (CRM-native, lower risk)
- [x] C1. **Relationship resolution** (✅ works for CRM): `resolve_includes` resolves both
  `contact.company` (belongsTo) and `contact.deals` (hasMany) as nested objects/arrays. Proven in
  `crm_dogfood` (step 5b). **Latent gap (documented, not fixed)**: resolution is *convention-based*
  — it infers the related model from the relationship name (`{rel}Id` → `capitalize(rel)`), NOT
  from the schema's declared `relationships` block (`{type, model, foreignKey}`). The CRM works
  only because its relationship names align with model names; a relationship whose name differs
  from its target model (e.g. `owner: { model: "User" }`) would resolve to the wrong/nonexistent
  model. Fixing = make `resolve_includes` read the `relationships` block (needs parsing it from
  the export-const-schema first — same parser-format family as access/validation). Deferred: no
  CRM-visible payoff.
- [x] C2. **Soft-delete/restore/pagination/orderBy via CRM** (✅ done — **found+fixed a real bug**):
  the dogfood now exercises orderBy(value DESC)+limit+offset and the full soft-delete→trash→restore
  lifecycle on Deals. **Bug caught**: the `find_many` cache key was `{where}{orderBy}` and omitted
  `limit`/`offset`, so two queries differing only in pagination collided — **page 2 returned page 1's
  rows**. A silent correctness hazard for every paginated view (Kanban, lists). Fixed: key now
  includes limit+offset. (Soft-delete/restore/orderBy themselves worked.)
- [x] C3. **Event sourcing + replay** (✅ done — works): `crm_deal_event_history_replays` drives a
  Deal Created → Updated → Updated → Deleted and reconstructs it exactly via
  `EventStore::entity_history`. No fix needed — but it **validates the B2 delete fix in a second
  context**: `entity_history` filters by `data->>'id'`, so before B2 (empty delete events) the
  Deleted event would have been invisible to history. `replay`/`entity_history` themselves worked.
  (Note: this is event *log/history*, distinct from projection *rebuild*-replay, still deferred B2a.)
- [x] C4. **Cache conformance** (✅ done): `find_many` populates the read cache and a create
  invalidates it — the next identical query returns fresh rows incl. the new Deal (dogfood step
  7b). No fix needed (the real cache bug was the C2 pagination-key collision, already fixed).
  Deferred LOW-risk polish: `find_unique` uncached, no background eviction, Debug-format keys.

### Phase D — Supplementary harnesses (what CRM can't reach alone)
- [ ] D1. Multi-tenant isolation harness (pairs with S3)
- [ ] D2. AI/pgvector: embed Contact notes, semantic search
- [ ] D3. OAuth: mock-IdP harness
- [ ] D4. CLI smoke test: `init` → `migrate` → `codegen` on the CRM schema in a temp dir

### Cross-cutting (do alongside, not after)
- [ ] CI: wire the DB-gated suite into the existing **manual `workflow_dispatch`** job (auto-triggers stay off for cost); make "run conformance" a one-click gate before any release tag
- [ ] Roadmap honesty: as each capability passes, update `roadmap.md` from "✅ implemented" to "✅ conformance-tested via CRM"

## Known gaps carried in (as of this plan)

- **SECURITY: RBAC fully bypassed** — access rules never parsed from `export const schema`; every model is allow-all (Phase S1). Verified.
- **SECURITY: WebSocket `/graphql/ws` unauthenticated** — anyone subscribes to all model changes (Phase S2). Reported.
- **SECURITY: multi-tenant non-functional + leaky** — no `tenant_id` column generated; subscriptions leak; header unvalidated (Phase S3). Verified (column).
- **Workflows are a facade** — CRM's `sales-pipeline.yml` never loads; steps are no-ops (Phase B1). Reported.
- **Projections silently corrupt** — deletes never remove rows; numeric fields → `""`; rebuild loses data (Phase B2). Reported.
- `tableName` ignored → `company` table becomes `companys`; drifts from hand-written migrations (Phase A1).
- Validation not enforced on **update** (partial updates would wrongly trip `required`) — needs update-aware validation (Phase B3).
- `exists:<table>,<col>` is a documented no-op in the sync validator (needs a pool; FKs cover integrity) (Phase B3).
- GraphQL keeps its own inline validation copy; data layer now also validates (harmless dup; consolidate eventually).

## Caveats / cost

- DB-gated tests are slow (~20s each with fuel-metered plugins); a full run is minutes. Keep it manual-dispatch, not per-push.
- **`http_e2e` tests share one `atomo_test` DB and FAIL under parallel execution** (they seed users / create tables and clobber each other) — run with `--test-threads=1`. Same shared-DB-singleton constraint that prevents parallel *implementation*. Worth fixing with per-test DBs/schemas eventually.
- Disk is finite (wasmtime builds + `.wasm` fixtures); watch `target/` size.
- This is a multi-week effort — correct *if* the goal is a trustworthy platform; the wrong call if the near-term goal is shipping features fast. That's a product decision.
- Findings are mostly subagent reports with file:line; RBAC + tenant_id were spot-verified by direct read. **Reconfirm each via its conformance test before trusting** — a couple may be partially inaccurate. Do not treat "implemented" as "working" until a test says so.
- The docs/roadmap currently claim several of these as "✅ implemented" / "✅ completed" — those claims are **misleading** and should be corrected as each is fixed+tested.
