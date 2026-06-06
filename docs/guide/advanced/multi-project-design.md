---
title: Multi-Project Platform (Design)
description: A control plane that runs many isolated Atomo projects on shared infrastructure — silo-per-project databases, pool-per-tenant inside each project.
---

# Multi-Project Platform — Design

> **Status:** Design proposal. Nothing here ships in the core server yet. The current
> `atomo-server` is **single-project by construction** (one schema, one database, one port);
> this document specifies how to run *many* isolated projects on shared infrastructure
> **without rearchitecting that core**.

## Summary

Run N independent Atomo applications ("projects") on one set of infrastructure, each fully
isolated, provisioned and routed by a thin **control plane** that sits *in front of* unmodified
`atomo-server` instances.

The design rests on one principle established by the SaaS field: **the right isolation strategy
is decided by count scale.**

- **Projects** (distinct apps, distinct schemas) number in the *handful-to-dozens*. → **Silo:
  one database per project.**
- **Tenants** (customers inside one app, one schema) can number in the *thousands-plus*. → **Pool:
  shared schema + `tenant_id` + Row-Level Security inside each project.**

This is AWS's [Bridge model](https://docs.aws.amazon.com/wellarchitected/latest/saas-lens/silo-pool-and-bridge-models.html)
applied across two axes, and it matches how Supabase (dedicated database per project) and Frappe
(database per site) actually build.

### Goals

- Provision a new, fully-isolated Atomo project in one command / one API call.
- Strong isolation between projects: separate databases, separate processes, separate crash domains.
- Reuse the existing, shipped `atomo-server` binary **unchanged** as the per-project unit.
- Cost-efficient: many project databases on one Postgres instance; many instances on one host —
  promote a hot project to dedicated hardware by editing registry config, not by rearchitecting.
- A clear, independent path to the second axis (multi-tenant-within-a-project via RLS).

### Non-goals

- This is **not** a public, sell-it-to-other-developers PaaS (no untrusted-tenant DDL, no
  customer-supplied code sandboxing beyond the existing WASM plugin model). It targets an operator
  running *their own* portfolio of projects on shared infra.
- It does **not** replace per-project distribution. A better engine lowers build cost; it does not
  acquire users. Evaluate this work on build-velocity and ownership, not revenue.

## Why the core doesn't need rebuilding

`atomo-server` is already a perfectly-isolated project unit, configured entirely by environment:

| Concern | Today | Implication |
| --- | --- | --- |
| Schema | one `ATOMO_SCHEMA_PATH` loaded into one `Atomo` (`server.rs`) | one project = one instance |
| Database | one `DATABASE_URL`, one `PgPool` powering **every** service (auth, audit, registry, media, projector, workflows) | nothing to namespace — point it at a dedicated DB |
| Migrations | `enable_migrations(true)` → auto-migrate on boot | provisioning is just "start it" |
| Platform tables | `ensure_platform_tables` creates `users`/`sessions`/`audit_log` per pool on boot | per-project identity is free |
| Event store | one `events`/`snapshots` table **per database** | **no `project_id` surgery** — separate DB ⇒ separate event log |
| Schema reload | `spawn_schema_watcher` exits on change → orchestrator restarts → re-migrates | per-project edit-and-live already works |
| Listen address | one `HOST`/`PORT` | one upstream per project |

Because every per-project concern is already isolated to a pool and a process, the multi-project
layer is **purely additive**: a registry, a provisioner, and a gateway. The shipped core stays
untouched — which is the main reason this design is low-risk.

## Architecture options considered

### Option A — Supervisor + Gateway (chosen)

A control plane provisions and supervises **one `atomo-server` per project** (each its own DB,
schema, listen address). A gateway routes inbound requests to the right instance.

- **Pros:** ~zero core changes; strongest isolation (process + DB + crash domain); per-project
  schema hot-reload already works; cost model = many DBs on one Postgres, many instances on one
  host; no risk to the shipped server.
