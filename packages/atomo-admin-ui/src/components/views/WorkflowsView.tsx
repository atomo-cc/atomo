/**
 * Workflows View — manage and run workflows via the backend REST API.
 *
 * Backend: GET /workflows, POST /workflows, POST /workflows/{name}/run
 */

import React, { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { Workflow as WorkflowIcon, Play, Plus, Trash2, Edit } from 'lucide-react'
import { apiClient } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Textarea } from '../ui/Textarea'
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
      <div className="flex items-center gap-3">
        <WorkflowIcon className="h-7 w-7 text-primary-600" />
        <div>
          <h1 className="text-3xl font-bold text-gray-900">Workflows</h1>
          <p className="text-gray-600 mt-1">Register, view, and manually run workflows</p>
        </div>
        <Button className="ml-auto" onClick={() => navigate('/workflows/design')}>
          <Plus className="h-4 w-4 mr-1" /> New in Designer
        </Button>
      </div>

      {error && (
        <div className="rounded-md bg-danger-50 border border-danger-500 px-4 py-3 text-sm text-danger-700">
          {error}
        </div>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Registered Workflows</CardTitle>
          <CardDescription>Event-triggered workflows run automatically; you can also run them manually here.</CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <Spinner />
          ) : workflows.length === 0 ? (
            <p className="text-gray-500 text-sm">No workflows registered yet.</p>
          ) : (
            <ul className="divide-y divide-gray-200">
              {workflows.map((name) => (
                <li key={name} className="flex items-center justify-between py-3">
                  <span className="font-medium text-gray-900">{name}</span>
                  <div className="flex items-center gap-2">
                    <Button size="sm" variant="ghost" onClick={() => handleEdit(name)}>
                      <Edit className="h-4 w-4 mr-1" /> Edit
                    </Button>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => run.mutate(name)}
                      disabled={run.isPending}
                    >
                      <Play className="h-4 w-4 mr-1" /> Run
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      onClick={() => remove.mutate(name)}
                      disabled={remove.isPending}
                    >
                      <Trash2 className="h-4 w-4 mr-1" /> Delete
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>

      {lastRun && (
        <Card>
          <CardHeader>
            <CardTitle>Last Run Result</CardTitle>
          </CardHeader>
          <CardContent>
            <pre className="text-xs bg-gray-50 rounded-md p-3 overflow-auto">
              {JSON.stringify(lastRun, null, 2)}
            </pre>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Register Workflow</CardTitle>
          <CardDescription>Paste a workflow JSON definition (trigger / steps).</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Textarea
            value={definition}
            onChange={(e) => setDefinition(e.target.value)}
            rows={14}
            className="font-mono text-xs"
          />
          <Button onClick={handleRegister} disabled={register.isPending}>
            <Plus className="h-4 w-4 mr-1" /> Register
          </Button>
        </CardContent>
      </Card>
    </div>
  )
}
