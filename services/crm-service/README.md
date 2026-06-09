# Atomo CRM Service

Dogfood CRM service exercising the Action/Worker architecture.

## Features exercised

- Schema-defined models (User, Company, Contact, Lead, Deal, Activity)
- Lifecycle events (created, updated, deleted)
- Typed actions with Pick<> inputs
- User-callable action (createLeadAndContact) with custom input + returns
- External worker execution via @atomo-cc/worker-sdk
- Worker CRUD callback through Rust checked path
- Validation (email, url, min/max, unique, required)
- RBAC (role-based + owner-based access)
- Multi-tenant scoping (tenantId + sameTenant + fromAuth)
- Computed fields (displayName)
- Compound constraints (unique, index)
- Conditional events (whenChanged, when)
- Origin-based loop prevention
- Generated typed client, actions, and worker types

## File map

```text
atomo/schema.ts          Models, validation, access, lifecycle events
atomo/actions.ts         Action contracts and inputs
workers/index.ts         Typed worker handlers
generated/types.ts       Generated model and input types
generated/actions.ts     Generated action map, TypedWorker, TypedCrudClient
generated/client.ts      Generated typed client
lib/                     Helper modules (email, enrichment, scoring, search, slack)
tests/crm.e2e.test.ts    E2E scenario checklist
```

## Run

```bash
cp .env.example .env
pnpm install
pnpm typecheck
pnpm worker
```
