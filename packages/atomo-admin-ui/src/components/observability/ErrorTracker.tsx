/**
 * Error Tracker
 *
 * Monitors and analyzes system errors, providing error statistics and detailed information
 */

import React from 'react'
import { useQuery } from '@tanstack/react-query'
import { 
  AlertTriangle, 
  XCircle, 
  Clock, 
  TrendingDown,
  Bug,
  Shield
} from 'lucide-react'

import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { formatDate } from '../../lib/utils'

interface ErrorRecord {
  id: string
  message: string
  type: 'client' | 'server' | 'database' | 'network'
  severity: 'low' | 'medium' | 'high' | 'critical'
  timestamp: Date
  count: number
  stack?: string
  userId?: string
  context: Record<string, any>
}

interface ErrorTrackerProps {
  timeRange: string
}

export function ErrorTracker({ timeRange }: ErrorTrackerProps) {
  const { data: errors, isLoading } = useQuery({
    queryKey: ['error-tracker', timeRange],
    queryFn: () => fetchErrors(timeRange),
    refetchInterval: 10000, // Refresh every 10s
  })

  const getSeverityColor = (severity: ErrorRecord['severity']) => {
    switch (severity) {
      case 'critical': return 'danger'
      case 'high': return 'warning'
      case 'medium': return 'secondary'
      case 'low': return 'secondary'
      default: return 'secondary'
    }
  }

  const getTypeIcon = (type: ErrorRecord['type']) => {
    switch (type) {
      case 'client': return <Bug className="h-4 w-4" />
      case 'server': return <XCircle className="h-4 w-4" />
      case 'database': return <AlertTriangle className="h-4 w-4" />
      case 'network': return <Shield className="h-4 w-4" />
      default: return <AlertTriangle className="h-4 w-4" />
    }
  }

  const stats = errors ? {
    total: errors.reduce((sum, error) => sum + error.count, 0),
    unique: errors.length,
    critical: errors.filter(e => e.severity === 'critical').length,
    resolved: 0 // Hardcoded for now
  } : null

  if (isLoading) {
    return (
      <Card>
        <CardContent className="py-8 text-center">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600 mx-auto"></div>
          <p className="mt-4 text-gray-600">Loading error data...</p>
        </CardContent>
      </Card>
    )
  }

  return (
    <div className="space-y-6">
      {/* Error statistics */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <Card>
          <CardContent className="p-4 text-center">
            <AlertTriangle className="h-6 w-6 text-red-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-red-600">
              {stats?.total || 0}
            </div>
            <div className="text-xs text-gray-600">Total Errors</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <XCircle className="h-6 w-6 text-orange-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-orange-600">
              {stats?.unique || 0}
            </div>
            <div className="text-xs text-gray-600">Unique Errors</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <Bug className="h-6 w-6 text-purple-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-purple-600">
              {stats?.critical || 0}
            </div>
            <div className="text-xs text-gray-600">Critical Errors</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <TrendingDown className="h-6 w-6 text-green-600 mx-auto mb-2" />
            <div className="text-2xl font-bold text-green-600">
              {stats?.resolved || 0}
            </div>
            <div className="text-xs text-gray-600">Resolved</div>
          </CardContent>
        </Card>
      </div>

      {/* Error list */}
      <Card>
        <CardHeader>
          <CardTitle>Error Details</CardTitle>
        </CardHeader>
        <CardContent>
          {errors && errors.length > 0 ? (
            <div className="space-y-4">
              {errors.map((error) => (
                <div key={error.id} className="border border-gray-200 rounded-md p-4">
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="flex items-center gap-2 mb-2">
                        {getTypeIcon(error.type)}
                        <Badge variant={getSeverityColor(error.severity) as any}>
                          {error.severity}
                        </Badge>
                        <Badge variant="secondary">
                          {error.type}
                        </Badge>
                        <Badge variant="secondary">
                          {error.count}x
                        </Badge>
                      </div>
                      
                      <h4 className="font-medium text-gray-900 mb-2">
                        {error.message}
                      </h4>
                      
                      <div className="flex items-center gap-4 text-sm text-gray-500">
                        <span className="flex items-center gap-1">
                          <Clock className="h-3 w-3" />
                          {formatDate(error.timestamp, 'time')}
                        </span>
                        {error.userId && (
                          <span>User: {error.userId}</span>
                        )}
                      </div>
                      
                      {error.stack && (
                        <details className="mt-3">
                          <summary className="cursor-pointer text-sm text-gray-600">
                            View Stack Trace
                          </summary>
                          <pre className="mt-2 text-xs bg-gray-50 p-3 rounded overflow-auto max-h-32">
                            {error.stack}
                          </pre>
                        </details>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="text-center py-8 text-gray-500">
              <Shield className="h-8 w-8 mx-auto mb-3 text-gray-400" />
              <p>No error records</p>
              <p className="text-sm mt-1">The system is operating normally</p>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

// Mock API function
async function fetchErrors(timeRange: string): Promise<ErrorRecord[]> {
  await new Promise(resolve => setTimeout(resolve, 600))
  
  const mockErrors: ErrorRecord[] = [
    {
      id: 'err-001',
      message: 'Database connection timeout',
      type: 'database',
      severity: 'high',
      timestamp: new Date(Date.now() - 300000),
      count: 5,
      stack: 'Error: Connection timeout\n  at Database.connect (/app/db.js:45)\n  at async query (/app/api.js:120)',
      context: {
        query: 'SELECT * FROM contacts',
        timeout: 5000
      }
    },
    {
      id: 'err-002',
      message: 'Validation failed for email field',
      type: 'client',
      severity: 'medium',
      timestamp: new Date(Date.now() - 600000),
      count: 12,
      userId: 'user-123',
      context: {
        field: 'email',
        value: 'invalid-email'
      }
    }
  ]
  
  return mockErrors
}
