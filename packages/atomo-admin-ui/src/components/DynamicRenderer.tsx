/**
 * Dynamic Renderer - Atomo Admin UI 的核心渲染引擎
 * 
 * 这是整个 Admin UI 的"大脑"，负责根据 Schema 元数据动态生成界面
 */

import React, { Suspense } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useLocation } from 'react-router-dom'
import { apiClient } from '../lib/api'
import { SchemaMetadata, ModelMetadata } from '../lib/types'
import { EntityListView } from './views/EntityListView'
import { EntityDetailView } from './views/EntityDetailView'
import { Dashboard } from './views/Dashboard'
import { WorkflowsView } from './views/WorkflowsView'
import { TrashView } from './views/TrashView'
import { Card, CardContent } from './ui/Card'
import { Spinner } from './ui/Spinner'
import { componentPluginManager } from '../lib/component-plugins'

export interface DynamicRendererProps {
  /**
   * 当前路由信息
   */
  route: {
    type: 'dashboard' | 'list' | 'detail' | 'create' | 'edit' | 'kanban' | 'timeline' | 'plugin' | 'workflows' | 'trash'
    modelName?: string
    entityId?: string
    contactId?: string
    pluginHandler?: any
  }
}

/**
 * 动态渲染引擎组件
 */
export function DynamicRenderer({ route }: DynamicRendererProps) {
  // 加载 Schema 元数据
  const { 
    data: schema, 
    isLoading, 
    error 
  } = useQuery({
    queryKey: ['schema-metadata'],
    queryFn: () => apiClient.getSchemaMetadata(),
    staleTime: 5 * 60 * 1000, // 5分钟缓存
  })

  // 错误状态
  if (error) {
    return (
      <Card className="m-6">
        <CardContent className="flex items-center justify-center py-8">
          <div className="text-center">
            <h3 className="text-lg font-semibold text-gray-900 mb-2">
              无法加载 Schema 元数据
            </h3>
            <p className="text-gray-600 mb-4">
              请检查 Atomo Server 是否正常运行
            </p>
            <button 
              onClick={() => window.location.reload()}
              className="px-4 py-2 bg-primary-600 text-white rounded-md hover:bg-primary-700"
            >
              重新加载
            </button>
          </div>
        </CardContent>
      </Card>
    )
  }

  // 加载状态
  if (isLoading || !schema) {
    return (
      <Card className="m-6">
        <CardContent className="flex items-center justify-center py-8">
          <div className="text-center">
            <Spinner className="mx-auto mb-4" />
            <p className="text-gray-600">正在加载 Schema 元数据...</p>
          </div>
        </CardContent>
      </Card>
    )
  }

  // 根据路由类型渲染对应视图
  switch (route.type) {
    case 'dashboard':
      return <Dashboard schema={schema} />

    case 'workflows':
      return <WorkflowsView />

    case 'trash':
      return <TrashView schema={schema} />
      
    case 'list':
      if (!route.modelName) {
        return <div>错误：缺少模型名称</div>
      }
      
      const modelMetadata = schema.models[route.modelName]
      if (!modelMetadata) {
        return <div>错误：未找到模型 {route.modelName}</div>
      }
      
      return (
        <EntityListView 
          modelName={route.modelName}
          modelMetadata={modelMetadata}
          schema={schema}
        />
      )
      
    case 'detail':
    case 'edit':
      if (!route.modelName || !route.entityId) {
        return <div>错误：缺少模型名称或实体ID</div>
      }
      
      const detailModelMetadata = schema.models[route.modelName]
      if (!detailModelMetadata) {
        return <div>错误：未找到模型 {route.modelName}</div>
      }
      
      return (
        <EntityDetailView
          modelName={route.modelName}
          entityId={route.entityId}
          modelMetadata={detailModelMetadata}
          schema={schema}
          mode={route.type}
        />
      )
      
    case 'create':
      if (!route.modelName) {
        return <div>错误：缺少模型名称</div>
      }
      
      const createModelMetadata = schema.models[route.modelName]
      if (!createModelMetadata) {
        return <div>错误：未找到模型 {route.modelName}</div>
      }
      
      return (
        <EntityDetailView
          modelName={route.modelName}
          modelMetadata={createModelMetadata}
          schema={schema}
          mode="create"
        />
      )
    case 'kanban':
      // Try to get component from plugin system
      const kanbanHandler = componentPluginManager.getRouteHandler('/deals/board')
      if (kanbanHandler) {
        const Component = kanbanHandler.component
        return (
          <Suspense fallback={
            <Card className="m-6">
              <CardContent className="flex items-center justify-center py-8">
                <div className="text-center">
                  <Spinner className="mx-auto mb-4" />
                  <p className="text-gray-600">正在加载看板组件...</p>
                </div>
              </CardContent>
            </Card>
          }>
            <Component {...kanbanHandler.props} />
          </Suspense>
        )
      }
      return <div>错误：未找到看板组件</div>
    case 'timeline':
      if (!route.contactId) return <div>错误：缺少联系人ID</div>
      // Try to get component from plugin system
      const timelinePath = `/contacts/${route.contactId}/timeline`
      const timelineHandler = componentPluginManager.getRouteHandler(timelinePath)
      if (timelineHandler) {
        const Component = timelineHandler.component
        return (
          <Suspense fallback={
            <Card className="m-6">
              <CardContent className="flex items-center justify-center py-8">
                <div className="text-center">
                  <Spinner className="mx-auto mb-4" />
                  <p className="text-gray-600">正在加载时间线组件...</p>
                </div>
              </CardContent>
            </Card>
          }>
            <Component {...timelineHandler.props} />
          </Suspense>
        )
      }
      return <div>错误：未找到时间线组件</div>
      
    case 'plugin':
      if (!route.pluginHandler) return <div>错误：缺少插件处理器</div>
      const PluginComponent = route.pluginHandler.component
      return (
        <Suspense fallback={
          <Card className="m-6">
            <CardContent className="flex items-center justify-center py-8">
              <div className="text-center">
                <Spinner className="mx-auto mb-4" />
                <p className="text-gray-600">正在加载插件组件...</p>
              </div>
            </CardContent>
          </Card>
        }>
          <PluginComponent {...route.pluginHandler.props} />
        </Suspense>
      )

    default:
      return <div>错误：未知的路由类型</div>
  }
}

