/**
 * Trash View — list soft-deleted records per model, restore or permanently purge them.
 *
 * Backend: query deletedRecords; mutations restore / hardDelete.
 */

import React, { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Trash2, RotateCcw, XCircle } from 'lucide-react'
import { apiClient } from '../../lib/api'
import { SchemaMetadata } from '../../lib/types'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
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
    onError: (e: any) => setError(e?.message || 'Failed to restore'),
  })

  const purge = useMutation({
    mutationFn: (id: string) => apiClient.hardDeleteEntity(model, id),
    onSuccess: () => {
      setError(null)
      queryClient.invalidateQueries({ queryKey: ['deleted', model] })
    },
    onError: (e: any) => setError(e?.message || 'Failed to purge'),
  })

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center gap-3">
        <Trash2 className="h-7 w-7 text-primary-600" />
        <div>
          <h1 className="text-3xl font-bold text-gray-900">回收站</h1>
          <p className="text-gray-600 mt-1">查看已软删除的记录，可还原或永久删除</p>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <label className="text-sm font-medium text-gray-700">模型</label>
        <select
          value={model}
          onChange={(e) => setModel(e.target.value)}
          className="rounded-md border border-gray-300 px-3 py-1.5 text-sm"
        >
          {models.map((m) => (
            <option key={m} value={m}>{m}</option>
          ))}
        </select>
      </div>

      {error && (
        <div className="rounded-md bg-danger-50 border border-danger-500 px-4 py-3 text-sm text-danger-700">
          {error}
        </div>
      )}

      <Card>
        <CardHeader>
          <CardTitle>已删除的 {model}</CardTitle>
          <CardDescription>还原会恢复记录；永久删除不可撤销。</CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <Spinner />
          ) : deleted.length === 0 ? (
            <p className="text-gray-500 text-sm">回收站为空。</p>
          ) : (
            <ul className="divide-y divide-gray-200">
              {deleted.map((row: any) => (
                <li key={row.id} className="flex items-center justify-between py-3">
                  <span className="font-mono text-xs text-gray-700">{row.id}</span>
                  <div className="flex items-center gap-2">
                    <Button size="sm" variant="secondary" onClick={() => restore.mutate(row.id)} disabled={restore.isPending}>
                      <RotateCcw className="h-4 w-4 mr-1" /> 还原
                    </Button>
                    <Button size="sm" variant="danger" onClick={() => purge.mutate(row.id)} disabled={purge.isPending}>
                      <XCircle className="h-4 w-4 mr-1" /> 永久删除
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
