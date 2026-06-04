/**
 * Observability Center - 可观测性中心
 * 
 * 提供系统的全方位监控和洞察能力，包括：
 * - 工作流状态监控
 * - 事件流可视化
 * - 性能指标展示
 * - 错误追踪和告警
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

  // 系统指标查询
  const { data: metrics, isLoading: metricsLoading } = useQuery({
    queryKey: ['system-metrics', selectedTimeRange],
    queryFn: () => fetchSystemMetrics(selectedTimeRange),
    refetchInterval: autoRefresh ? 30000 : false, // 30秒刷新
  })

  // 系统健康状态查询
  const { data: health, isLoading: healthLoading } = useQuery({
    queryKey: ['system-health'],
    queryFn: fetchSystemHealth,
    refetchInterval: autoRefresh ? 10000 : false, // 10秒刷新
  })

  const timeRangeOptions = [
    { value: '15m', label: '15分钟' },
    { value: '1h', label: '1小时' },
    { value: '6h', label: '6小时' },
    { value: '24h', label: '24小时' },
    { value: '7d', label: '7天' },
    { value: '30d', label: '30天' },
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
      {/* 页面标题和控制栏 */}
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-3xl font-bold text-gray-900 flex items-center gap-3">
            <Eye className="h-8 w-8 text-primary-600" />
            可观测性中心
          </h1>
          <p className="text-gray-600 mt-2">
            系统健康监控、工作流状态追踪和事件流分析
          </p>
        </div>

        <div className="flex items-center gap-3">
          {/* 时间范围选择 */}
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

          {/* 自动刷新开关 */}
          <Button
            variant={autoRefresh ? "primary" : "secondary"}
            size="sm"
            onClick={() => setAutoRefresh(!autoRefresh)}
          >
            <Activity className={cn("h-4 w-4 mr-2", autoRefresh && "animate-pulse")} />
            {autoRefresh ? '自动刷新' : '手动刷新'}
          </Button>

          {/* 导出报告 */}
          <Button variant="secondary" size="sm">
            <Download className="h-4 w-4 mr-2" />
            导出报告
          </Button>
        </div>
      </div>

      {/* 系统健康状态概览 */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-6 gap-4">
        {/* 整体健康状态 */}
        <Card className="col-span-2">
          <CardContent className="p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-gray-600">系统状态</p>
                <div className="flex items-center gap-2 mt-1">
                  {health && (
                    <Badge className={getHealthStatusColor(health.status)}>
                      {health.status === 'healthy' && '正常'}
                      {health.status === 'warning' && '警告'}
                      {health.status === 'critical' && '严重'}
                    </Badge>
                  )}
                  <span className="text-xs text-gray-500">
                    {health && new Date(health.lastCheck).toLocaleTimeString()}
                  </span>
                </div>
              </div>
              <Activity className="h-8 w-8 text-primary-600" />
            </div>
            
            {/* 服务状态 */}
            {health && (
              <div className="mt-4 grid grid-cols-2 gap-2 text-xs">
                <div className="flex items-center justify-between">
                  <span>API</span>
                  <Badge variant={getServiceStatusColor(health.services.api) as any}>
                    {health.services.api}
                  </Badge>
                </div>
                <div className="flex items-center justify-between">
                  <span>数据库</span>
                  <Badge variant={getServiceStatusColor(health.services.database) as any}>
                    {health.services.database}
                  </Badge>
                </div>
                <div className="flex items-center justify-between">
                  <span>缓存</span>
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

        {/* 关键指标 */}
        {metrics && (
          <>
            <Card>
              <CardContent className="p-4 text-center">
                <Server className="h-6 w-6 text-blue-600 mx-auto mb-2" />
                <div className="text-2xl font-bold text-blue-600">
                  {(metrics.totalEvents ?? 0).toLocaleString()}
                </div>
                <div className="text-xs text-gray-600">总事件数</div>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-4 text-center">
                <Zap className="h-6 w-6 text-green-600 mx-auto mb-2" />
                <div className="text-2xl font-bold text-green-600">
                  {metrics.activeWorkflows}
                </div>
                <div className="text-xs text-gray-600">活跃工作流</div>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-4 text-center">
                <AlertTriangle className="h-6 w-6 text-yellow-600 mx-auto mb-2" />
                <div className="text-2xl font-bold text-yellow-600">
                  {(metrics.errorRate * 100).toFixed(2)}%
                </div>
                <div className="text-xs text-gray-600">错误率</div>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-4 text-center">
                <Users className="h-6 w-6 text-purple-600 mx-auto mb-2" />
                <div className="text-2xl font-bold text-purple-600">
                  {metrics.connectedUsers}
                </div>
                <div className="text-xs text-gray-600">在线用户</div>
              </CardContent>
            </Card>
          </>
        )}
      </div>

      {/* 详细监控面板 */}
      <Tabs defaultValue="workflows" className="space-y-6">
        <TabsList className="grid w-full grid-cols-4">
          <TabsTrigger value="workflows" className="flex items-center gap-2">
            <Zap className="h-4 w-4" />
            工作流监控
          </TabsTrigger>
          <TabsTrigger value="events" className="flex items-center gap-2">
            <Database className="h-4 w-4" />
            事件流
          </TabsTrigger>
          <TabsTrigger value="performance" className="flex items-center gap-2">
            <TrendingUp className="h-4 w-4" />
            性能指标
          </TabsTrigger>
          <TabsTrigger value="errors" className="flex items-center gap-2">
            <AlertTriangle className="h-4 w-4" />
            错误追踪
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

// 模拟API函数
async function fetchSystemMetrics(timeRange: string): Promise<SystemMetrics> {
  // 模拟API调用
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
  // 模拟API调用
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
