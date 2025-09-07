/**
 * Entity Detail View - 实体详情/编辑视图
 * 
 * 支持查看、编辑和创建实体，动态生成表单
 */

import React, { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { 
  ArrowLeft, 
  Save, 
  Edit2, 
  Trash2, 
  Eye,
  Clock,
  User
} from 'lucide-react'

import { SchemaMetadata, ModelMetadata, EntityData } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../ui/Tabs'
import { DynamicForm } from '../forms/DynamicForm'
import { formatDate, getFieldLabel } from '../../lib/utils'

interface EntityDetailViewProps {
  modelName: string
  entityId?: string // undefined 表示创建模式
  modelMetadata: ModelMetadata
  schema: SchemaMetadata
  mode: 'detail' | 'edit' | 'create'
}

export function EntityDetailView({ 
  modelName, 
  entityId, 
  modelMetadata, 
  schema, 
  mode: initialMode 
}: EntityDetailViewProps) {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [mode, setMode] = useState(initialMode)
  
  // 获取实体数据（仅在非创建模式下）
  const { 
    data: entity, 
    isLoading, 
    error 
  } = useQuery({
    queryKey: ['entity', modelName, entityId],
    queryFn: () => apiClient.getEntity(modelName, entityId!),
    enabled: !!entityId && mode !== 'create',
  })

  // 更新实体
  const updateMutation = useMutation({
    mutationFn: (data: Partial<EntityData>) => 
      apiClient.updateEntity(modelName, entityId!, data),
    onSuccess: () => {
      queryClient.invalidateQueries(['entity', modelName, entityId])
      queryClient.invalidateQueries(['entities', modelName])
      setMode('detail')
    },
  })

  // 创建实体
  const createMutation = useMutation({
    mutationFn: (data: Partial<EntityData>) => 
      apiClient.createEntity(modelName, data),
    onSuccess: (newEntity) => {
      queryClient.invalidateQueries(['entities', modelName])
      navigate(`/entities/${modelName}`)
    },
  })

  // 删除实体
  const deleteMutation = useMutation({
    mutationFn: () => apiClient.deleteEntity(modelName, entityId!),
    onSuccess: () => {
      queryClient.invalidateQueries(['entities', modelName])
      navigate(`/entities/${modelName}`)
    },
  })

  // 表单提交
  const handleSave = async (formData: any) => {
    try {
      if (mode === 'create') {
        await createMutation.mutateAsync(formData)
      } else {
        await updateMutation.mutateAsync(formData)
      }
    } catch (error) {
      console.error('保存失败:', error)
      alert('保存失败，请重试')
    }
  }

  // 删除确认
  const handleDelete = () => {
    if (confirm(`确定要删除这个 ${getFieldLabel(modelName)} 吗？`)) {
      deleteMutation.mutate()
    }
  }

  // 错误状态
  if (error) {
    return (
      <Card className="m-6">
        <CardContent className="py-8 text-center">
          <h3 className="text-lg font-semibold text-gray-900 mb-2">加载失败</h3>
          <p className="text-gray-600 mb-4">无法加载 {getFieldLabel(modelName)} 数据</p>
          <Button onClick={() => navigate(`/entities/${modelName}`)}>
            返回列表
          </Button>
        </CardContent>
      </Card>
    )
  }

  const isCreate = mode === 'create'
  const isEdit = mode === 'edit'
  const isDetail = mode === 'detail'

  return (
    <div className="p-6 space-y-6">
      {/* 页面标题和操作 */}
      <div className="flex justify-between items-center">
        <div className="flex items-center gap-4">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => navigate(`/entities/${modelName}`)}
          >
            <ArrowLeft className="h-4 w-4 mr-2" />
            返回列表
          </Button>
          
          <div>
            <h1 className="text-3xl font-bold text-gray-900">
              {isCreate ? `新建 ${getFieldLabel(modelName)}` : 
               isEdit ? `编辑 ${getFieldLabel(modelName)}` : 
               getFieldLabel(modelName)}
            </h1>
            {entity && (
              <p className="text-gray-600 mt-1">
                ID: {entity.id}
              </p>
            )}
          </div>
        </div>
        
        <div className="flex gap-3">
          {modelName === 'Contact' && entity && isDetail && (
            <Button
              variant="secondary"
              onClick={() => navigate(`/contacts/${entity.id}/timeline`)}
            >
              查看时间线
            </Button>
          )}
          {isDetail && (
            <>
              <Button
                variant="secondary"
                onClick={() => setMode('edit')}
              >
                <Edit2 className="h-4 w-4 mr-2" />
                编辑
              </Button>
              
              <Button
                variant="danger"
                onClick={handleDelete}
                disabled={deleteMutation.isLoading}
              >
                <Trash2 className="h-4 w-4 mr-2" />
                删除
              </Button>
            </>
          )}
          
          {isEdit && (
            <Button
              variant="secondary"
              onClick={() => setMode('detail')}
            >
              <Eye className="h-4 w-4 mr-2" />
              查看
            </Button>
          )}
        </div>
      </div>

      {/* 主要内容 */}
      {isLoading ? (
        <Card>
          <CardContent className="py-8 text-center">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600 mx-auto"></div>
            <p className="mt-4 text-gray-600">加载中...</p>
          </CardContent>
        </Card>
      ) : (
        <Tabs defaultValue="details" className="space-y-6">
          <TabsList>
            <TabsTrigger value="details">详情</TabsTrigger>
            {entity && (
              <>
                <TabsTrigger value="history">历史记录</TabsTrigger>
                <TabsTrigger value="relations">关联数据</TabsTrigger>
              </>
            )}
          </TabsList>

          {/* 详情标签页 */}
          <TabsContent value="details">
            <Card>
              <CardContent className="p-6">
                <DynamicForm
                  modelName={modelName}
                  modelMetadata={modelMetadata}
                  schema={schema}
                  initialData={entity}
                  onSubmit={handleSave}
                  mode={isDetail ? 'view' : isEdit ? 'edit' : 'create'}
                  loading={updateMutation.isLoading || createMutation.isLoading}
                />
              </CardContent>
            </Card>
          </TabsContent>

          {/* 历史记录标签页 */}
          {entity && (
            <TabsContent value="history">
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <Clock className="h-5 w-5" />
                    变更历史
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="space-y-4">
                    <div className="flex items-center gap-3 p-3 bg-gray-50 rounded-md">
                      <User className="h-4 w-4 text-gray-500" />
                      <div className="flex-1">
                        <div className="text-sm font-medium">创建记录</div>
                        <div className="text-xs text-gray-500">
                          {formatDate(entity.createdAt, 'time')}
                        </div>
                      </div>
                    </div>
                    
                    {entity.updatedAt !== entity.createdAt && (
                      <div className="flex items-center gap-3 p-3 bg-gray-50 rounded-md">
                        <Edit2 className="h-4 w-4 text-gray-500" />
                        <div className="flex-1">
                          <div className="text-sm font-medium">最后更新</div>
                          <div className="text-xs text-gray-500">
                            {formatDate(entity.updatedAt, 'time')}
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                </CardContent>
              </Card>
            </TabsContent>
          )}

          {/* 关联数据标签页 */}
          {entity && (
            <TabsContent value="relations">
              <Card>
                <CardHeader>
                  <CardTitle>关联数据</CardTitle>
                </CardHeader>
                <CardContent>
                  <p className="text-gray-600">关联数据功能正在开发中...</p>
                </CardContent>
              </Card>
            </TabsContent>
          )}
        </Tabs>
      )}
    </div>
  )
}
