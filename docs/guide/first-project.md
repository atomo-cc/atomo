# Your First Project

Use the checked-in CRM example service as the current MVP project.

```bash
# From repo root
pnpm dev:admin
pnpm --filter @atomo-cc/client-sdk dev
pnpm --filter atomo-crm-service generate
```

Key files:

- `schema.ts` — data model and rules
- `plugins/` — WASM extensions
- `workflows/` — business flows

Edit `services/crm-service/schema.ts`, regenerate CRM artifacts, then verify SDK output with `pnpm --filter @atomo-cc/client-sdk build`.

Keep the frontend baseline green with `pnpm --filter "./packages/*" test`, which type-checks the Admin UI and SDK packages.

## Demo Data Loop

The CRM demo is intentionally small but complete:

- `Company` and `Contact` records show account relationships.
- `Deal.stage` and `Deal.position` drive the Kanban board.
- `Activity` records plus contact notes drive the contact timeline.

After changing the CRM schema or seed data:

```bash
pnpm --filter atomo-crm-service generate
pnpm --filter @atomo-cc/client-sdk build
pnpm --filter "./packages/*" test
```

To refresh local demo data against a running Postgres database:

```bash
export DATABASE_URL=postgresql://user:pass@localhost:5432/atomo_dev
pnpm seed:sql --filter ./services/crm-service
```

The seed creates four companies, four contacts, deals in all six pipeline stages, Kanban positions within each stage, and contact timeline activity for notes, calls, meetings, email, and tasks.
