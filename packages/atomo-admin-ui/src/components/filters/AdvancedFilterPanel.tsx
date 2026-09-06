/**
 * Advanced Filter Panel
 *
 * Supports complex multi-condition filtering, including:
 * - Combining filters across multiple fields
 * - Different operators (equals, contains, range, etc.)
 * - Logical combinations (AND/OR)
 * - Saving and reusing filter conditions
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
import { getEnumValues } from '../../lib/enums'
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
  equals: 'Equals',
  not_equals: 'Not equals',
  contains: 'Contains',
  not_contains: 'Does not contain',
  starts_with: 'Starts with',
  ends_with: 'Ends with',
  greater_than: 'Greater than',
  less_than: 'Less than',
  greater_than_or_equal: 'Greater than or equal',
  less_than_or_equal: 'Less than or equal',
  between: 'Between',
  in: 'In list',
  not_in: 'Not in list',
  is_null: 'Is null',
  is_not_null: 'Is not null',
  is_empty: 'Is empty',
  is_not_empty: 'Is not empty'
}

// Only operators the server's where JSON can express (see lib/where.ts).
// `not_contains` / `is_empty` / `is_not_empty` were offered before but had no
// server mapping — a filter that renders a chip and does nothing.
const getFieldOperators = (field: FieldMetadata, isEnum: boolean): FilterOperator[] => {
  // Fixed-domain (in:-constrained) fields: substring operators are meaningless —
  // offer set membership, mirroring what the record form renders (feedback #13A).
  if (isEnum) {
    return ['equals', 'not_equals', 'in', 'not_in', 'is_null', 'is_not_null']
  }
  switch (field.type) {
    case 'string':
    case 'text':
      return ['equals', 'not_equals', 'contains', 'starts_with', 'ends_with', 'is_null', 'is_not_null']

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
      return ['contains', 'is_null', 'is_not_null']

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

  // Filterable fields (searchable fields plus some commonly used fields)
  const filterableFields = Object.values(modelMetadata.fields).filter(field => 
    modelMetadata.searchable?.includes(field.name) || 
    ['createdAt', 'updatedAt', 'id'].includes(field.name) ||
    !field.attributes.includes('readonly')
  )

  useEffect(() => {
    onFiltersChange(conditions)
  }, [conditions, onFiltersChange])

  // Load saved filters from localStorage
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
    const enumValues = getEnumValues(modelMetadata, condition.field)

    // Operators that don't require a value
    if (['is_null', 'is_not_null', 'is_empty', 'is_not_empty'].includes(condition.operator)) {
      return (
        <div className="flex items-center text-sm text-gray-500">
          No value needed
        </div>
      )
    }

    // Enum fields (in:-constrained): pick from the declared values instead of
    // typing free text — the same source the record form's dropdown uses.
    if (enumValues && ['equals', 'not_equals'].includes(condition.operator)) {
      return (
        <Select
          value={condition.value || ''}
          onValueChange={(value) => updateCondition(condition.id, { value })}
        >
          <SelectTrigger className="w-48">
            <SelectValue placeholder="Select value" />
          </SelectTrigger>
          <SelectContent>
            {enumValues.map((v) => (
              <SelectItem key={v} value={v}>
                {v}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )
    }

    // Range operator
    if (condition.operator === 'between') {
      return (
        <div className="flex items-center gap-2">
          <Input
            type={field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : 'text'}
            value={condition.value?.[0] || ''}
            onChange={(e) => updateCondition(condition.id, {
              value: [e.target.value, condition.value?.[1] || '']
            })}
            placeholder="Start value"
            className="w-24"
          />
          <span className="text-gray-500">to</span>
          <Input
            type={field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : 'text'}
            value={condition.value?.[1] || ''}
            onChange={(e) => updateCondition(condition.id, {
              value: [condition.value?.[0] || '', e.target.value]
            })}
            placeholder="End value"
            className="w-24"
          />
        </div>
      )
    }

    // List operators
    if (['in', 'not_in'].includes(condition.operator)) {
      return (
        <Input
          value={Array.isArray(condition.value) ? condition.value.join(', ') : condition.value || ''}
          onChange={(e) => updateCondition(condition.id, {
            value: e.target.value.split(',').map(v => v.trim()).filter(Boolean)
          })}
          placeholder={
            enumValues ? `Comma-separated: ${enumValues.join(', ')}` : 'Separate multiple values with commas'
          }
          className="w-48"
        />
      )
    }

    // Boolean type
    if (field.type === 'boolean') {
      return (
        <Select
          value={condition.value}
          onValueChange={(value) => updateCondition(condition.id, { value })}
        >
          <SelectTrigger className="w-32">
            <SelectValue placeholder="Select value" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="true">Yes</SelectItem>
            <SelectItem value="false">No</SelectItem>
          </SelectContent>
        </Select>
      )
    }

    // Select type (fields with options)
    if (field.ui?.options) {
      return (
        <Select
          value={condition.value}
          onValueChange={(value) => updateCondition(condition.id, { value })}
        >
          <SelectTrigger className="w-48">
            <SelectValue placeholder="Select value" />
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

    // Default input
    return (
      <Input
        type={field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : 'text'}
        value={condition.value || ''}
        onChange={(e) => updateCondition(condition.id, { value: e.target.value })}
        placeholder="Enter filter value"
        className="w-48"
      />
    )
  }

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-end bg-black/60 backdrop-blur-sm">
      <Card className="w-full max-w-2xl h-full overflow-auto m-0 rounded-none border-l border-bn-border bg-content-box shadow-bn text-foreground">
        <CardHeader className="border-b border-bn-border/60 py-4 px-6">
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2">
              <Filter className="h-4 w-4 text-primary" />
              Advanced Filters
            </CardTitle>
            <Button variant="ghost" size="sm" onClick={onClose} className="h-8 w-8 p-0">
              <X className="h-4 w-4" />
            </Button>
          </div>
        </CardHeader>

        <CardContent className="p-6 space-y-6">
          {/* Filter conditions */}
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-xs font-semibold text-icon-muted uppercase tracking-wider">Filter Rules</h3>
              <div className="flex gap-2">
                <Button variant="ghost" size="sm" onClick={clearAllConditions}>
                  <RotateCcw className="h-3.5 w-3.5 mr-1" />
                  Clear all
                </Button>
                <Button size="sm" onClick={addCondition}>
                  <Plus className="h-3.5 w-3.5 mr-1" />
                  Add condition
                </Button>
              </div>
            </div>

            {conditions.length === 0 ? (
              <div className="text-center py-8 text-xs text-icon-muted border border-dashed border-bn-border rounded-bn">
                No filter conditions active. Click "Add condition" to filter records.
              </div>
            ) : (
              <div className="space-y-3">
                {conditions.map((condition, index) => {
                  const field = modelMetadata.fields[condition.field]
                  return (
                    <div key={condition.id} className="p-4 border border-bn-border rounded-bn bg-content-bg/50 space-y-3">
                      {index > 0 && (
                        <div className="flex items-center gap-2">
                          <span className="text-xs font-semibold text-primary uppercase">AND</span>
                        </div>
                      )}

                      {/* Condition settings */}
                      <div className="flex items-center gap-3 flex-wrap">
                        {/* Field selection */}
                        <Select
                          value={condition.field}
                          onValueChange={(value) => updateCondition(condition.id, {
                            field: value,
                            operator: 'equals',
                            value: ''
                          })}
                        >
                          <SelectTrigger className="w-40 h-8 text-xs">
                            <SelectValue placeholder="Select field" />
                          </SelectTrigger>
                          <SelectContent>
                            {filterableFields.map((field) => (
                              <SelectItem key={field.name} value={field.name}>
                                {field.ui?.label || field.name}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>

                        {/* Operator selection */}
                        <Select
                          value={condition.operator}
                          onValueChange={(value: FilterOperator) =>
                            updateCondition(condition.id, {
                              operator: value,
                              value: ''
                            })
                          }
                        >
                          <SelectTrigger className="w-32 h-8 text-xs">
                            <SelectValue placeholder="Operator" />
                          </SelectTrigger>
                          <SelectContent>
                            {field && getFieldOperators(field, !!getEnumValues(modelMetadata, field.name)).map((op) => (
                              <SelectItem key={op} value={op}>
                                {OPERATORS[op]}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>

                        {/* Value input */}
                        {renderConditionValue(condition)}

                        {/* Remove button */}
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => removeCondition(condition.id)}
                          className="text-rose-500 hover:text-rose-600 hover:bg-rose-500/10 h-8 w-8 p-0"
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

          {/* Save filter */}
          {conditions.length > 0 && (
            <div className="space-y-3 pt-4 border-t border-bn-border">
              <h3 className="text-xs font-semibold text-icon-muted uppercase tracking-wider">Save Active Filter</h3>

              {!showSaveDialog ? (
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => setShowSaveDialog(true)}
                >
                  <Save className="h-3.5 w-3.5 mr-1.5" />
                  Save current filter
                </Button>
              ) : (
                <div className="flex gap-2">
                  <Input
                    value={saveFilterName}
                    onChange={(e) => setSaveFilterName(e.target.value)}
                    placeholder="Enter filter preset name"
                    className="flex-1 h-8 text-xs"
                  />
                  <Button size="sm" onClick={saveCurrentFilter} disabled={!saveFilterName.trim()}>
                    Save
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => {
                      setShowSaveDialog(false)
                      setSaveFilterName('')
                    }}
                  >
                    Cancel
                  </Button>
                </div>
              )}
            </div>
          )}

          {/* Saved filters */}
          {savedFilters.length > 0 && (
            <div className="space-y-3 pt-4 border-t border-bn-border">
              <h3 className="text-xs font-semibold text-icon-muted uppercase tracking-wider">Saved Filter Presets</h3>
              <div className="space-y-2">
                {savedFilters.map((filter) => (
                  <div key={filter.id} className="flex items-center justify-between p-3 bg-content-bg border border-bn-border rounded-bn">
                    <div className="flex-1">
                      <div className="font-medium text-xs text-foreground">{filter.name}</div>
                      <div className="text-[11px] text-icon-muted">
                        {filter.conditions.length} rules • {new Date(filter.createdAt).toLocaleDateString()}
                      </div>
                    </div>
                    <div className="flex gap-1">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => loadSavedFilter(filter)}
                        className="h-7 w-7 p-0"
                        title="Load filter"
                      >
                        <FolderOpen className="h-3.5 w-3.5 text-primary" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => deleteSavedFilter(filter.id)}
                        className="h-7 w-7 p-0 text-rose-500 hover:text-rose-600 hover:bg-rose-500/10"
                        title="Delete filter"
                      >
                        <X className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Apply button */}
          <div className="flex justify-end gap-2 pt-4 border-t border-bn-border">
            <Button variant="secondary" size="sm" onClick={onClose}>
              Cancel
            </Button>
            <Button size="sm" onClick={onClose}>
              <Search className="h-3.5 w-3.5 mr-1.5" />
              Apply Filters
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
