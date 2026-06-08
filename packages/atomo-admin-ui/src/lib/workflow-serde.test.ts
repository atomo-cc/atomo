import { describe, it, expect } from 'vitest'
import {
  workflowToGraph,
  graphToWorkflow,
  emptyGraph,
  type Workflow,
} from './workflow-serde'

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
    ],
  },
]

describe('workflow serde', () => {
  it.each(fixtures)('round-trips $name losslessly', (wf) => {
    expect(graphToWorkflow(workflowToGraph(wf))).toEqual(wf)
  })

  it('assigns a stable id per step on import', () => {
    const g = workflowToGraph(fixtures[2])
    const ids = g.steps.map((s) => s.id)
    expect(new Set(ids).size).toBe(ids.length) // all unique
    expect(g.steps.every((s) => typeof s.id === 'string' && s.id.length > 0)).toBe(true)
  })

  it('emptyGraph produces a Manual trigger with no steps', () => {
    const g = emptyGraph('new')
    expect(g).toEqual({ name: 'new', trigger: 'Manual', steps: [] })
    expect(graphToWorkflow(g)).toEqual({ name: 'new', trigger: 'Manual', steps: [] })
  })
})
