/**
 * Reference Select - 关联数据选择组件
 * 
 * 用于选择关联模型的数据，支持搜索和异步加载
 */

import React, { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { ChevronDown, Check } from 'lucide-react'

import { RelationshipConfig, SchemaMetadata } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { Button } from '../ui/Button'
import { Input } from '../ui/Input'

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
  placeholder = '请选择...'
}: ReferenceSelectProps) {
  const [isOpen, setIsOpen] = useState(false)
  const [searchTerm, setSearchTerm] = useState('')

  // 获取关联模型的元数据
  const relatedModel = schema.models[relationship.model]
  if (!relatedModel) {
    return (
      <div className="p-2 text-sm text-red-600 border border-red-300 rounded">
        错误：关联模型 {relationship.model} 未找到
      </div>
    )
  }

  // 查询关联数据
  const { data: options, isLoading } = useQuery({
    queryKey: ['reference-options', relationship.model, searchTerm],
    queryFn: () => apiClient.listEntities(relationship.model, {
      search: searchTerm,
      limit: 50
    }),
    enabled: isOpen, // 只有在打开时才加载
  })

  // 获取当前选中项的显示文本
  const { data: selectedItem } = useQuery({
    queryKey: ['reference-item', relationship.model, value],
    queryFn: () => apiClient.getEntity(relationship.model, value!),
    enabled: !!value,
  })

  const getDisplayText = (item: any) => {
    const displayField = relatedModel.ui.displayField
    if (Array.isArray(displayField)) {
      return displayField.map(field => item[field]).filter(Boolean).join(' ')
    }
    return item[displayField] || item.name || item.title || item.id
  }

  const handleSelect = (optionValue: string) => {
    onChange(optionValue)
    setIsOpen(false)
    setSearchTerm('')
  }

  return (
    <div className="relative">
      {/* 主选择按钮 */}
      <Button
        type="button"
        variant="secondary"
        onClick={() => !disabled && setIsOpen(!isOpen)}
        disabled={disabled}
        className="w-full justify-between text-left font-normal"
      >
        <span className={!selectedItem ? 'text-gray-500' : ''}>
          {selectedItem ? getDisplayText(selectedItem) : placeholder}
        </span>
        <ChevronDown className="h-4 w-4 opacity-50" />
      </Button>

      {/* 错误提示 */}
      {error && (
        <p className="mt-1 text-sm text-danger-600">{error}</p>
      )}

      {/* 下拉选项 */}
      {isOpen && (
        <div className="absolute z-50 w-full mt-1 bg-white border border-gray-200 rounded-md shadow-lg">
          {/* 搜索框 */}
          <div className="p-2 border-b border-gray-200">
            <Input
              placeholder={`搜索 ${relationship.model}...`}
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="h-8"
            />
          </div>

          {/* 选项列表 */}
          <div className="max-h-60 overflow-auto">
            {isLoading ? (
              <div className="p-4 text-center text-gray-500">加载中...</div>
            ) : options?.data.length === 0 ? (
              <div className="p-4 text-center text-gray-500">
                {searchTerm ? '未找到匹配项' : '暂无选项'}
              </div>
            ) : (
              <>
                {/* 清空选项 */}
                <button
                  type="button"
                  onClick={() => handleSelect('')}
                  className="w-full px-3 py-2 text-left hover:bg-gray-100 border-b border-gray-100"
                >
                  <span className="text-gray-500">清空选择</span>
                </button>

                {/* 数据选项 */}
                {options?.data.map((option) => (
                  <button
                    key={option.id}
                    type="button"
                    onClick={() => handleSelect(option.id)}
                    className="w-full px-3 py-2 text-left hover:bg-gray-100 flex items-center justify-between"
                  >
                    <span>{getDisplayText(option)}</span>
                    {value === option.id && (
                      <Check className="h-4 w-4 text-primary-600" />
                    )}
                  </button>
                ))}
              </>
            )}
          </div>
        </div>
      )}

      {/* 点击外部关闭 */}
      {isOpen && (
        <div
          className="fixed inset-0 z-40"
          onClick={() => setIsOpen(false)}
        />
      )}
    </div>
  )
}
