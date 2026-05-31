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
- **React hooks** - Optimized for React applications
- **Caching** - Intelligent query caching and invalidation
- **Offline support** - Local-first data management
- **Real-time** - WebSocket-based live updates

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

### Common Operations

```typescript
// Create a new contact
const contact = await atomo.contacts.create({
  firstName: "John",
  lastName: "Doe", 
  email: "john@example.com"
})

// Query with relationships
const contacts = await atomo.contacts.findMany({
  include: {
    company: true,
    deals: true
  },
  where: {
    email: {
      contains: "@example.com"
    }
  }
})

// Real-time subscription
const subscription = atomo.contacts.subscribe({
  where: { companyId: "company-123" },
  onUpdate: (contact) => {
    console.log("Contact updated:", contact)
  }
})
```

### CLI Quick Commands

```bash
# Start development server
atomo dev

# Generate client code
atomo codegen --output ./src/generated

# Run database migrations
atomo migrate

# Build for production
atomo build

# Deploy to Atomo Cloud
atomo deploy --env production
```

## Authentication

All API access requires authentication. Atomo supports multiple authentication methods:

### Development Mode
In development, authentication is optional for localhost requests:

```typescript
const client = new AtomoClient({
  endpoint: 'http://localhost:3000/graphql'
  // No auth token needed in development
})
```

### Production Mode
Production deployments require API tokens:

```typescript
const client = new AtomoClient({
  endpoint: 'https://your-app.atomo.cc/graphql',
  authToken: process.env.ATOMO_API_TOKEN
})
```

### User Authentication
For user-facing applications, use session-based auth:

```typescript
// Login user
const session = await atomo.auth.login({
  email: 'user@example.com',
  password: 'secure-password'
})

// Use session token
const client = new AtomoClient({
  endpoint: 'https://your-app.atomo.cc/graphql',
  authToken: session.token
})
```

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

Atomo APIs are versioned to ensure backward compatibility:

- **Current version**: `v1`
- **Endpoint format**: `https://api.atomo.cc/v1/graphql`
- **Deprecation policy**: 6 months notice for breaking changes

## Support

Need help with the APIs?

- 📖 **[Guides](/guide/)** - Step-by-step tutorials
- 💬 **[Discord](https://discord.gg/atomo)** - Community support
- 🐛 **[GitHub Issues](https://github.com/Chris533/atomo/issues)** - Bug reports
- 📧 **[Email](mailto:api-support@atomo.cc)** - Direct API support