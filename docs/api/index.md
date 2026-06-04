# API Reference

Welcome to the Atomo API documentation. This section provides comprehensive reference material for all Atomo APIs and interfaces.

## Overview

Atomo provides multiple API layers to suit different development needs:

### 🖥️ CLI Commands
The `atomo` command-line interface is your primary development tool:
- **Project management** - Initialize, build, and deploy projects
- **Development workflow** - Hot reload, code generation, and testing
- **Database operations** - Migrations, seeding, and schema management

[→ CLI Reference](/api/cli)

### 🌐 GraphQL API
Auto-generated GraphQL APIs from your TypeScript schemas:
- **Type-safe queries** - Fully typed operations
- **Real-time subscriptions** - Live data updates
- **Automatic CRUD** - Generated operations for all models
- **Custom resolvers** - Business logic hooks

[→ GraphQL Schema](/api/graphql)

### 📦 TypeScript SDK
Type-safe client library for frontend applications:
- **React hooks** - scaffolding for React applications
- **Caching** - query caching and invalidation
- **Offline queue** - *experimental, not yet integration-tested*

[→ TypeScript SDK](/api/typescript-sdk)

### 🦀 Rust APIs
Core platform APIs for advanced customization:
- **Event sourcing** - Event store and stream management
- **Authentication** - User management and authorization
- **Plugin system** - WASM runtime and plugin interfaces
- **Content management** - Rich content and block APIs

[→ Rust APIs](/api/rust)

### 🧩 Plugin APIs
WebAssembly plugin development interfaces:
- **Event handlers** - React to domain events
- **Content processors** - Transform and enrich content
- **External integrations** - Connect to third-party services
- **Custom UI components** - Extend the admin interface

[→ Plugin APIs](/api/plugins)

## Quick Reference

The service API is **model-generic**: operations take a `model` argument plus JSON
`data`/`where`/`orderBy` payloads (there is no per-model typed client today). Send these as
GraphQL operations to `/graphql`.

```graphql
# Create a contact
mutation { create(model: "Contact", data: { firstName: "John", lastName: "Doe", email: "john@example.com" }) }

# Query with a filter (records/paginatedRecords exclude soft-deleted rows)
query {
  records(model: "Contact", where: { email: { contains: "@example.com" } }, orderBy: { createdAt: "DESC" })
}

# Subscribe to model changes (WebSocket at /graphql/ws)
subscription { modelChanges(model: "Contact") { eventType modelName eventId } }
```

See [GraphQL API](/api/graphql) for the full operation set, `where` operators, and the
soft-delete lifecycle.

For the **ephemeral** real-time tier — presence, live fan-out, "who's-typing" —
connect a WebSocket to `/realtime/ws?token=<jwt>` and exchange JSON frames
(`{"op":"subscribe","channel":"deal:42"}`, `{"op":"publish",…}`). This traffic is
never event-sourced; see [Realtime Channels & Presence](/guide/advanced/realtime-channels-proposal)
for the protocol and boundary. Durable data changes stay on GraphQL Subscriptions above.

### CLI Quick Commands

```bash
atomo dev                              # dev runtime (codegen + server + hot reload)
atomo migrate                          # apply migrations
atomo migrate --generate --name <name> # generate a migration from schema changes
atomo build                            # build
```

## Authentication

The GraphQL API is protected: obtain a JWT via `POST /auth/login`, then send it as
`Authorization: Bearer <token>` on each request. RBAC is enforced per model from the schema's
`access` rules.

```bash
# Log in to get a token
curl -s -X POST http://localhost:3000/auth/login \
  -H 'content-type: application/json' \
  -d '{"email":"admin@example.com","password":"..."}'   # -> { "token": "<jwt>" }

# Use it on GraphQL requests
curl -s -X POST http://localhost:3000/graphql \
  -H "authorization: Bearer <jwt>" -H 'content-type: application/json' \
  -d '{"query":"{ records(model: \"Contact\") }"}'
```

An admin user is seeded on first boot from `ADMIN_EMAIL`/`ADMIN_PASSWORD` (see
[Configuration](/guide/configuration)). For multi-tenant scoping, also send `X-Tenant-ID: <id>`.

## Error Handling

Atomo APIs use consistent error formats:

```typescript
try {
  const contact = await atomo.contacts.create(data)
} catch (error) {
  if (error.code === 'VALIDATION_ERROR') {
    // Handle validation errors
    console.log('Validation errors:', error.details)
  } else if (error.code === 'PERMISSION_DENIED') {
    // Handle authorization errors
    console.log('Access denied:', error.message)
  } else {
    // Handle other errors
    console.log('Unexpected error:', error)
  }
}
```

## Rate Limiting

The server applies a per-IP token-bucket rate limiter (in-memory). It is configurable via environment variables:

- `RATE_LIMIT_RPS` — max requests per window (default `100`)
- `RATE_LIMIT_WINDOW_SECS` — window length in seconds (default `60`)

The client IP is taken from the `X-Forwarded-For` header (first hop) when present. Requests over the limit receive `429 Too Many Requests`.

## Versioning

Atomo is pre-1.0; APIs may change between releases. There is no hosted API service yet — you run
`atomo-server` yourself (see [Getting Started](/guide/getting-started)).

## Support

- 📖 **[Guides](/guide/)** - Step-by-step tutorials
- 🐛 **[GitHub Issues](https://github.com/atomo-cc/atomo/issues)** - Bug reports
- 💬 **[GitHub Discussions](https://github.com/atomo-cc/atomo/discussions)** - Questions and discussion