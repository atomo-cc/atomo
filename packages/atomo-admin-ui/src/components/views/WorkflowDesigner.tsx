/**
 * Workflow Designer — list-based structured editor (milestones 2-3 of the proposal).
 *
 * Edits a workflow's name, trigger, and an ordered list of steps, then saves via the
 * existing registerWorkflow (upsert). Uses the workflow-serde types as the contract.
 * Per the proposal we use a list editor (engine is sequential-only) rather than a canvas.
 */

import React, { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { ArrowUp, ArrowDown, Trash2, Plus, Save } from 'lucide-react'
import { apiClient } from '../../lib/api'
import {
  emptyGraph,
  graphToWorkflow,
  workflowToGraph,
  defaultStep,
  type WorkflowGraph,
  type WorkflowTrigger,
} from '../../lib/workflow-serde'
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Input } from '../ui/Input'
import { Textarea } from '../ui/Textarea'

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
    <div className="p-6 space-y-6 max-w-3xl">
      <h1 className="text-3xl font-bold text-gray-900">工作流设计器</h1>

      {error && (
        <div className="rounded-md bg-danger-50 border border-danger-500 px-4 py-3 text-sm text-danger-700">{error}</div>
      )}
      {saved && (
        <div className="rounded-md bg-success-50 border border-success-500 px-4 py-3 text-sm text-success-700">已保存</div>
      )}

      <Card>
        <CardHeader><CardTitle>基本信息</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          <Input label="名称" value={graph.name} disabled={!!name}
            onChange={(e) => update({ name: e.target.value })} placeholder="my-workflow" />

          <div className="space-y-2">
            <label className="text-sm font-medium text-gray-700">触发器</label>
            <select value={tk} onChange={(e) => setTriggerKind(e.target.value)}
              className="block rounded-md border border-gray-300 px-3 py-2 text-sm">
              <option value="Manual">Manual</option>
              <option value="OnEvent">OnEvent</option>
              <option value="Schedule">Schedule</option>
            </select>
          </div>

          {tk === 'OnEvent' && graph.trigger !== 'Manual' && 'OnEvent' in graph.trigger && (
            <div className="flex gap-3">
              <Input label="模型" value={graph.trigger.OnEvent.model}
                onChange={(e) => update({ trigger: { OnEvent: { ...(graph.trigger as any).OnEvent, model: e.target.value } } })} />
              <Input label="事件类型" value={graph.trigger.OnEvent.event_type}
                onChange={(e) => update({ trigger: { OnEvent: { ...(graph.trigger as any).OnEvent, event_type: e.target.value } } })} />
            </div>
          )}
          {tk === 'Schedule' && graph.trigger !== 'Manual' && 'Schedule' in graph.trigger && (
            <Input label="Cron (6 字段)" value={graph.trigger.Schedule.cron}
              onChange={(e) => update({ trigger: { Schedule: { cron: e.target.value } } })} />
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle>步骤 ({graph.steps.length})</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          {graph.steps.map((step, idx) => (
            <div key={step.id} className="rounded-md border border-gray-200 p-4 space-y-3">
              <div className="flex items-center gap-2">
                <Input value={step.name} placeholder="step name"
                  onChange={(e) => patchStep(step.id, { name: e.target.value })} className="max-w-xs" />
                <div className="ml-auto flex gap-1">
                  <Button size="sm" variant="ghost" onClick={() => moveStep(idx, -1)} disabled={idx === 0}><ArrowUp className="h-4 w-4" /></Button>
                  <Button size="sm" variant="ghost" onClick={() => moveStep(idx, 1)} disabled={idx === graph.steps.length - 1}><ArrowDown className="h-4 w-4" /></Button>
                  <Button size="sm" variant="danger" onClick={() => removeStep(step.id)}><Trash2 className="h-4 w-4" /></Button>
                </div>
              </div>

              <div className="flex gap-3 items-end">
                <div className="space-y-1">
                  <label className="text-xs font-medium text-gray-600">动作</label>
                  <select
                    value={Object.keys(step.action)[0]}
                    onChange={(e) => patchStep(step.id, { action: defaultStep(e.target.value).action })}
                    className="block rounded-md border border-gray-300 px-2 py-1.5 text-sm">
                    {ACTION_KINDS.map((k) => <option key={k} value={k}>{k}</option>)}
                  </select>
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-medium text-gray-600">失败策略</label>
                  <select
                    value={typeof step.on_failure === 'string' ? step.on_failure : 'Retry'}
                    onChange={(e) => patchStep(step.id, { on_failure: e.target.value === 'Retry' ? { Retry: { max_attempts: 3 } } : e.target.value })}
                    className="block rounded-md border border-gray-300 px-2 py-1.5 text-sm">
                    {FAILURE_KINDS.map((k) => <option key={k} value={k}>{k}</option>)}
                  </select>
                </div>
              </div>

              <Textarea
                label="动作参数 (JSON)"
                className="font-mono text-xs"
                rows={4}
                value={JSON.stringify(Object.values(step.action)[0], null, 2)}
                onChange={(e) => {
                  try {
                    const body = JSON.parse(e.target.value)
                    const kind = Object.keys(step.action)[0]
                    patchStep(step.id, { action: { [kind]: body } })
                    setError(null)
                  } catch {
                    setError(`步骤 "${step.name}" 的动作参数 JSON 无效`)
                  }
                }}
              />
            </div>
          ))}
          <Button variant="secondary" onClick={addStep}><Plus className="h-4 w-4 mr-1" /> 添加步骤</Button>
        </CardContent>
      </Card>

      <div className="flex gap-2">
        <Button onClick={save}><Save className="h-4 w-4 mr-1" /> 保存</Button>
        <Button variant="ghost" onClick={() => navigate('/workflows')}>取消</Button>
      </div>
    </div>
  )
}
