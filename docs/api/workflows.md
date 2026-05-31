# Workflows (REST)

The workflow engine runs multi-step workflows with triggers, conditions, and retry policies. Definitions are loaded from `./workflows/*.json` at server boot and can also be registered at runtime via REST. Event-triggered workflows fire automatically on model events.

## Endpoints

```http
GET /workflows                 # list registered workflow names
POST /workflows                # register a workflow definition (JSON body)
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