export function useRouteParser(): DynamicRendererProps['route'] {
  const location = useLocation()

  const parseRoute = (path: string): DynamicRendererProps['route'] => {
    // Dashboard
    if (path === '/' || path === '/dashboard') {
      return { type: 'dashboard' as const }
    }

    // Workflows management page
    if (path === '/workflows') {
      return { type: 'workflows' as const }
    }

    // Trash / soft-delete management page
    if (path === '/trash') {
      return { type: 'trash' as const }
    }
    
    // Entity routes: /entities/:modelName
    const entityListMatch = path.match(/^\/entities\/([^\/]+)$/)
    if (entityListMatch) {
      return {
        type: 'list' as const,
        modelName: entityListMatch[1]
      }
    }
    
    // Create new: /entities/:modelName/new (必须在详情路由之前匹配)
    const entityCreateMatch = path.match(/^\/entities\/([^\/]+)\/new$/)
    if (entityCreateMatch) {
      return {
        type: 'create' as const,
        modelName: entityCreateMatch[1]
      }
    }
    
    // Entity detail/edit: /entities/:modelName/:entityId
    const entityDetailMatch = path.match(/^\/entities\/([^\/]+)\/([^\/]+)$/)
    if (entityDetailMatch) {
      return {
        type: 'detail' as const,
        modelName: entityDetailMatch[1],
        entityId: entityDetailMatch[2]
      }
    }
    
    // Check for plugin-handled routes first
    const pluginHandler = componentPluginManager.getRouteHandler(path)
    if (pluginHandler) {
      return { type: 'plugin' as const, pluginHandler }
    }

    // Fallback to generic routes
    return { type: 'dashboard' as const }
  }

  return parseRoute(location.pathname)
}
