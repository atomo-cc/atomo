# Getting Started

Welcome to Atomo. This guide shows the current monorepo MVP loop: Admin UI, TypeScript SDK, and the CRM demo service.

## What is Atomo?

Atomo is a **Content Core** - not just a CMS, but the "Arc Reactor" that powers your entire application. Think of it as:

- 🏗️ **Event-sourced backend** generated from TypeScript schemas (CRUD, GraphQL, audit, projections)
- 🔐 **Auth + RBAC + multi-tenant scoping** enforced from your schema's access rules
- 🧩 **WASM + JavaScript plugin system** with permission-gated capabilities
- 🤝 *Planned:* real-time collaboration (CRDT) and local-first offline sync — see the [roadmap](/roadmap)

## Prerequisites

- **Node.js** 18+ and **pnpm** 8+
- **Rust** 1.70+ only if you are working on the CLI/server crates
- **PostgreSQL** 14+ when running persistence-backed service flows

## Installation

> Atomo is pre-1.0 and installed from source today (there is no hosted installer or published
> binary yet).

```bash
# Clone and build the CLI + server from source
git clone https://github.com/Chris533/atomo.git
cd atomo
cargo build --release            # builds the workspace (atomo-cli, atomo-server, ...)
cargo install --path crates/atomo_cli   # optional: put `atomo` on your PATH
```

## Your First Project

Let's build a simple CRM system to demonstrate Atomo's capabilities:

### 1. Initialize Project

```bash
# Create new project with CRM template
atomo init my-crm --template crm
cd my-crm

# Project structure created:
# ├── schema.ts          # Your data model
# ├── atomo.config.ts    # Configuration
# ├── migrations/        # Database migrations
# └── plugins/           # Custom plugins
```



### Manual Installation - Frontend
```bash
git clone https://github.com/Chris533/atomo.git
cd atomo
pnpm install
```

## Current MVP Loop

Use the checked-in CRM demo rather than `atomo init` templates.

```bash
# Terminal 1: Admin UI
pnpm dev:admin

# Terminal 2: SDK watch/build loop
pnpm --filter @atomo/client-sdk dev

# Regenerate CRM demo artifacts after schema changes
pnpm --filter atomo-crm-service generate
```

Keep the frontend baseline green with `pnpm --filter "./packages/*" test`, which type-checks the Admin UI and SDK packages.

## Explore the CRM Schema

Open `services/crm-service/schema.ts` to see the current demo model:

```typescript
// services/crm-service/schema.ts - This drives everything!
export interface Contact {
  id: string
  firstName: string
  lastName: string
  email: string
  phone?: string
  companyId?: string
  tags: string[]
  notes: ContentBlock[]  // Rich content support
  createdAt: Date
  updatedAt: Date
}

export interface Company {
  id: string
  name: string
  website?: string
  industry?: string
  contacts: Contact[]    // Automatic relationships
  deals: Deal[]
}


// Atomo automatically generates:
// ✅ Database tables and migrations
// ✅ GraphQL schema and resolvers  
// ✅ Admin UI forms and views
// ✅ TypeScript client SDK
// ✅ Real-time subscriptions
```

### 3. Run the server (verified path)

The most reliable way to run a service today is the **standalone server** booted against the
service's `schema.ts` and a Postgres database. Set the environment and run the `atomo-server`
binary:

```bash
# from the repo root (after `cargo build --release`)
export DATABASE_URL="postgresql://user:pass@localhost/atomo_dev"
export ATOMO_SCHEMA_PATH="services/crm-service/schema.ts"
export JWT_SECRET="change-me"
export ADMIN_EMAIL="admin@example.com"   # seeds an admin on first boot (idempotent)
export ADMIN_PASSWORD="change-me-too"
export PORT=3000

./target/release/atomo-server
```

On boot it parses the schema, runs migrations (tables + FK constraints), ensures platform
tables, seeds the admin, and starts the audit / CQRS projector / workflow listeners. See
[Configuration](/guide/configuration) for the full env-var list.