- **Cons:** N OS processes (each a lightweight Rust binary); needs a supervisor + a gateway.

### Option B — In-process multi-runtime (rejected)

Refactor `AtomoServer` to hold `HashMap<ProjectId, ProjectRuntime>` and dispatch per host inside
one process.

- **Pros:** one process; denser RAM; shared edge layers.
- **Cons:** large, risky refactor — `run()` consumes `self` and builds one GraphQL schema + three
  long-lived background tasks bound to a single event stream; would need N task-sets, dynamic
  per-host axum dispatch, and a reimplementation of schema reload; one panic becomes a shared
  blast radius. High risk to a stable core for a density gain that doesn't matter at dozens of
  projects.

**Decision: Option A.** Revisit B only if process density becomes a real constraint (hundreds of
projects on one host) — and even then, prefer more hosts first.

## Architecture overview

```
                          ┌──────────────────────────────────────────┐
   project-a.example  ───▶│                 GATEWAY                    │
   project-b.example  ───▶│  resolve hostname / X-Atomo-Project header │
   X-Atomo-Project: c ───▶│  TLS termination · shared edge policies    │
                          └────────┬────────────┬────────────┬────────┘
                                   ▼            ▼            ▼
                          atomo-server  atomo-server  atomo-server     ◀── unmodified binary,
                           (project A)   (project B)   (project C)         one per project
                                   │            │            │
                                   ▼            ▼            ▼
                              db: proj_a    db: proj_b    db: proj_c   ◀── separate DATABASES
                          └──────────── one Postgres instance ───────────┘  (split out when hot)

        ┌───────────────────────────────────────────────────────────────┐
        │ CONTROL PLANE                                                    │
        │  • registry (projects table — own DB)                            │
        │  • provisioner (create DB · place schema · start · health)       │
        │  • reconciler (registry == running instances)                    │
        │  • gateway config generator                                      │
        └───────────────────────────────────────────────────────────────┘
```

Three trust planes, deliberately separate:

1. **Control plane** — operator-only. Owns the registry, provisioning, and gateway config. Its
   own database; never shares a DB with a project.
2. **Gateway** — public ingress. Stateless; routing table derived from the registry.
3. **Project instances** — each its own world (DB, identity, event log, plugins).

## Components

### 1. Project registry

The single source of truth, in the control plane's own database (never a project DB):

```sql
CREATE TABLE projects (
  id            TEXT PRIMARY KEY,          -- stable slug, e.g. "acme"
  display_name  TEXT NOT NULL,
  hostname      TEXT UNIQUE,               -- primary routing key: acme.example.com
  aliases       TEXT[] NOT NULL DEFAULT '{}',
  database_url  TEXT NOT NULL,             -- SSM ref to the dedicated DB URL (same PG instance ok)
  schema_ref    JSONB NOT NULL,            -- { type:"git", repo, path, ref:<commit-sha> } — see Schema source of truth
  schema_version TEXT,                     -- deployed commit SHA (drift detection)
  upstream      TEXT,                      -- host:port or unix socket the instance listens on
  env           JSONB NOT NULL DEFAULT '{}',-- per-project overrides (feature flags, secrets refs)
  status        TEXT NOT NULL,             -- see lifecycle state machine
  desired_state TEXT NOT NULL DEFAULT 'running', -- running | stopped (reconciler target)
  last_health   JSONB,                     -- last probe result + timestamp
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE project_events (   -- audit of control-plane actions (who created/stopped what)
  id          BIGSERIAL PRIMARY KEY,
  project_id  TEXT NOT NULL REFERENCES projects(id),
  action      TEXT NOT NULL,              -- create | start | stop | schema_update | delete
  actor       TEXT,
  detail      JSONB,
  at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Secrets (the project's `DATABASE_URL` password, `JWT_SECRET`) are **referenced**, not stored in
plaintext — see [Secrets](#secrets-management). `env` holds non-secret overrides; secret values
live in the secret store and are injected at instance start.

### 2. Provisioner (lifecycle)

Every operation is **idempotent** and recorded in `project_events`. Lifecycle state machine:

```
   create
  ┌────────┐  create_db ok   ┌────────────┐  schema applied  ┌──────────┐
  │ absent │ ───────────────▶│ provisioning│ ───────────────▶│  running │
  └────────┘                 └─────┬───────┘                 └────┬─────┘
                                   │ failure                      │ stop
                                   ▼                              ▼
                               ┌────────┐                    ┌─────────┐
                               │ failed │                    │ stopped │
                               └────────┘                    └─────────┘
                                                                  │ delete (guarded)
                                                                  ▼  (optional DROP DATABASE)
                                                              ┌────────┐
                                                              │ absent │
                                                              └────────┘
