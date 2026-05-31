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
| Subscriptions (WebSocket) | yes | ❌ | no integration test |
| RBAC enforcement | yes | 🔬 | CRM declares `sales\|manager\|admin`; **likely a silent gap** |
| Audit logging | yes | 🟡 | synthetic only |
| Workflows | yes | 🟡 | CRM ships `sales-pipeline.yml`; its own flow untested |
| WASM/JS plugins | yes | ✅ | `host_api`, `js_*`, `boot_wiring`, `example_plugin` |
| Caching (TTL + invalidation) | yes | 🔬 | no direct test |
| CQRS projections / aggregate | yes | ❌ | no integration test |
| AI / pgvector | partial | ❌ | semantic search over notes; AI path not wired in a test |
| Multi-tenant (RLS) | no (needs 2-tenant) | ❌ | supplementary harness |
| OAuth/OIDC | no (needs mock IdP) | ❌ | supplementary harness |
| Rate limiting | infra | ✅ | `middleware.rs` |
| CLI (init/dev/migrate/codegen) | no (process-level) | ❌ | largest untested surface (`dev.rs`) |
| SDK offline queue/sync | no (client harness) | ❌ | types only |
| Admin UI | via E2E | 🟡 | Playwright (timeline, kanban) — may use demo fallback |

## Phases

Each phase grows `crm_dogfood` (or sibling CRM-driven tests) and ends with the platform
demonstrably running its flagship for that capability. **Expect some phases to be
bug-discovery sessions** — fixing what the test surfaces is part of the phase, not a clean
add-on (Phase B especially).

### Phase A — Finish the data pipeline (highest ROI, CRM-native)
- [ ] A1. Honor explicit `tableName`; reconcile/retire the 7 hand-written CRM migrations (kill the drift)
- [ ] A2. Relationship resolution: `include` company-on-contact, deals-on-contact (nested reads)
- [ ] A3. Soft-delete / restore / pagination / orderBy re-driven through CRM models
- [ ] A4. Event sourcing + replay over CRM mutations (rebuild a Deal's history)
- Exit: the entire data layer is proven on the flagship.

### Phase B — Enforcement & correctness (where silent gaps likely hide)
- [ ] B1. RBAC: prove a `viewer` is denied create, a `sales` is allowed (CRM access rules)
- [ ] B2. Update-aware validation + the `exists:` referential rule
- [ ] B3. Audit-on-CRM-mutation with the real actor
- Exit: declared security/validation rules are provably enforced, not parsed-and-dropped.

### Phase C — Reactive layer
- [ ] C1. Subscriptions: subscribe to Deal changes, mutate, assert delivery (Kanban real-time)
- [ ] C2. Workflows: load the CRM's own `sales-pipeline.yml`, trigger via a Deal stage change
- [ ] C3. CQRS projection + cache invalidation driven by CRM events
- Exit: the event-driven half of the platform is proven on the flagship.

### Phase D — Supplementary harnesses (what CRM can't reach alone)
- [ ] D1. Multi-tenant: two tenants, assert isolation
- [ ] D2. AI/pgvector: embed Contact notes, semantic search
- [ ] D3. OAuth: mock-IdP harness
- [ ] D4. CLI smoke test: `init` → `migrate` → `codegen` on the CRM schema in a temp dir

### Cross-cutting (do alongside, not after)
- [ ] CI: wire the DB-gated suite into the existing **manual `workflow_dispatch`** job (auto-triggers stay off for cost); make "run conformance" a one-click gate before any release tag
- [ ] Roadmap honesty: as each capability passes, update `roadmap.md` from "✅ implemented" to "✅ conformance-tested via CRM"

## Known gaps carried in (as of this plan)

- `tableName` ignored → `company` table becomes `companys`; drifts from hand-written migrations (Phase A1)
- Validation not enforced on **update** (partial updates would wrongly trip `required`) — needs update-aware validation (Phase B2)
- `exists:<table>,<col>` is a documented no-op in the sync validator (needs a pool; FKs cover integrity) (Phase B2)
- GraphQL keeps its own inline validation copy; data layer now also validates (harmless dup; consolidate eventually)

## Caveats / cost

- DB-gated tests are slow (~20s each with fuel-metered plugins); a full run is minutes. Keep it manual-dispatch, not per-push.
- Disk is finite (wasmtime builds + `.wasm` fixtures); watch `target/` size.
- This is a multi-week effort — correct *if* the goal is a trustworthy platform; the wrong call if the near-term goal is shipping features fast. That's a product decision.
- RBAC, subscriptions, workflows, projections are **read-only/unverified** — do not treat "implemented" as "working" until a conformance test says so.
