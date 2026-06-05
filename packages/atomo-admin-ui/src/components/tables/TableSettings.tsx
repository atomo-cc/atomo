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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
      <Card className="w-full max-w-2xl max-h-[80vh] overflow-hidden">
        <CardHeader className="border-b">
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2">
              <Settings className="h-5 w-5" />
              Table Settings
            </CardTitle>
            <Button variant="ghost" size="sm" onClick={onClose}>
              <X className="h-4 w-4" />
            </Button>
          </div>
          
          {/* Tab switcher */}
          <div className="flex gap-1 bg-gray-100 p-1 rounded-md">
            <button
              onClick={() => setActiveTab('columns')}
              className={cn(
                'flex-1 px-3 py-2 text-sm font-medium rounded-sm transition-colors',
                activeTab === 'columns'
                  ? 'bg-white text-gray-900 shadow-sm'
                  : 'text-gray-600 hover:text-gray-900'
              )}
            >
              <Columns className="h-4 w-4 mr-2 inline" />
              Columns
            </button>
            <button
              onClick={() => setActiveTab('export')}
              className={cn(
                'flex-1 px-3 py-2 text-sm font-medium rounded-sm transition-colors',
                activeTab === 'export'
                  ? 'bg-white text-gray-900 shadow-sm'
                  : 'text-gray-600 hover:text-gray-900'
              )}
            >
              <Download className="h-4 w-4 mr-2 inline" />
              Export Data
            </button>
          </div>
        </CardHeader>

        <CardContent className="p-0">
          {activeTab === 'columns' && (
            <div className="p-6 space-y-6 max-h-96 overflow-y-auto">
              {/* Summary info */}
              <div className="flex items-center justify-between text-sm">
                <div className="flex gap-4">
                  <span className="text-gray-600">
                    Visible columns: <span className="font-medium">{visibleColumnsCount}/{columns.length}</span>
                  </span>
                  <span className="text-gray-600">
                    Pinned columns: <span className="font-medium">Left {pinnedLeftCount} | Right {pinnedRightCount}</span>
                  </span>
                </div>
                <Button variant="ghost" size="sm" onClick={resetToDefaults}>
                  Reset to defaults
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
                      'flex items-center gap-3 p-3 border border-gray-200 rounded-md bg-white',
                      'hover:border-gray-300 cursor-move transition-colors',
                      draggedColumn === column.key && 'opacity-50'
                    )}
                  >
                    {/* Drag handle */}
                    <GripVertical className="h-4 w-4 text-gray-400" />

                    {/* Column info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-sm">{column.label}</span>
                        {column.pinned && (
                          <Badge variant="secondary" className="text-xs">
                            Pinned {column.pinned === 'left' ? 'left' : 'right'}
                          </Badge>
                        )}
                      </div>
                      <div className="text-xs text-gray-500">
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
                        className="w-16 px-2 py-1 text-xs border border-gray-300 rounded"
                        min="50"
                        max="500"
                      />

                      {/* Pin column buttons */}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => toggleColumnPin(column.key, 'left')}
                        className={cn(
                          'h-8 w-8 p-0',
                          column.pinned === 'left' && 'bg-primary-100 text-primary-700'
                        )}
                        title="Pin to left"
                      >
                        <SortAsc className="h-3 w-3" />
                      </Button>

                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => toggleColumnPin(column.key, 'right')}
                        className={cn(
                          'h-8 w-8 p-0',
                          column.pinned === 'right' && 'bg-primary-100 text-primary-700'
                        )}
                        title="Pin to right"
                      >
                        <SortDesc className="h-3 w-3" />
                      </Button>

                      {/* Visibility toggle */}
                      <div className="flex items-center gap-1">
                        {column.visible ? (
                          <Eye className="h-4 w-4 text-green-600" />
                        ) : (
                          <EyeOff className="h-4 w-4 text-gray-400" />
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
                <div className="space-y-2">
                  <h3 className="text-lg font-medium text-gray-900">Export Data</h3>
                  <p className="text-sm text-gray-600">
                    Choose an export format to export the currently filtered and sorted data to a file
                  </p>
                </div>

                <div className="grid grid-cols-2 gap-4 max-w-md mx-auto">
                  <Button
                    variant="secondary"
                    onClick={() => onExport('csv')}
                    className="h-20 flex flex-col gap-2"
                  >
                    <Download className="h-6 w-6" />
                    <span>CSV File</span>
                    <span className="text-xs text-gray-500">Excel compatible</span>
                  </Button>

                  <Button
                    variant="secondary"
                    onClick={() => onExport('excel')}
                    className="h-20 flex flex-col gap-2"
                  >
                    <Download className="h-6 w-6" />
                    <span>Excel File</span>
                    <span className="text-xs text-gray-500">Native format</span>
                  </Button>
                </div>

                <div className="text-sm text-gray-500 space-y-1">
                  <p>• Exported data includes the current filter conditions</p>
                  <p>• Only data from visible columns is exported</p>
                  <p>• Hidden columns are not included in the export file</p>
                </div>
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
