/**
 * Performance Metrics - 性能指标监控组件
 * 
 * 展示系统性能相关的各种指标和图表
 */

import React from 'react'
import { useQuery } from '@tanstack/react-query'
import { 
  TrendingUp, 
  Clock, 
  Server, 
  Cpu, 
  HardDrive,
  Wifi,
  Database
} from 'lucide-react'

import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'

interface PerformanceMetricsProps {
  timeRange: string
}

export function PerformanceMetrics({ timeRange }: PerformanceMetricsProps) {
  const { data: metrics, isLoading } = useQuery({
    queryKey: ['performance-metrics', timeRange],
    queryFn: () => fetchPerformanceMetrics(timeRange),
    refetchInterval: 30000, // 30秒刷新
  })

  if (isLoading) {
    return (
      <Card>
        <CardContent className="py-8 text-center">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600 mx-auto"></div>
          <p className="mt-4 text-gray-600">加载性能指标...</p>
        </CardContent>
      </Card>
    )
  }

  return (
    <div className="space-y-6">
      {/* 性能概览 */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <Card>
          <CardContent className="p-4 text-center">
            <Clock className="h-6 w-6 text-blue-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-blue-600">
              {metrics?.responseTime || 0}ms
            </div>
            <div className="text-xs text-gray-600">平均响应时间</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <TrendingUp className="h-6 w-6 text-green-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-green-600">
              {metrics?.throughput || 0}
            </div>
            <div className="text-xs text-gray-600">请求/秒</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <Cpu className="h-6 w-6 text-orange-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-orange-600">
              {metrics?.cpuUsage || 0}%
            </div>
            <div className="text-xs text-gray-600">CPU使用率</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <HardDrive className="h-6 w-6 text-purple-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-purple-600">
              {metrics?.memoryUsage || 0}%
            </div>
            <div className="text-xs text-gray-600">内存使用率</div>
          </CardContent>
        </Card>
      </div>

      {/* 详细指标 */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardHeader>
            <CardTitle>响应时间趋势</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="h-48 flex items-center justify-center text-gray-500">
              <div className="text-center">
                <TrendingUp className="h-8 w-8 mx-auto mb-2 text-gray-400" />
                <p>图表组件将在后续版本中集成</p>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>吞吐量监控</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="h-48 flex items-center justify-center text-gray-500">
              <div className="text-center">
                <Server className="h-8 w-8 mx-auto mb-2 text-gray-400" />
                <p>实时吞吐量图表</p>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

// 模拟API函数
async function fetchPerformanceMetrics(timeRange: string) {
  await new Promise(resolve => setTimeout(resolve, 500))
  
  return {
    responseTime: Math.floor(Math.random() * 200) + 50,
    throughput: Math.floor(Math.random() * 100) + 50,
    cpuUsage: Math.floor(Math.random() * 80) + 10,
    memoryUsage: Math.floor(Math.random() * 70) + 20
  }
}
