/**
 * Entity Table - Virtualized data table component
 *
 * Supports high-performance rendering of large datasets using virtualization
 */

import React, { useRef, useState, useEffect } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { 
  ChevronUp, 
  ChevronDown, 
  MoreHorizontal,
  Eye,
  Edit2,
  Trash2
} from 'lucide-react'

import { EntityData, ColumnConfig } from '../../lib/types'
import { Button } from '../ui/Button'
import { Checkbox } from '../ui/Checkbox'
import { cn } from '../../lib/utils'

interface EntityTableProps {
  data: EntityData[]
  columns: ColumnConfig[]
  loading?: boolean
  selectedRows?: string[]
  onSelectionChange?: (selectedIds: string[]) => void
  onSort?: (field: string) => void
  sortField?: string
  sortOrder?: 'asc' | 'desc'
  onRowClick?: (row: EntityData) => void
  enableVirtualization?: boolean
  maxHeight?: number
  modelName?: string
  onRowEdit?: (row: EntityData) => void
  onRowDelete?: (row: EntityData) => void
}

export function EntityTable({ 
  data,
  columns,
  loading = false,
  selectedRows = [],
  onSelectionChange,
  onSort,
  sortField,
  sortOrder,
  onRowClick,
  enableVirtualization = true,
  maxHeight = 600,
  modelName,
  onRowEdit,
  onRowDelete
}: EntityTableProps) {
  const tableRef = useRef<HTMLDivElement>(null)
  const [openMenuRowId, setOpenMenuRowId] = useState<string | null>(null)

  // Close the menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (openMenuRowId) {
        const target = event.target as Element
        if (!target.closest('.row-action-menu')) {
          setOpenMenuRowId(null)
        }
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
    }
  }, [openMenuRowId])
  
  // Virtualization config
  const virtualizer = useVirtualizer({
    count: data.length,
    getScrollElement: () => tableRef.current,
    estimateSize: () => 60, // Estimated height per row
    enabled: enableVirtualization && data.length > 50, // Enable virtualization for large datasets
  })

  // Select all / deselect all
  const handleSelectAll = () => {
    if (!onSelectionChange) return
    
    if (selectedRows.length === data.length) {
      onSelectionChange([])
    } else {
      onSelectionChange(data.map(row => row.id))
    }
  }

  // Single row selection
  const handleRowSelect = (rowId: string) => {
    if (!onSelectionChange) return
    
    if (selectedRows.includes(rowId)) {
      onSelectionChange(selectedRows.filter(id => id !== rowId))
    } else {
      onSelectionChange([...selectedRows, rowId])
    }
  }

  // Sort icon
  const SortIcon = ({ field }: { field: string }) => {
    if (sortField !== field) {
      return <div className="w-4 h-4" />
    }
    
    return sortOrder === 'asc' 
      ? <ChevronUp className="w-4 h-4" />
      : <ChevronDown className="w-4 h-4" />
  }

  // Row action menu
  const RowActionMenu = ({ row }: { row: EntityData }) => {
    const isMenuOpen = openMenuRowId === row.id

    return (
      <div className="relative row-action-menu">
        <Button
          variant="ghost"
          size="sm"
          onClick={(e) => {
            e.stopPropagation()
            setOpenMenuRowId(isMenuOpen ? null : row.id)
          }}
        >
          <MoreHorizontal className="h-4 w-4" />
        </Button>
        
        {isMenuOpen && (
          <div className="absolute right-0 mt-1 w-32 bg-white border border-gray-200 rounded-md shadow-lg z-50">
            <div className="py-1">
              <button
                className="flex items-center w-full px-3 py-2 text-sm text-gray-700 hover:bg-gray-100"
                onClick={(e) => {
                  e.stopPropagation()
                  onRowClick?.(row)
                  setOpenMenuRowId(null)
                }}
              >
                <Eye className="h-4 w-4 mr-2" />
                View
              </button>
              
              {onRowEdit && (
                <button
                  className="flex items-center w-full px-3 py-2 text-sm text-gray-700 hover:bg-gray-100"
                  onClick={(e) => {
                    e.stopPropagation()
                    onRowEdit(row)
                    setOpenMenuRowId(null)
                  }}
                >
                  <Edit2 className="h-4 w-4 mr-2" />
                  Edit
                </button>
              )}
              
              {onRowDelete && (
                <button
                  className="flex items-center w-full px-3 py-2 text-sm text-red-600 hover:bg-red-50"
                  onClick={(e) => {
                    e.stopPropagation()
                    if (confirm('Are you sure you want to delete this item?')) {
                      onRowDelete(row)
                    }
                    setOpenMenuRowId(null)
                  }}
                >
                  <Trash2 className="h-4 w-4 mr-2" />
                  Delete
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    )
  }

  // Column layout via an explicit CSS grid template — the previous
  // `col-span-${computed}` classes were invisible to Tailwind's static scan,
  // got purged from the build, and left the columns unsized.
  const gridTemplate = {
    gridTemplateColumns: `${onSelectionChange ? '48px ' : ''}repeat(${Math.max(columns.length, 1)}, minmax(0, 1fr)) 96px`,
  }

  // Table header
  const renderHeader = () => (
    <div className="grid bg-gray-50 border-b border-gray-200 sticky top-0 z-10" style={gridTemplate}>
      {/* Selection column */}
      {onSelectionChange && (
        <div className="px-4 py-3 flex items-center">
          <Checkbox
            checked={selectedRows.length === data.length && data.length > 0}
            onCheckedChange={handleSelectAll}
          />
        </div>
      )}

      {/* Data columns */}
      {columns.map((column) => (
        <div
          key={column.key}
          className={cn(
            'px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider',
            column.sortable && 'cursor-pointer hover:bg-gray-100'
          )}
          onClick={() => column.sortable && onSort?.(column.key)}
        >
          <div className="flex items-center gap-2">
            {column.label}
            {column.sortable && <SortIcon field={column.key} />}
          </div>
        </div>
      ))}

      {/* Actions column */}
      <div className="px-4 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">
        Actions
      </div>
    </div>
  )

  // Table row
  const renderRow = (row: EntityData, index: number) => {
    const isSelected = selectedRows.includes(row.id)

    return (
      <div
        key={row.id}
        className={cn(
          'grid border-b border-gray-200 hover:bg-gray-50 transition-colors',
          isSelected && 'bg-primary-50'
        )}
        style={gridTemplate}
      >
        {/* Selection column */}
        {onSelectionChange && (
          <div className="px-4 py-4 flex items-center">
            <Checkbox
              checked={isSelected}
              onCheckedChange={() => handleRowSelect(row.id)}
            />
          </div>
        )}

        {/* Data columns */}
        {columns.map((column) => (
          <div
            key={column.key}
            className="px-4 py-4 text-sm text-gray-900 cursor-pointer truncate"
            onClick={() => onRowClick?.(row)}
          >
            {column.render ? column.render(row[column.key], row) : row[column.key]}
          </div>
        ))}

        {/* Actions column */}
        <div className="px-4 py-4 text-right">
          <RowActionMenu row={row} />
        </div>
      </div>
    )
  }

  // Loading state
  if (loading) {
    return (
      <div className="p-8 text-center">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600 mx-auto"></div>
        <p className="mt-4 text-gray-600">Loading...</p>
      </div>
    )
  }

  // Empty state
  if (data.length === 0) {
    return (
      <div className="p-8 text-center">
        <p className="text-gray-600">No data</p>
      </div>
    )
  }

  // Virtualized rendering
  if (enableVirtualization && virtualizer.getVirtualItems().length > 0) {
    return (
      <div className="w-full">
        {renderHeader()}
        
        <div
          ref={tableRef}
          className="w-full overflow-auto"
          style={{ height: `${maxHeight}px` }}
        >
          <div
            style={{
              height: `${virtualizer.getTotalSize()}px`,
              width: '100%',
              position: 'relative',
            }}
          >
            {virtualizer.getVirtualItems().map((virtualItem) => {
              const row = data[virtualItem.index]
              return (
                <div
                  key={virtualItem.key}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    height: `${virtualItem.size}px`,
                    transform: `translateY(${virtualItem.start}px)`,
                  }}
                >
                  {renderRow(row, virtualItem.index)}
                </div>
              )
            })}
          </div>
        </div>
      </div>
    )
  }

  // Standard rendering (for smaller datasets)
  return (
    <div className="w-full max-h-screen overflow-auto">
      {renderHeader()}
      <div>
        {data.map((row, index) => renderRow(row, index))}
      </div>
    </div>
  )
}
