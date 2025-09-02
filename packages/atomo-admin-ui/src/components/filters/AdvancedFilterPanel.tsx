/**
 * Advanced Filter Panel - 高级筛选面板
 * 
 * 支持复杂的多条件筛选，包括：
 * - 多字段组合筛选
 * - 不同操作符（等于、包含、范围等）
 * - 逻辑组合（AND/OR）
 * - 筛选条件的保存和复用
 */

import React, { useState, useEffect } from 'react'
import { 
  Plus, 
  X, 
  Filter, 
  Save, 
  FolderOpen,
  RotateCcw,
  Search
} from 'lucide-react'

import { ModelMetadata, FieldMetadata } from '../../lib/types'
import { Button } from '../ui/Button'
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../ui/Select'
import { Input } from '../ui/Input'
import { Switch } from '../ui/Switch'
import { Badge } from '../ui/Badge'
import { cn } from '../../lib/utils'

export interface FilterCondition {
  id: string
  field: string
  operator: FilterOperator
  value: any
  logicalOperator?: 'AND' | 'OR'
}

export type FilterOperator = 
  | 'equals' | 'not_equals'
  | 'contains' | 'not_contains'
  | 'starts_with' | 'ends_with'
  | 'greater_than' | 'less_than'
  | 'greater_than_or_equal' | 'less_than_or_equal'
  | 'between' | 'in' | 'not_in'
  | 'is_null' | 'is_not_null'
  | 'is_empty' | 'is_not_empty'

export interface SavedFilter {
  id: string
  name: string
  conditions: FilterCondition[]
  createdAt: Date
}

interface AdvancedFilterPanelProps {
  modelMetadata: ModelMetadata
  onFiltersChange: (conditions: FilterCondition[]) => void
  initialConditions?: FilterCondition[]
  isOpen: boolean
  onClose: () => void
}

const OPERATORS: Record<FilterOperator, string> = {
  equals: '等于',
  not_equals: '不等于',
  contains: '包含',
  not_contains: '不包含',
  starts_with: '开头是',
  ends_with: '结尾是',
  greater_than: '大于',
  less_than: '小于',
  greater_than_or_equal: '大于等于',
  less_than_or_equal: '小于等于',
  between: '介于',
  in: '在列表中',
  not_in: '不在列表中',
  is_null: '为空',
  is_not_null: '不为空',
  is_empty: '为空值',
  is_not_empty: '不为空值'
}

const getFieldOperators = (field: FieldMetadata): FilterOperator[] => {
  switch (field.type) {
    case 'string':
    case 'text':
      return ['equals', 'not_equals', 'contains', 'not_contains', 'starts_with', 'ends_with', 'is_null', 'is_not_null', 'is_empty', 'is_not_empty']
    
    case 'number':
      return ['equals', 'not_equals', 'greater_than', 'less_than', 'greater_than_or_equal', 'less_than_or_equal', 'between', 'is_null', 'is_not_null']
    
    case 'boolean':
      return ['equals', 'not_equals', 'is_null', 'is_not_null']
    
    case 'date':
    case 'datetime':
      return ['equals', 'not_equals', 'greater_than', 'less_than', 'greater_than_or_equal', 'less_than_or_equal', 'between', 'is_null', 'is_not_null']
    
    case 'reference':
      return ['equals', 'not_equals', 'in', 'not_in', 'is_null', 'is_not_null']
    
    case 'array':
      return ['contains', 'not_contains', 'is_empty', 'is_not_empty']
    
    default:
      return ['equals', 'not_equals', 'is_null', 'is_not_null']
  }
}

