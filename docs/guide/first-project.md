# Your First Project

Create and run the CRM example service.

```bash
# From repo root
cd services/crm-service
pnpm dev        # boots codegen, server, and admin UI
```

Key files:
- `schema.ts` — data model and rules
- `plugins/` — WASM extensions
- `workflows/` — business flows

Edit `schema.ts` and save to regenerate backend + UI automatically.
