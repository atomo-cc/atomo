# Atomo Dogfood CRM Example

This is a reference dogfood CRM app for Atomo's Action/Worker architecture.

It is designed to exercise the core platform features:

- schema-defined models
- lifecycle events
- typed actions
- external worker execution
- worker → Rust CRUD callback
- validation / RBAC / tenant scoping preservation
- event sourcing
- origin-based loop prevention
- generated client / worker types

## File map

```text
atomo/schema.ts          CRM models, validation, access, lifecycle events
atomo/actions.ts         Action contracts and inputs
workers/index.ts         Typed worker handlers
generated/types.ts       Example generated model types
generated/actions.ts     Example generated action and TypedWorker types
generated/client.ts      Example generated client shape
tests/crm.e2e.test.ts    E2E scenario checklist
```

## Architecture

```text
User / GraphQL / Client
  ↓
Rust checked CRUD
  ↓
Event store commit
  ↓
ActionDispatcher matches model events
  ↓
JobStore enqueue { action, input, actor, origin }
  ↓
External worker leases job
  ↓
Worker executes Node/Bun/npm logic
  ↓
ctx.crud calls Rust worker CRUD API
  ↓
validation / RBAC / tenant / event sourcing preserved
```

## Models

- `User` tests welcome email action and user RBAC.
- `Company` tests enrichment and event loop prevention.
- `Contact` tests unique tenant email, owner access, delete side effects.
- `Lead` tests scoring, rollups, validation, and update-triggered actions.
- `Deal` tests conditional lifecycle action on `stage = won`.
- `Activity` tests relation update and cross-model worker CRUD.

## Suggested worker token capabilities

For the full demo worker:

```json
[
  "action:sendWelcomeEmail",
  "action:enrichCompany",
  "action:scoreLead",
  "action:rollupLeadStats",
  "action:notifyDealWon",
  "action:updateContactLastActivity",
  "action:removeFromSearchIndex",
  "action:createLeadAndContact",
  "crud:User:update",
  "crud:Company:create",
  "crud:Company:read",
  "crud:Company:update",
  "crud:Contact:create",
  "crud:Contact:update",
  "crud:Lead:create",
  "crud:Lead:read",
  "crud:Lead:update"
]
```

For MVP builds that only support broad capabilities, use:

```json
["crud:*"]
```

## Loop prevention

Worker CRUD calls pass an `origin` option:

```ts
await crud.update('Lead', id, { score }, { origin: 'scoreLead' })
```

The dispatcher should suppress re-enqueueing the same action when the event origin matches the action name.

## Run sketch

```bash
cp .env.example .env
pnpm install
pnpm typecheck
pnpm worker
```

In a real repo checkout, `atomo dev` should generate the `generated/*` files from `atomo/schema.ts` and `atomo/actions.ts`.

## What this example intentionally avoids

- No WASM.
- No Dynamic JS hook in CRUD hot path.
- No direct worker database connection.
- No public REST CRUD dependency.

Workers always call back through Rust checked CRUD so invariants are preserved.
