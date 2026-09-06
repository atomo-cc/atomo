/**
 * Workflow Designer — Dashin List-Based Structured Pipeline Editor
 */

import React, { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { ArrowUp, ArrowDown, Trash2, Plus, Save, Workflow as WorkflowIcon, CheckCircle2, AlertCircle } from 'lucide-react'
import { apiClient } from '../../lib/api'
import {
  emptyGraph,
  graphToWorkflow,
  workflowToGraph,
  defaultStep,
  type WorkflowGraph,
  type WorkflowTrigger,
} from '../../lib/workflow-serde'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Input } from '../ui/Input'
import { ActionEditor } from './ActionEditor'
import { WorkflowGraphView } from './WorkflowGraphView'

const ACTION_KINDS = ['SetVariable', 'Delay', 'Http', 'Mutation', 'Plugin']
const FAILURE_KINDS = ['Continue', 'Stop', 'Retry']

function triggerKind(t: WorkflowTrigger): string {
  if (t === 'Manual') return 'Manual'
  if ('OnEvent' in t) return 'OnEvent'
  return 'Schedule'
}

export interface WorkflowDesignerProps {
  /** Name of an existing workflow to edit; omit to create a new one. */
  workflowName?: string
}

export function WorkflowDesigner({ workflowName }: WorkflowDesignerProps) {
  const name = workflowName
  const navigate = useNavigate()
  const [graph, setGraph] = useState<WorkflowGraph>(emptyGraph())
  const [error, setError] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    if (name) {
      apiClient.getWorkflow(name)
        .then((wf) => setGraph(workflowToGraph(wf)))
        .catch((e) => setError(e?.message || 'Failed to load workflow'))
    }
  }, [name])

  const update = (patch: Partial<WorkflowGraph>) => setGraph((g) => ({ ...g, ...patch }))

  const setTriggerKind = (kind: string) => {
    const trigger: WorkflowTrigger =
      kind === 'OnEvent' ? { OnEvent: { model: '', event_type: 'Created' } }
        : kind === 'Schedule' ? { Schedule: { cron: '0 0 * * * *' } }
          : 'Manual'
    update({ trigger })
  }

  const addStep = () => {
    const step = defaultStep('SetVariable')
    setGraph((g) => ({ ...g, steps: [...g.steps, { ...step, id: `n${Date.now()}` }] }))
  }

  const removeStep = (id: string) =>
    setGraph((g) => ({ ...g, steps: g.steps.filter((s) => s.id !== id) }))

  const moveStep = (idx: number, dir: -1 | 1) =>
    setGraph((g) => {
      const steps = [...g.steps]
      const j = idx + dir
      if (j < 0 || j >= steps.length) return g
      ;[steps[idx], steps[j]] = [steps[j], steps[idx]]
      return { ...g, steps }
    })

  const patchStep = (id: string, patch: any) =>
    setGraph((g) => ({ ...g, steps: g.steps.map((s) => (s.id === id ? { ...s, ...patch } : s)) }))

  const save = async () => {
    try {
      const wf = graphToWorkflow(graph)
      if (!wf.name) { setError('Workflow name is required'); return }
      await apiClient.registerWorkflow(wf)
      setError(null); setSaved(true)
      setTimeout(() => navigate('/workflows'), 600)
    } catch (e: any) {
      setError(e?.message || 'Failed to save workflow')
    }
  }

  const tk = triggerKind(graph.trigger)

  return (
    <div className="p-6 space-y-6 max-w-4xl">
      {/* Header */}
      <div className="flex items-center gap-3">
        <div className="w-9 h-9 rounded-bn bg-primary/10 flex items-center justify-center text-primary">
          <WorkflowIcon className="h-5 w-5" />
        </div>
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">
            {name ? `Edit Workflow: ${name}` : 'New Workflow'}
          </h1>
          <p className="text-xs text-icon-muted mt-0.5">Visually design sequential action steps and triggers</p>
        </div>
      </div>

      {error && (
        <div className="flex items-center gap-2 rounded-bn bg-rose-500/10 border border-rose-500/20 px-4 py-3 text-xs text-rose-600 dark:text-rose-400 font-medium">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </div>
      )}
      {saved && (
        <div className="flex items-center gap-2 rounded-bn bg-emerald-500/10 border border-emerald-500/20 px-4 py-3 text-xs text-emerald-600 dark:text-emerald-400 font-medium">
          <CheckCircle2 className="h-4 w-4 shrink-0" />
          Workflow successfully registered! Redirecting…
        </div>
      )}

      {/* Basic information */}
      <Card>
        <CardHeader className="py-4 border-b border-bn-border/60">
          <CardTitle>Basic Information</CardTitle>
          <CardDescription>Workflow identification and invocation trigger configuration</CardDescription>
        </CardHeader>
        <CardContent className="p-5 space-y-4">
          <Input
            label="Workflow Name"
            value={graph.name}
            disabled={!!name}
            onChange={(e) => update({ name: e.target.value })}
            placeholder="e.g. notify-on-new-contact"
          />

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-foreground block">Trigger Type</label>
            <select
              value={tk}
              onChange={(e) => setTriggerKind(e.target.value)}
              className="flex h-9 w-full rounded-bn border border-bn-border bg-content-box px-3 py-1.5 text-sm text-foreground shadow-sm focus:outline-none focus:ring-2 focus:ring-primary/40 focus:border-primary transition-colors"
            >
              <option value="Manual">Manual</option>
              <option value="OnEvent">OnEvent (Schema Change)</option>
              <option value="Schedule">Schedule (Cron Expression)</option>
            </select>
          </div>

          {tk === 'OnEvent' && graph.trigger !== 'Manual' && 'OnEvent' in graph.trigger && (
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <Input
                label="Target Model"
                value={graph.trigger.OnEvent.model}
                onChange={(e) => update({ trigger: { OnEvent: { ...(graph.trigger as any).OnEvent, model: e.target.value } } })}
                placeholder="e.g. GenerationJob"
              />
              <Input
                label="Event Type"
                value={graph.trigger.OnEvent.event_type}
                onChange={(e) => update({ trigger: { OnEvent: { ...(graph.trigger as any).OnEvent, event_type: e.target.value } } })}
                placeholder="e.g. Created"
              />
            </div>
          )}
          {tk === 'Schedule' && graph.trigger !== 'Manual' && 'Schedule' in graph.trigger && (
            <Input
              label="Cron (6 fields)"
              value={graph.trigger.Schedule.cron}
              onChange={(e) => update({ trigger: { Schedule: { cron: e.target.value } } })}
              placeholder="0 0 * * * *"
            />
          )}
        </CardContent>
      </Card>

      {/* Steps */}
      <Card>
        <CardHeader className="py-4 border-b border-bn-border/60 flex flex-row items-center justify-between">
          <div>
            <CardTitle>Pipeline Steps ({graph.steps.length})</CardTitle>
            <CardDescription>Sequential operations executed when this workflow fires</CardDescription>
          </div>
          <Button size="sm" variant="secondary" onClick={addStep}>
            <Plus className="h-3.5 w-3.5 mr-1 text-primary" /> Add Step
          </Button>
        </CardHeader>
        <CardContent className="p-5 space-y-4">
          {graph.steps.length === 0 ? (
            <div className="text-center py-6 text-xs text-icon-muted">No steps configured yet. Click "Add Step" above.</div>
          ) : (
            graph.steps.map((step, idx) => (
              <div key={step.id} className="rounded-bn border border-bn-border bg-content-box p-4 space-y-4 shadow-sm hover:border-primary/40 transition-colors">
                <div className="flex items-center gap-3">
                  <Input
                    value={step.name}
                    placeholder="Step name"
                    onChange={(e) => patchStep(step.id, { name: e.target.value })}
                    className="max-w-xs h-8"
                  />
                  <div className="ml-auto flex gap-1">
                    <Button size="sm" variant="ghost" onClick={() => moveStep(idx, -1)} disabled={idx === 0} title="Move up">
                      <ArrowUp className="h-3.5 w-3.5" />
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => moveStep(idx, 1)} disabled={idx === graph.steps.length - 1} title="Move down">
                      <ArrowDown className="h-3.5 w-3.5" />
                    </Button>
                    <Button size="sm" variant="danger" onClick={() => removeStep(step.id)} title="Remove step">
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <label className="text-xs font-medium text-foreground block">Action Type</label>
                    <select
                      value={Object.keys(step.action)[0]}
                      onChange={(e) => patchStep(step.id, { action: defaultStep(e.target.value).action })}
                      className="flex h-8 w-full rounded-bn border border-bn-border bg-content-box px-2.5 text-xs text-foreground shadow-sm focus:outline-none focus:ring-2 focus:ring-primary/40"
                    >
                      {ACTION_KINDS.map((k) => <option key={k} value={k}>{k}</option>)}
                    </select>
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-xs font-medium text-foreground block">Failure Policy</label>
                    <select
                      value={typeof step.on_failure === 'string' ? step.on_failure : 'Retry'}
                      onChange={(e) => patchStep(step.id, { on_failure: e.target.value === 'Retry' ? { Retry: { max_attempts: 3 } } : e.target.value })}
                      className="flex h-8 w-full rounded-bn border border-bn-border bg-content-box px-2.5 text-xs text-foreground shadow-sm focus:outline-none focus:ring-2 focus:ring-primary/40"
                    >
                      {FAILURE_KINDS.map((k) => <option key={k} value={k}>{k}</option>)}
                    </select>
                  </div>
                </div>

                <div className="pt-2 border-t border-bn-border/40">
                  <ActionEditor
                    action={step.action}
                    onChange={(action) => patchStep(step.id, { action })}
                  />
                </div>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      {/* Preview */}
      <Card>
        <CardHeader className="py-4 border-b border-bn-border/60">
          <CardTitle>Topology Preview</CardTitle>
          <CardDescription>Live visual representation of the execution pipeline</CardDescription>
        </CardHeader>
        <CardContent className="p-4 bg-content-bg/30">
          <WorkflowGraphView graph={graph} />
        </CardContent>
      </Card>

      {/* Actions */}
      <div className="flex gap-2">
        <Button onClick={save}>
          <Save className="h-3.5 w-3.5 mr-1.5" /> Save Workflow
        </Button>
        <Button variant="ghost" onClick={() => navigate('/workflows')}>Cancel</Button>
      </div>
    </div>
  )
}
