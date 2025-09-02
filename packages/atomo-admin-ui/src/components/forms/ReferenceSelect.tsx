/**
 * Reference Select - 关联数据选择器
 * 
 * 用于选择关联模型的数据，支持搜索和分页
 */

import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Search, X } from 'lucide-react'

import { RelationshipConfig, SchemaMetadata, EntityData } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../ui/Select'
import { Input } from '../ui/Input'
import { Spinner } from '../ui/Spinner'
import { cn, getFieldLabel } from '../../lib/utils'

interface ReferenceSelectProps {
  value?: string
  onChange: (value: string | undefined) => void
  relationship: RelationshipConfig
  schema: SchemaMetadata
  disabled?: boolean
  error?: string
  placeholder?: string
}

export function ReferenceSelect({
  value,
  onChange,
  relationship,
  schema,
  disabled = false,
  error,
  placeholder
}: ReferenceSelectProps) {
  const [search, setSearch] = useState('')
  const [isOpen, setIsOpen] = useState(false)
  
  const relatedModel = schema.models[relationship.model]
  if (!relatedModel) {
    return (
      <div className="p-2 text-sm text-gray-500 border border-gray-300 rounded-md">
        关联模型 {relationship.model} 不存在
      </div>
    )
  }

  // 获取关联数据
  const { data: options, isLoading } = useQuery({
    queryKey: ['reference-options', relationship.model, search],
    queryFn: () => apiClient.listEntities(relationship.model, {
      search,
      limit: 50
    }),
    enabled: isOpen, // 只在下拉框打开时加载
  })

  // 获取当前选中项的详情
  const { data: selectedItem } = useQuery({
    queryKey: ['reference-item', relationship.model, value],
    queryFn: () => apiClient.getEntity(relationship.model, value!),
    enabled: !!value,
  })

  const displayField = relatedModel.ui.displayField
  const displayFields = Array.isArray(displayField) ? displayField : [displayField]

  // 格式化显示文本
  const formatDisplayText = (item: EntityData) => {
    return displayFields
      .map(field => item[field])
      .filter(Boolean)
      .join(' - ')
  }

  const handleSelect = (selectedValue: string) => {
    onChange(selectedValue === value ? undefined : selectedValue)
    setIsOpen(false)
  }

  const handleClear = () => {
    onChange(undefined)
    setIsOpen(false)
  }

  return (
    <div className="space-y-2">
      <Select
        value={value || ''}
        onValueChange={handleSelect}
        open={isOpen}
        onOpenChange={setIsOpen}
        disabled={disabled}
      >
        <SelectTrigger className={cn(error && 'border-danger-500')}>
          <SelectValue 
            placeholder={placeholder || `选择 ${getFieldLabel(relationship.model)}`}
          >
            {selectedItem ? formatDisplayText(selectedItem) : ''}
          </SelectValue>
        </SelectTrigger>
        
        <SelectContent>
          {/* 搜索框 */}
          <div className="p-2 border-b border-gray-200">
            <div className="relative">
              <Search className="absolute left-2 top-1/2 transform -translate-y-1/2 h-4 w-4 text-gray-400" />
              <Input
                placeholder="搜索..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="pl-8 h-8"
              />
            </div>
          </div>

          {/* 清空选项 */}
          {value && (
            <SelectItem value="" onSelect={handleClear}>
              <div className="flex items-center gap-2 text-gray-500">
                <X className="h-4 w-4" />
                清空选择
              </div>
            </SelectItem>
          )}

          {/* 加载状态 */}
          {isLoading && (
            <div className="p-4 text-center">
              <Spinner size="sm" />
              <p className="text-sm text-gray-500 mt-2">加载中...</p>
            </div>
          )}

          {/* 选项列表 */}
          {options?.data && options.data.length > 0 && (
            <>
              {options.data.map((item) => (
                <SelectItem key={item.id} value={item.id}>
                  <div className="flex flex-col">
                    <span>{formatDisplayText(item)}</span>
                    <span className="text-xs text-gray-500">ID: {item.id}</span>
                  </div>
                </SelectItem>
              ))}
            </>
          )}

          {/* 空状态 */}
          {options?.data && options.data.length === 0 && (
            <div className="p-4 text-center text-gray-500 text-sm">
              {search ? '未找到匹配项' : '暂无数据'}
            </div>
          )}

          {/* 更多结果提示 */}
          {options?.data && options.data.length === 50 && (
            <div className="p-2 text-xs text-gray-500 text-center border-t border-gray-200">
              显示前50项，使用搜索查找更多
            </div>
          )}
        </SelectContent>
      </Select>

      {error && (
        <p className="text-sm text-danger-600">{error}</p>
      )}
    </div>
  )
}