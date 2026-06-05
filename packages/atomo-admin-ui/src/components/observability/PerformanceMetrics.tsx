/**
 * Performance Metrics
 *
 * Displays various system performance metrics and charts
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
    refetchInterval: 30000, // Refresh every 30s
  })

  if (isLoading) {
    return (
      <Card>
        <CardContent className="py-8 text-center">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600 mx-auto"></div>
          <p className="mt-4 text-gray-600">Loading performance metrics...</p>
        </CardContent>
      </Card>
    )
  }

  return (
    <div className="space-y-6">
      {/* Performance overview */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <Card>
          <CardContent className="p-4 text-center">
            <Clock className="h-6 w-6 text-blue-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-blue-600">
              {metrics?.responseTime || 0}ms
            </div>
            <div className="text-xs text-gray-600">Average Response Time</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <TrendingUp className="h-6 w-6 text-green-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-green-600">
              {metrics?.throughput || 0}
            </div>
            <div className="text-xs text-gray-600">Requests/sec</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <Cpu className="h-6 w-6 text-orange-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-orange-600">
              {metrics?.cpuUsage || 0}%
            </div>
            <div className="text-xs text-gray-600">CPU Usage</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <HardDrive className="h-6 w-6 text-purple-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-purple-600">
              {metrics?.memoryUsage || 0}%
            </div>
            <div className="text-xs text-gray-600">Memory Usage</div>
          </CardContent>
        </Card>
      </div>

      {/* Detailed metrics */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardHeader>
            <CardTitle>Response Time Trend</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="h-48 flex items-center justify-center text-gray-500">
              <div className="text-center">
                <TrendingUp className="h-8 w-8 mx-auto mb-2 text-gray-400" />
                <p>Chart component will be integrated in a future release</p>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Throughput Monitor</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="h-48 flex items-center justify-center text-gray-500">
              <div className="text-center">
                <Server className="h-8 w-8 mx-auto mb-2 text-gray-400" />
                <p>Real-time throughput chart</p>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

// Mock API function
async function fetchPerformanceMetrics(timeRange: string) {
  await new Promise(resolve => setTimeout(resolve, 500))
  
  return {
    responseTime: Math.floor(Math.random() * 200) + 50,
    throughput: Math.floor(Math.random() * 100) + 50,
    cpuUsage: Math.floor(Math.random() * 80) + 10,
    memoryUsage: Math.floor(Math.random() * 70) + 20
  }
}
