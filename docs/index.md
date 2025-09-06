---
layout: home

hero:
  name: "Atomo"
  text: "Next-Generation Content Core"
  tagline: "Build powerful, collaborative applications with event sourcing, real-time sync, and schema-driven development"
  image:
    src: /logo.svg
    alt: Atomo
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: View on GitHub
      link: https://github.com/atomo-org/atomo
    - theme: alt
      text: Try Playground
      link: https://playground.atomo.cc

features:
  - icon: ⚡
    title: Schema-Driven Development
    details: Define your data model in TypeScript, get a complete Rust backend with GraphQL APIs automatically generated.
  
  - icon: 🌊
    title: Event Sourcing
    details: Built on "事件的河流" - every change is an immutable event, giving you time travel, audit trails, and bulletproof reliability.
  
  - icon: 🤝
    title: Real-time Collaboration
    details: Figma-like collaborative editing with CRDTs, conflict-free merging, and seamless multi-user experiences.
  
  - icon: 📱
    title: Local-First Architecture
    details: Apps work offline, sync when online. Data sovereignty for users, performance for developers.
  
  - icon: 🧩
    title: WASM Plugin System
    details: Extend functionality with WebAssembly plugins. Safe, fast, and language-agnostic extensibility.
  
  - icon: 🚀
    title: Instant Development
    details: Hot reload, instant compilation, zero-config setup. From schema to running service in seconds.
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
```

## Why Atomo?

<div class="tip custom-block" style="padding-top: 8px">

**Traditional CMS** are passive content warehouses. **Atomo** is an active Content Core - the "Arc Reactor" that powers your entire business system.

</div>

### From Vision to Reality

Atomo transforms how you build applications by providing:

- **🏗️ Unified Architecture**: One platform for web, mobile, and desktop
- **⏱️ Time Travel**: Complete audit trails and point-in-time recovery
- **🔄 Real-time Everything**: Live collaboration, instant updates, reactive UIs
- **🛡️ Type Safety**: End-to-end type safety from database to UI
- **🌐 Offline-First**: Apps that work anywhere, anytime

## Get Started Today

```bash
# Install Atomo CLI
curl -fsSL https://install.atomo.cc | sh

# Create your first project
atomo init my-app --template crm
cd my-app

# Start developing
atomo dev
```

Your complete application stack is ready in under 30 seconds! 🚀

---

<div style="text-align: center; margin-top: 2rem;">
  <a href="/guide/getting-started" class="button">Start Building →</a>
</div>
