import React from 'react'
import { Zap, ArrowDown } from 'lucide-react'
import type { WorkflowGraph, WorkflowTrigger, StepAction, FailurePolicy } from '../../lib/workflow-serde'

export interface WorkflowGraphViewProps { graph: WorkflowGraph }

function triggerLabel(t: WorkflowTrigger): string {
  if (t === 'Manual') return 'Manual'
  if (typeof t === 'object' && 'OnEvent' in t) return `On ${t.OnEvent.model} ${t.OnEvent.event_type}`
  if (typeof t === 'object' && 'Schedule' in t) return `Schedule: ${t.Schedule.cron}`
  return 'Unknown'
}

function actionLabel(action: StepAction): string {
  const key = Object.keys(action)[0] as string
  const body = (action as any)[key]
  switch (key) {
    case 'Delay': return `Delay ${body.seconds}s`
    case 'Http': return `Http ${body.method} ${body.url}`
    case 'SetVariable': return `SetVariable ${body.key}`
    case 'Plugin': return `Plugin ${body.plugin_name}.${body.function}`
    case 'Mutation': return 'Mutation'
    default: return key
  }
}

function failureLabel(p: FailurePolicy): string {
  if (p === 'Stop') return 'Stop'
  if (p === 'Continue') return 'Continue'
  if (typeof p === 'object' && 'Retry' in p) return `Retry x${p.Retry.max_attempts}`
  return 'Unknown'
}

function Connector() {
  return (
    <div className="flex flex-col items-center">
      <div className="h-4 w-px bg-gray-300" />
      <ArrowDown className="h-4 w-4 text-gray-400" />
    </div>
  )
}

export function WorkflowGraphView({ graph }: WorkflowGraphViewProps) {
  return (
    <div className="flex flex-col items-center gap-0 py-6">
      {/* Trigger node */}
      <div className="rounded-md border border-primary-300 bg-primary-50 px-4 py-3 shadow-sm w-64 text-center">
        <div className="flex items-center justify-center gap-1 text-xs font-semibold text-primary-700 uppercase mb-1">
          <Zap className="h-3 w-3" /> Trigger
        </div>
        <div className="text-sm">{triggerLabel(graph.trigger)}</div>
      </div>

      {graph.steps.length === 0 ? (
        <>
          <Connector />
          <div className="rounded-md border border-gray-200 bg-white px-4 py-3 shadow-sm w-64 text-center text-sm text-gray-400 italic">
            No steps
          </div>
        </>
      ) : (
        graph.steps.map((step) => (
          <React.Fragment key={step.id}>
            <Connector />
            <div className="rounded-md border border-gray-200 bg-white px-4 py-3 shadow-sm w-64">
              <div className="font-bold text-sm">{step.name}</div>
              <span className="inline-block mt-1 rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-700">
                {actionLabel(step.action)}
              </span>
              {step.condition && (
                <div className="mt-1 text-xs text-gray-500">
                  if {step.condition.field} {step.condition.operator} {String(step.condition.value)}
                </div>
              )}
              <div className="mt-1 text-xs text-gray-400">{failureLabel(step.on_failure)}</div>
            </div>
          </React.Fragment>
        ))
      )}
    </div>
  )
}
