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

## Common Commands

```bash
# Start dev pipeline (codegen, server, admin UI)
atomo dev --service <name>

# Build service for production
atomo build --service <name>

# Database operations
atomo migrate --service <name>
atomo seed --service <name>

# Test service workflows and plugins
atomo test --service <name>
```

See also: `package.json` scripts at repo root and in `services/<name>/package.json`.