export function AdvancedFilterPanel({
  modelMetadata,
  onFiltersChange,
  initialConditions = [],
  isOpen,
  onClose
}: AdvancedFilterPanelProps) {
  const [conditions, setConditions] = useState<FilterCondition[]>(initialConditions)
  const [savedFilters, setSavedFilters] = useState<SavedFilter[]>([])
  const [saveFilterName, setSaveFilterName] = useState('')
  const [showSaveDialog, setShowSaveDialog] = useState(false)

  // 可筛选的字段（支持搜索的字段 + 一些常用字段）
  const filterableFields = Object.values(modelMetadata.fields).filter(field => 
    modelMetadata.searchable?.includes(field.name) || 
    ['createdAt', 'updatedAt', 'id'].includes(field.name) ||
    !field.attributes.includes('readonly')
  )

  useEffect(() => {
    onFiltersChange(conditions)
  }, [conditions, onFiltersChange])

  // 从localStorage加载保存的筛选器
  useEffect(() => {
    const saved = localStorage.getItem(`saved-filters-${modelMetadata.tableName}`)
    if (saved) {
      try {
        setSavedFilters(JSON.parse(saved))
      } catch (error) {
        console.error('Failed to load saved filters:', error)
      }
    }
  }, [modelMetadata.tableName])

  const addCondition = () => {
    const newCondition: FilterCondition = {
      id: `condition_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      field: filterableFields[0]?.name || '',
      operator: 'equals',
      value: '',
      logicalOperator: conditions.length > 0 ? 'AND' : undefined
    }
    setConditions([...conditions, newCondition])
  }

  const updateCondition = (id: string, updates: Partial<FilterCondition>) => {
    setConditions(conditions.map(condition => 
      condition.id === id ? { ...condition, ...updates } : condition
    ))
  }

  const removeCondition = (id: string) => {
    setConditions(conditions.filter(condition => condition.id !== id))
  }

  const clearAllConditions = () => {
    setConditions([])
  }

  const saveCurrentFilter = () => {
    if (!saveFilterName.trim() || conditions.length === 0) return

    const newFilter: SavedFilter = {
      id: `filter_${Date.now()}`,
      name: saveFilterName.trim(),
      conditions: [...conditions],
      createdAt: new Date()
    }

    const updatedFilters = [...savedFilters, newFilter]
    setSavedFilters(updatedFilters)
    localStorage.setItem(`saved-filters-${modelMetadata.tableName}`, JSON.stringify(updatedFilters))
    
    setSaveFilterName('')
    setShowSaveDialog(false)
  }

  const loadSavedFilter = (filter: SavedFilter) => {
    setConditions([...filter.conditions])
  }

  const deleteSavedFilter = (filterId: string) => {
    const updatedFilters = savedFilters.filter(f => f.id !== filterId)
    setSavedFilters(updatedFilters)
    localStorage.setItem(`saved-filters-${modelMetadata.tableName}`, JSON.stringify(updatedFilters))
  }

  const renderConditionValue = (condition: FilterCondition) => {
    const field = modelMetadata.fields[condition.field]
    if (!field) return null

    // 不需要值的操作符
    if (['is_null', 'is_not_null', 'is_empty', 'is_not_empty'].includes(condition.operator)) {
      return (
        <div className="flex items-center text-sm text-gray-500">
          无需输入值
        </div>
      )
    }

    // 范围操作符
    if (condition.operator === 'between') {
      return (
        <div className="flex items-center gap-2">
          <Input
            type={field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : 'text'}
            value={condition.value?.[0] || ''}
            onChange={(e) => updateCondition(condition.id, {
              value: [e.target.value, condition.value?.[1] || '']
            })}
            placeholder="开始值"
            className="w-24"
          />
          <span className="text-gray-500">到</span>
          <Input
            type={field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : 'text'}
            value={condition.value?.[1] || ''}
            onChange={(e) => updateCondition(condition.id, {
              value: [condition.value?.[0] || '', e.target.value]
            })}
            placeholder="结束值"
            className="w-24"
          />
        </div>
      )
    }

    // 列表操作符
    if (['in', 'not_in'].includes(condition.operator)) {
      return (
        <Input
          value={Array.isArray(condition.value) ? condition.value.join(', ') : condition.value || ''}
          onChange={(e) => updateCondition(condition.id, {
            value: e.target.value.split(',').map(v => v.trim()).filter(Boolean)
          })}
          placeholder="多个值用逗号分隔"
          className="w-48"
        />
      )
    }

    // 布尔类型
    if (field.type === 'boolean') {
      return (
        <Select
          value={condition.value}
          onValueChange={(value) => updateCondition(condition.id, { value })}
        >
          <SelectTrigger className="w-32">
            <SelectValue placeholder="选择值" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="true">是</SelectItem>
            <SelectItem value="false">否</SelectItem>
          </SelectContent>
        </Select>
      )
    }

    // 选择器类型（有选项的字段）
    if (field.ui?.options) {
      return (
        <Select
          value={condition.value}
          onValueChange={(value) => updateCondition(condition.id, { value })}
        >
          <SelectTrigger className="w-48">
            <SelectValue placeholder="选择值" />
          </SelectTrigger>
          <SelectContent>
            {field.ui.options.map((option: any) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )
    }

    // 默认输入
    return (
      <Input
        type={field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : 'text'}
        value={condition.value || ''}
        onChange={(e) => updateCondition(condition.id, { value: e.target.value })}
        placeholder="输入筛选值"
        className="w-48"
      />
    )
  }

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-end bg-black bg-opacity-50">
      <Card className="w-full max-w-2xl h-full overflow-auto m-0 rounded-none">
        <CardHeader className="border-b">
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2">
              <Filter className="h-5 w-5" />
              高级筛选
            </CardTitle>
            <Button variant="ghost" size="sm" onClick={onClose}>
              <X className="h-4 w-4" />
            </Button>
          </div>
        </CardHeader>

        <CardContent className="p-6 space-y-6">
          {/* 筛选条件 */}
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-medium">筛选条件</h3>
              <div className="flex gap-2">
                <Button variant="ghost" size="sm" onClick={clearAllConditions}>
                  <RotateCcw className="h-4 w-4 mr-1" />
                  清空
                </Button>
                <Button size="sm" onClick={addCondition}>
                  <Plus className="h-4 w-4 mr-1" />
                  添加条件
                </Button>
              </div>
            </div>

            {conditions.length === 0 ? (
              <div className="text-center py-8 text-gray-500 border-2 border-dashed border-gray-300 rounded-lg">
                暂无筛选条件，点击"添加条件"开始
              </div>
            ) : (
              <div className="space-y-3">
                {conditions.map((condition, index) => {
                  const field = modelMetadata.fields[condition.field]
                  return (
                    <div key={condition.id} className="p-4 border border-gray-200 rounded-lg space-y-3">
                      {/* 逻辑操作符 */}
                      {index > 0 && (
                        <div className="flex items-center gap-2">
                          <span className="text-sm text-gray-500">逻辑关系:</span>
                          <Select
                            value={condition.logicalOperator || 'AND'}
                            onValueChange={(value: 'AND' | 'OR') => 
                              updateCondition(condition.id, { logicalOperator: value })
                            }
                          >
                            <SelectTrigger className="w-20">
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                              <SelectItem value="AND">AND</SelectItem>
                              <SelectItem value="OR">OR</SelectItem>
                            </SelectContent>
                          </Select>
                        </div>
                      )}

                      {/* 条件设置 */}
                      <div className="flex items-center gap-3 flex-wrap">
                        {/* 字段选择 */}
                        <Select
                          value={condition.field}
                          onValueChange={(value) => updateCondition(condition.id, { 
                            field: value,
                            operator: 'equals', // 重置操作符
                            value: '' // 重置值
                          })}
                        >
                          <SelectTrigger className="w-40">
                            <SelectValue placeholder="选择字段" />
                          </SelectTrigger>
                          <SelectContent>
                            {filterableFields.map((field) => (
                              <SelectItem key={field.name} value={field.name}>
                                {field.ui?.label || field.name}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>

                        {/* 操作符选择 */}
                        <Select
                          value={condition.operator}
                          onValueChange={(value: FilterOperator) => 
                            updateCondition(condition.id, { 
                              operator: value,
                              value: '' // 重置值
                            })
                          }
                        >
                          <SelectTrigger className="w-32">
                            <SelectValue placeholder="操作符" />
                          </SelectTrigger>
                          <SelectContent>
                            {field && getFieldOperators(field).map((op) => (
                              <SelectItem key={op} value={op}>
                                {OPERATORS[op]}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>

                        {/* 值输入 */}
                        {renderConditionValue(condition)}

                        {/* 删除按钮 */}
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => removeCondition(condition.id)}
                          className="text-red-600 hover:text-red-700"
                        >
                          <X className="h-4 w-4" />
                        </Button>
                      </div>
                    </div>
                  )
                })}
              </div>
            )}
          </div>

          {/* 保存筛选器 */}
          {conditions.length > 0 && (
            <div className="space-y-3 pt-4 border-t">
              <h3 className="text-sm font-medium">保存筛选器</h3>
              
              {!showSaveDialog ? (
                <Button
                  variant="secondary"
                  onClick={() => setShowSaveDialog(true)}
                >
                  <Save className="h-4 w-4 mr-2" />
                  保存当前筛选器
                </Button>
              ) : (
                <div className="flex gap-2">
                  <Input
                    value={saveFilterName}
                    onChange={(e) => setSaveFilterName(e.target.value)}
                    placeholder="输入筛选器名称"
                    className="flex-1"
                  />
                  <Button onClick={saveCurrentFilter} disabled={!saveFilterName.trim()}>
                    保存
                  </Button>
                  <Button
                    variant="ghost"
                    onClick={() => {
                      setShowSaveDialog(false)
                      setSaveFilterName('')
                    }}
                  >
                    取消
                  </Button>
                </div>
              )}
            </div>
          )}

          {/* 已保存的筛选器 */}
          {savedFilters.length > 0 && (
            <div className="space-y-3 pt-4 border-t">
              <h3 className="text-sm font-medium">已保存的筛选器</h3>
              <div className="space-y-2">
                {savedFilters.map((filter) => (
                  <div key={filter.id} className="flex items-center justify-between p-3 bg-gray-50 rounded-md">
                    <div className="flex-1">
                      <div className="font-medium text-sm">{filter.name}</div>
                      <div className="text-xs text-gray-500">
                        {filter.conditions.length} 个条件 • {new Date(filter.createdAt).toLocaleDateString()}
                      </div>
                    </div>
                    <div className="flex gap-2">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => loadSavedFilter(filter)}
                      >
                        <FolderOpen className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => deleteSavedFilter(filter.id)}
                        className="text-red-600 hover:text-red-700"
                      >
                        <X className="h-4 w-4" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* 应用按钮 */}
          <div className="flex justify-end gap-3 pt-4 border-t">
            <Button variant="secondary" onClick={onClose}>
              取消
            </Button>
            <Button onClick={onClose}>
              <Search className="h-4 w-4 mr-2" />
              应用筛选
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
