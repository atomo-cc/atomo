/**
 * Trash View — Dashin-styled Soft-Delete Recovery and Purge Management
 */

import React, { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Trash2, RotateCcw, XCircle, AlertCircle } from 'lucide-react'
import { apiClient } from '../../lib/api'
import { SchemaMetadata } from '../../lib/types'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Spinner } from '../ui/Spinner'

interface TrashViewProps {
  schema: SchemaMetadata
}

export function TrashView({ schema }: TrashViewProps) {
  const queryClient = useQueryClient()
  const models = Object.keys(schema.models)
  const [model, setModel] = useState(models[0] || '')
  const [error, setError] = useState<string | null>(null)

  const { data: deleted = [], isLoading } = useQuery({
    queryKey: ['deleted', model],
    queryFn: () => apiClient.listDeleted(model),
    enabled: !!model,
  })

  const restore = useMutation({
    mutationFn: (id: string) => apiClient.restoreEntity(model, id),
    onSuccess: () => {
      setError(null)
      queryClient.invalidateQueries({ queryKey: ['deleted', model] })
    },
    onError: (e: any) => setError(e?.message || 'Failed to restore record'),
  })

  const purge = useMutation({
    mutationFn: (id: string) => apiClient.hardDeleteEntity(model, id),
    onSuccess: () => {
      setError(null)
      queryClient.invalidateQueries({ queryKey: ['deleted', model] })
    },
    onError: (e: any) => setError(e?.message || 'Failed to purge record permanently'),
  })

  return (
    <div className="p-6 space-y-6 max-w-4xl">
      {/* Header */}
      <div className="flex items-center gap-3">
        <div className="w-9 h-9 rounded-bn bg-rose-500/10 flex items-center justify-center text-rose-500">
          <Trash2 className="h-5 w-5" />
        </div>
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Trash & Recovery</h1>
          <p className="text-xs text-icon-muted mt-0.5">Inspect soft-deleted records, restore them into active state, or permanently purge</p>
        </div>
      </div>

      {/* Model filter */}
      <div className="flex items-center gap-3">
        <label className="text-xs font-semibold text-icon-muted uppercase tracking-wider">Select Model:</label>
        <select
          value={model}
          onChange={(e) => setModel(e.target.value)}
          className="flex h-9 rounded-bn border border-bn-border bg-content-box px-3 text-sm text-foreground shadow-sm focus:outline-none focus:ring-2 focus:ring-primary/40 focus:border-primary transition-colors min-w-[200px]"
        >
          {models.map((m) => (
            <option key={m} value={m}>{m}</option>
          ))}
        </select>
      </div>

      {error && (
        <div className="flex items-center gap-2 rounded-bn bg-rose-500/10 border border-rose-500/20 px-4 py-3 text-xs text-rose-600 dark:text-rose-400 font-medium">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      {/* Deleted records list */}
      <Card>
        <CardHeader className="py-4 border-b border-bn-border/60">
          <CardTitle>Deleted {model} Records</CardTitle>
          <CardDescription>Restoring reinstates the record in read/write views; purging permanently removes it from the database.</CardDescription>
        </CardHeader>
        <CardContent className="p-4">
          {isLoading ? (
            <div className="py-8 text-center"><Spinner /></div>
          ) : deleted.length === 0 ? (
            <div className="py-12 text-center text-xs text-icon-muted">
              Trash is empty for model <span className="font-semibold text-foreground">{model}</span>.
            </div>
          ) : (
            <div className="divide-y divide-bn-border/60">
              {deleted.map((row: any) => (
                <div key={row.id} className="flex items-center justify-between py-3 px-2 rounded-bn hover:bg-content-bg/50 transition-colors">
                  <div className="flex items-center gap-2">
                    <Badge variant="danger" className="text-xs font-mono">deleted</Badge>
                    <span className="font-mono text-xs text-foreground">{row.id}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => restore.mutate(row.id)}
                      disabled={restore.isPending}
                    >
                      <RotateCcw className="h-3.5 w-3.5 mr-1 text-emerald-500" /> Restore
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      onClick={() => purge.mutate(row.id)}
                      disabled={purge.isPending}
                    >
                      <XCircle className="h-3.5 w-3.5 mr-1" /> Purge
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
