import React, { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { 
  Search, 
  Plus, 
  ChevronLeft, 
  ChevronRight, 
  RefreshCw, 
  Trash2, 
  Eye, 
  Layers,
  Image as ImageIcon
} from 'lucide-react'

import { SchemaMetadata, ModelMetadata, EntityData } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { DetailDrawer } from '../DetailDrawer'
import { getFieldLabel, formatDate } from '../../lib/utils'
import { canPerform } from '../../lib/permissions'

interface EntityListViewProps {
  modelName: string
  modelMetadata: ModelMetadata
  schema: SchemaMetadata
}

export function EntityListView({ modelName, modelMetadata, schema }: EntityListViewProps) {
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(15)
  const [searchTerm, setSearchTerm] = useState('')
  const [selectedRecord, setSelectedRecord] = useState<EntityData | null>(null)
  const [isDrawerOpen, setIsDrawerOpen] = useState(false)

  const role = apiClient.currentUser?.role
  const canCreate = canPerform(modelMetadata, 'create', role)
  const canDelete = canPerform(modelMetadata, 'delete', role)

  // Fields to display in list view
  const visibleFields = useMemo(() => {
    if (modelMetadata.ui?.listView && modelMetadata.ui.listView.length > 0) {
      return modelMetadata.ui.listView
    }
    return Object.keys(modelMetadata.fields).slice(0, 6)
  }, [modelMetadata])

  // Determine search target
  const searchField = useMemo(() => {
    const candidates = modelMetadata.searchable || []
    if (candidates.length > 0) return candidates[0]
    const stringFields = Object.entries(modelMetadata.fields)
      .filter(([_, f]) => f.type === 'string' && f.name !== 'id')
      .map(([k]) => k)
    return stringFields[0] || 'name'
  }, [modelMetadata])

  // Data query
  const { data, isLoading, refetch } = useQuery<{ data: EntityData[]; total: number }>({
    queryKey: ['entity-list', modelName, page, pageSize, searchTerm],
    queryFn: async () => {
      const res = await apiClient.listEntities(modelName, {
        page,
        limit: pageSize,
        search: searchTerm.trim() || undefined,
        searchField,
      })
      return res
    },
  })

  const records = data?.data || []
  const total = data?.total || 0
  const totalPages = Math.ceil(total / pageSize) || 1

  const handleOpenDetail = (rec: EntityData) => {
    setSelectedRecord(rec)
    setIsDrawerOpen(true)
  }

  const handleDelete = async (e: React.MouseEvent, id: string) => {
    e.stopPropagation()
    if (!window.confirm(`Are you sure you want to delete this record (${id})?`)) return
    try {
      await apiClient.deleteEntity(modelName, id)
      refetch()
    } catch (err: any) {
      alert(err?.message || 'Delete failed')
    }
  }

  return (
    <div className="p-4 sm:p-6 lg:p-8 space-y-6 max-w-7xl mx-auto animate-fade-in">
      {/* Top Header Card */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <div className="flex items-center space-x-3">
            <h1 className="text-2xl font-bold text-foreground tracking-tight">
              {getFieldLabel(modelName, modelMetadata.tableName)}
            </h1>
            <span className="px-2.5 py-0.5 text-xs font-semibold rounded-full bg-primary/10 text-primary border border-primary/20">
              {total} records
            </span>
          </div>
          <p className="text-sm text-icon-muted mt-1">
            Table: <code className="font-mono text-xs">{modelMetadata.tableName || modelName}</code>
          </p>
        </div>

        {/* Action Controls */}
        <div className="flex items-center space-x-3">
          <button
            onClick={() => refetch()}
            className="p-2.5 text-icon-muted hover:text-foreground bg-content-box border border-bn-border rounded-bn hover:bg-content-bg transition-colors shadow-sm"
            title="Refresh Table"
          >
            <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} />
          </button>

          {canCreate && (
            <button
              onClick={() => handleOpenDetail({ id: '' } as EntityData)}
              className="flex items-center space-x-2 px-4 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-bn font-medium text-sm transition-all shadow-bn"
            >
              <Plus className="w-4 h-4" />
              <span>Create {modelName}</span>
            </button>
          )}
        </div>
      </div>

      {/* Table & Controls Container */}
      <div className="bg-content-box rounded-bn border border-bn-border shadow-bn overflow-hidden">
        {/* Search Bar */}
        <div className="p-4 border-b border-bn-border bg-content-bg/30 flex items-center justify-between gap-4">
          <div className="relative flex-1 max-w-md">
            <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-icon-muted" />
            <input
              type="text"
              value={searchTerm}
              onChange={(e) => {
                setSearchTerm(e.target.value)
                setPage(1)
              }}
              placeholder={`Search ${modelName} by ${searchField}...`}
              className="w-full pl-9 pr-4 py-2 bg-content-box border border-bn-border rounded-bn text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-primary/20 transition-all placeholder:text-icon-muted"
            />
          </div>
        </div>

        {/* Main Table */}
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="border-b border-bn-border bg-content-bg/50">
                {visibleFields.map((field) => (
                  <th
                    key={field}
                    className="px-4 py-3 text-xs font-semibold text-icon-muted uppercase tracking-wider whitespace-nowrap"
                  >
                    {field}
                  </th>
                ))}
                <th className="px-4 py-3 text-xs font-semibold text-icon-muted uppercase tracking-wider text-right">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-bn-border">
              {isLoading && records.length === 0 ? (
                <tr>
                  <td colSpan={visibleFields.length + 1} className="py-12 text-center text-icon-muted text-sm">
                    Loading {modelName} records…
                  </td>
                </tr>
              ) : records.length === 0 ? (
                <tr>
                  <td colSpan={visibleFields.length + 1} className="py-12 text-center text-icon-muted text-sm">
                    No records found
                  </td>
                </tr>
              ) : (
                records.map((row: EntityData) => {
                  const id = String(row[modelMetadata.primaryKey || 'id'] || '')
                  return (
                    <tr
                      key={id}
                      onClick={() => handleOpenDetail(row)}
                      className="hover:bg-primary/5 transition-colors cursor-pointer group"
                    >
                      {visibleFields.map((field) => {
                        const val = row[field]
                        const isFile = field.toLowerCase().includes('mediaid')
                        const isStatus = field.toLowerCase() === 'status'
                        const isDate = field.toLowerCase().includes('createdat') || field.toLowerCase().includes('updatedat')

                        return (
                          <td key={field} className="px-4 py-3 text-sm text-foreground max-w-xs truncate">
                            {val === null || val === undefined ? (
                              <span className="text-icon-muted italic text-xs">—</span>
                            ) : isStatus ? (
                              <span
                                className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium capitalize ${
                                  String(val) === 'done' || String(val) === 'active'
                                    ? 'bg-success/15 text-success border border-success/30'
                                    : String(val) === 'pending' || String(val) === 'processing'
                                    ? 'bg-warning/15 text-warning border border-warning/30'
                                    : String(val) === 'failed'
                                    ? 'bg-danger/15 text-danger border border-danger/30'
                                    : 'bg-primary/10 text-primary border border-primary/20'
                                }`}
                              >
                                {String(val)}
                              </span>
                            ) : isFile ? (
                              <div className="flex items-center space-x-2">
                                <div className="w-8 h-8 rounded bg-content-bg border border-bn-border overflow-hidden flex items-center justify-center flex-shrink-0">
                                  <img
                                    src={`/media/${val}`}
                                    alt="thumb"
                                    className="w-full h-full object-cover"
                                    onError={(e) => {
                                      (e.target as HTMLElement).style.display = 'none'
                                    }}
                                  />
                                  <ImageIcon className="w-4 h-4 text-icon-muted" />
                                </div>
                                <span className="font-mono text-xs truncate max-w-[100px]">{String(val)}</span>
                              </div>
                            ) : isDate ? (
                              <span className="text-xs text-icon-muted">{formatDate(val)}</span>
                            ) : typeof val === 'object' ? (
                              <span className="font-mono text-xs text-icon-muted">{JSON.stringify(val).slice(0, 30)}</span>
                            ) : (
                              <span>{String(val)}</span>
                            )}
                          </td>
                        )
                      })}

                      {/* Action Column */}
                      <td className="px-4 py-3 text-right whitespace-nowrap">
                        <div className="flex items-center justify-end space-x-2" onClick={(e) => e.stopPropagation()}>
                          <button
                            onClick={() => handleOpenDetail(row)}
                            className="p-1.5 text-icon-muted hover:text-primary rounded hover:bg-content-bg transition-colors"
                            title="View Details"
                          >
                            <Eye className="w-4 h-4" />
                          </button>
                          {canDelete && (
                            <button
                              onClick={(e) => handleDelete(e, id)}
                              className="p-1.5 text-icon-muted hover:text-danger rounded hover:bg-content-bg transition-colors"
                              title="Delete Record"
                            >
                              <Trash2 className="w-4 h-4" />
                            </button>
                          )}
                        </div>
                      </td>
                    </tr>
                  )
                })
              )}
            </tbody>
          </table>
        </div>

        {/* Pagination Bar */}
        <div className="p-4 border-t border-bn-border bg-content-bg/30 flex items-center justify-between text-xs text-icon-muted">
          <div>
            Showing <span className="font-medium text-foreground">{(page - 1) * pageSize + 1}</span> to{' '}
            <span className="font-medium text-foreground">{Math.min(page * pageSize, total)}</span> of{' '}
            <span className="font-medium text-foreground">{total}</span>
          </div>

          <div className="flex items-center space-x-2">
            <button
              onClick={() => setPage((p) => Math.max(1, p - 1))}
              disabled={page <= 1}
              className="p-2 rounded-bn border border-bn-border bg-content-box hover:bg-content-bg disabled:opacity-40 transition-colors"
            >
              <ChevronLeft className="w-4 h-4" />
            </button>
            <span className="px-2 font-medium text-foreground">
              Page {page} of {totalPages}
            </span>
            <button
              onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
              disabled={page >= totalPages}
              className="p-2 rounded-bn border border-bn-border bg-content-box hover:bg-content-bg disabled:opacity-40 transition-colors"
            >
              <ChevronRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      {/* Slide-out Detail Drawer */}
      <DetailDrawer
        modelName={modelName}
        modelMetadata={modelMetadata}
        record={selectedRecord}
        isOpen={isDrawerOpen}
        onClose={() => setIsDrawerOpen(false)}
      />
    </div>
  )
}
