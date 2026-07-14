/**
 * Observability — queue health + recent activity, every number from a real
 * endpoint (GET /jobs/stats, /jobs/recent, /audit/logs). Admin-gated on the
 * server; a non-admin sees the 403 state, not fake data. This replaces the
 * old mock ObservabilityCenter, which rendered Math.random() metrics.
 */

import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Activity, AlertTriangle, RefreshCw } from 'lucide-react'

import { apiClient } from '../../lib/api'
import { Card, CardContent, CardHeader, CardTitle } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../ui/Select'
import { formatDate } from '../../lib/utils'

const REFRESH_MS = 10_000
const JOB_STATUSES = ['queued', 'running', 'succeeded', 'failed', 'dead'] as const

function statusVariant(status: string): 'default' | 'secondary' | 'destructive' {
  if (status === 'failed' || status === 'dead') return 'destructive'
  if (status === 'succeeded') return 'secondary'
  return 'default'
}

function isForbidden(error: unknown): boolean {
  return (error as any)?.response?.status === 403
}

export function ObservabilityView() {
  const [statusFilter, setStatusFilter] = useState<string>('all')

  const stats = useQuery({
    queryKey: ['job-stats'],
    queryFn: () => apiClient.getJobStats(),
    refetchInterval: REFRESH_MS,
    retry: false,
  })

  const recent = useQuery({
    queryKey: ['recent-jobs', statusFilter],
    queryFn: () =>
      apiClient.listRecentJobs({
        status: statusFilter === 'all' ? undefined : statusFilter,
        limit: 25,
      }),
    refetchInterval: REFRESH_MS,
    retry: false,
  })

  const audit = useQuery({
    queryKey: ['audit-recent'],
    queryFn: () => apiClient.getAuditLogs(15),
    refetchInterval: REFRESH_MS,
    retry: false,
  })

  if (isForbidden(stats.error)) {
    return (
      <Card className="m-6">
        <CardContent className="py-8 text-center text-gray-600">
          Observability is admin-only. Sign in with an admin account to view queue health.
        </CardContent>
      </Card>
    )
  }

  const byStatus = stats.data?.byStatus ?? {}
  const oldestQueued = stats.data?.oldestQueuedSeconds

  return (
    <div className="p-6 space-y-6">
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-3xl font-bold text-gray-900 flex items-center gap-2">
            <Activity className="h-7 w-7" />
            Observability
          </h1>
          <p className="text-gray-600 mt-1">
            Job queue health and recent activity — refreshes every {REFRESH_MS / 1000}s
          </p>
        </div>
        <Button
          variant="secondary"
          onClick={() => {
            stats.refetch()
            recent.refetch()
            audit.refetch()
          }}
        >
          <RefreshCw className="h-4 w-4 mr-2" />
          Refresh
        </Button>
      </div>

      {/* Queue-health tiles */}
      <div className="grid grid-cols-2 md:grid-cols-5 gap-4">
        {JOB_STATUSES.map((s) => (
          <Card key={s}>
            <CardContent className="p-4">
              <div className="text-xs text-gray-500 uppercase tracking-wider">{s}</div>
              <div
                className={`text-2xl font-bold ${
                  (s === 'failed' || s === 'dead') && (byStatus[s] ?? 0) > 0
                    ? 'text-red-600'
                    : 'text-gray-900'
                }`}
              >
                {stats.isLoading ? '—' : byStatus[s] ?? 0}
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Oldest-queued warning: a growing age means no worker is draining the queue. */}
      {typeof oldestQueued === 'number' && oldestQueued > 60 && (
        <div className="flex items-center gap-2 rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-800">
          <AlertTriangle className="h-4 w-4 shrink-0" />
          Oldest queued job has been waiting {Math.round(oldestQueued / 60)} min — is a worker
          running?
        </div>
      )}

      {/* Recent jobs */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>Recent jobs</CardTitle>
          <Select value={statusFilter} onValueChange={setStatusFilter}>
            <SelectTrigger className="w-36">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All statuses</SelectItem>
              {JOB_STATUSES.map((s) => (
                <SelectItem key={s} value={s}>
                  {s}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </CardHeader>
        <CardContent className="p-0">
          {recent.isLoading ? (
            <div className="p-6 text-center text-gray-500">Loading…</div>
          ) : (recent.data?.jobs?.length ?? 0) === 0 ? (
            <div className="p-6 text-center text-gray-500">No jobs yet</div>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b bg-gray-50 text-left text-xs uppercase tracking-wider text-gray-500">
                  <th className="px-4 py-2">Kind</th>
                  <th className="px-4 py-2">Queue</th>
                  <th className="px-4 py-2">Status</th>
                  <th className="px-4 py-2">Attempts</th>
                  <th className="px-4 py-2">Created</th>
                  <th className="px-4 py-2">Error</th>
                </tr>
              </thead>
              <tbody>
                {recent.data!.jobs.map((j) => (
                  <tr key={j.id} className="border-b hover:bg-gray-50">
                    <td className="px-4 py-2 font-medium">{j.kind}</td>
                    <td className="px-4 py-2 text-gray-600">{j.queue}</td>
                    <td className="px-4 py-2">
                      <Badge variant={statusVariant(j.status) as any}>{j.status}</Badge>
                    </td>
                    <td className="px-4 py-2 text-gray-600">
                      {j.attempts}/{j.maxAttempts}
                    </td>
                    <td className="px-4 py-2 text-gray-600">{formatDate(j.createdAt, 'time')}</td>
                    <td className="px-4 py-2 text-red-700 max-w-xs truncate" title={j.error ?? ''}>
                      {j.error ?? ''}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </CardContent>
      </Card>

      {/* Recent audit activity */}
      <Card>
        <CardHeader>
          <CardTitle>Recent audit activity</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          {audit.isLoading ? (
            <div className="p-6 text-center text-gray-500">Loading…</div>
          ) : isForbidden(audit.error) || (audit.data?.length ?? 0) === 0 ? (
            <div className="p-6 text-center text-gray-500">No audit entries</div>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b bg-gray-50 text-left text-xs uppercase tracking-wider text-gray-500">
                  <th className="px-4 py-2">Entity</th>
                  <th className="px-4 py-2">Operation</th>
                  <th className="px-4 py-2">Actor</th>
                  <th className="px-4 py-2">When</th>
                </tr>
              </thead>
              <tbody>
                {audit.data!.map((e: any) => (
                  <tr key={e.id} className="border-b hover:bg-gray-50">
                    <td className="px-4 py-2 font-medium">{e.entity_type ?? e.entityType}</td>
                    <td className="px-4 py-2 text-gray-600">
                      {String(e.operation ?? '').toString()}
                    </td>
                    <td className="px-4 py-2 text-gray-600">{e.user_id ?? e.userId ?? '—'}</td>
                    <td className="px-4 py-2 text-gray-600">
                      {formatDate(e.created_at ?? e.createdAt, 'time')}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
