# CRM Example

Multi-model, multi-worker example exercising events, RBAC, validation,
relationships, cross-model writes, and origin-based loop prevention.

## Architecture

```
                   ┌──────────────┐
                   │   schema.ts  │
                   └──────┬───────┘
                          │ parsed by Atomo
        ┌─────────────────┼─────────────────┐
        ▼                 ▼                 ▼
   ┌─────────┐     ┌──────────┐     ┌────────────┐
   │ Company │     │ Contact  │     │   Deal     │
   └─────────┘     └────┬─────┘     └─────┬──────┘
                        │                 │
              events:   │                 │  events:
              created → onNewContact      │  updated → onDealStatusChange
              updated → onStageChange     │
                        │                 │
               ┌────────┘                 └────────┐
               ▼                                   ▼
      ┌──────────────────┐              ┌────────────────────┐
      │  contact-worker  │              │    deal-worker      │
      │  onNewContact    │              │  onDealStatusChange │
      │  onStageChange   │◄─────────────│    (deal won →      │
      └──────────────────┘  triggers    │  update Contact     │
                            onStage     │  stage="customer")  │
                            Change      └────────────────────┘
```

### Cross-model write + loop prevention

1. A Deal's `status` changes to `"won"`
2. `deal-worker` handles `onDealStatusChange`, updates Contact `stage` to `"customer"` with `origin: "onDealStatusChange"`
3. Contact.updated fires — `onStageChange` action fires normally (different action)
4. But `onDealStatusChange` is **not** re-enqueued (origin matches — loop prevented)

## Models

| Model | Events | Validation | RBAC |
|-------|--------|------------|------|
| Company | — | name: required, website: url | create: sales\|admin, read: authenticated |
| Contact | created → onNewContact, updated → onStageChange | email: email, name: required\|min:1\|max:100 | create: sales\|admin, read: authenticated, delete: admin |
| Deal | updated → onDealStatusChange | title: required, value: min:0 | create: sales\|admin, read: authenticated, update: sales\|admin, delete: admin |
| Activity | — | — | create: sales\|admin, read: authenticated |

## Worker capabilities (least privilege)

```
# contact-worker
crud:Contact:read,update
action:onNewContact
action:onStageChange

# deal-worker
crud:Deal:read
crud:Contact:update
action:onDealStatusChange
```

## Files

| File | Purpose |
|------|---------|
| `schema.ts` | 4-model CRM schema with events, actions, validation, RBAC, relationships |
| `workers/contact-worker.ts` | Handles onNewContact + onStageChange |
| `workers/deal-worker.ts` | Handles onDealStatusChange with cross-model write + loop prevention |
| `generated/client.ts` | Typed client generated from the schema |

## Running

```bash
# 1. Start Atomo with the CRM schema
atomo serve --schema ./schema.ts

# 2. Register worker tokens with least-privilege capabilities
atomo worker-token create --name "contact-worker" \
  --capabilities "crud:Contact:read,update,action:onNewContact,action:onStageChange"

atomo worker-token create --name "deal-worker" \
  --capabilities "crud:Deal:read,crud:Contact:update,action:onDealStatusChange"

# 3. Start workers (separate terminals)
CONTACT_WORKER_TOKEN=<token> npx tsx workers/contact-worker.ts
DEAL_WORKER_TOKEN=<token> npx tsx workers/deal-worker.ts

# 4. Create some data and watch the event flow
curl -X POST http://localhost:3000/api/worker/crud/Deal \
  -H "Content-Type: application/json" \
  -H "X-Worker-Token: <token>" \
  -d '{"data": {"title": "Enterprise plan", "value": 50000, "contactId": "<id>", "status": "won"}}'
```
