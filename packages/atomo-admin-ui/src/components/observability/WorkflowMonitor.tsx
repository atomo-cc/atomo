/**
 * Workflow Monitor
 *
 * Monitors the execution status and performance metrics of all workflows in real time
 */

import React, { useState, useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { 
  Play, 
  Pause, 
  CheckCircle, 
  XCircle, 
  Clock, 
  AlertTriangle,
  Activity,
  MoreHorizontal,
  Filter,
  Search,
  Zap
} from 'lucide-react'

import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Input } from '../ui/Input'
import { Progress } from '../ui/Progress'
import { formatDate, cn } from '../../lib/utils'

function formatDuration(duration?: number) {
  if (!duration) return '0s'
  const seconds = Math.floor(duration / 1000)
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return minutes > 0 ? `${minutes}m ${remainingSeconds}s` : `${remainingSeconds}s`
}

export interface WorkflowInstance {
  id: string
  name: string
  status: 'running' | 'completed' | 'failed' | 'paused' | 'pending'
  startTime: Date
  endTime?: Date
  duration?: number
  progress: number
  currentStep: string
  totalSteps: number
  completedSteps: number
  errorMessage?: string
  metadata: {
    triggeredBy: string
    entityType: string
    entityId: string
  }
}

interface WorkflowMonitorProps {
  timeRange: string
}

