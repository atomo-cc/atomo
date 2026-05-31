/**
 * Workflow serialization layer (milestone 1 of the visual workflow designer).
 *
 * Pure functions converting between the backend `Workflow` JSON (the exact serde
 * representation the engine deserializes) and a flat editor-friendly `WorkflowGraph`.
 * No UI, no dependencies — this is the contract the designer is built on.
 *
 * Backend contract (from crates/atomo/src/workflow.rs):
 *   Workflow { name, trigger, steps }
 *   trigger: { OnEvent: { model, event_type } } | "Manual" | { Schedule: { cron } }
 *   step: { name, action, condition: Condition|null, on_failure }
 *   action: { SetVariable:{key,value} } | { Delay:{seconds} }
 *          | { Http:{method,url,body} } | { Mutation:{query,variables} }
 *          | { Plugin:{plugin_name,function} }
 *   condition: { field, operator, value }
 *   on_failure: "Stop" | "Continue" | { Retry: { max_attempts } }
 */

// ── Backend JSON types ────────────────────────────────────────────────
export type WorkflowTrigger =
  | { OnEvent: { model: string; event_type: string } }
  | 'Manual'
  | { Schedule: { cron: string } }

export type StepAction =
  | { SetVariable: { key: string; value: any } }
  | { Delay: { seconds: number } }
  | { Http: { method: string; url: string; body?: any } }
  | { Mutation: { query: string; variables: Record<string, any> } }
  | { Plugin: { plugin_name: string; function: string } }

export interface Condition {
  field: string
  operator: string
  value: any
}

export type FailurePolicy = 'Stop' | 'Continue' | { Retry: { max_attempts: number } }

export interface WorkflowStep {
  name: string
  action: StepAction
  condition: Condition | null
  on_failure: FailurePolicy
}

export interface Workflow {
  name: string
  trigger: WorkflowTrigger
  steps: WorkflowStep[]
}

// ── Editor graph model ────────────────────────────────────────────────
// The engine runs steps sequentially, so the "graph" is an ordered list with a
// single trigger. Each node carries a stable client-side id for UI selection.
export interface GraphStep extends WorkflowStep {
  id: string
}

export interface WorkflowGraph {
  name: string
  trigger: WorkflowTrigger
  steps: GraphStep[]
}

let _seq = 0
function nextId(): string {
  _seq += 1
  return `n${_seq}`
}

/** Convert a backend Workflow into the editor graph (adds per-step ids). */
export function workflowToGraph(wf: Workflow): WorkflowGraph {
  return {
    name: wf.name,
    trigger: wf.trigger,
    steps: wf.steps.map((s) => ({ ...s, id: nextId() })),
  }
}

/** Convert the editor graph back into a backend Workflow (strips ids). */
export function graphToWorkflow(graph: WorkflowGraph): Workflow {
  return {
    name: graph.name,
    trigger: graph.trigger,
    steps: graph.steps.map(({ id, ...step }) => step),
  }
}

/** An empty graph for a new workflow. */
export function emptyGraph(name = ''): WorkflowGraph {
  return { name, trigger: 'Manual', steps: [] }
}

/** Default step factory per action kind, used by the editor's "add step" control. */
export function defaultStep(kind: string): WorkflowStep {
  const action: StepAction =
    kind === 'Delay'
      ? { Delay: { seconds: 1 } }
      : kind === 'Http'
        ? { Http: { method: 'GET', url: '' } }
        : kind === 'Mutation'
          ? { Mutation: { query: '', variables: {} } }
          : kind === 'Plugin'
            ? { Plugin: { plugin_name: '', function: '' } }
            : { SetVariable: { key: '', value: null } }
  return { name: 'step', action, condition: null, on_failure: 'Continue' }
}

/**
 * Round-trip self-check: graphToWorkflow(workflowToGraph(wf)) must deep-equal wf
 * across every trigger and action variant. Returns the list of failures (empty = ok).
 * Exposed so the UI/dev can assert correctness without a separate test runner.
 */
export function roundTripCheck(): string[] {
  const fixtures: Workflow[] = [
    { name: 'manual', trigger: 'Manual', steps: [] },
    {
      name: 'on-event',
      trigger: { OnEvent: { model: 'Contact', event_type: 'Created' } },
      steps: [
        { name: 'flag', action: { SetVariable: { key: 'k', value: true } }, condition: null, on_failure: 'Continue' },
      ],
    },
    {
      name: 'scheduled',
      trigger: { Schedule: { cron: '0 0 2 * * *' } },
      steps: [
        { name: 'wait', action: { Delay: { seconds: 5 } }, condition: { field: 'x', operator: 'eq', value: 1 }, on_failure: 'Stop' },
        { name: 'call', action: { Http: { method: 'POST', url: 'http://x', body: { a: 1 } } }, condition: null, on_failure: { Retry: { max_attempts: 3 } } },
        { name: 'mut', action: { Mutation: { query: 'q', variables: { id: '1' } } }, condition: null, on_failure: 'Continue' },
        { name: 'plug', action: { Plugin: { plugin_name: 'p', function: 'f' } }, condition: null, on_failure: 'Continue' },
      ],
    },
  ]

  const failures: string[] = []
  for (const wf of fixtures) {
    const restored = graphToWorkflow(workflowToGraph(wf))
    if (JSON.stringify(restored) !== JSON.stringify(wf)) {
      failures.push(`round-trip mismatch for "${wf.name}": ${JSON.stringify(restored)}`)
    }
  }
  return failures
}
