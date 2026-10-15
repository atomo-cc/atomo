---
title: Multi-tenant
description: Verified request identity, tenant isolation and explicit global models.
---

# Multi-tenant

The unreleased M1 security repair makes generated models tenant-required by default.
A verified user's `users.tenant_id` binds requests even without `X-Tenant-ID`.
A conflicting header returns 403. An unbound non-admin cannot select a tenant;
an unbound administrator may explicitly select one. This is single-tenant identity
binding, not relational membership or multi-tenant identity support.

## Schema policy

```ts
export const Record = model('records', {
  tenant: 'required', // default
  fields: {
    id: text().id(),
    tenantId: text().fromAuth('tenantId'),
    ownerId: text().fromAuth('userId'),
    title: text().required(),
  },
  access: {
    create: allow.authenticated(),
    read: allow.owner('ownerId'),
    update: allow.any([allow.owner('ownerId'), allow.role('admin')]),
  },
});
```

Use `tenant: 'global'` only for a model deliberately containing shared, null-tenant
records. The legacy metadata format accepts the same `tenant` option. A global
model admits only null-tenant rows; it does not expose tenant-owned rows to everyone.
Model access rules still apply. A tenant-required model rejects requests without
an authorized tenant and stamps the tenant on creates. Tenant changes are rejected.

`.fromAuth('userId')` and `.fromAuth('tenantId')` use the verified request context.
A conflicting supplied value is rejected; updating a source field is rejected,
even if the supplied value is unchanged. Unknown sources reject schema loading.
Authentication sources in action input declarations are rejected; action workers
receive the verified actor separately.

`owner`, `sameTenant`, `any` and `all` remain structured rules. Reads, point
lookups, counts, includes and pagination use SQL predicates. Updates check old
rows in the UPDATE predicate and check the resulting row before commit. A caller
cannot claim an existing row by changing its owner first. Unknown access syntax
fails schema loading; legacy arbitrary query expressions are rejected when loading a runtime schema.

## Relationships

Shared writes verify belongs-to parents inside the write transaction using the
parent's read policy and a shared row lock. Parents must be visible and have the
same tenant. An explicitly global parent is allowed only when its read policy
allows the reference. Creation, rebinding and shared bulk writes use this path;
a failed check rolls back the rows and their events. Tenant fields cannot change
through these request paths. Trusted Rust system APIs outside a principal scope
remain privileged; do not expose them as request handlers.

## Upgrade and existing null rows

Existing rows are not deleted, reassigned or backfilled. Null rows in a required
model become inaccessible through request CRUD. Inventory them using a separately
authorized maintenance role before upgrading. If an entire model is deliberately
global, explicitly declare it global. Mixed global/tenant rows in one model are
not supported by this initial policy. There is no automatic fallback to legacy
unscoped access. A header cannot substitute for membership authorization.

## PostgreSQL RLS

`ATOMO_ENABLE_RLS=true` enables database enforcement in addition to shared-layer
filtering. It remains opt-in; application filtering applies independently.
Server startup generates ENABLE/FORCE ROW LEVEL SECURITY and replaces each
model's `atomo_tenant_isolation` policy. Required models use:

```sql
USING (tenant_id IS NOT NULL AND tenant_id = current_setting('atomo.tenant_id', true))
WITH CHECK (tenant_id IS NOT NULL AND tenant_id = current_setting('atomo.tenant_id', true))
```

Global models use `tenant_id IS NULL` for both clauses. Setup is idempotent;
policy failures abort startup. The legacy low-level `policy_statements_for` helper
retains its permissive policy for explicit library callers; the server uses
`ensure_schema_rls` and the schema policies above.

Tenant binding uses `SET LOCAL` in the same transaction as the protected SQL,
so it resets on commit/rollback and works with transaction pooling. Read cache
keys include the actual policy predicates and tenant. WebSocket connection init
uses the same tenant resolver; model events are also checked against row access
and tenant before delivery. Events without sufficient row fields are withheld.

`FORCE` constrains the table owner, but superusers/BYPASSRLS bypass policies.
Startup therefore rejects such roles when RLS is enabled. Use a non-superuser,
non-BYPASSRLS application role that can own/migrate its schema.
`ATOMO_RLS_ALLOW_BYPASS_ROLE=true` explicitly overrides the boot refusal and logs
an error; it does not make PostgreSQL enforce policies for privileged roles.

All model policies are installed in **one transaction**, in a stable table order.
Any installation error (including a missing table, missing `tenant_id`, or insufficient
ownership privileges) **refuses startup** and rolls back the entire batch, restoring
previous policies and RLS flags. Earlier versions only warned and could continue with
incomplete protection. Repair schema/ownership errors before restarting an RLS-enabled
deployment; they can no longer be ignored.

### Connect with a non-privileged role (enforced)

`FORCE` covers the table owner, but Postgres **superusers** and roles with **BYPASSRLS**
skip policies unconditionally — RLS would be silently inert for that connection. So when
`ATOMO_ENABLE_RLS` is on, the server inspects `pg_roles` at boot and **refuses to start**
if the connected role is `rolsuper` or `rolbypassrls`. Connect as a non-superuser,
non-BYPASSRLS role that owns the managed tables (or has the required ownership-role
membership). PostgreSQL requires table ownership to manage policies; ordinary CRUD
grants alone do not suffice ([PostgreSQL row security documentation](https://www.postgresql.org/docs/16/ddl-rowsecurity.html)).
For a new, dedicated database, an administrator can provision its application owner:

```sql
CREATE ROLE atomo_app LOGIN PASSWORD '…';
CREATE DATABASE atomo_service OWNER atomo_app;
-- Connect the server as atomo_app so migrations create tables under that owner.
```

If the environment genuinely cannot offer a non-privileged role (some managed PGs), set
`ATOMO_RLS_ALLOW_BYPASS_ROLE=true` to proceed anyway — the refusal is downgraded to a
loud `ERROR` log at every boot.
This flag only bypasses the connection-role check; it never suppresses policy
installation errors. For an existing database, check ownership of each managed
table before enabling RLS; granting database access does not transfer table ownership.

## Remaining scope

Relational membership, a user belonging to several tenants, event-store tenant
APIs, synchronous atomic commands and old-data migration need separate work.
The server does not infer membership from an arbitrary model or request header.

See [Access control](/guide/advanced/access-hooks), [Workers](/guide/workers),
and [GraphQL](/api/graphql).

Access properties in both builder and legacy schema objects are read structurally: bare, single-quoted and double-quoted keys have the same meaning, including whitespace and comments around separators. Duplicate properties (including escaped aliases), dynamic access values, spreads, computed keys, shorthand and methods reject schema loading. Schema/model containers must be inline objects; ambiguous declarations never select the first or last rule. Comments, strings and nested options cannot supply a model's access policy. Missing access retains its existing default semantics; an explicit unsupported declaration never becomes missing access.