export function WorkflowMonitor({ timeRange }: WorkflowMonitorProps) {
  const [searchTerm, setSearchTerm] = useState('')
  const [statusFilter, setStatusFilter] = useState<string>('all')
  const [selectedWorkflow, setSelectedWorkflow] = useState<WorkflowInstance | null>(null)

  // Workflow instance query
  const { data: workflows, isLoading, refetch } = useQuery({
    queryKey: ['workflows', timeRange, searchTerm, statusFilter],
    queryFn: () => fetchWorkflows(timeRange, searchTerm, statusFilter),
    refetchInterval: 5000, // Refresh every 5s
  })

  const getStatusIcon = (status: WorkflowInstance['status']) => {
    switch (status) {
      case 'running': return <Play className="h-4 w-4 text-blue-600" />
      case 'completed': return <CheckCircle className="h-4 w-4 text-green-600" />
      case 'failed': return <XCircle className="h-4 w-4 text-red-600" />
      case 'paused': return <Pause className="h-4 w-4 text-yellow-600" />
      case 'pending': return <Clock className="h-4 w-4 text-gray-600" />
      default: return <Activity className="h-4 w-4 text-gray-600" />
    }
  }

  const getStatusColor = (status: WorkflowInstance['status']) => {
    switch (status) {
      case 'running': return 'secondary'
      case 'completed': return 'success'
      case 'failed': return 'danger'
      case 'paused': return 'warning'
      case 'pending': return 'secondary'
      default: return 'secondary'
    }
  }

  const formatDuration = (ms?: number) => {
    if (!ms) return '-'
    
    const seconds = Math.floor(ms / 1000)
    const minutes = Math.floor(seconds / 60)
    const hours = Math.floor(minutes / 60)
    
    if (hours > 0) return `${hours}h ${minutes % 60}m`
    if (minutes > 0) return `${minutes}m ${seconds % 60}s`
    return `${seconds}s`
  }

  // Statistics
  const stats = workflows ? {
    total: workflows.length,
    running: workflows.filter(w => w.status === 'running').length,
    completed: workflows.filter(w => w.status === 'completed').length,
    failed: workflows.filter(w => w.status === 'failed').length,
    avgDuration: workflows
      .filter(w => w.duration)
      .reduce((sum, w) => sum + (w.duration || 0), 0) / workflows.filter(w => w.duration).length
  } : null

  return (
    <div className="space-y-6">
      {/* Statistics overview */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-blue-600">
              {stats?.total || 0}
            </div>
            <div className="text-sm text-gray-600">Total Workflows</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-green-600">
              {stats?.completed || 0}
            </div>
            <div className="text-sm text-gray-600">Completed</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-orange-600">
              {stats?.running || 0}
            </div>
            <div className="text-sm text-gray-600">Running</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-red-600">
              {stats?.failed || 0}
            </div>
            <div className="text-sm text-gray-600">Failed</div>
          </CardContent>
        </Card>
      </div>

      {/* Search and filter */}
      <Card>
        <CardContent className="p-4">
          <div className="flex gap-4 items-center">
            <div className="flex-1 relative">
              <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-gray-400" />
              <Input
                placeholder="Search workflows by name or ID..."
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
                className="pl-9"
              />
            </div>

            <select
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value)}
              className="px-3 py-2 border border-gray-300 rounded-md text-sm"
            >
              <option value="all">All Statuses</option>
              <option value="running">Running</option>
              <option value="completed">Completed</option>
              <option value="failed">Failed</option>
              <option value="paused">Paused</option>
              <option value="pending">Pending</option>
            </select>

            <Button variant="secondary" onClick={() => refetch()}>
              Refresh
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Workflow list */}
      <div className="grid gap-4">
        {isLoading ? (
          <Card>
            <CardContent className="py-8 text-center">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600 mx-auto"></div>
              <p className="mt-4 text-gray-600">Loading workflow data...</p>
            </CardContent>
          </Card>
        ) : workflows && workflows.length > 0 ? (
          workflows.map((workflow) => (
            <Card key={workflow.id} className="hover:shadow-md transition-shadow">
              <CardContent className="p-4">
                <div className="flex items-start justify-between">
                  <div className="flex-1">
                    <div className="flex items-center gap-3">
                      {getStatusIcon(workflow.status)}
                      <h3 className="font-medium text-gray-900">{workflow.name}</h3>
                      <Badge variant={getStatusColor(workflow.status) as any}>
                        {workflow.status}
                      </Badge>
                    </div>
                    
                    <div className="mt-2 text-sm text-gray-600">
                      <p>ID: {workflow.id}</p>
                      <p>Triggered by: {workflow.metadata.triggeredBy}</p>
                      <p>Entity: {workflow.metadata.entityType}#{workflow.metadata.entityId}</p>
                    </div>

                    <div className="mt-3 grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
                      <div>
                        <span className="font-medium text-gray-700">Start Time</span>
                        <p className="text-gray-600">{formatDate(workflow.startTime, 'time')}</p>
                      </div>

                      <div>
                        <span className="font-medium text-gray-700">Duration</span>
                        <p className="text-gray-600">{formatDuration(workflow.duration)}</p>
                      </div>

                      <div>
                        <span className="font-medium text-gray-700">Current Step</span>
                        <p className="text-gray-600">{workflow.currentStep}</p>
                      </div>

                      <div>
                        <span className="font-medium text-gray-700">Progress</span>
                        <div className="flex items-center gap-2">
                          <Progress value={workflow.progress} className="h-2 flex-1" />
                          <span className="text-xs">{workflow.progress}%</span>
                        </div>
                      </div>
                    </div>

                    {workflow.status === 'failed' && workflow.errorMessage && (
                      <div className="mt-3 p-3 bg-red-50 border border-red-200 rounded-md">
                        <div className="flex items-center gap-2 text-red-800">
                          <AlertTriangle className="h-4 w-4" />
                          <span className="font-medium">Error Message</span>
                        </div>
                        <p className="text-sm text-red-700 mt-1">{workflow.errorMessage}</p>
                      </div>
                    )}
                  </div>

                  <div className="flex items-center gap-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setSelectedWorkflow(workflow)}
                    >
                      View Details
                    </Button>
                    
                    <Button variant="ghost" size="sm">
                      <MoreHorizontal className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              </CardContent>
            </Card>
          ))
        ) : (
          <Card>
            <CardContent className="py-8 text-center">
              <Zap className="h-8 w-8 text-gray-400 mx-auto mb-3" />
              <p className="text-gray-600">No workflow data yet</p>
              <p className="text-sm text-gray-500 mt-1">
                No matching workflow instances found in the selected time range
              </p>
            </CardContent>
          </Card>
        )}
      </div>

      {/* Workflow detail modal */}
      {selectedWorkflow && (
        <WorkflowDetailModal
          workflow={selectedWorkflow}
          onClose={() => setSelectedWorkflow(null)}
        />
      )}
    </div>
  )
}

// Workflow detail modal component
interface WorkflowDetailModalProps {
  workflow: WorkflowInstance
  onClose: () => void
}

