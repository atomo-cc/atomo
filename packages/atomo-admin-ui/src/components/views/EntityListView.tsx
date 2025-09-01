/**
 * Entity List View - 实体列表视图
 * 
 * 动态生成任何模型的列表界面，支持：
 * - 虚拟化滚动
 * - 搜索和筛选
 * - 排序
 * - 批量操作
 */

import React, { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { useVirtualizer } from '@tanstack/react-virtual'
import { 
  Search, 
  Plus, 
  MoreHorizontal, 
  ChevronLeft,
  ChevronRight,
  RefreshCw,
  Download,
  Trash2
} from 'lucide-react'

import { SchemaMetadata, ModelMetadata, EntityData, QueryOptions } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Input } from '../ui/Input'
import { EntityTable } from '../tables/EntityTable'
import { formatDate, getFieldLabel } from '../../lib/utils'

interface EntityListViewProps {
  modelName: string
  modelMetadata: ModelMetadata
  schema: SchemaMetadata
}

export function EntityListView({ modelName, modelMetadata, schema }: EntityListViewProps) {
  const navigate = useNavigate()
  
  // 状态管理
  const [queryOptions, setQueryOptions] = useState<QueryOptions>({
    page: 1,
    limit: schema.config.defaultPageSize || 20,
    sort: 'createdAt',
    order: 'desc',
    filters: {},
    search: ''
  })
  
  const [selectedRows, setSelectedRows] = useState<string[]>([])
  
  // 数据查询
  const { 
    data, 
    isLoading, 
    error, 
    refetch,
    isFetching 
  } = useQuery({
    queryKey: ['entities', modelName, queryOptions],
    queryFn: () => apiClient.listEntities(modelName, queryOptions),
    keepPreviousData: true,
  })

  // 列配置（基于 schema 的 listView 配置）
  const columns = useMemo(() => {
    return modelMetadata.ui.listView.map(fieldName => {
      const field = modelMetadata.fields[fieldName]
      return {
        key: fieldName,
        label: getFieldLabel(fieldName, field?.ui?.label),
        type: field?.type || 'string',
        sortable: true,
        render: (value: any, row: EntityData) => {
          // 根据字段类型渲染不同的内容
          switch (field?.type) {
            case 'date':
            case 'datetime':
              return value ? formatDate(value) : '-'
            case 'boolean':
              return value ? '是' : '否'
            case 'reference':
              return value?.name || value?.title || value || '-'
            case 'array':
              return Array.isArray(value) ? value.join(', ') : '-'
            default:
              return value || '-'
          }
        }
      }
    })
  }, [modelMetadata])

  // 搜索处理
  const handleSearch = (searchTerm: string) => {
    setQueryOptions(prev => ({
      ...prev,
      search: searchTerm,
      page: 1 // 重置到第一页
    }))
  }

  // 排序处理
  const handleSort = (field: string) => {
    setQueryOptions(prev => ({
      ...prev,
      sort: field,
      order: prev.sort === field && prev.order === 'asc' ? 'desc' : 'asc',
      page: 1
    }))
  }

  // 分页处理
  const handlePageChange = (page: number) => {
    setQueryOptions(prev => ({ ...prev, page }))
  }

  // 批量删除
  const handleBulkDelete = async () => {
    if (selectedRows.length === 0) return
    
    if (!confirm(`确定要删除选中的 ${selectedRows.length} 个项目吗？`)) {
      return
    }
    
    try {
      await apiClient.bulkDelete(modelName, selectedRows)
      setSelectedRows([])
      refetch()
    } catch (error) {
      console.error('批量删除失败:', error)
      alert('删除失败，请重试')
    }
  }

  if (error) {
    return (
      <Card className="m-6">
        <CardContent className="py-8 text-center">
          <h3 className="text-lg font-semibold text-gray-900 mb-2">加载失败</h3>
          <p className="text-gray-600 mb-4">无法加载 {modelName} 数据</p>
          <Button onClick={() => refetch()}>重试</Button>
        </CardContent>
      </Card>
    )
  }

  const totalPages = data ? Math.ceil(data.total / queryOptions.limit!) : 0

  return (
    <div className="p-6 space-y-6">
      {/* 页面标题和操作 */}
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">
            {getFieldLabel(modelName)} 列表
          </h1>
          <p className="text-gray-600 mt-1">
            {data && `共 ${data.total} 个项目`}
          </p>
        </div>
        
        <div className="flex gap-3">
          <Button
            variant="secondary"
            onClick={() => refetch()}
            disabled={isFetching}
          >
            <RefreshCw className={`h-4 w-4 mr-2 ${isFetching ? 'animate-spin' : ''}`} />
            刷新
          </Button>
          
          <Button onClick={() => navigate(`/entities/${modelName}/new`)}>
            <Plus className="h-4 w-4 mr-2" />
            新建 {getFieldLabel(modelName)}
          </Button>
        </div>
      </div>

      {/* 搜索和筛选栏 */}
      <Card>
        <CardContent className="p-4">
          <div className="flex gap-4 items-center">
            <div className="flex-1">
              <Input
                placeholder={`搜索 ${getFieldLabel(modelName)}...`}
                value={queryOptions.search}
                onChange={(e) => handleSearch(e.target.value)}
                className="max-w-sm"
              />
            </div>
            
            {selectedRows.length > 0 && (
              <div className="flex items-center gap-2">
                <span className="text-sm text-gray-600">
                  已选择 {selectedRows.length} 个项目
                </span>
                <Button
                  variant="danger"
                  size="sm"
                  onClick={handleBulkDelete}
                >
                  <Trash2 className="h-4 w-4 mr-1" />
                  删除选中
                </Button>
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      {/* 数据表格 */}
      <Card>
        <CardContent className="p-0">
          <EntityTable
            data={data?.data || []}
            columns={columns}
            loading={isLoading}
            selectedRows={selectedRows}
            onSelectionChange={setSelectedRows}
            onSort={handleSort}
            sortField={queryOptions.sort}
            sortOrder={queryOptions.order}
            onRowClick={(row) => navigate(`/entities/${modelName}/${row.id}`)}
          />
        </CardContent>
      </Card>

      {/* 分页 */}
      {data && totalPages > 1 && (
        <div className="flex justify-between items-center">
          <div className="text-sm text-gray-600">
            显示第 {(queryOptions.page! - 1) * queryOptions.limit! + 1} - {Math.min(queryOptions.page! * queryOptions.limit!, data.total)} 个，
            共 {data.total} 个项目
          </div>
          
          <div className="flex gap-2">
            <Button
              variant="secondary"
              size="sm"
              onClick={() => handlePageChange(queryOptions.page! - 1)}
              disabled={queryOptions.page! <= 1}
            >
              <ChevronLeft className="h-4 w-4" />
              上一页
            </Button>
            
            <span className="flex items-center px-3 py-1 text-sm">
              第 {queryOptions.page} / {totalPages} 页
            </span>
            
            <Button
              variant="secondary"
              size="sm"
              onClick={() => handlePageChange(queryOptions.page! + 1)}
              disabled={queryOptions.page! >= totalPages}
            >
              下一页
              <ChevronRight className="h-4 w-4" />
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
