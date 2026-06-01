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

## Limits & roadmap

- **Postgres Row-Level Security (RLS)** is **not yet implemented** — scoping is enforced in the
  application layer (the `WHERE tenant_id = ...` clauses + write stamping). RLS as a
  defense-in-depth layer (generated `CREATE POLICY` + per-transaction session var) is a planned
  follow-up; doing it safely under a shared connection pool is a deliberate design step.
- **Event-store tenant scoping** (per-event tenant metadata) is also planned.

So: app-layer isolation works and is tested; do not rely on DB-enforced RLS yet.

## See also
- [Access Control (RBAC)](/guide/advanced/access-hooks)
- [Security & Auth](/guide/advanced/security)
