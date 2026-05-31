# Design Proposal: Visual Workflow Designer

Status: IMPLEMENTED (list-based editor). The milestones below are delivered:
serialization layer (`workflow-serde`, vitest-tested), list editor (`WorkflowDesigner`),
typed `ActionEditor`, and read-only `WorkflowGraphView`. The reactflow drag-and-drop
canvas remains intentionally deferred (engine is sequential-only). This document is
retained as the design rationale.

## Goal

A drag-and-drop UI in the admin app for authoring the same `Workflow` definitions
that the backend already executes (triggers, steps, conditions, retry policies),
serialized to the exact JSON the existing REST API accepts.

## What already exists (build on, don't rebuild)

- Backend engine: `WorkflowEngine` with triggers (`OnEvent`, `Manual`, `Schedule{cron}`),
  step actions (`Mutation`, `Plugin`, `Http`, `Delay`, `SetVariable`), conditions, and
  failure policies (`Stop`, `Continue`, `Retry`). Persisted to a `workflows` table.
- REST API: `GET/POST /workflows`, `GET/DELETE /workflows/{name}`, `POST /workflows/{name}/run`.
- Admin UI `WorkflowsView`: list / run / register-via-JSON / edit (load JSON) / delete.

The designer is a **better editor for the same JSON** — it must round-trip losslessly
with the JSON textarea that already works.

## Scope (v1)

In scope:
- A canvas showing a workflow as a linear/branching node graph: one trigger node +
  ordered step nodes.
- Node inspector panel to edit the selected node's fields (typed per action kind).
- Add/remove/reorder steps; set per-step condition and on_failure.
- Serialize canvas -> `Workflow` JSON and POST to `/workflows` (reusing `registerWorkflow`).
- Load existing workflow (`getWorkflow`) -> canvas.

Explicitly out of scope for v1 (avoid scope creep):
- Arbitrary DAGs / parallel branches (engine executes steps sequentially today).
- Live execution visualization / step-by-step debugger.
- Versioning / draft-vs-published.

## Proposed architecture

- Library: `reactflow` (already-common React node-graph lib) for the canvas. Add as a
  dependency of `atomo-admin-ui` only.
- Component: `src/components/views/WorkflowDesigner.tsx`, routed at `/workflows/design`
  (and `/workflows/design/:name` to edit an existing one). Add a "设计器" button on
  `WorkflowsView` rows and a "New" action.
- State: a `WorkflowGraph` model in the component:
  - `triggerNode: { kind: 'OnEvent'|'Manual'|'Schedule', ... }`
  - `stepNodes: Array<{ id, name, action: StepAction, condition?, on_failure }>`
  - edges are implicit sequential order (trigger -> step1 -> step2 ...).
- Serialization: pure functions `graphToWorkflow(graph): Workflow` and
  `workflowToGraph(wf): WorkflowGraph`, unit-tested for round-trip equality. This is the
  riskiest part and must be tested first.
- Node editors: one small form per `StepAction` variant; a discriminated union keyed by
  the action tag (`SetVariable | Delay | Http | Mutation | Plugin`).

## Data contract (must match backend exactly)

The serializer must emit the serde representation the engine deserializes, e.g.:

```json
{
  "name": "notify",
  "trigger": { "OnEvent": { "model": "Contact", "event_type": "Created" } },
  "steps": [
    { "name": "wait", "action": { "Delay": { "seconds": 5 } },
      "condition": null, "on_failure": "Continue" }
  ]
}
```

Round-trip test: `graphToWorkflow(workflowToGraph(wf)) deep-equals wf` for a fixture
set covering every trigger and action variant.

## Milestones

1. Serialization layer + round-trip unit tests (no UI). Lowest risk, highest leverage.
2. Read-only canvas rendering an existing workflow (load via `getWorkflow`).
3. Editing: node inspector + add/remove/reorder; save via `registerWorkflow`.
4. Trigger editor + condition/failure-policy editors.
5. Polish: validation messages, empty states, wiring from `WorkflowsView`.

## Risks / decisions to confirm

- **reactflow dependency size** — it's sizable; confirm acceptable for the admin bundle
  (already over the 500kB warning). Alternative: a simpler custom vertical-list editor
  with no canvas lib (less "visual" but zero new deps). **Recommend starting with the
  no-canvas list editor** to ship value fast, add reactflow only if a true graph canvas
  is required.
- Sequential-only execution today means a "graph" is really an ordered list; a full DAG
  canvas would over-promise vs. engine behavior. Keep the UI honest about this.

## Recommendation

Implement milestone 1 (serialization + tests) and a **list-based editor** first — it
delivers the core value (structured editing instead of raw JSON) with no heavy deps and
no mismatch with the sequential engine. Defer the reactflow canvas until there's a
concrete need for visual branching.
