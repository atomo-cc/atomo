/**
 * Table Settings - Table settings panel
 *
 * Supports showing/hiding columns, ordering, width adjustment, and more
 */

import React, { useState } from 'react'
import { 
  Settings, 
  Eye, 
  EyeOff, 
  Download, 
  Columns, 
  SortAsc,
  SortDesc,
  GripVertical,
  X,
  Check
} from 'lucide-react'

import { ColumnConfig } from '../../lib/types'
import { Button } from '../ui/Button'
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Switch } from '../ui/Switch'
import { Badge } from '../ui/Badge'
import { cn } from '../../lib/utils'

export interface TableColumn extends ColumnConfig {
  visible: boolean
  width?: number
  pinned?: 'left' | 'right' | null
  sortOrder?: number
}

interface TableSettingsProps {
  columns: TableColumn[]
  onColumnsChange: (columns: TableColumn[]) => void
  onExport: (format: 'csv' | 'excel') => void
  isOpen: boolean
  onClose: () => void
}

export function TableSettings({
  columns,
  onColumnsChange,
  onExport,
  isOpen,
  onClose
}: TableSettingsProps) {
  const [activeTab, setActiveTab] = useState<'columns' | 'export'>('columns')
  const [draggedColumn, setDraggedColumn] = useState<string | null>(null)

  if (!isOpen) return null

  const toggleColumnVisibility = (columnKey: string) => {
    const updatedColumns = columns.map(col => 
      col.key === columnKey 
        ? { ...col, visible: !col.visible }
        : col
    )
    onColumnsChange(updatedColumns)
  }

  const toggleColumnPin = (columnKey: string, pin: 'left' | 'right' | null) => {
    const updatedColumns = columns.map(col => 
      col.key === columnKey 
        ? { ...col, pinned: col.pinned === pin ? null : pin }
        : col
    )
    onColumnsChange(updatedColumns)
  }

  const updateColumnWidth = (columnKey: string, width: number) => {
    const updatedColumns = columns.map(col => 
      col.key === columnKey 
        ? { ...col, width }
        : col
    )
    onColumnsChange(updatedColumns)
  }

  const reorderColumns = (fromIndex: number, toIndex: number) => {
    const updatedColumns = [...columns]
    const [movedColumn] = updatedColumns.splice(fromIndex, 1)
    updatedColumns.splice(toIndex, 0, movedColumn)
    onColumnsChange(updatedColumns)
  }

  const resetToDefaults = () => {
    const resetColumns = columns.map(col => ({
      ...col,
      visible: true,
      width: undefined,
      pinned: null
    }))
    onColumnsChange(resetColumns)
  }

  const handleDragStart = (e: React.DragEvent, columnKey: string) => {
    setDraggedColumn(columnKey)
    e.dataTransfer.effectAllowed = 'move'
  }

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault()
    e.dataTransfer.dropEffect = 'move'
  }

  const handleDrop = (e: React.DragEvent, targetColumnKey: string) => {
    e.preventDefault()
    
    if (!draggedColumn || draggedColumn === targetColumnKey) {
      setDraggedColumn(null)
      return
    }

    const fromIndex = columns.findIndex(col => col.key === draggedColumn)
    const toIndex = columns.findIndex(col => col.key === targetColumnKey)

    if (fromIndex !== -1 && toIndex !== -1) {
      reorderColumns(fromIndex, toIndex)
    }

    setDraggedColumn(null)
  }

  const visibleColumnsCount = columns.filter(col => col.visible).length
  const pinnedLeftCount = columns.filter(col => col.pinned === 'left').length
  const pinnedRightCount = columns.filter(col => col.pinned === 'right').length

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
      <Card className="w-full max-w-2xl max-h-[85vh] overflow-hidden rounded-bn border border-bn-border bg-content-box text-foreground shadow-bn">
        <CardHeader className="border-b border-bn-border/60 py-4 px-6">
          <div className="flex items-center justify-between mb-3">
            <CardTitle className="flex items-center gap-2">
              <Settings className="h-4 w-4 text-primary" />
              Table Column Settings
            </CardTitle>
            <Button variant="ghost" size="sm" onClick={onClose} className="h-8 w-8 p-0">
              <X className="h-4 w-4" />
            </Button>
          </div>
          
          {/* Tab switcher */}
          <div className="flex gap-1 bg-content-bg p-1 rounded-bn border border-bn-border">
            <button
              onClick={() => setActiveTab('columns')}
              className={cn(
                'flex-1 px-3 py-1.5 text-xs font-medium rounded-bn transition-colors',
                activeTab === 'columns'
                  ? 'bg-content-box text-foreground shadow-sm'
                  : 'text-icon-muted hover:text-foreground'
              )}
            >
              <Columns className="h-3.5 w-3.5 mr-1.5 inline" />
              Visible Columns
            </button>
            <button
              onClick={() => setActiveTab('export')}
              className={cn(
                'flex-1 px-3 py-1.5 text-xs font-medium rounded-bn transition-colors',
                activeTab === 'export'
                  ? 'bg-content-box text-foreground shadow-sm'
                  : 'text-icon-muted hover:text-foreground'
              )}
            >
              <Download className="h-3.5 w-3.5 mr-1.5 inline" />
              Export Records
            </button>
          </div>
        </CardHeader>

        <CardContent className="p-0">
          {activeTab === 'columns' && (
            <div className="p-6 space-y-4 max-h-96 overflow-y-auto">
              {/* Summary info */}
              <div className="flex items-center justify-between text-xs">
                <div className="flex gap-4">
                  <span className="text-icon-muted">
                    Visible: <span className="font-semibold text-foreground">{visibleColumnsCount}/{columns.length}</span>
                  </span>
                  <span className="text-icon-muted">
                    Pinned: <span className="font-semibold text-foreground">Left {pinnedLeftCount} | Right {pinnedRightCount}</span>
                  </span>
                </div>
                <Button variant="ghost" size="sm" onClick={resetToDefaults} className="text-xs h-7">
                  Reset defaults
                </Button>
              </div>

              {/* Column configuration list */}
              <div className="space-y-2">
                {columns.map((column, index) => (
                  <div
                    key={column.key}
                    draggable
                    onDragStart={(e) => handleDragStart(e, column.key)}
                    onDragOver={handleDragOver}
                    onDrop={(e) => handleDrop(e, column.key)}
                    className={cn(
                      'flex items-center gap-3 p-3 border border-bn-border rounded-bn bg-content-box',
                      'hover:border-primary/40 cursor-move transition-colors shadow-sm',
                      draggedColumn === column.key && 'opacity-50'
                    )}
                  >
                    {/* Drag handle */}
                    <GripVertical className="h-4 w-4 text-icon-muted" />

                    {/* Column info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="font-semibold text-xs text-foreground">{column.label}</span>
                        {column.pinned && (
                          <Badge variant="secondary" className="text-[10px]">
                            Pinned {column.pinned === 'left' ? 'left' : 'right'}
                          </Badge>
                        )}
                      </div>
                      <div className="text-[11px] text-icon-muted">
                        {column.key} • Type: {column.type}
                        {column.width && ` • Width: ${column.width}px`}
                      </div>
                    </div>

                    {/* Column controls */}
                    <div className="flex items-center gap-2">
                      {/* Width setting */}
                      <input
                        type="number"
                        value={column.width || ''}
                        onChange={(e) => updateColumnWidth(column.key, parseInt(e.target.value) || 0)}
                        placeholder="Width"
                        className="w-16 px-2 py-1 text-xs border border-bn-border bg-content-bg rounded-bn text-foreground"
                        min="50"
                        max="500"
                      />

                      {/* Pin column buttons */}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => toggleColumnPin(column.key, 'left')}
                        className={cn(
                          'h-7 w-7 p-0',
                          column.pinned === 'left' && 'bg-primary/10 text-primary'
                        )}
                        title="Pin to left"
                      >
                        <SortAsc className="h-3.5 w-3.5" />
                      </Button>

                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => toggleColumnPin(column.key, 'right')}
                        className={cn(
                          'h-7 w-7 p-0',
                          column.pinned === 'right' && 'bg-primary/10 text-primary'
                        )}
                        title="Pin to right"
                      >
                        <SortDesc className="h-3.5 w-3.5" />
                      </Button>

                      {/* Visibility toggle */}
                      <div className="flex items-center gap-1.5 ml-1">
                        {column.visible ? (
                          <Eye className="h-3.5 w-3.5 text-emerald-500" />
                        ) : (
                          <EyeOff className="h-3.5 w-3.5 text-icon-muted" />
                        )}
                        <Switch
                          checked={column.visible}
                          onCheckedChange={() => toggleColumnVisibility(column.key)}
                        />
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {activeTab === 'export' && (
            <div className="p-6 space-y-6">
              <div className="text-center space-y-4">
                <div className="space-y-1">
                  <h3 className="text-base font-semibold text-foreground">Export Data Records</h3>
                  <p className="text-xs text-icon-muted">
                    Export the current table dataset with active filters and sorting applied.
                  </p>
                </div>

                <div className="grid grid-cols-2 gap-4 max-w-md mx-auto">
                  <Button
                    variant="secondary"
                    onClick={() => onExport('csv')}
                    className="h-20 flex flex-col gap-2 rounded-bn border border-bn-border hover:border-primary/40"
                  >
                    <Download className="h-5 w-5 text-primary" />
                    <span className="text-xs font-semibold">CSV File</span>
                    <span className="text-[10px] text-icon-muted">Comma-delimited</span>
                  </Button>

                  <Button
                    variant="secondary"
                    onClick={() => onExport('excel')}
                    className="h-20 flex flex-col gap-2 rounded-bn border border-bn-border hover:border-primary/40"
                  >
                    <Download className="h-5 w-5 text-primary" />
                    <span className="text-xs font-semibold">Excel Spreadsheet</span>
                    <span className="text-[10px] text-icon-muted">XLSX workbook</span>
                  </Button>
                </div>

                <div className="text-xs text-icon-muted space-y-1 pt-2 border-t border-bn-border/60">
                  <p>• Exported records reflect current active filter criteria.</p>
                  <p>• Only visible columns will be included in the exported sheet.</p>
                </div>
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

