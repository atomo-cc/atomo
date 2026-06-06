---
title: Multi-tenant
description: Tenant-scoped reads, writes, and subscriptions via a generated tenant_id column.
---

# Multi-tenant

Atomo scopes data per tenant at the **application layer** today: every generated table gets a
nullable `tenant_id` column, and requests carrying a tenant are scoped to it.

## How it works

- **Column:** migrations add `tenant_id TEXT` to every model table (nullable — single-tenant
  deployments simply leave it `NULL` and nothing is scoped, so it's fully backward compatible).
- **Request scoping:** send the tenant on each request:

  ```
  X-Tenant-ID: <tenant-id>
  ```

  When present (and the request is authenticated), reads filter by `tenant_id`, writes stamp it,
  and subscriptions only deliver that tenant's events.
- **Per-user binding:** a user row may have a `tenant_id`. If it does, the `X-Tenant-ID` header
  **must match the user's tenant** — a user bound to tenant A cannot act as tenant B (a mismatched
  header is dropped). Users with no binding may pass any tenant (single-tenant / admin use).

## Example

```bash
# Tenant A only ever sees tenant A's contacts
curl -s -X POST http://localhost:3000/graphql \
  -H "authorization: Bearer $JWT" -H "x-tenant-id: tenant-a" \
  -H 'content-type: application/json' \
  -d '{"query":"{ records(model: \"Contact\") }"}'
```

Tenant A creating a record and Tenant B listing will not see each other's rows (verified by the
`test_two_tenant_isolation` integration test).

## Opt-in DB-enforced RLS (defense-in-depth)

Postgres **Row-Level Security** is available as an **opt-in, additive** layer on top of the
app-layer scoping. It is **off by default** — when disabled, behavior is byte-for-byte unchanged.

### Enable it

```bash
ATOMO_ENABLE_RLS=true   # accepts true / 1; default false
```

On boot (after tables exist) the server runs, for every model table:

```sql
ALTER TABLE <t> ENABLE ROW LEVEL SECURITY;
ALTER TABLE <t> FORCE  ROW LEVEL SECURITY;   -- also constrains the table-owning role
DROP POLICY IF EXISTS atomo_tenant_isolation ON <t>;
CREATE POLICY atomo_tenant_isolation ON <t>
  USING      (tenant_id IS NULL OR tenant_id = current_setting('atomo.tenant_id', true))
  WITH CHECK (tenant_id IS NULL OR tenant_id = current_setting('atomo.tenant_id', true));
```

Setup is **idempotent** (safe on every boot). The policy is **permissive**: rows with
`tenant_id IS NULL` stay visible/writable, so single-tenant deployments are unaffected.

### What it enforces

A forgotten `WHERE tenant_id = …` cannot leak across tenants: Postgres filters every row against
the `atomo.tenant_id` session variable set for the request, so cross-tenant reads/writes are
rejected by the database itself — not just the application.

### Pooling note (important)

The session variable is set with `SET LOCAL atomo.tenant_id = …` (via `set_config(…, true)`), which
is **transaction-scoped** — it resets at COMMIT/ROLLBACK. This makes it **safe under PgBouncer
transaction pooling**: the binding can never leak onto another tenant's request reusing the same
physical connection. The bind and the queries it protects **must run in the same transaction**.

Helper: `atomo_server::rls::bind_tenant(&mut tx, tenant_id)`.

### Per-transaction wiring status — **wired**

Per-request enforcement is now active end to end:

- **Request boundary:** `graphql_handler` wraps execution in
  `atomo::graphql::with_tenant_scope(tenant, schema.execute(req))` — the tenant is the same
  header value that gates `TenantCtx`, so app-layer scoping and RLS binding can never disagree.
- **Executor:** when `ATOMO_ENABLE_RLS` is on and a scope is active, `AtomoClient`'s read/write
  methods run each statement inside `pool.begin()` + `SET LOCAL atomo.tenant_id` on the same
  connection, then commit. The read cache is **tenant-keyed** so a cached read can't cross tenants.
- **Off / unscoped callers** (SDK, projectors, seeding) run directly on the pool — unchanged.

Proven against Postgres by `crates/atomo_server/tests/rls_enforcement.rs` (policy + primitive) and
`crates/atomo/tests/rls_executor.rs` (`find_many` under scope, incl. the "forgot the WHERE" case and
the tenant-keyed cache).

**Not yet scoped:** the GraphQL **subscription/WS** path, and event-store per-tenant scoping (below).

## Limits & roadmap

- **Event-store tenant scoping** (per-event tenant metadata) is planned.

## See also
- [Access Control (RBAC)](/guide/advanced/access-hooks)
- [Security & Auth](/guide/advanced/security)
