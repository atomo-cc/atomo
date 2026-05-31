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
| Validation rules | yes | ✅ | now data-layer enforced; `exists`/update-aware missing |
| Relationships (belongsTo/hasMany) | yes | 🟡 | dogfood queries deals-by-contact; `include`/nested untested |
| Soft delete / restore / hard delete | yes | 🟡 | synthetic only |
| Pagination + where/orderBy | yes | 🟡 | synthetic (Note) only |
| Event sourcing + replay | yes | 🟡 | synthetic only |
| GraphQL resolvers | yes | 🟡 | `http_e2e`, synthetic |
| Subscriptions (WebSocket) | yes | 🔴 GAP | works + filters by model, but `/graphql/ws` has **NO auth** (`handlers.rs:253`) → full RBAC bypass; SDK `SubscriptionBuilder` filter args are dead code |
| RBAC enforcement | yes | 🔴 GAP | **access rules never parsed** from `export const schema` (only `defineModel` DSL); `Model.access` always `None` → `check_access` defaults to allow-all (`graphql.rs:49-53`). Verified. Complete bypass. |
| Audit logging | yes | 🟡 | synthetic only |
| Workflows | yes | 🔴 GAP | CRM's `sales-pipeline.yml` is **inert**: loader is JSON-only (`lib.rs:148`, no `serde_yaml`), struct schema mismatch, and Http/Mutation/Plugin steps are **no-ops** (`workflow.rs:230-260`). 3 stacked silent failures. |
| WASM/JS plugins | yes | ✅ | `host_api`, `js_*`, `boot_wiring`, `example_plugin` |
| Caching (TTL + invalidation) | yes | 🟢 | works (find_many cached, invalidated on writes); minor: `find_unique` uncached, no eviction, Debug-format keys — all LOW |
| CQRS projections / aggregate | yes | 🔴 GAP | Deleted events never remove rows (empty event data, `id` lookup None); numeric fields stored as `""` (`as_str()` on number → None); rebuild truncates with no replay |
| AI / pgvector | partial | ❌ | semantic search over notes; AI path not wired in a test |
| Multi-tenant (RLS) | no (needs 2-tenant) | 🔴 GAP | **no `tenant_id` column ever generated** (`schema.rs:29-75`) → tenant-scoped insert/select fail at SQL; subscriptions leak cross-tenant; no header→user validation; no PG RLS. False security. |
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
- [ ] S1. **RBAC**: parse `access` from the `export const schema` format (the missing `parse_access_rules`, mirroring `parse_validation_rules`); enforce in BOTH the GraphQL resolver and the data-layer `client.create/update/delete` (not just GraphQL). Handle `public`/`authenticated` tokens. Test: viewer denied create, sales allowed, delete gated to manager|admin.
- [ ] S2. **WebSocket auth**: require auth on `/graphql/ws`; inject `UserRoleCtx`/`TenantCtx` into the subscription context; gate `model_changes` by read access. Test: unauth subscribe rejected.
- [ ] S3. **Multi-tenant**: auto-generate a `tenant_id` column; scope reads AND writes; filter subscriptions by tenant; validate the `x-tenant-id` header against the authenticated user. (Largest; may split.) Test: 2 tenants, assert isolation incl. subscriptions.

### Phase B — Correctness holes
- [ ] B1. **Workflows**: add a YAML loader (`serde_yaml`) + a deserialization shim from the CRM's YAML shape to the `Workflow` struct; implement the no-op Http/Mutation/Plugin step actions. Test: load `sales-pipeline.yml`, Deal stage change triggers it, step actually runs.
- [ ] B2. **Projections**: fix Deleted-row removal (carry `id` in delete events), correct non-string column types, make rebuild replay from the event store. Test: create/delete Deal → projection matches; numeric `value` preserved.
- [ ] B3. Update-aware validation + the `exists:` referential rule.
- [ ] B4. Audit-on-CRM-mutation with the real actor.

### Phase C — Data-pipeline polish (CRM-native, lower risk)
- [ ] C1. Relationship resolution: `include` company-on-contact, deals-on-contact (nested reads)
- [ ] C2. Soft-delete / restore / pagination / orderBy re-driven through CRM models
- [ ] C3. Event sourcing + replay over CRM mutations (rebuild a Deal's history)
- [ ] C4. Cache conformance (find_many cached/invalidated) + the LOW-risk cache polish

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
- Disk is finite (wasmtime builds + `.wasm` fixtures); watch `target/` size.
- This is a multi-week effort — correct *if* the goal is a trustworthy platform; the wrong call if the near-term goal is shipping features fast. That's a product decision.
- Findings are mostly subagent reports with file:line; RBAC + tenant_id were spot-verified by direct read. **Reconfirm each via its conformance test before trusting** — a couple may be partially inaccurate. Do not treat "implemented" as "working" until a test says so.
- The docs/roadmap currently claim several of these as "✅ implemented" / "✅ completed" — those claims are **misleading** and should be corrected as each is fixed+tested.
