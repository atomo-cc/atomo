# Workflows (REST)

The workflow engine runs multi-step workflows with triggers, conditions, and retry policies. Workflows registered via REST are persisted to a `workflows` table and reloaded on the next server boot. Definitions in `./workflows/*.json` are also loaded at boot. Event-triggered workflows fire automatically on model events.

## Endpoints

```http
GET /workflows                 # list registered workflow names
POST /workflows                # register a workflow definition (JSON body)
DELETE /workflows/{name}        # remove a registered workflow (404 if absent)
POST /workflows/{name}/run     # execute a workflow with a JSON context body
```

## Definition Shape

```json
{
  "name": "notify-on-new-contact",
  "trigger": { "OnEvent": { "model": "Contact", "event_type": "Created" } },
  "steps": [
    {
      "name": "set-flag",
      "action": { "SetVariable": { "key": "notified", "value": true } },
      "condition": null,
      "on_failure": "Continue"
    }
  ]
}
```

- **Triggers**: `{ "OnEvent": { "model", "event_type" } }`, `"Manual"`, or `{ "Schedule": { "cron" } }`.
- **Actions**: `Mutation`, `Plugin`, `Http`, `Delay`, `SetVariable`.
- **Conditions**: `{ "field", "operator", "value" }` with operators `eq | neq | gt | lt | contains`.
- **Failure policy**: `"Stop"`, `"Continue"`, or `{ "Retry": { "max_attempts": N } }`.

## Run Response

```json
{ "workflow": "notify-on-new-contact", "status": "Completed", "steps_run": 1, "errors": [] }
```

`status` is one of `Running | Completed | Failed | Paused`.

> Event-triggered workflows (`OnEvent`) execute automatically when a matching model event is emitted — no manual `run` call is required.

## Scheduled (cron) Triggers

Workflows with a `Schedule { cron }` trigger run automatically. A background scheduler started at server boot checks every 30 seconds and fires any workflow whose cron schedule has an occurrence in the elapsed window.

```json
{
  "name": "nightly-digest",
  "trigger": { "Schedule": { "cron": "0 0 2 * * *" } },
  "steps": [ /* ... */ ]
}
```

- Cron expressions use the 6-field form: `sec min hour day month weekday` (e.g. `0 0 2 * * *` = 02:00 daily).
- Invalid cron expressions are logged and skipped (they do not crash the scheduler).
- The scheduler tick is 30s, so the effective minimum granularity is ~30 seconds.
