/**
 * Observability Center
 *
 * Provides comprehensive system monitoring and insight, including:
 * - Workflow status monitoring
 * - Event stream visualization
 * - Performance metrics display
 * - Error tracking and alerting
 */

import React, { useState, useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { 
  Activity, 
  AlertTriangle, 
  CheckCircle, 
  Clock, 
  TrendingUp,
  Users,
  Server,
  Database,
  Zap,
  Eye,
  Filter,
  Download,
  Calendar
} from 'lucide-react'

import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../ui/Tabs'
import { WorkflowMonitor } from './WorkflowMonitor'
import { EventStreamViewer } from './EventStreamViewer'
import { PerformanceMetrics } from './PerformanceMetrics'
import { ErrorTracker } from './ErrorTracker'
import { cn } from '../../lib/utils'

interface SystemMetrics {
  totalEvents: number
  activeWorkflows: number
  errorRate: number
  responseTime: number
  uptime: string
  connectedUsers: number
}

interface SystemHealth {
  status: 'healthy' | 'warning' | 'critical'
  services: {
    api: 'up' | 'down' | 'degraded'
    database: 'up' | 'down' | 'degraded'
    cache: 'up' | 'down' | 'degraded'
    websocket: 'up' | 'down' | 'degraded'
  }
  lastCheck: Date
}

export function ObservabilityCenter() {
  const [selectedTimeRange, setSelectedTimeRange] = useState('1h')
  const [autoRefresh, setAutoRefresh] = useState(true)

  // System metrics query
  const { data: metrics, isLoading: metricsLoading } = useQuery({
    queryKey: ['system-metrics', selectedTimeRange],
    queryFn: () => fetchSystemMetrics(selectedTimeRange),
    refetchInterval: autoRefresh ? 30000 : false, // Refresh every 30s
  })

  // System health status query
  const { data: health, isLoading: healthLoading } = useQuery({
    queryKey: ['system-health'],
    queryFn: fetchSystemHealth,
    refetchInterval: autoRefresh ? 10000 : false, // Refresh every 10s
  })

  const timeRangeOptions = [
    { value: '15m', label: '15 minutes' },
    { value: '1h', label: '1 hour' },
    { value: '6h', label: '6 hours' },
    { value: '24h', label: '24 hours' },
    { value: '7d', label: '7 days' },
    { value: '30d', label: '30 days' },
  ]

  const getHealthStatusColor = (status: SystemHealth['status']) => {
    switch (status) {
      case 'healthy': return 'text-green-600 bg-green-100'
      case 'warning': return 'text-yellow-600 bg-yellow-100'
      case 'critical': return 'text-red-600 bg-red-100'
      default: return 'text-gray-600 bg-gray-100'
    }
  }

  const getServiceStatusColor = (status: 'up' | 'down' | 'degraded') => {
    switch (status) {
      case 'up': return 'success'
      case 'degraded': return 'warning'
      case 'down': return 'danger'
      default: return 'secondary'
    }
  }

  return (
    <div className="p-6 space-y-6">
      {/* Page title and control bar */}
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-3xl font-bold text-gray-900 flex items-center gap-3">
            <Eye className="h-8 w-8 text-primary-600" />
            Observability Center
          </h1>
          <p className="text-gray-600 mt-2">
            System health monitoring, workflow status tracking, and event stream analysis
          </p>
        </div>

        <div className="flex items-center gap-3">
          {/* Time range selector */}
          <select
            value={selectedTimeRange}
            onChange={(e) => setSelectedTimeRange(e.target.value)}
            className="px-3 py-2 border border-gray-300 rounded-md text-sm"
          >
            {timeRangeOptions.map(option => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>

          {/* Auto-refresh toggle */}
          <Button
            variant={autoRefresh ? "primary" : "secondary"}
            size="sm"
            onClick={() => setAutoRefresh(!autoRefresh)}
          >
            <Activity className={cn("h-4 w-4 mr-2", autoRefresh && "animate-pulse")} />
            {autoRefresh ? 'Auto Refresh' : 'Manual Refresh'}
          </Button>

          {/* Export report */}
          <Button variant="secondary" size="sm">
            <Download className="h-4 w-4 mr-2" />
            Export Report
          </Button>
        </div>
      </div>

      {/* System health overview */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-6 gap-4">
        {/* Overall health status */}
        <Card className="col-span-2">
          <CardContent className="p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-gray-600">System Status</p>
                <div className="flex items-center gap-2 mt-1">
                  {health && (
                    <Badge className={getHealthStatusColor(health.status)}>
                      {health.status === 'healthy' && 'Healthy'}
                      {health.status === 'warning' && 'Warning'}
                      {health.status === 'critical' && 'Critical'}
                    </Badge>
                  )}
                  <span className="text-xs text-gray-500">
                    {health && new Date(health.lastCheck).toLocaleTimeString()}
                  </span>
                </div>
              </div>
              <Activity className="h-8 w-8 text-primary-600" />
            </div>
            
            {/* Service status */}
            {health && (
              <div className="mt-4 grid grid-cols-2 gap-2 text-xs">
                <div className="flex items-center justify-between">
                  <span>API</span>
                  <Badge variant={getServiceStatusColor(health.services.api) as any}>
                    {health.services.api}
                  </Badge>
                </div>
                <div className="flex items-center justify-between">
                  <span>Database</span>
                  <Badge variant={getServiceStatusColor(health.services.database) as any}>
                    {health.services.database}
                  </Badge>
                </div>
                <div className="flex items-center justify-between">
                  <span>Cache</span>
                  <Badge variant={getServiceStatusColor(health.services.cache) as any}>
                    {health.services.cache}
                  </Badge>
                </div>
                <div className="flex items-center justify-between">
                  <span>WebSocket</span>
                  <Badge variant={getServiceStatusColor(health.services.websocket) as any}>
                    {health.services.websocket}
                  </Badge>
                </div>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Key metrics */}
        {metrics && (
          <>
            <Card>
              <CardContent className="p-4 text-center">
                <Server className="h-6 w-6 text-blue-600 mx-auto mb-2" />
                <div className="text-2xl font-bold text-blue-600">
                  {(metrics.totalEvents ?? 0).toLocaleString()}
                </div>
                <div className="text-xs text-gray-600">Total Events</div>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-4 text-center">
                <Zap className="h-6 w-6 text-green-600 mx-auto mb-2" />
                <div className="text-2xl font-bold text-green-600">
                  {metrics.activeWorkflows}
                </div>
                <div className="text-xs text-gray-600">Active Workflows</div>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-4 text-center">
                <AlertTriangle className="h-6 w-6 text-yellow-600 mx-auto mb-2" />
                <div className="text-2xl font-bold text-yellow-600">
                  {(metrics.errorRate * 100).toFixed(2)}%
                </div>
                <div className="text-xs text-gray-600">Error Rate</div>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-4 text-center">
                <Users className="h-6 w-6 text-purple-600 mx-auto mb-2" />
                <div className="text-2xl font-bold text-purple-600">
                  {metrics.connectedUsers}
                </div>
                <div className="text-xs text-gray-600">Online Users</div>
              </CardContent>
            </Card>
          </>
        )}
      </div>

      {/* Detailed monitoring panels */}
      <Tabs defaultValue="workflows" className="space-y-6">
        <TabsList className="grid w-full grid-cols-4">
          <TabsTrigger value="workflows" className="flex items-center gap-2">
            <Zap className="h-4 w-4" />
            Workflow Monitor
          </TabsTrigger>
          <TabsTrigger value="events" className="flex items-center gap-2">
            <Database className="h-4 w-4" />
            Event Stream
          </TabsTrigger>
          <TabsTrigger value="performance" className="flex items-center gap-2">
            <TrendingUp className="h-4 w-4" />
            Performance Metrics
          </TabsTrigger>
          <TabsTrigger value="errors" className="flex items-center gap-2">
            <AlertTriangle className="h-4 w-4" />
            Error Tracking
          </TabsTrigger>
        </TabsList>

        <TabsContent value="workflows">
          <WorkflowMonitor timeRange={selectedTimeRange} />
        </TabsContent>

        <TabsContent value="events">
          <EventStreamViewer timeRange={selectedTimeRange} />
        </TabsContent>

        <TabsContent value="performance">
          <PerformanceMetrics timeRange={selectedTimeRange} />
        </TabsContent>

        <TabsContent value="errors">
          <ErrorTracker timeRange={selectedTimeRange} />
        </TabsContent>
      </Tabs>
    </div>
  )
}

// Mock API functions
async function fetchSystemMetrics(timeRange: string): Promise<SystemMetrics> {
  // Simulate an API call
  await new Promise(resolve => setTimeout(resolve, 500))
  
  return {
    totalEvents: Math.floor(Math.random() * 100000) + 50000,
    activeWorkflows: Math.floor(Math.random() * 50) + 10,
    errorRate: Math.random() * 0.05,
    responseTime: Math.floor(Math.random() * 200) + 50,
    uptime: '99.9%',
    connectedUsers: Math.floor(Math.random() * 20) + 5
  }
}

async function fetchSystemHealth(): Promise<SystemHealth> {
  // Simulate an API call
  await new Promise(resolve => setTimeout(resolve, 300))
  
  const statuses: Array<'up' | 'down' | 'degraded'> = ['up', 'up', 'up', 'degraded']
  const getRandomStatus = () => statuses[Math.floor(Math.random() * statuses.length)]
  
  return {
    status: 'healthy',
    services: {
      api: getRandomStatus(),
      database: getRandomStatus(),
      cache: getRandomStatus(),
      websocket: getRandomStatus()
    },
    lastCheck: new Date()
  }
}