### 4. Verify it works

```bash
# Health
curl http://localhost:3000/health            # -> OK

# Log in (returns a JWT)
TOKEN=$(curl -s -X POST http://localhost:3000/auth/login \
  -H 'content-type: application/json' \
  -d '{"email":"admin@example.com","password":"change-me-too"}' | jq -r .token)

# Create a Contact via GraphQL
curl -s -X POST http://localhost:3000/graphql \
  -H 'content-type: application/json' -H "authorization: Bearer $TOKEN" \
  -d '{"query":"mutation { create(model: \"Contact\", data: { firstName: \"Ada\", lastName: \"Lovelace\", email: \"ada@example.com\" }) }"}'

# List Contacts
curl -s -X POST http://localhost:3000/graphql \
  -H 'content-type: application/json' -H "authorization: Bearer $TOKEN" \
  -d '{"query":"{ records(model: \"Contact\") }"}'
```

A create persists the record and propagates to the event log, audit log, and read-model
projection — the full event-sourced path runs end-to-end.

::: tip Admin UI
The admin UI discovers models from the server's `/meta/schema` endpoint, so it renders your real
models against the standalone `atomo-server` (point it at the server's host/port; in local dev on
:5173 it targets `http://localhost:3000`). If the server is unreachable it falls back to built-in
demo data. The CLI dev/workspace flow (`pnpm dev:admin`) also works and adds schema hot-reload.
:::


// The MVP loop uses this schema to drive generated CRM artifacts,
// SDK types, and Admin UI integration work.
```

## Make a Schema Change

Edit `schema.ts` to add a new field:

```typescript
export interface Contact {
  id: string
  firstName: string
  lastName: string
  email: string
  phone?: string
  companyId?: string
  tags: string[]
  
  // Add this new field:
  linkedInUrl?: string
  
  notes: ContentBlock[]
  createdAt: Date
  updatedAt: Date
}
```

Under the CLI dev runtime (`atomo dev` / workspace dev — see [Dev Runtime](/guide/dev-runtime)),
saving `schema.ts` triggers:
1. 🔄 Detect the schema change
2. 🗄️ Generate a database migration
3. 🔨 Recompile the Rust backend
4. 🎨 Update the admin UI forms

With the **standalone `atomo-server`**, restart the process to pick up schema changes (it
re-parses `schema.ts` and applies new migrations on boot).

## What Just Happened?

Atomo's "instant compilation" workflow:

1. **Schema Analysis**: Parses your TypeScript interfaces
2. **Code Generation**: Creates specialized Rust structs, GraphQL resolvers, and database models
3. **Compilation**: Uses Cargo's incremental compilation for speed
4. **Hot Reload**: Automatically restarts services and updates UI

This gives you the **performance of Rust** with the **productivity of TypeScript**.

Then run:

```bash
pnpm --filter atomo-crm-service generate
pnpm --filter @atomo/client-sdk build
```

Use the Admin UI dev server to inspect the UI impact while the CRM schema and SDK types converge.

## Next Steps

Now that you have a running application, explore these features:

- 📝 **[Custom Content Types](/guide/tutorials/content-types)** - Rich, block-based content
- 🤝 **[Real-time Collaboration](/guide/collaboration)** - Multi-user editing
- 🧩 **[Plugin Development](/guide/tutorials/plugin-dev)** - Extend with WASM
- 🚀 **[Deployment](/guide/tutorials/deployment)** - Go to production

## Need Help?

- 📖 **[Core Concepts](/guide/event-sourcing)** - Understand Atomo's architecture
- ⚙️ **[Configuration](/guide/configuration)** - Full environment-variable reference
- 🐛 **[GitHub Issues](https://github.com/Chris533/atomo/issues)** - Report bugs or request features
- 💬 **[GitHub Discussions](https://github.com/Chris533/atomo/discussions)** - Questions and discussion

Welcome to the future of application development! 🚀
