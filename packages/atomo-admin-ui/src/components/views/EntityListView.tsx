/**
 * Entity List View
 *
 * Dynamically generates a list interface for any model, supporting:
 * - Virtualized scrolling
 * - Search and filtering
 * - Sorting
 * - Bulk operations
 */

import { useState, useMemo, useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'

import { 
  Search, 
  Plus, 
  ChevronLeft,
  ChevronRight,
  RefreshCw,
  Trash2,
  Filter,
  X,
  Settings
} from 'lucide-react'

import { SchemaMetadata, ModelMetadata, EntityData, QueryOptions } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { Card, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Input } from '../ui/Input'
import { EntityTable } from '../tables/EntityTable'
import { AdvancedFilterPanel, FilterCondition } from '../filters/AdvancedFilterPanel'
import { TableSettings, TableColumn } from '../tables/TableSettings'
import { Badge } from '../ui/Badge'
import { formatDate, getFieldLabel } from '../../lib/utils'
import { exportData } from '../../lib/export'

interface EntityListViewProps {
  modelName: string
  modelMetadata: ModelMetadata
  schema: SchemaMetadata
}

export function EntityListView({ modelName, modelMetadata, schema }: EntityListViewProps) {
  const navigate = useNavigate()
  
  // State management
  const [queryOptions, setQueryOptions] = useState<QueryOptions>({
    page: 1,
    limit: schema.config.defaultPageSize || 20,
    sort: 'createdAt',
    order: 'desc',
    filters: {},
    search: ''
  })
  
  const [selectedRows, setSelectedRows] = useState<string[]>([])
  const [showAdvancedFilter, setShowAdvancedFilter] = useState(false)
  const [activeFilters, setActiveFilters] = useState<FilterCondition[]>([])
  const [showTableSettings, setShowTableSettings] = useState(false)
  const [tableColumns, setTableColumns] = useState<TableColumn[]>([])
  
  // Data query
  const {
    data, 
    isLoading, 
    error, 
    refetch,
    isFetching 
  } = useQuery({
    queryKey: ['entities', modelName, queryOptions],
    queryFn: () => apiClient.listEntities(modelName, queryOptions),
    // Omit keepPreviousData so we don't show the previous model's data
    staleTime: 5 * 1000, // Data is considered fresh for 5 seconds
  })

  // Column config (based on the schema's listView configuration)
  const baseColumns = useMemo(() => {
    if (!modelMetadata?.ui?.listView || !modelMetadata?.fields) {
      return []
    }
    
    return modelMetadata.ui.listView.map(fieldName => {
      const field = modelMetadata.fields[fieldName]
      return {
        key: fieldName,
        label: getFieldLabel(fieldName, field?.ui?.label),
        type: field?.type || 'string',
        sortable: true,
        visible: true,
        render: (value: any, _row: EntityData) => {
          // Render different content depending on the field type
          switch (field?.type) {
            case 'date':
              return value ? formatDate(value) : '-'
            case 'datetime':
              // Timestamps need time-of-day: event logs / ledgers cluster many
              // rows in one day, and a date-only cell makes them indistinguishable.
              return value ? formatDate(value, 'time') : '-'
            case 'boolean':
              return value ? 'Yes' : 'No'
            case 'reference':
              return value?.name || value?.title || value || '-'
            case 'array':
              return Array.isArray(value) ? value.join(', ') : '-'
            case 'json':
              // Display handling for JSON fields
              if (!value) return '-'
              if (typeof value === 'string') {
                try {
                  const parsed = JSON.parse(value)
                  return Array.isArray(parsed) ? `[${parsed.length} items]` : `{${Object.keys(parsed).length} fields}`
                } catch {
                  return value.substring(0, 50) + (value.length > 50 ? '...' : '')
                }
              }
              if (typeof value === 'object') {
                return Array.isArray(value) ? `[${value.length} items]` : `{${Object.keys(value).length} fields}`
              }
              return JSON.stringify(value).substring(0, 50) + '...'
            default:
              return value || '-'
          }
        }
      }
    })
  }, [modelMetadata, modelName])

  // 🔧 Reset table state when switching models
  useEffect(() => {
    // Reset query options to defaults
    setQueryOptions({
      page: 1,
      limit: schema.config.defaultPageSize || 20,
      sort: 'createdAt',
      order: 'desc',
      filters: {},
      search: ''
    })
    // Reset other state
    setSelectedRows([])
    setActiveFilters([])
    setShowAdvancedFilter(false)
    setShowTableSettings(false)
    // Reset table column config so baseColumns reinitializes
    setTableColumns([])
  }, [modelName, schema.config.defaultPageSize])

  // Initialize table column config
  useEffect(() => {
    if (baseColumns.length > 0) {
      setTableColumns(baseColumns)
    }
  }, [baseColumns])

  // Currently visible columns
  const visibleColumns = tableColumns.filter(col => col.visible)

  // Handle search
  const handleSearch = (searchTerm: string) => {
    setQueryOptions(prev => ({
      ...prev,
      search: searchTerm,
      page: 1 // Reset to the first page
    }))
  }

  // Handle sorting
  const handleSort = (field: string) => {
    setQueryOptions(prev => ({
      ...prev,
      sort: field,
      order: prev.sort === field && prev.order === 'asc' ? 'desc' : 'asc',
      page: 1
    }))
  }

  // Handle pagination
  const handlePageChange = (page: number) => {
    setQueryOptions(prev => ({ ...prev, page }))
  }

  // Bulk delete
  const handleBulkDelete = async () => {
    if (selectedRows.length === 0) return

    if (!confirm(`Are you sure you want to delete the ${selectedRows.length} selected items?`)) {
      return
    }

    try {
      await apiClient.bulkDelete(modelName, selectedRows)
      setSelectedRows([])
      refetch()
    } catch (error) {
      console.error('Bulk delete failed:', error)
      alert('Delete failed, please try again')
    }
  }

  // Delete a single row
  const handleRowDelete = async (row: EntityData) => {
    try {
      await apiClient.deleteEntity(modelName, row.id)
      refetch()
    } catch (error) {
      console.error('Delete failed:', error)
      alert('Delete failed, please try again')
    }
  }

  // Edit a row
  const handleRowEdit = (row: EntityData) => {
    navigate(`/entities/${modelName}/${row.id}`)
  }

  // Export data
  const handleExport = async (format: 'csv' | 'excel') => {
    if (!data?.data || visibleColumns.length === 0) {
      alert('No data to export')
      return
    }

    try {
      await exportData(
        data.data,
        visibleColumns,
        format,
        `${modelName}_${new Date().toISOString().split('T')[0]}`
      )
    } catch (error) {
      console.error('Export failed:', error)
      alert('Export failed, please try again')
    }
  }

  if (error) {
    return (
      <Card className="m-6">
        <CardContent className="py-8 text-center">
          <h3 className="text-lg font-semibold text-gray-900 mb-2">Failed to Load</h3>
          <p className="text-gray-600 mb-4">Unable to load {modelName} data</p>
          <Button onClick={() => refetch()}>Retry</Button>
        </CardContent>
      </Card>
    )
  }

  const totalPages = data ? Math.ceil(data.total / queryOptions.limit!) : 0

  return (
    <div className="p-6 space-y-6">
      {/* Page header and actions */}
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">
            {getFieldLabel(modelName)} List
          </h1>
          <p className="text-gray-600 mt-1">
            {data && `${data.total} items total`}
          </p>
        </div>
        
        <div className="flex gap-3">
          <Button
            variant="secondary"
            onClick={() => refetch()}
            disabled={isFetching}
          >
            <RefreshCw className={`h-4 w-4 mr-2 ${isFetching ? 'animate-spin' : ''}`} />
            Refresh
          </Button>

          <Button
            variant="secondary"
            onClick={() => setShowTableSettings(true)}
          >
            <Settings className="h-4 w-4 mr-2" />
            Table Settings
          </Button>

          <Button onClick={() => navigate(`/entities/${modelName}/new`)}>
            <Plus className="h-4 w-4 mr-2" />
            New {getFieldLabel(modelName)}
          </Button>
        </div>
      </div>

      {/* Search and filter bar */}
      <Card>
        <CardContent className="p-4">
          <div className="space-y-4">
            {/* Search bar and tool buttons */}
            <div className="flex gap-4 items-center">
              <div className="flex-1 relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-gray-400" />
                <Input
                  placeholder={`Search ${getFieldLabel(modelName)}...`}
                  value={queryOptions.search}
                  onChange={(e) => handleSearch(e.target.value)}
                  className="pl-9 max-w-sm"
                />
              </div>
              
              <Button
                variant="secondary"
                onClick={() => setShowAdvancedFilter(true)}
                className={activeFilters.length > 0 ? 'bg-primary-100 text-primary-700' : ''}
              >
                <Filter className="h-4 w-4 mr-2" />
                Advanced Filter
                {activeFilters.length > 0 && (
                  <Badge variant="default" className="ml-2 h-5 w-5 p-0 flex items-center justify-center">
                    {activeFilters.length}
                  </Badge>
                )}
              </Button>
              
              {selectedRows.length > 0 && (
                <div className="flex items-center gap-2">
                  <span className="text-sm text-gray-600">
                    {selectedRows.length} items selected
                  </span>
                  <Button
                    variant="danger"
                    size="sm"
                    onClick={handleBulkDelete}
                  >
                    <Trash2 className="h-4 w-4 mr-1" />
                    Delete Selected
                  </Button>
                </div>
              )}
            </div>

            {/* Active filter conditions */}
            {activeFilters.length > 0 && (
              <div className="flex items-center gap-2 flex-wrap">
                <span className="text-sm text-gray-600">Active filters:</span>
                {activeFilters.map((filter, index) => (
                  <Badge
                    key={filter.id}
                    variant="secondary"
                    className="flex items-center gap-1"
                  >
                    {index > 0 && (
                      <span className="text-xs opacity-70">
                        {filter.logicalOperator || 'AND'}
                      </span>
                    )}
                    <span>
                      {modelMetadata.fields[filter.field]?.ui?.label || filter.field}
                    </span>
                    <span className="opacity-70">
                      {filter.operator === 'equals' ? '=' :
                       filter.operator === 'contains' ? 'contains' :
                       filter.operator}
                    </span>
                    {!['is_null', 'is_not_null', 'is_empty', 'is_not_empty'].includes(filter.operator) && (
                      <span>{filter.value}</span>
                    )}
                    <button
                      onClick={() => {
                        const newFilters = activeFilters.filter(f => f.id !== filter.id)
                        setActiveFilters(newFilters)
                        // This should trigger the actual filtering logic
                      }}
                      className="ml-1 hover:bg-gray-200 rounded-full p-0.5"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </Badge>
                ))}
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setActiveFilters([])}
                  className="text-gray-500 hover:text-gray-700"
                >
                  Clear Filters
                </Button>
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Data table */}
      <Card>
        <CardContent className="p-0">
          {visibleColumns.length === 0 ? (
            <div className="p-8 text-center text-gray-500">
              Loading {modelName} list configuration...
            </div>
          ) : (
            <EntityTable
              data={data?.data || []}
              columns={visibleColumns}
              loading={isLoading}
              selectedRows={selectedRows}
              onSelectionChange={setSelectedRows}
              onSort={handleSort}
              sortField={queryOptions.sort}
              sortOrder={queryOptions.order}
              onRowClick={(row) => navigate(`/entities/${modelName}/${row.id}`)}
            />
          )}
        </CardContent>
      </Card>

      {/* Pagination */}
      {data && totalPages > 1 && (
        <div className="flex justify-between items-center">
          <div className="text-sm text-gray-600">
            Showing {(queryOptions.page! - 1) * queryOptions.limit! + 1} - {Math.min(queryOptions.page! * queryOptions.limit!, data.total)} of {data.total} items
          </div>
          
          <div className="flex gap-2">
            <Button
              variant="secondary"
              size="sm"
              onClick={() => handlePageChange(queryOptions.page! - 1)}
              disabled={queryOptions.page! <= 1}
            >
              <ChevronLeft className="h-4 w-4" />
              Previous
            </Button>

            <span className="flex items-center px-3 py-1 text-sm">
              Page {queryOptions.page} / {totalPages}
            </span>

            <Button
              variant="secondary"
              size="sm"
              onClick={() => handlePageChange(queryOptions.page! + 1)}
              disabled={queryOptions.page! >= totalPages}
            >
              Next
              <ChevronRight className="h-4 w-4" />
            </Button>
          </div>
        </div>
      )}

      {/* Advanced filter panel */}
      <AdvancedFilterPanel
        modelMetadata={modelMetadata}
        onFiltersChange={setActiveFilters}
        initialConditions={activeFilters}
        isOpen={showAdvancedFilter}
        onClose={() => setShowAdvancedFilter(false)}
      />

      {/* Table settings panel */}
      <TableSettings
        columns={tableColumns}
        onColumnsChange={setTableColumns}
        onExport={handleExport}
        isOpen={showTableSettings}
        onClose={() => setShowTableSettings(false)}
      />
    </div>
  )
}
