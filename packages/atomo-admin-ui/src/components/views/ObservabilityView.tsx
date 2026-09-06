/**
 * Observability — Dashin-styled Queue Health and Audit Activity Panel
 *
 * Real endpoints: GET /jobs/stats, /jobs/recent, /audit/logs.
 */

import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Activity, AlertTriangle, RefreshCw, CheckCircle2, Clock, PlayCircle, XCircle, Skull } from 'lucide-react'

import { apiClient } from '../../lib/api'
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../ui/Select'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table'
import { formatDate } from '../../lib/utils'

const REFRESH_MS = 10_000
const JOB_STATUSES = ['queued', 'running', 'succeeded', 'failed', 'dead'] as const

function getStatusIcon(status: string) {
  switch (status) {
    case 'queued': return <Clock className="h-4 w-4 text-amber-500" />
    case 'running': return <PlayCircle className="h-4 w-4 text-blue-500" />
    case 'succeeded': return <CheckCircle2 className="h-4 w-4 text-emerald-500" />
    case 'failed': return <XCircle className="h-4 w-4 text-rose-500" />
    case 'dead': return <Skull className="h-4 w-4 text-rose-600" />
    default: return null
  }
}

function statusBadgeVariant(status: string): 'default' | 'secondary' | 'success' | 'danger' | 'warning' {
  if (status === 'failed' || status === 'dead') return 'danger'
  if (status === 'succeeded') return 'success'
  if (status === 'running') return 'default'
  if (status === 'queued') return 'warning'
  return 'secondary'
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
      <div className="p-6">
        <Card>
          <CardContent className="py-12 text-center text-icon-muted">
            <AlertTriangle className="h-8 w-8 text-amber-500 mx-auto mb-3" />
            <h3 className="text-base font-semibold text-foreground mb-1">Access Restricted</h3>
            <p className="text-xs">Observability is admin-only. Sign in with an admin account to view queue metrics.</p>
          </CardContent>
        </Card>
      </div>
    )
  }

  const byStatus = stats.data?.byStatus ?? {}
  const oldestQueued = stats.data?.oldestQueuedSeconds

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 rounded-bn bg-primary/10 flex items-center justify-center text-primary">
              <Activity className="h-4 w-4" />
            </div>
            <h1 className="text-2xl font-bold tracking-tight text-foreground">Observability</h1>
          </div>
          <p className="text-xs text-icon-muted mt-1">
            Job queue health, worker throughput, and real-time audit logs — auto-refreshes every {REFRESH_MS / 1000}s
          </p>
        </div>
        <Button
          variant="secondary"
          size="sm"
          onClick={() => {
            stats.refetch()
            recent.refetch()
            audit.refetch()
          }}
        >
          <RefreshCw className="h-3.5 w-3.5 mr-1.5" />
          Refresh
        </Button>
      </div>

      {/* Queue-health KPI tiles */}
      <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
        {JOB_STATUSES.map((s) => {
          const count = byStatus[s] ?? 0
          const isProblem = (s === 'failed' || s === 'dead') && count > 0
          return (
            <Card key={s} className="hover:border-primary/30 transition-colors">
              <CardContent className="p-4 flex items-center justify-between">
                <div>
                  <div className="text-[11px] font-semibold text-icon-muted uppercase tracking-wider">{s}</div>
                  <div className={`text-2xl font-bold tracking-tight mt-0.5 ${isProblem ? 'text-rose-500' : 'text-foreground'}`}>
                    {stats.isLoading ? '—' : count}
                  </div>
                </div>
                <div className="p-2 rounded-bn bg-content-bg border border-bn-border">
                  {getStatusIcon(s)}
                </div>
              </CardContent>
            </Card>
          )
        })}
      </div>

      {/* Oldest-queued warning */}
      {typeof oldestQueued === 'number' && oldestQueued > 60 && (
        <div className="flex items-center gap-2.5 rounded-bn border border-amber-500/30 bg-amber-500/10 px-4 py-3 text-xs text-amber-700 dark:text-amber-400 font-medium">
          <AlertTriangle className="h-4 w-4 shrink-0" />
          Oldest queued job has been waiting {Math.round(oldestQueued / 60)} min — is a background worker running?
        </div>
      )}

      {/* Recent jobs */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between py-4 border-b border-bn-border/60">
          <div>
            <CardTitle>Recent Jobs</CardTitle>
            <CardDescription>Execution status and retry attempts of background tasks</CardDescription>
          </div>
          <Select value={statusFilter} onValueChange={setStatusFilter}>
            <SelectTrigger className="w-36 h-8 text-xs">
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
            <div className="p-8 text-center text-xs text-icon-muted">Loading jobs…</div>
          ) : (recent.data?.jobs?.length ?? 0) === 0 ? (
            <div className="p-8 text-center text-xs text-icon-muted">No background jobs found.</div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Kind</TableHead>
                  <TableHead>Queue</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Attempts</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead>Error</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {recent.data!.jobs.map((j) => (
                  <TableRow key={j.id}>
                    <TableCell className="font-medium">{j.kind}</TableCell>
                    <TableCell className="text-icon-muted">{j.queue}</TableCell>
                    <TableCell>
                      <Badge variant={statusBadgeVariant(j.status)}>{j.status}</Badge>
                    </TableCell>
                    <TableCell className="text-icon-muted">
                      {j.attempts}/{j.maxAttempts}
                    </TableCell>
                    <TableCell className="text-icon-muted">{formatDate(j.createdAt, 'time')}</TableCell>
                    <TableCell className="text-rose-600 dark:text-rose-400 max-w-xs truncate" title={j.error ?? ''}>
                      {j.error || '—'}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {/* Recent audit activity */}
      <Card>
        <CardHeader className="py-4 border-b border-bn-border/60">
          <CardTitle>Recent Audit Activity</CardTitle>
          <CardDescription>Security-sensitive operations and administrative actions log</CardDescription>
        </CardHeader>
        <CardContent className="p-0">
          {audit.isLoading ? (
            <div className="p-8 text-center text-xs text-icon-muted">Loading audit entries…</div>
          ) : isForbidden(audit.error) || (audit.data?.length ?? 0) === 0 ? (
            <div className="p-8 text-center text-xs text-icon-muted">No audit entries recorded yet.</div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Entity</TableHead>
                  <TableHead>Operation</TableHead>
                  <TableHead>Actor</TableHead>
                  <TableHead>When</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {audit.data!.map((e: any) => (
                  <TableRow key={e.id}>
                    <TableCell className="font-medium">{e.entity_type ?? e.entityType}</TableCell>
                    <TableCell>
                      <Badge variant="secondary">{String(e.operation ?? '').toString()}</Badge>
                    </TableCell>
                    <TableCell className="text-icon-muted">{e.user_id ?? e.userId ?? '—'}</TableCell>
                    <TableCell className="text-icon-muted">
                      {formatDate(e.created_at ?? e.createdAt, 'time')}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
