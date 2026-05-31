/**
 * Workflows View — manage and run workflows via the backend REST API.
 *
 * Backend: GET /workflows, POST /workflows, POST /workflows/{name}/run
 */

import React, { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Workflow as WorkflowIcon, Play, Plus, Trash2 } from 'lucide-react'
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
          <h1 className="text-3xl font-bold text-gray-900">工作流</h1>
          <p className="text-gray-600 mt-1">注册、查看并手动运行工作流</p>
        </div>
      </div>

      {error && (
        <div className="rounded-md bg-danger-50 border border-danger-500 px-4 py-3 text-sm text-danger-700">
          {error}
        </div>
      )}

      <Card>
        <CardHeader>
          <CardTitle>已注册的工作流</CardTitle>
          <CardDescription>事件触发的工作流会自动运行；也可在此手动运行。</CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <Spinner />
          ) : workflows.length === 0 ? (
            <p className="text-gray-500 text-sm">暂无已注册的工作流。</p>
          ) : (
            <ul className="divide-y divide-gray-200">
              {workflows.map((name) => (
                <li key={name} className="flex items-center justify-between py-3">
                  <span className="font-medium text-gray-900">{name}</span>
                  <div className="flex items-center gap-2">
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => run.mutate(name)}
                      disabled={run.isPending}
                    >
                      <Play className="h-4 w-4 mr-1" /> 运行
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      onClick={() => remove.mutate(name)}
                      disabled={remove.isPending}
                    >
                      <Trash2 className="h-4 w-4 mr-1" /> 删除
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
            <CardTitle>上次运行结果</CardTitle>
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
          <CardTitle>注册工作流</CardTitle>
          <CardDescription>粘贴工作流 JSON 定义（trigger / steps）。</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Textarea
            value={definition}
            onChange={(e) => setDefinition(e.target.value)}
            rows={14}
            className="font-mono text-xs"
          />
          <Button onClick={handleRegister} disabled={register.isPending}>
            <Plus className="h-4 w-4 mr-1" /> 注册
          </Button>
        </CardContent>
      </Card>
    </div>
  )
}
