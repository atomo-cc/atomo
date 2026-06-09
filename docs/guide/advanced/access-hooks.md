---
title: Access Control (RBAC)
description: Declare per-model access rules in your schema; Atomo parses and enforces them.
---

# Access Control (RBAC)

Atomo enforces **role-based access control** from rules you declare on each model in your
`export const schema` metadata. Rules are parsed at load time and checked on every GraphQL
operation.

## Declaring access rules

Each model takes an `access` block with one rule per operation (`create`/`read`/`update`/`delete`):

```ts
export const schema = {
  models: {
    Contact: {
      tableName: 'contact',
      access: {
        create: 'sales|manager|admin',
        read:   'authenticated',
        update: 'sales|manager|admin',
        delete: 'manager|admin',
      },
      // ... fields, validation, relationships
    },
  },
}
```

A rule value is a string. Three forms are supported:

| Rule value | Meaning |
|---|---|
| `'public'` | anyone, including unauthenticated requests |
| `'authenticated'` | any logged-in user (a valid JWT) |
| `'role'` or `'roleA\|roleB\|roleC'` | one of the listed roles (pipe = OR; case-insensitive) |

Roles come from the platform user model: `Admin | Manager | Sales | Support | Viewer`.
**If a model declares no `access` rule for an operation, that operation is allowed** (open by
default) — so declare rules explicitly for anything sensitive.

## How it's enforced

- Requests carry a JWT: `Authorization: Bearer <token>` (obtain one from `POST /auth/login`).
- The GraphQL resolver checks the rule before the operation. Denials return:
  - `UNAUTHORIZED` (401) when a rule needs a role/auth and the request is anonymous,
  - `FORBIDDEN` (403) when the user's role isn't permitted.
- Enforcement also runs in the **data layer** for create/update/delete (the GraphQL mutations
  route through `*_checked` methods), so the rule is applied through one shared decision path.

> **Note on direct/SDK callers:** the data layer also exposes *unchecked* `create`/`update_many`/
> `delete_many` for trusted system code (seeding, migrations, internal jobs). Those bypass RBAC by
> design — request handling always uses the checked path.

## Example

```bash
# 'viewer' creating a Contact (create requires sales|manager|admin) -> denied
curl -s -X POST http://localhost:3000/graphql \
  -H "authorization: Bearer $VIEWER_JWT" -H 'content-type: application/json' \
  -d '{"query":"mutation { create(model: \"Contact\", data: { firstName: \"x\" }) }"}'
# -> errors: [{ extensions: { code: "FORBIDDEN" } }]
```

## Lifecycle Actions

Side effects after mutations (calling external APIs, sending notifications, running pipelines) are
handled by **actions** — not inline hooks. Declare event bindings on a model:

```ts
events: {
  created: [{ action: 'processPost' }],
  updated: [{ action: 'onStatusChange', condition: { ChangedAny: ['status'] } }],
}
```

When the matching event commits, the **action dispatcher** enqueues a durable job. An external
**worker** (TypeScript, via `@atomo-cc/worker-sdk`) picks it up, runs your code, and reports
success or failure. The CRUD path stays pure Rust — no JS/RPC in the hot path.

Direct (non-lifecycle) actions are invoked via `POST /api/actions/:name`.

See [Jobs & Workers](/guide/advanced/jobs-and-workers) for the full worker model.

Field-level access and attribute/owner-based (ABAC) rules are not yet implemented; today's model
is operation-level role checks as above.

## See also
- [Validation Rules](/guide/advanced/validation) — declarative field validation
- [Multi-tenant](/guide/advanced/multi-tenant) — tenant scoping
- [Jobs & Workers](/guide/advanced/jobs-and-workers) — external workers and durable jobs
