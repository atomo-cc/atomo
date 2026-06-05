/**
 * Dashboard View
 *
 * Displays a system overview and quick actions.
 */

import React from 'react'
import { useQuery } from '@tanstack/react-query'
import { SchemaMetadata } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { useNavigate } from 'react-router-dom'
import { 
  Users,
  Building2,
  DollarSign,
  Activity,
  Plus,
  TrendingUp
} from 'lucide-react'

interface DashboardProps {
  schema: SchemaMetadata
}

export function Dashboard({ schema }: DashboardProps) {
  const navigate = useNavigate()

  // Extract the list of models from the schema
  const models = Object.keys(schema.models)

  // Real per-model record counts (one cheap count query per model). A failed count
  // shows "—" rather than breaking the card.
  const { data: counts } = useQuery({
    queryKey: ['dashboard-counts', models],
    queryFn: async () => {
      const entries = await Promise.all(
        models.map(async (m) => {
          try {
            const res = await apiClient.listEntities(m, { limit: 1 })
            return [m, res.total] as const
          } catch {
            return [m, null] as const
          }
        })
      )
      return Object.fromEntries(entries) as Record<string, number | null>
    },
    staleTime: 60_000,
  })

  // Icon mapping
  const getModelIcon = (modelName: string) => {
    const iconMap: Record<string, React.ComponentType<any>> = {
      contact: Users,
      company: Building2,
      deal: DollarSign,
      user: Users,
      default: Activity
    }
    
    return iconMap[modelName.toLowerCase()] || iconMap.default
  }

  return (
    <div className="p-6 space-y-6">
      {/* Header section */}
      <div>
        <h1 className="text-3xl font-bold text-gray-900">Dashboard</h1>
        <p className="text-gray-600 mt-2">Welcome to Atomo Admin UI</p>
      </div>

      {/* Stat cards section */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        {models.map((modelName) => {
          const IconComponent = getModelIcon(modelName)
          const modelMeta = schema.models[modelName]
          
          return (
            <Card key={modelName} className="hover:shadow-md transition-shadow">
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">
                  {modelMeta.ui.displayField || modelName}
                </CardTitle>
                <IconComponent className="h-4 w-4 text-gray-600" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {counts?.[modelName] ?? (counts === undefined ? '…' : '—')}
                </div>
                <p className="text-xs text-gray-600">
                  Total {modelName.toLowerCase()} count
                </p>
                <div className="mt-4">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => navigate(`/entities/${modelName}`)}
                    className="w-full"
                  >
                    View all
                  </Button>
                </div>
              </CardContent>
            </Card>
          )
        })}
      </div>

      {/* Quick actions section */}
      <div>
        <h2 className="text-xl font-semibold text-gray-900 mb-4">Quick Actions</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {models.map((modelName) => {
            const modelMeta = schema.models[modelName]
            const IconComponent = getModelIcon(modelName)
            
            return (
              <Card key={`action-${modelName}`} className="hover:shadow-md transition-shadow">
                <CardContent className="p-4">
                  <div className="flex items-center space-x-3">
                    <div className="p-2 bg-primary-100 rounded-lg">
                      <IconComponent className="h-5 w-5 text-primary-600" />
                    </div>
                    <div className="flex-1">
                      <h3 className="font-medium text-gray-900">
                        New {modelMeta.ui.displayField || modelName}
                      </h3>
                      <p className="text-sm text-gray-600">
                        Quickly create a new {modelName.toLowerCase()}
                      </p>
                    </div>
                    <Button
                      size="sm"
                      onClick={() => navigate(`/entities/${modelName}/new`)}
                    >
                      <Plus className="h-4 w-4" />
                    </Button>
                  </div>
                </CardContent>
              </Card>
            )
          })}
        </div>
      </div>

      {/* System info */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <TrendingUp className="h-5 w-5" />
            System Information
          </CardTitle>
          <CardDescription>
            Status of the currently connected Atomo service
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div>
              <div className="font-medium text-gray-900">Models</div>
              <div className="text-gray-600">{models.length}</div>
            </div>
            <div>
              <div className="font-medium text-gray-900">Audit Log</div>
              <div className="text-gray-600">
                {schema.config.auditLog ? 'Enabled' : 'Disabled'}
              </div>
            </div>
            <div>
              <div className="font-medium text-gray-900">Soft Deletes</div>
              <div className="text-gray-600">
                {schema.config.softDeletes ? 'Enabled' : 'Disabled'}
              </div>
            </div>
            <div>
              <div className="font-medium text-gray-900">Page Size</div>
              <div className="text-gray-600">
                {schema.config.defaultPageSize || 20}
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
