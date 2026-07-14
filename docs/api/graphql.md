# GraphQL API

Atomo services expose a generated GraphQL API. When running `atomo dev`, visit `/graphql` for the IDE. The server merges service queries with platform queries (users, sessions, audit).

The service API is model-generic: operations take a `model` argument and JSON `where`/`orderBy`/`data` payloads.

## Where operators

Per field, the `where` JSON accepts: `equals`/`eq`, `not`/`neq`, `contains`/`like`,
`startsWith`, `endsWith`, `gt`, `gte`, `lt`, `lte`, `in`, `notIn`, and `isNull`.
`isNull: true` matches `IS NULL`; `isNull: false` matches `IS NOT NULL`. Multiple
fields — and multiple operators on one field (`{ value: { gte: 1, lte: 9 } }`) —
combine with **AND**; there is no OR combinator.

## Example Operations

```graphql
# List records with Hasura-style filtering and ordering
query {
  records(
    model: "Contact"
    where: { email: { contains: "@example.com" } }
    orderBy: { createdAt: "DESC" }
    limit: 20
    offset: 0
  )
}
```

```graphql
# Paginated list with page metadata
query {
  paginatedRecords(model: "Contact", limit: 20, offset: 0) {
    data
    pageInfo { totalCount hasNextPage hasPreviousPage }
  }
}
```

```graphql
# Fetch one by id
query { record(model: "Contact", id: "<id>") }
```

```graphql
# Create / update / delete (update and delete honor the where filter)
mutation { create(model: "Contact", data: { firstName: "John", email: "john@example.com" }) }
mutation { update(model: "Contact", where: { id: { equals: "<id>" } }, data: { phone: "555" }) }
mutation { delete(model: "Contact", where: { id: { equals: "<id>" } }) }

# id shorthand — equivalent to where: { id: { equals: "<id>" } }
mutation { update(model: "Contact", id: "<id>", data: { phone: "555" }) }
mutation { delete(model: "Contact", id: "<id>") }
mutation { restore(model: "Contact", id: "<id>") }
mutation { hardDelete(model: "Contact", id: "<id>") }
```

```graphql
# Bulk update — update many records in one call; returns all updated records.
mutation {
  updateMany(model: "Contact", items: [
    { id: "<id1>", data: { phone: "555" } },
    { id: "<id2>", data: { phone: "666" } }
  ])
}
```

```graphql
# Soft-delete lifecycle: delete soft-deletes; restore brings back; hardDelete purges.
mutation { restore(model: "Contact", where: { id: { equals: "<id>" } }) }
mutation { hardDelete(model: "Contact", where: { id: { equals: "<id>" } }) }

# List soft-deleted records (the trash view), with pagination metadata.
query {
  deletedRecords(model: "Contact", limit: 20, offset: 0) {
    data
    pageInfo { totalCount hasNextPage hasPreviousPage }
  }
}
```

```graphql
# Paginated list with filtering, sorting, and total count.
query {
  paginatedRecords(
    model: "Contact"
    where: { email: { contains: "@example.com" } }
    orderBy: { createdAt: "DESC" }
    limit: 20
    offset: 0
  ) {
    data
    pageInfo { totalCount hasNextPage hasPreviousPage }
  }
}
```

```graphql
# Subscribe to model changes (over WebSocket at /graphql/ws)
subscription { modelChanges(model: "Contact") { eventType modelName eventId } }
```

Notes
- **Casing:** query results return camelCase keys (`firstName`, `createdAt`). Mutation inputs accept both camelCase and snake_case — the server normalizes to the schema's field names.
- `where` operators: `equals`, `not`, `contains`, `startsWith`, `endsWith`, `gt`, `gte`, `lt`, `lte`, `in`, `notIn`, `isNull`.
- **`id` shorthand:** `update`, `delete`, `restore`, and `hardDelete` accept an `id` argument as sugar for `where: { id: { equals: "..." } }`. An error is returned if both `id` and `where` are provided, or if neither is.
- `delete` is a soft delete (sets `deleted_at`); use `restore` to undo or `hardDelete` to purge. `records`/`paginatedRecords` exclude soft-deleted rows; `deletedRecords` shows only them.
- **`updateMany`:** accepts an `items` array of `{ id, data }` pairs and returns all updated records. Each item updates one record by id.
- Access is enforced per model from the schema `access` rules (RBAC). Send `Authorization: Bearer <jwt>`.
- Multi-tenant scoping: send `X-Tenant-ID: <id>` to scope all operations to a tenant.
- Mutations are audit-logged with the acting user (from the JWT) as `user_id`.
- Errors carry codes in extensions: `NOT_FOUND`, `UNAUTHORIZED`, `FORBIDDEN`, `VALIDATION_ERROR`, `INTERNAL_ERROR`.

## Local Endpoints
- Dev server default: `http://localhost:3000/graphql`
- Subscriptions (WebSocket): `ws://localhost:3000/graphql/ws`
- Override the port with `atomo dev --port <n>` or `atomo-server --port <n>`

See also: the `schema.ts` in each service (e.g., `services/crm-service/schema.ts`).
