---
layout: home

hero:
  name: "Atomo"
  text: "Next-Generation Content Core"
  tagline: "Build schema-driven backends with event sourcing, GraphQL, RBAC, and plugins - generated from TypeScript"
  image:
    src: /logo.svg
    alt: Atomo
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: View on GitHub
      link: https://github.com/Chris533/atomo

features:
  - icon: ⚡
    title: Schema-Driven Development
    details: Define your data model in TypeScript, get a Rust backend with GraphQL APIs, migrations, and an admin UI generated from it.
  
  - icon: 🌊
    title: Event Sourcing
    details: Built on "事件的河流" - every change is an immutable event, with event-log persistence, replay, and audit trails.
  
  - icon: 🔐
    title: Auth, RBAC & Multi-tenant
    details: JWT auth, role-based access enforced from your schema's access rules, and tenant-scoped reads/writes.
  
  - icon: 🧩
    title: WASM + JS Plugins
    details: Extend with WebAssembly or drop-in JavaScript plugins (Javy) - permission-gated, sandboxed, with CRUD lifecycle hooks.

  - icon: 📊
    title: CQRS Projections & Caching
    details: Event-driven read models and a TTL read cache with automatic invalidation.

  - icon: 🤝
    title: Planned - Collaboration & Local-First
    details: Real-time CRDT collaboration and offline-first sync are on the roadmap, not yet shipped.
---

## Quick Example

Create a complete CRM system in minutes:

```typescript
// schema.ts - Define your data model
export interface Contact {
  id: string
  firstName: string
  lastName: string
  email: string
  company?: Company
  deals: Deal[]
  notes: ContentBlock[]
}

export interface Company {
  id: string
  name: string
  website?: string
  contacts: Contact[]
}
```

```bash
# Generate and run your service
atomo dev
# ✨ Complete GraphQL API, Admin UI, and TypeScript SDK generated automatically

# From the monorepo
pnpm dev:admin
pnpm --filter @atomo-cc/client-sdk dev
pnpm --filter atomo-crm-service generate
```

## Why Atomo?

<div class="tip custom-block" style="padding-top: 8px">

**Traditional CMS** are passive content warehouses. **Atomo** is an active Content Core - the "Arc Reactor" that powers your entire business system.

</div>

### From Vision to Reality

Atomo transforms how you build applications by providing:

- **🏗️ Schema-driven backend**: TypeScript schema → Rust + GraphQL + migrations + admin UI
- **⏱️ Event history**: event-log persistence, replay, and complete audit trails
- **🛡️ Type Safety**: end-to-end types from schema to generated SDK
- **🔐 Security from the schema**: auth, RBAC, and multi-tenant scoping enforced from access rules
- **🤝 Planned**: real-time collaboration and offline-first sync — see the [roadmap](/roadmap)

## Get Started

Atomo is pre-1.0 and runs from source today (no hosted installer yet):

```bash
git clone https://github.com/Chris533/atomo.git
cd atomo
cargo build --release        # build the workspace (atomo-cli, atomo-server, ...)
```

Then boot a service against Postgres — see the [Getting Started guide](/guide/getting-started)
for the verified end-to-end run (env vars, login, GraphQL).


git clone https://github.com/Chris533/atomo.git
cd atomo
pnpm install
pnpm dev:admin
```

Use `pnpm --filter "./packages/*" test` as the current frontend/SDK baseline while the CRM generation loop matures.

---

<div style="text-align: center; margin-top: 2rem;">
  <a href="/guide/getting-started" class="button">Start Building →</a>
</div>
