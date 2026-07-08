# Atomo Quickstart

A minimal blog (Posts + Comments) that runs in 60 seconds. No Rust toolchain needed.

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and Docker Compose

## Run

```bash
docker compose up
```

Wait for `listening on 0.0.0.0:3000`, then:

- **Admin UI** — http://localhost:3000/admin (login: `admin@example.com` / `admin123`)
- **GraphQL IDE** — http://localhost:3000/graphql
- **Health check** — http://localhost:3000/health

## Try it

### Create a post

```graphql
mutation {
  create(model: "Post", data: {
    title: "Hello World",
    content: "My first post on Atomo.",
    status: "published"
  })
}
```

### Query posts

```graphql
query {
  records(model: "Post", where: { status: { equals: "published" } }, limit: 10)
}
```

### Add a comment

```graphql
mutation {
  create(model: "Comment", data: {
    body: "Welcome!",
    authorName: "Alice",
    postId: "<id from above>"
  })
}
```

### Seed sample data

```bash
bash seed.sh
```

## Edit the schema

Open `schema.ts`, add a field or model, and save. The server auto-reloads and
migrates — no restart needed.

## What you get

From a single `schema.ts`, Atomo generates:

- **GraphQL API** with CRUD, filtering, pagination, and sorting
- **Event sourcing** — every mutation is logged for replay and audit
- **Admin UI** — browse, create, edit, and delete records
- **RBAC** — access rules enforced per model from the schema
- **Real-time** — GraphQL subscriptions over WebSocket

## Next steps

- [Full documentation](https://atomo.cc)
- [GraphQL API reference](https://atomo.cc/api/graphql)
- [CRM demo service](../../services/crm-service/) — a more complete example with actions, workers, and multi-tenant scoping
