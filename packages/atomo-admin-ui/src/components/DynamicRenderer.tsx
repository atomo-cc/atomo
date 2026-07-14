/**
 * Dynamic Renderer — the core rendering engine of the Atomo Admin UI
 * 
 * Drives the whole Admin UI: builds the interface dynamically from the schema metadata.
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
import { WorkflowDesigner } from './views/WorkflowDesigner'
import { TrashView } from './views/TrashView'
import { ObservabilityView } from './views/ObservabilityView'
import { Settings } from './views/Settings'
import { Help } from './views/Help'
import { Card, CardContent } from './ui/Card'
import { Spinner } from './ui/Spinner'
import { componentPluginManager } from '../lib/component-plugins'

export interface DynamicRendererProps {
  /**
   * Current route info
   */
  route: {
    type: 'dashboard' | 'list' | 'detail' | 'create' | 'edit' | 'plugin' | 'workflows' | 'trash' | 'workflow-design' | 'settings' | 'help' | 'observability' | 'not-found'
    modelName?: string
    entityId?: string
    pluginHandler?: any
    workflowName?: string
  }
}

/**
 * Dynamic rendering engine component
 */
export function DynamicRenderer({ route }: DynamicRendererProps) {
  // Load schema metadata
  const {
    data: schema,
    isLoading,
    error
  } = useQuery({
    queryKey: ['schema-metadata'],
    queryFn: () => apiClient.getSchemaMetadata(),
    staleTime: 5 * 60 * 1000, // 5-minute cache
  })

  // Error state
  if (error) {
    return (
      <Card className="m-6">
        <CardContent className="flex items-center justify-center py-8">
          <div className="text-center">
            <h3 className="text-lg font-semibold text-gray-900 mb-2">
              Couldn’t load schema metadata
            </h3>
            <p className="text-gray-600 mb-4">
              Check that the Atomo server is running and reachable.
            </p>
            <button
              onClick={() => window.location.reload()}
              className="px-4 py-2 bg-primary-600 text-white rounded-md hover:bg-primary-700"
            >
              Reload
            </button>
          </div>
        </CardContent>
      </Card>
    )
  }

  // Loading state
  if (isLoading || !schema) {
    return (
      <Card className="m-6">
        <CardContent className="flex items-center justify-center py-8">
          <div className="text-center">
            <Spinner className="mx-auto mb-4" />
            <p className="text-gray-600">Loading schema metadata…</p>
          </div>
        </CardContent>
      </Card>
    )
  }

  // Inline error state for a malformed route or an unknown model.
  const routeError = (message: string) => (
    <Card className="m-6">
      <CardContent className="py-8 text-center text-gray-600">{message}</CardContent>
    </Card>
  )

  // Render the view for the current route type
  switch (route.type) {
    case 'dashboard':
      return <Dashboard schema={schema} />

    case 'workflows':
      return <WorkflowsView />

    case 'workflow-design':
      return <WorkflowDesigner workflowName={route.workflowName} />

    case 'trash':
      return <TrashView schema={schema} />

    case 'observability':
      return <ObservabilityView />

    case 'settings':
      return <Settings schema={schema} />

    case 'help':
      return <Help />

    case 'not-found':
      return (
        <Card className="m-6">
          <CardContent className="py-12 text-center">
            <h3 className="text-lg font-semibold text-gray-900 mb-1">Page not found</h3>
            <p className="text-gray-600 mb-4">This route doesn’t exist in the admin.</p>
            <a
              href={(import.meta as any).env.BASE_URL}
              className="inline-block px-4 py-2 bg-primary-600 text-white rounded-md hover:bg-primary-700"
            >
              Back to dashboard
            </a>
          </CardContent>
        </Card>
      )

    case 'list':
      if (!route.modelName) {
        return routeError('Missing model name.')
      }
      
      const modelMetadata = schema.models[route.modelName]
      if (!modelMetadata) {
        return routeError(`Model not found: ${route.modelName}`)
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
        return routeError('Missing model name or record ID.')
      }
      
      const detailModelMetadata = schema.models[route.modelName]
      if (!detailModelMetadata) {
        return routeError(`Model not found: ${route.modelName}`)
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
        return routeError('Missing model name.')
      }
      
      const createModelMetadata = schema.models[route.modelName]
      if (!createModelMetadata) {
        return routeError(`Model not found: ${route.modelName}`)
      }
      
      return (
        <EntityDetailView
          modelName={route.modelName}
          modelMetadata={createModelMetadata}
          schema={schema}
          mode="create"
        />
      )
    case 'plugin':
      if (!route.pluginHandler) return routeError('Missing plugin handler.')
      const PluginComponent = route.pluginHandler.component
      return (
        <Suspense fallback={
          <Card className="m-6">
            <CardContent className="flex items-center justify-center py-8">
              <div className="text-center">
                <Spinner className="mx-auto mb-4" />
                <p className="text-gray-600">Loading plugin component…</p>
              </div>
            </CardContent>
          </Card>
        }>
          <PluginComponent {...route.pluginHandler.props} />
        </Suspense>
      )

    default:
      return routeError('Unknown route type.')
  }
}

export function useRouteParser(): DynamicRendererProps['route'] {
  const location = useLocation()

  const parseRoute = (path: string): DynamicRendererProps['route'] => {
    // Dashboard
    if (path === '/' || path === '/dashboard') {
      return { type: 'dashboard' as const }
    }

    // Workflow designer (must match before the bare /workflows route)
    const wfDesignEdit = path.match(/^\/workflows\/design\/(.+)$/)
    if (wfDesignEdit) {
      return { type: 'workflow-design' as const, workflowName: decodeURIComponent(wfDesignEdit[1]) }
    }
    if (path === '/workflows/design') {
      return { type: 'workflow-design' as const }
    }

    // Workflows management page
    if (path === '/workflows') {
      return { type: 'workflows' as const }
    }

    // Trash / soft-delete management page
    if (path === '/trash') {
      return { type: 'trash' as const }
    }

    // Observability: job-queue health + recent activity (admin-gated server-side)
    if (path === '/observability') {
      return { type: 'observability' as const }
    }

    // Settings + Help pages
    if (path === '/settings') {
      return { type: 'settings' as const }
    }
    if (path === '/help') {
      return { type: 'help' as const }
    }

    // Entity routes: /entities/:modelName
    const entityListMatch = path.match(/^\/entities\/([^\/]+)$/)
    if (entityListMatch) {
      return {
        type: 'list' as const,
        modelName: entityListMatch[1]
      }
    }
    
    // Create new: /entities/:modelName/new (must match before the detail route)
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

    // Anything unmatched is a real 404 (previously this silently showed the dashboard).
    return { type: 'not-found' as const }
  }

  return parseRoute(location.pathname)
}
