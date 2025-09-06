# Getting Started

Welcome to Atomo! This guide will help you create your first application in under 5 minutes.

## What is Atomo?

Atomo is a **Content Core** - not just a CMS, but the "Arc Reactor" that powers your entire application. Think of it as:

- 🏗️ **Event-sourced backend** that generates from TypeScript schemas
- 🤝 **Real-time collaboration** with conflict-free merging
- 📱 **Local-first architecture** for offline-capable apps
- 🧩 **WASM plugin system** for unlimited extensibility

## Prerequisites

- **Node.js** 18+ and **pnpm** 8+
- **Rust** 1.70+ (for CLI compilation)
- **PostgreSQL** 14+ (for persistence)

## Installation

### Quick Install (Recommended)

```bash
# Install Atomo CLI
curl -fsSL https://install.atomo.cc | sh

# Verify installation
atomo --version
```

### Manual Installation

```bash
# Clone and build from source
git clone https://github.com/atomo-org/atomo.git
cd atomo
cargo install --path crates/atomo_cli
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

### 2. Explore the Schema

Open `schema.ts` to see your data model:

```typescript
// schema.ts - This drives everything!
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

### 3. Start Development

```bash
# Start the development server
atomo dev

# 🚀 This automatically:
# - Generates Rust backend code from your schema
# - Compiles and starts the GraphQL server
# - Launches the admin UI
# - Enables hot reload for schema changes
```

You'll see:
```
🚀 Atomo CLI
   The Next-Generation Content Core

✨ Generating backend from schema.ts...
🔨 Compiling Rust service...
🌐 GraphQL server running at http://localhost:3000/graphql
🎨 Admin UI available at http://localhost:3000/admin
📡 WebSocket subscriptions at ws://localhost:3000/ws

🔥 Hot reload enabled - edit schema.ts to see changes instantly!
```

### 4. Explore Your Application

Open your browser to see what Atomo generated:

- **Admin UI**: `http://localhost:3000/admin`
  - Complete CRUD interface for all your models
  - Real-time collaboration indicators
  - Rich content editing with blocks
  - Automatic form generation and validation

- **GraphQL Playground**: `http://localhost:3000/graphql`
  - Fully typed GraphQL API
  - Real-time subscriptions
  - Automatic CRUD operations
  - Custom business logic hooks

### 5. Make Your First Change

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

Save the file and watch Atomo automatically:
1. 🔄 Detect the schema change
2. 🗄️ Generate database migration
3. 🔨 Recompile the Rust backend
4. 🎨 Update the admin UI forms
5. 📡 Refresh your browser

**That's it!** Your application now supports LinkedIn URLs with zero additional code.

## What Just Happened?

Atomo's "instant compilation" workflow:

1. **Schema Analysis**: Parses your TypeScript interfaces
2. **Code Generation**: Creates specialized Rust structs, GraphQL resolvers, and database models
3. **Compilation**: Uses Cargo's incremental compilation for speed
4. **Hot Reload**: Automatically restarts services and updates UI

This gives you the **performance of Rust** with the **productivity of TypeScript**.

## Next Steps

Now that you have a running application, explore these features:

- 📝 **[Custom Content Types](/guide/tutorials/content-types)** - Rich, block-based content
- 🤝 **[Real-time Collaboration](/guide/collaboration)** - Multi-user editing
- 🧩 **[Plugin Development](/guide/tutorials/plugin-dev)** - Extend with WASM
- 🚀 **[Deployment](/guide/tutorials/deployment)** - Go to production

## Need Help?

- 📖 **[Core Concepts](/guide/event-sourcing)** - Understand Atomo's architecture
- 💬 **[Discord Community](https://discord.gg/atomo)** - Get help from other developers
- 🐛 **[GitHub Issues](https://github.com/atomo-org/atomo/issues)** - Report bugs or request features
- 📧 **[Email Support](mailto:support@atomo.cc)** - Direct support from the team

Welcome to the future of application development! 🚀