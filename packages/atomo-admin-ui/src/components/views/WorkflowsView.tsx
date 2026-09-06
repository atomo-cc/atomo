/**
 * Workflows View — Dashin-styled Workflow Orchestration Management
 *
 * Backend: GET /workflows, POST /workflows, POST /workflows/{name}/run
 */

import React, { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { Workflow as WorkflowIcon, Play, Plus, Trash2, Edit, AlertCircle, Terminal } from 'lucide-react'
import { apiClient } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Textarea } from '../ui/Textarea'
import { Badge } from '../ui/Badge'
import { Spinner } from '../ui/Spinner'

const SAMPLE = JSON.stringify(
  {
    name: 'notify-on-new-contact',
    trigger: { OnEvent: { model: 'Contact', event_type: 'Created' } },
    steps: [
      {
        name: 'set-flag',
        action: { SetVariable: { key: 'notified', value: true } },
        condition: null,
        on_failure: 'Continue',
      },
    ],
  },
  null,
  2,
)

export function WorkflowsView() {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const [definition, setDefinition] = useState(SAMPLE)
  const [error, setError] = useState<string | null>(null)
  const [lastRun, setLastRun] = useState<any>(null)

  const { data: workflows = [], isLoading } = useQuery({
    queryKey: ['workflows'],
    queryFn: () => apiClient.listWorkflows(),
  })

  const register = useMutation({
    mutationFn: (wf: any) => apiClient.registerWorkflow(wf),
    onSuccess: () => {
      setError(null)
      queryClient.invalidateQueries({ queryKey: ['workflows'] })
    },
    onError: (e: any) => setError(e?.message || 'Failed to register workflow'),
  })

  const run = useMutation({
    mutationFn: (name: string) => apiClient.runWorkflow(name, {}),
    onSuccess: (res) => setLastRun(res),
    onError: (e: any) => setError(e?.message || 'Failed to run workflow'),
  })

  const remove = useMutation({
    mutationFn: (name: string) => apiClient.deleteWorkflow(name),
    onSuccess: () => {
      setError(null)
      queryClient.invalidateQueries({ queryKey: ['workflows'] })
    },
    onError: (e: any) => setError(e?.message || 'Failed to delete workflow'),
  })

  const handleEdit = async (name: string) => {
    try {
      const wf = await apiClient.getWorkflow(name)
      setDefinition(JSON.stringify(wf, null, 2))
      setError(null)
    } catch (e: any) {
      setError(e?.message || 'Failed to load workflow')
    }
  }

  const handleRegister = () => {
    try {
      const parsed = JSON.parse(definition)
      register.mutate(parsed)
    } catch {
      setError('Invalid JSON in workflow definition')
    }
  }

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-bn bg-primary/10 flex items-center justify-center text-primary">
            <WorkflowIcon className="h-5 w-5" />
          </div>
          <div>
            <h1 className="text-2xl font-bold tracking-tight text-foreground">Workflows</h1>
            <p className="text-xs text-icon-muted mt-0.5">Register, orchestrate, and manually execute declarative workflows</p>
          </div>
        </div>
        <Button size="sm" onClick={() => navigate('/workflows/design')}>
          <Plus className="h-3.5 w-3.5 mr-1.5" />
          New in Designer
        </Button>
      </div>

      {error && (
        <div className="flex items-center gap-2 rounded-bn bg-rose-500/10 border border-rose-500/20 px-4 py-3 text-xs text-rose-600 dark:text-rose-400 font-medium">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      {/* Registered Workflows */}
      <Card>
        <CardHeader className="py-4 border-b border-bn-border/60">
          <CardTitle>Registered Workflows</CardTitle>
          <CardDescription>Event-triggered workflows run automatically; you can also test and execute them manually.</CardDescription>
        </CardHeader>
        <CardContent className="p-4">
          {isLoading ? (
            <div className="py-8 text-center"><Spinner /></div>
          ) : workflows.length === 0 ? (
            <div className="py-8 text-center text-xs text-icon-muted">No workflows registered yet. Use the designer or JSON editor below to add one.</div>
          ) : (
            <div className="divide-y divide-bn-border/60">
              {workflows.map((name) => (
                <div key={name} className="flex items-center justify-between py-3 px-2 rounded-bn hover:bg-content-bg/50 transition-colors">
                  <div className="flex items-center gap-2">
                    <Badge variant="secondary" className="font-mono text-xs font-medium">workflow</Badge>
                    <span className="font-medium text-sm text-foreground">{name}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Button size="sm" variant="ghost" onClick={() => handleEdit(name)}>
                      <Edit className="h-3.5 w-3.5 mr-1" /> Edit
                    </Button>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => run.mutate(name)}
                      disabled={run.isPending}
                    >
                      <Play className="h-3.5 w-3.5 mr-1 text-primary" /> Run
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      onClick={() => remove.mutate(name)}
                      disabled={remove.isPending}
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Last Run Output */}
      {lastRun && (
        <Card>
          <CardHeader className="py-4 border-b border-bn-border/60">
            <div className="flex items-center gap-2">
              <Terminal className="h-4 w-4 text-primary" />
              <CardTitle>Execution Result</CardTitle>
            </div>
          </CardHeader>
          <CardContent className="p-4">
            <pre className="text-xs bg-content-bg border border-bn-border rounded-bn p-4 overflow-auto font-mono text-foreground leading-relaxed">
              {JSON.stringify(lastRun, null, 2)}
            </pre>
          </CardContent>
        </Card>
      )}

      {/* Register / Update Workflow via JSON */}
      <Card>
        <CardHeader className="py-4 border-b border-bn-border/60">
          <CardTitle>Register Workflow (JSON)</CardTitle>
          <CardDescription>Paste or edit a declarative workflow JSON definition (trigger / pipeline steps).</CardDescription>
        </CardHeader>
        <CardContent className="p-5 space-y-4">
          <Textarea
            value={definition}
            onChange={(e) => setDefinition(e.target.value)}
            rows={12}
            className="font-mono text-xs leading-relaxed"
          />
          <div className="flex justify-end">
            <Button size="sm" onClick={handleRegister} disabled={register.isPending}>
              <Plus className="h-3.5 w-3.5 mr-1" /> Register Workflow
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
