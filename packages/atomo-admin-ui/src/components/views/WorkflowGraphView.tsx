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
      <div className="h-4 w-px bg-bn-border" />
      <ArrowDown className="h-3.5 w-3.5 text-icon-muted" />
    </div>
  )
}

export function WorkflowGraphView({ graph }: WorkflowGraphViewProps) {
  return (
    <div className="flex flex-col items-center gap-0 py-6">
      {/* Trigger node */}
      <div className="rounded-bn border border-primary/30 bg-primary/10 px-4 py-3 shadow-sm w-64 text-center">
        <div className="flex items-center justify-center gap-1.5 text-xs font-semibold text-primary uppercase mb-1 tracking-wider">
          <Zap className="h-3.5 w-3.5" /> Trigger
        </div>
        <div className="text-sm font-medium text-foreground">{triggerLabel(graph.trigger)}</div>
      </div>

      {graph.steps.length === 0 ? (
        <>
          <Connector />
          <div className="rounded-bn border border-bn-border bg-content-box px-4 py-3 shadow-sm w-64 text-center text-xs text-icon-muted italic">
            No steps configured
          </div>
        </>
      ) : (
        graph.steps.map((step) => (
          <React.Fragment key={step.id}>
            <Connector />
            <div className="rounded-bn border border-bn-border bg-content-box px-4 py-3 shadow-bn w-64 transition-colors hover:border-primary/40">
              <div className="font-semibold text-sm text-foreground">{step.name}</div>
              <span className="inline-block mt-1.5 rounded-bn bg-content-bg border border-bn-border px-2 py-0.5 text-xs text-foreground font-mono">
                {actionLabel(step.action)}
              </span>
              {step.condition && (
                <div className="mt-1.5 text-xs text-icon-muted">
                  if {step.condition.field} {step.condition.operator} {String(step.condition.value)}
                </div>
              )}
              <div className="mt-1 text-[11px] text-icon-muted font-medium">Policy: {failureLabel(step.on_failure)}</div>
            </div>
          </React.Fragment>
        ))
      )}
    </div>
  )
}
