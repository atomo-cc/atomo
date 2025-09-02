/**
 * Entity Table - 虚拟化数据表格组件
 * 
 * 支持大量数据的高性能渲染，使用虚拟化技术
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

  // 点击外部关闭菜单
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
  
  // 虚拟化配置
  const virtualizer = useVirtualizer({
    count: data.length,
    getScrollElement: () => tableRef.current,
    estimateSize: () => 60, // 每行预估高度
    enabled: enableVirtualization && data.length > 50, // 数据量大时启用虚拟化
  })

  // 全选/取消全选
  const handleSelectAll = () => {
    if (!onSelectionChange) return
    
    if (selectedRows.length === data.length) {
      onSelectionChange([])
    } else {
      onSelectionChange(data.map(row => row.id))
    }
  }

  // 单行选择
  const handleRowSelect = (rowId: string) => {
    if (!onSelectionChange) return
    
    if (selectedRows.includes(rowId)) {
      onSelectionChange(selectedRows.filter(id => id !== rowId))
    } else {
      onSelectionChange([...selectedRows, rowId])
    }
  }

  // 排序图标
  const SortIcon = ({ field }: { field: string }) => {
    if (sortField !== field) {
      return <div className="w-4 h-4" />
    }
    
    return sortOrder === 'asc' 
      ? <ChevronUp className="w-4 h-4" />
      : <ChevronDown className="w-4 h-4" />
  }

  // 行操作菜单
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
                查看
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
                  编辑
                </button>
              )}
              
              {onRowDelete && (
                <button
                  className="flex items-center w-full px-3 py-2 text-sm text-red-600 hover:bg-red-50"
                  onClick={(e) => {
                    e.stopPropagation()
                    if (confirm('确定要删除这个项目吗？')) {
                      onRowDelete(row)
                    }
                    setOpenMenuRowId(null)
                  }}
                >
                  <Trash2 className="h-4 w-4 mr-2" />
                  删除
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    )
  }

  // 表格头部
  const renderHeader = () => (
    <div className="grid grid-cols-12 bg-gray-50 border-b border-gray-200 sticky top-0 z-10">
      {/* 选择列 */}
      {onSelectionChange && (
        <div className="col-span-1 px-4 py-3 flex items-center">
          <Checkbox
            checked={selectedRows.length === data.length && data.length > 0}
            onCheckedChange={handleSelectAll}
          />
        </div>
      )}
      
      {/* 数据列 */}
      {columns.map((column, index) => (
        <div
          key={column.key}
          className={cn(
            'px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider',
            column.sortable && 'cursor-pointer hover:bg-gray-100',
            `col-span-${Math.floor((12 - (onSelectionChange ? 2 : 1)) / columns.length)}`
          )}
          onClick={() => column.sortable && onSort?.(column.key)}
        >
          <div className="flex items-center gap-2">
            {column.label}
            {column.sortable && <SortIcon field={column.key} />}
          </div>
        </div>
      ))}
      
      {/* 操作列 */}
      <div className="col-span-1 px-4 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">
        操作
      </div>
    </div>
  )

  // 表格行
  const renderRow = (row: EntityData, index: number) => {
    const isSelected = selectedRows.includes(row.id)
    
    return (
      <div
        key={row.id}
        className={cn(
          'grid grid-cols-12 border-b border-gray-200 hover:bg-gray-50 transition-colors',
          isSelected && 'bg-primary-50'
        )}
      >
        {/* 选择列 */}
        {onSelectionChange && (
          <div className="col-span-1 px-4 py-4 flex items-center">
            <Checkbox
              checked={isSelected}
              onCheckedChange={() => handleRowSelect(row.id)}
            />
          </div>
        )}
        
        {/* 数据列 */}
        {columns.map((column) => (
          <div
            key={column.key}
            className={cn(
              'px-4 py-4 text-sm text-gray-900 cursor-pointer',
              `col-span-${Math.floor((12 - (onSelectionChange ? 2 : 1)) / columns.length)}`
            )}
            onClick={() => onRowClick?.(row)}
          >
            {column.render ? column.render(row[column.key], row) : row[column.key]}
          </div>
        ))}
        
        {/* 操作列 */}
        <div className="col-span-1 px-4 py-4 text-right">
          <RowActionMenu row={row} />
        </div>
      </div>
    )
  }

  // 加载状态
  if (loading) {
    return (
      <div className="p-8 text-center">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600 mx-auto"></div>
        <p className="mt-4 text-gray-600">加载中...</p>
      </div>
    )
  }

  // 空状态
  if (data.length === 0) {
    return (
      <div className="p-8 text-center">
        <p className="text-gray-600">暂无数据</p>
      </div>
    )
  }

  // 虚拟化渲染
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

  // 常规渲染（数据量较少时）
  return (
    <div className="w-full max-h-screen overflow-auto">
      {renderHeader()}
      <div>
        {data.map((row, index) => renderRow(row, index))}
      </div>
    </div>
  )
}
