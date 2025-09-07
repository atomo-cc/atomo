# CLI Reference

The `atomo` CLI streamlines local development, code generation, and service ops.

## Usage

```bash
# Run CLI from repo root
pnpm atomo -- <command> [options]

# Or use package scripts inside a service (e.g., CRM)
cd services/crm-service
pnpm dev        # atomo dev --service crm
pnpm migrate    # atomo migrate --service crm
```

## Commands & Options

```bash
# Start dev pipeline (codegen, server, admin UI)
atomo dev [-p, --port 3000]

# Build service for production
atomo build --service <name>

# Database operations
atomo migrate [--database-url <url>] [--generate --name <migration>]

# Code generation
atomo codegen [-o, --output generated]

# Seed database (run inside a service directory)
atomo seed [--file ./seed.sql]
```

### Workspace Dev
For core contributors working on the core crates and a service together:
```bash
atomo workspace-dev [--service-path services/<name>] [-p 3000]
```

Init/build/deploy:
```bash
atomo init <project-name> [--template <name>]
atomo build
atomo deploy [-e, --env production]
```

See also: `package.json` scripts at repo root and in `services/<name>/package.json`.

### Seed Command
- Runs from the service root (where `schema.ts` lives).
- Reads `.env` in the current directory for `DATABASE_URL`.
- Executes the given SQL file in a transaction (defaults to `./seed.sql`).

Examples:
```bash
cd services/crm-service
# Using .env DATABASE_URL and default seed file
atomo seed

# Specify a custom seed file
atomo seed --file ./seed-demo.sql
```
