# GraphQL API

Atomo services expose a generated GraphQL API. When running `atomo dev`, visit `/graphql` for the IDE. The server merges service queries with platform queries (users, sessions, audit).

The service API is model-generic: operations take a `model` argument and JSON `where`/`orderBy`/`data` payloads.

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
query { record(model: "Contact", id: "<uuid>") }
```

```graphql
# Create / update / delete
mutation { create(model: "Contact", data: { firstName: "John", email: "john@example.com" }) }
mutation { update(model: "Contact", where: { id: { equals: "<uuid>" } }, data: { phone: "555" }) }
mutation { delete(model: "Contact", where: { id: { equals: "<uuid>" } }) }
```

```graphql
# Subscribe to model changes (over WebSocket at /graphql/ws)
subscription { modelChanges(model: "Contact") { eventType modelName eventId } }
```

Notes
- `where` operators: `equals`, `not`, `contains`, `startsWith`, `endsWith`, `gt`, `gte`, `lt`, `lte`, `in`, `notIn`, `isNull`.
- Access is enforced per model from the schema `access` rules (RBAC). Send `Authorization: Bearer <jwt>`.
- Multi-tenant scoping: send `X-Tenant-ID: <id>` to scope all operations to a tenant.
- Errors carry codes in extensions: `NOT_FOUND`, `UNAUTHORIZED`, `FORBIDDEN`, `VALIDATION_ERROR`, `INTERNAL_ERROR`.

## Local Endpoints
- Dev server default: `http://localhost:3000/graphql`
- Subscriptions (WebSocket): `ws://localhost:3000/graphql/ws`
- Override the port with `atomo dev --port <n>` or `atomo-server --port <n>`

See also: the `schema.ts` in each service (e.g., `services/crm-service/schema.ts`).