```

Operations:

- **create** — `CREATE DATABASE <db>` on the target Postgres → materialize `schema.ts` from
  `schema_ref` → start an instance with the project's env. Atomo then **auto-migrates** and
  **auto-ensures platform tables** on boot. No core change.
- **start / stop / restart** — manage the instance via the chosen driver (below).
- **schema_update** — bump `schema_ref.ref` to a new commit SHA → check out that file to the
  volume; with `ATOMO_SCHEMA_WATCH` on, the instance exits and the driver restarts it,
  re-migrating. Set `schema_version` to the new SHA.
- **delete** — stop instance; **`DROP DATABASE` is guarded** (requires explicit confirm flag and a
  fresh backup; never the default).

**Driver abstraction.** The provisioner targets a `Driver` interface so the same control plane
works across environments:

| Driver | Use | Start mechanism |
| --- | --- | --- |
| `docker` / `compose` | single host (default) | render a service from the `ghcr.io/atomo-cc/atomo-server` image + per-project env |
| `nomad` / `k8s` | multi-host scale-out | a Job/Deployment per project |
| `process` | bare metal / dev | spawn the binary with env, supervise PID |

MVP ships `docker`. The interface keeps `nomad`/`k8s` as later additions, not rewrites.

### 3. Gateway (ingress + routing)

Stateless. Routing table is generated from the registry (`hostname` + `aliases` → `upstream`).

- **Resolution precedence:** exact `hostname` → `aliases` → `X-Atomo-Project` header → 404.
  The header path supports non-DNS clients and local/dev; hostname is canonical in production.
- **TLS:** terminated at the gateway; automatic certificates (per-hostname) via the gateway's ACME
  integration.
- **Edge policies (optional, shared):** rate limiting, IP allow/deny, request-ID propagation,
  body-size limits. Per-project auth stays *inside* each instance (the gateway does not validate
  JWTs — it only routes).

Two implementations, same registry-driven config:

- **Caddy / Traefik** (recommended) — TLS + host routing for free; the control plane writes a
  config file / talks the provider API when the registry changes.
- **Small axum reverse-proxy** — only if you want first-party shared edge middleware (atomo's
  `rate_limit.rs` could be lifted to the edge). More code; defer unless needed.

### 4. The per-project instance

Runs the **current binary, unmodified**. Configured by the env atomo already reads
(`DATABASE_URL`, `ATOMO_SCHEMA_PATH`, `HOST`/`PORT`, `JWT_SECRET`, `ATOMO_ENV`, CORS, feature
flags). One small, optional core addition is worthwhile for operability:

- **`ATOMO_PROJECT_ID`** — a label stamped into every log line / trace span so cross-project
  observability can filter by project. One-line change in the tracing setup; backward-compatible
  (absent ⇒ unchanged behavior).

Nothing else in the core is required for multi-project.

## The second axis — multi-tenant *within* a project (RLS)

Silo (DB-per-project) is one axis. The other is **many customers inside one project**, where the
field's answer at scale is **pool: shared schema + `tenant_id` + Row-Level Security**. Atomo
already has the app-layer half:

- `users.tenant_id` column (`ensure_platform_tables`), `X-Tenant-ID` header, per-user tenant
  binding and mismatch rejection (see [Multi-tenant](/guide/advanced/multi-tenant)).

The remaining work is **DB-enforced isolation** as defense-in-depth:

1. Generate `ALTER TABLE … ENABLE ROW LEVEL SECURITY` + `CREATE POLICY` per model table, keyed on
   a session variable.
2. Set the variable per transaction (`SET LOCAL atomo.tenant_id = …`) from the authenticated
   request's tenant.
3. Make it pooling-safe: `SET LOCAL` is transaction-scoped, so it survives PgBouncer transaction
   mode; verify with a transaction-pooled connection in tests.
4. Optional: scope the event store per tenant (per-event tenant metadata + filtered reads).

This axis is **independent** of the control plane and can ship on its own timeline. A project that
serves a single tenant (or only trusted internal tenants) runs fine on the app-layer scoping that
exists today; turn on RLS when a project onboards untrusted multi-tenant customers.

## Per-project autonomy & developer velocity

A common worry about platform-ization is that it sacrifices per-project flexibility and slows the
dev loop. The silo architecture is chosen specifically so it does **neither**.

### Each project stays a complete, autonomous app

The control plane provisions and routes; it has **no say over a project's internals**. Every
project keeps full freedom to define:

- its own `schema.ts` (entirely different data models per project),
- its own WASM/JS **plugins** and custom `/ext/<plugin>` **routes**,
- its own **workflows**, auth configuration, feature flags, CORS — even its own atomo version.

A bespoke feature in one project is invisible to the others. There is no shared schema or shared
data plane to constrain what a project may become.

### The control plane is an *ops* layer, not a *dev-time* gate

This is the key property: **local development is unchanged.** A developer runs a *single*
`atomo-server` against one schema, edits `schema.ts`, and gets ~2s edit-and-live via the schema
watcher and auto-migrate. None of the registry / provisioner / gateway machinery exists at dev
time — it only describes the deployed fleet. You never pay control-plane complexity while iterating
on a project.

### Maintenance: shared once, bespoke isolated

| Concern | Where it lives | Maintenance cost |
| --- | --- | --- |
| Core capabilities (auth, audit, GraphQL, events, plugins runtime) | the `atomo-server` binary | maintained **once**; every project inherits improvements on next restart |
| Project-specific logic | that project's schema / plugins / workflows | **isolated** — touched only when that project needs it; cannot break another project |
| A feature wanted across *all* projects | either the core (build once, all inherit) **or** replicated per project | a normal platform decision; Phase 5 "shared plugins/templates" makes the build-once path easy |

Net: silo makes per-project features **cheaper** to maintain (isolated blast radius), and keeps the
fast schema-driven loop fully intact.

## Footprint & positioning vs alternatives

Resource footprint is not a side detail for a multi-project platform — it *is* a core advantage,
and it compounds with project count. Running many isolated projects on shared infrastructure
favors a lean per-project unit.

| | **Atomo** | **PayloadCMS** | **Supabase (self-host)** | **Appwrite (self-host)** |
| --- | --- | --- | --- | --- |
| Per-project runtime | **1 static Rust binary (~tens of MB)** + Postgres | Node.js + `node_modules` (hundreds of MB–~1 GB) per app | ~8–10 containers (auth, REST, realtime, storage, gateway, studio…), multi-GB | multi-container stack (DB, cache, proxy, executors…), multi-GB |
| RAM per project | Low (lightweight process) | Higher (a Node runtime each) | the whole stack (shared) | the whole stack (shared) |
| Dev loop | edit `schema.ts` → ~2s reload, auto-migrate, GraphQL + admin | define collections in TS → instant admin + APIs (very mature DX) | SQL/Studio → instant REST/GraphQL + auth | console/SDKs → instant DB/auth/storage |
| Batteries out of the box | auth, RBAC, audit, realtime, media, workflows, **event-sourcing**, **WASM plugins** | rich (mature field types, hooks, access control, large ecosystem) | auth, **RLS**, storage, edge functions, pgvector | auth, DB, storage, functions, **native multi-project** |
| Multi-project model | **silo via this design** (planned) | run N heavy instances | project = dedicated DB/instance | **native, many projects per instance** |
| Honest gaps | billing, **RLS (in progress)**, smaller ecosystem | heavy footprint; not natively multi-project | heavy self-host; vendor pull if hosted | heavy footprint |

**Where this design wins:** N isolated projects = N lightweight processes on one host, each with a
dedicated database, at a fraction of the disk/RAM of N Node apps or N heavy container stacks. The
footprint edge is *why* a lean, owned core scales to a portfolio of projects more economically than
batteries-rich alternatives.

**Where it loses (stated honestly):** breadth and maturity — Payload's field/admin ecosystem,
Supabase's already-shipped RLS and feature surface, and Appwrite's native multi-project are real
present-day advantages. The trade is deliberate: a small, fast, owned, event-sourced core in
exchange for ecosystem breadth. It pays off when those properties (footprint, ownership,
event-sourcing, WASM extensibility) matter; for plain generic CRUD with no such needs, a mature
batteries-included tool still wins on day one.

## Cross-cutting concerns

### Identity & auth

- **Per-project identity is the default** (zero work): each instance has its own `users`,
  `sessions`, and `JWT_SECRET`. A user in project A is unrelated to project B. This matches silo
  isolation and is correct for distinct apps.
- **Control-plane auth is separate** from project auth — operators authenticate to the control
  plane; that credential never grants access to project data.
- **Shared identity / SSO across projects** is a *future* extension (see below), built only if a
  real need appears (one human needing one login across projects). Do not build speculatively.

### Secrets management

- **Store: AWS SSM Parameter Store** (from day one). Per-project `DATABASE_URL` (with password),
  `JWT_SECRET`, and git credentials are stored as `SecureString` parameters under a per-project
  path (e.g. `/atomo/<project-id>/...`). The registry holds the SSM parameter **reference**, never
  the plaintext value.
- Values are **resolved and injected as env at instance start** by the provisioner — the running
  `atomo-server` reads ordinary env (`DATABASE_URL`, `JWT_SECRET`), unchanged.
- `JWT_SECRET` is **required in production** (the core already `bail!`s without it). The provisioner
  generates a strong per-project secret on create and writes it to SSM.
- Rotation: update the SSM parameter, restart the instance. (Rotating `JWT_SECRET` invalidates that
  project's live sessions — acceptable, documented.) Use SSM parameter versions for auditability.
- IAM scopes the control plane's read access by parameter path; project instances never get broad
  secret-store access — only their own injected env.

### Observability

- **Logs/traces:** every instance emits structured logs (`LOG_FORMAT=json`) already; add
  `ATOMO_PROJECT_ID` so a central collector can filter per project. Request IDs already propagate.
- **Metrics:** scrape per-instance health; the control plane aggregates into a fleet view (status,
  request rate, error rate, DB connections per project).
- **Health probes:** the reconciler probes each instance and writes `last_health`; the gateway can
  fail a route over to a maintenance page when an instance is down.

### Backups & disaster recovery

- **Per-database backups** are a silo advantage: `pg_dump`/PITR per project DB, independent restore,
  no cross-project entanglement. The provisioner schedules per-project backups; restore targets a
  single project without touching others.
- Control-plane DB is backed up separately (it's small but is the source of truth).

### Schema evolution & migrations

- Each project migrates independently on boot. There is **no fleet-wide migration** to coordinate —
  a silo benefit (the shared-schema pain of "migrate every tenant in lockstep" does not apply
  across projects).
- `schema_version` (digest) in the registry enables **drift detection**: the reconciler can flag a
  project whose running schema differs from `schema_ref`.
- Roll-forward is the model (atomo migrates forward on boot). Destructive migrations should be
  gated behind a backup, same as `delete`.

### Resource isolation & scaling

- **Noisy-neighbor:** processes give CPU/memory isolation; separate DBs give I/O isolation at the
  logical level. For hard guarantees, the driver can set per-container resource limits.
- **Connection budget:** N project DBs on one Postgres share `max_connections`. At dozens of
  projects this is comfortable; size each project's pool modestly and front Postgres with PgBouncer
  if the sum grows. (This is *not* the database-per-*tenant* connection explosion — projects are
  few by definition.)
- **Scale-out path:** promote a hot project by (a) pointing its `database_url` at a dedicated
  Postgres instance and (b) moving its app instance to its own host — both are registry edits +
  a data migration, not a rearchitecture.

### Networking

- **Ports vs unix sockets:** ports are simplest with Docker (default); sockets avoid port
  bookkeeping on a single host. The `upstream` field abstracts either.
- Project instances bind to a private network; only the gateway is publicly exposed.

### Data residency / compliance

- Silo makes residency tractable: a project requiring a region/jurisdiction gets a `database_url`
  (and instance) in that region. The registry records it; the gateway routes accordingly.

## Phased delivery plan (full)

> Each phase is independently useful and shippable. Phases 1–3 deliver the platform; 4–5 are
> hardening and optional extensions.

### Phase 0 — Foundations (prep, no user-facing change)
- Add `ATOMO_PROJECT_ID` label to tracing in the core (one-line, backward-compatible).
- Define the `Driver` trait and the registry schema (`projects`, `project_events`).
- Decide secret store (env-injection for MVP; pluggable interface).
- **Deliverable:** schema + interfaces merged; core still single-project, no behavior change.

### Phase 1 — Provisioner CLI + manual routing (the MVP)
- `atomo project create | start | stop | list | delete` in `atomo_cli`, backed by the registry and
  the `docker` driver: create DB, place schema, start instance, auto-migrate.
- Routing via a generated Caddy/Traefik config (hostname → upstream).
- Guarded `delete` (confirm + backup).
- **Deliverable:** "spin up an isolated Atomo project in one command," reachable over TLS. **Zero
  core changes beyond Phase 0.**

### Phase 2 — Control-plane API + reconciler
- HTTP API over the registry (CRUD projects, trigger lifecycle actions).
- **Reconciler loop:** make running instances match `desired_state`; restart crashed instances;
  reconcile on control-plane boot; write `last_health`.
- Gateway config regenerated automatically on registry change.
- **Deliverable:** declarative fleet — edit the registry, the platform converges.

### Phase 3 — Multi-tenant-within-project (RLS)
- Generated `CREATE POLICY` + `SET LOCAL` per transaction; pooling-safe; tested under transaction
  pooling.
- Optional event-store tenant scoping.
- **Deliverable:** a single project can safely serve many untrusted tenants (the second axis), on
  demand, per project.

### Phase 4 — Operability hardening
- Per-project scheduled backups + one-command restore.
- Fleet observability dashboard (status, rates, DB connections, schema drift).
- Resource limits per instance; health-based gateway failover to a maintenance page.
- Secret rotation flow.
- **Deliverable:** production-grade operations for the fleet.

### Phase 5 — Optional extensions (build only on real need)
- **Multi-host drivers** (`nomad` / `k8s`) for horizontal scale-out.
- **Shared identity plane** (SSO across projects) — only if a human needs one login across projects.
- **Self-serve provisioning** (if this ever becomes a product surface for others) — would require
  revisiting the "not a public PaaS" non-goal and the untrusted-tenant threat model.
- **Per-project billing/metering hooks** — usage export per project DB.

## Sizing & risk

| Work | Size | Risk | Notes |
| --- | --- | --- | --- |
| Phase 0 (label + interfaces + schema) | S | Low | Additive; no behavior change |
| Phase 1 (provisioner CLI + Caddy routing) | M | Low | Reuses the shipped image; the bulk of "wow" value |
| Phase 2 (API + reconciler) | M | Low–Med | Standard control-loop engineering |
| Phase 3 (RLS) | M | Med | The one piece touching SQL generation; do it carefully under pooling |
| Phase 4 (backups/obs/limits) | M | Low | Operational, well-trodden |
| Phase 5 (multi-host / SSO / self-serve) | L | Varies | Only on demonstrated need |

The genuinely hard parts are **operational** (provisioning orchestration, TLS/routing, backups),
not atomo-core — by design. The single highest-care item is **Phase 3 RLS under connection
pooling**; everything else is additive plumbing around an unchanged server.

## Decisions (confirmed for v1)

1. **Driver:** **Docker/Compose on one host.** (`nomad`/`k8s` remain later additions behind the
   `Driver` interface; not built for v1.)
2. **Gateway:** **Caddy** — registry-driven config; automatic per-hostname TLS.
3. **Secret store:** **AWS SSM Parameter Store from day one**, injected as env at instance start.
   The registry holds SSM parameter *references*, never plaintext. (No env-only interim stage.)
4. **Schema source of truth:** **Git, pinned to a commit SHA, materialized to a volume file** for
   the runtime. A schema change *is* a migration, so it needs review / history / rollback /
   reproducibility — which git gives natively and a bare volume or object store do not. See
   [Schema source of truth](#schema-source-of-truth-detail) for the `schema_ref` shape and flow.
5. **Identity:** **Per-project identity** (each instance owns its `users`/`sessions`/`JWT_SECRET`).
   The shared SSO plane is **deferred** until a real cross-project-login need appears.

### Schema source of truth (detail)

Two layers are involved and should not be conflated:

- **Source of truth** — where the canonical schema lives and how it is changed: **Git**.
- **Runtime materialization** — what the instance reads: a **file** at `ATOMO_SCHEMA_PATH` (atomo's
  watcher keys on file mtime). The provisioner writes this file from the pinned git ref.

`schema_ref` shape — **pin to a commit SHA, never a branch** (immutable, reproducible deploys):

```
schema_ref = {
  type: "git",
  repo: "<schemas repo>",            // one schemas monorepo for the portfolio
  path: "projects/<id>/schema.ts",   // a path per project
  ref:  "<commit-sha>"               // pinned; NOT a branch name
}
```

- `schema_version` in the registry **= the deployed commit SHA**; drift = running SHA ≠ desired SHA.
- **No build step** — atomo parses `schema.ts` directly, so materializing is fetching one file at
  the pinned SHA onto the volume.
- **Production flow:** bump `ref` in the registry → provisioner checks out the SHA → writes the
  file to the volume → the existing schema watcher exits the instance → the driver restarts it →
  auto-migrate. (Reuses machinery already in the core.)
- **Local dev:** the volume file *is* the loop — edit `schema.ts`, ~2s hot reload, no git in the
  path. Volume-only is the **dev** mode, not the production source of truth.
- **Object store (S3):** optional artifact *cache* for fast cold starts (mirror the resolved file);
  **never** the source of truth.
- Git credentials live in **AWS SSM** alongside the other secrets.

## A standing caveat (from the portfolio thesis)

This control plane is real engineering whose payoff is **build velocity + ownership**, a cost-side
win — it does **not** acquire users or solve distribution, which remains the binding constraint.
Build it when the project count and Atomo's specific edge (event-sourcing / audit / WASM plugins)
justify owning the stack over renting an off-the-shelf backend per app. The architecture does not
expire; phase it in when the count makes it pay.

## See also
- [Multi-tenant](/guide/advanced/multi-tenant) — the within-project tenant axis (app-layer today)
- [Custom Routes — Phase 3 design](/guide/advanced/custom-routes-phase3-design)
- AWS SaaS Lens — [Silo, Pool, and Bridge Models](https://docs.aws.amazon.com/wellarchitected/latest/saas-lens/silo-pool-and-bridge-models.html)
- [Supabase architecture](https://supabase.com/docs/guides/getting-started/architecture) — dedicated database per project