function WorkflowDetailModal({ workflow, onClose }: WorkflowDetailModalProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
      <Card className="w-full max-w-4xl max-h-[80vh] overflow-auto m-4">
        <CardHeader className="border-b">
          <div className="flex items-center justify-between">
            <CardTitle>Workflow Details: {workflow.name}</CardTitle>
            <Button variant="ghost" onClick={onClose}>
              ×
            </Button>
          </div>
        </CardHeader>
        <CardContent className="p-6">
          <div className="space-y-6">
            {/* Basic information */}
            <div>
              <h3 className="font-medium mb-3">Basic Information</h3>
              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <span className="font-medium text-gray-700">Workflow ID</span>
                  <p className="text-gray-600">{workflow.id}</p>
                </div>
                <div>
                  <span className="font-medium text-gray-700">Status</span>
                  <p className="text-gray-600">{workflow.status}</p>
                </div>
                <div>
                  <span className="font-medium text-gray-700">Start Time</span>
                  <p className="text-gray-600">{formatDate(workflow.startTime, 'time')}</p>
                </div>
                <div>
                  <span className="font-medium text-gray-700">Duration</span>
                  <p className="text-gray-600">{formatDuration(workflow.duration)}</p>
                </div>
              </div>
            </div>

            {/* Execution progress */}
            <div>
              <h3 className="font-medium mb-3">Execution Progress</h3>
              <div className="space-y-3">
                <div className="flex items-center justify-between">
                  <span className="text-sm text-gray-600">
                    Step {workflow.completedSteps}/{workflow.totalSteps}
                  </span>
                  <span className="text-sm font-medium">{workflow.progress}%</span>
                </div>
                <Progress value={workflow.progress} className="h-3" />
                <p className="text-sm text-gray-600">
                  Current step: {workflow.currentStep}
                </p>
              </div>
            </div>

            {/* Step details - more detailed step execution history can be added here */}
            <div>
              <h3 className="font-medium mb-3">Execution History</h3>
              <div className="text-sm text-gray-600">
                Detailed step execution history will be available in a future release...
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

// Mock API function
async function fetchWorkflows(
  timeRange: string,
  search: string,
  statusFilter: string
): Promise<WorkflowInstance[]> {
  // Simulate an API call
  await new Promise(resolve => setTimeout(resolve, 800))

  const mockWorkflows: WorkflowInstance[] = [
    {
      id: 'wf-001',
      name: 'Customer Data Sync Workflow',
      status: 'running',
      startTime: new Date(Date.now() - 300000),
      duration: 300000,
      progress: 65,
      currentStep: 'Data validation',
      totalSteps: 5,
      completedSteps: 3,
      metadata: {
        triggeredBy: 'system',
        entityType: 'Contact',
        entityId: 'contact-123'
      }
    },
    {
      id: 'wf-002',
      name: 'Order Processing Workflow',
      status: 'completed',
      startTime: new Date(Date.now() - 600000),
      endTime: new Date(Date.now() - 120000),
      duration: 480000,
      progress: 100,
      currentStep: 'Completed',
      totalSteps: 4,
      completedSteps: 4,
      metadata: {
        triggeredBy: 'user@example.com',
        entityType: 'Deal',
        entityId: 'deal-456'
      }
    },
    {
      id: 'wf-003',
      name: 'Email Notification Workflow',
      status: 'failed',
      startTime: new Date(Date.now() - 900000),
      duration: 60000,
      progress: 25,
      currentStep: 'Sending email',
      totalSteps: 3,
      completedSteps: 1,
      errorMessage: 'SMTP server connection timed out',
      metadata: {
        triggeredBy: 'automation',
        entityType: 'Company',
        entityId: 'company-789'
      }
    }
  ]

  // Apply filters
  let filtered = mockWorkflows
  
  if (search) {
    filtered = filtered.filter(w => 
      w.name.toLowerCase().includes(search.toLowerCase()) ||
      w.id.toLowerCase().includes(search.toLowerCase())
    )
  }
  
  if (statusFilter !== 'all') {
    filtered = filtered.filter(w => w.status === statusFilter)
  }
  
  return filtered
}
