/**
 * Event Stream Viewer
 *
 * The core visualization realizing Atomo's "river of events" philosophy, providing:
 * - Real-time event stream display
 * - Time-travel capability
 * - Event correlation analysis
 * - Audit trail support
 */

import React, { useState, useEffect, useRef } from 'react'
import { useQuery } from '@tanstack/react-query'
import { 
  Clock, 
  Filter, 
  Search, 
  Play, 
  Pause, 
  SkipBack, 
  SkipForward,
  Rewind,
  FastForward,
  Eye,
  GitBranch,
  Database,
  User,
  Calendar,
  ArrowRight,
  Circle
} from 'lucide-react'

import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Input } from '../ui/Input'
import { formatDate, cn } from '../../lib/utils'

export interface StreamEvent {
  id: string
  type: string
  timestamp: Date
  entityType: string
  entityId: string
  userId: string
  userName: string
  action: string
  payload: any
  metadata: {
    ip?: string
    userAgent?: string
    correlationId?: string
    parentEventId?: string
  }
  aggregateVersion: number
}

interface EventFilter {
  eventTypes: string[]
  entityTypes: string[]
  users: string[]
  timeRange: {
    start: Date
    end: Date
  }
  searchTerm: string
}

interface EventStreamViewerProps {
  timeRange: string
}

export function EventStreamViewer({ timeRange }: EventStreamViewerProps) {
  const [isPlaying, setIsPlaying] = useState(true)
  const [playbackSpeed, setPlaybackSpeed] = useState(1)
  const [selectedEvent, setSelectedEvent] = useState<StreamEvent | null>(null)
  const [filters, setFilters] = useState<Partial<EventFilter>>({
    eventTypes: [],
    entityTypes: [],
    users: [],
    searchTerm: ''
  })
  const [currentTime, setCurrentTime] = useState(new Date())
  const streamRef = useRef<HTMLDivElement>(null)

  // Event stream query
  const { data: events, isLoading, refetch } = useQuery({
    queryKey: ['event-stream', timeRange, filters],
    queryFn: () => fetchEventStream(timeRange, filters),
    refetchInterval: isPlaying ? 3000 / playbackSpeed : false, // Adjust refresh interval based on playback speed
  })

  // Time-travel control
  useEffect(() => {
    if (isPlaying) {
      const interval = setInterval(() => {
        setCurrentTime(new Date())
      }, 1000 / playbackSpeed)
      return () => clearInterval(interval)
    }
  }, [isPlaying, playbackSpeed])

  // Auto-scroll to the latest event
  useEffect(() => {
    if (isPlaying && streamRef.current) {
      streamRef.current.scrollTop = streamRef.current.scrollHeight
    }
  }, [events, isPlaying])

  const getEventTypeColor = (type: string) => {
    const colorMap: Record<string, string> = {
      create: 'bg-green-100 text-green-800 border-green-200',
      update: 'bg-blue-100 text-blue-800 border-blue-200',
      delete: 'bg-red-100 text-red-800 border-red-200',
      login: 'bg-purple-100 text-purple-800 border-purple-200',
      logout: 'bg-gray-100 text-gray-800 border-gray-200',
      workflow: 'bg-orange-100 text-orange-800 border-orange-200',
      system: 'bg-yellow-100 text-yellow-800 border-yellow-200'
    }
    return colorMap[type] || 'bg-gray-100 text-gray-800 border-gray-200'
  }

  const getEventIcon = (type: string) => {
    switch (type) {
      case 'create': return <Circle className="h-3 w-3 fill-current" />
      case 'update': return <GitBranch className="h-3 w-3" />
      case 'delete': return <Circle className="h-3 w-3 fill-current" />
      case 'login': return <User className="h-3 w-3" />
      case 'workflow': return <ArrowRight className="h-3 w-3" />
      default: return <Database className="h-3 w-3" />
    }
  }

  const playbackSpeeds = [0.5, 1, 2, 4, 8]

  // Statistics
  const stats = events ? {
    total: events.length,
    byType: events.reduce((acc, event) => {
      acc[event.type] = (acc[event.type] || 0) + 1
      return acc
    }, {} as Record<string, number>),
    uniqueUsers: new Set(events.map(e => e.userId)).size,
    timeSpan: events.length > 0
      ? Math.max(...events.map(e => e.timestamp.getTime())) -
        Math.min(...events.map(e => e.timestamp.getTime()))
      : 0
  } : null

  return (
    <div className="space-y-6">
      {/* Event stream statistics */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-blue-600">
              {stats?.total || 0}
            </div>
            <div className="text-sm text-gray-600">Total Events</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-green-600">
              {stats?.uniqueUsers || 0}
            </div>
            <div className="text-sm text-gray-600">Active Users</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-purple-600">
              {Object.keys(stats?.byType || {}).length}
            </div>
            <div className="text-sm text-gray-600">Event Types</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-orange-600">
              {stats?.timeSpan ? Math.round(stats.timeSpan / 1000 / 60) : 0}
            </div>
            <div className="text-sm text-gray-600">Time Span (minutes)</div>
          </CardContent>
        </Card>
      </div>

      {/* Time-travel control panel */}
      <Card>
        <CardContent className="p-4">
          <div className="flex items-center justify-between">
            {/* Playback controls */}
            <div className="flex items-center gap-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {/* Jump to start */}}
              >
                <SkipBack className="h-4 w-4" />
              </Button>

              <Button
                variant="ghost"
                size="sm"
                onClick={() => {/* Rewind */}}
              >
                <Rewind className="h-4 w-4" />
              </Button>

              <Button
                variant={isPlaying ? "secondary" : "primary"}
                size="sm"
                onClick={() => setIsPlaying(!isPlaying)}
              >
                {isPlaying ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
                {isPlaying ? 'Pause' : 'Play'}
              </Button>

              <Button
                variant="ghost"
                size="sm"
                onClick={() => {/* Fast forward */}}
              >
                <FastForward className="h-4 w-4" />
              </Button>

              <Button
                variant="ghost"
                size="sm"
                onClick={() => {/* Jump to latest */}}
              >
                <SkipForward className="h-4 w-4" />
              </Button>

              {/* Playback speed */}
              <select
                value={playbackSpeed}
                onChange={(e) => setPlaybackSpeed(Number(e.target.value))}
                className="px-2 py-1 border border-gray-300 rounded text-sm"
              >
                {playbackSpeeds.map(speed => (
                  <option key={speed} value={speed}>
                    {speed}x
                  </option>
                ))}
              </select>
            </div>

            {/* Current time */}
            <div className="flex items-center gap-2 text-sm text-gray-600">
              <Clock className="h-4 w-4" />
              <span>{formatDate(currentTime, 'time')}</span>
            </div>

            {/* Filter controls */}
            <div className="flex items-center gap-2">
              <div className="relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-gray-400" />
                <Input
                  placeholder="Search events..."
                  value={filters.searchTerm || ''}
                  onChange={(e) => setFilters(prev => ({ ...prev, searchTerm: e.target.value }))}
                  className="pl-9 w-48"
                />
              </div>

              <Button variant="secondary" size="sm">
                <Filter className="h-4 w-4 mr-1" />
                Filter
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Event stream timeline */}
      <div className="grid grid-cols-12 gap-6">
        {/* Event list */}
        <div className="col-span-8">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Database className="h-5 w-5" />
                Event Stream
                {isPlaying && (
                  <Badge variant="secondary" className="animate-pulse">
                    Live
                  </Badge>
                )}
              </CardTitle>
            </CardHeader>
            <CardContent className="p-0">
              <div 
                ref={streamRef}
                className="h-96 overflow-y-auto border-t border-gray-200"
              >
                {isLoading ? (
                  <div className="flex items-center justify-center h-full">
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
                  </div>
                ) : events && events.length > 0 ? (
                  <div className="relative">
                    {/* Timeline */}
                    <div className="absolute left-4 top-0 bottom-0 w-px bg-gradient-to-b from-blue-500 via-green-500 to-purple-500"></div>
                    
                    {events.map((event, index) => (
                      <div
                        key={event.id}
                        className={cn(
                          "relative pl-12 pr-4 py-3 border-b border-gray-100 hover:bg-gray-50 cursor-pointer transition-colors",
                          selectedEvent?.id === event.id && "bg-blue-50 border-blue-200"
                        )}
                        onClick={() => setSelectedEvent(event)}
                      >
                        {/* Timeline node */}
                        <div className="absolute left-2 top-4 w-4 h-4 bg-white border-2 border-blue-500 rounded-full flex items-center justify-center">
                          {getEventIcon(event.type)}
                        </div>

                        <div className="flex items-start justify-between">
                          <div className="flex-1">
                            <div className="flex items-center gap-2 mb-1">
                              <Badge className={getEventTypeColor(event.type)}>
                                {event.type}
                              </Badge>
                              <Badge variant="secondary">
                                {event.entityType}
                              </Badge>
                              <span className="text-xs text-gray-500">
                                v{event.aggregateVersion}
                              </span>
                            </div>
                            
                            <p className="text-sm font-medium text-gray-900">
                              {event.action}
                            </p>
                            
                            <div className="flex items-center gap-4 mt-1 text-xs text-gray-500">
                              <span className="flex items-center gap-1">
                                <User className="h-3 w-3" />
                                {event.userName}
                              </span>
                              <span className="flex items-center gap-1">
                                <Clock className="h-3 w-3" />
                                {formatDate(event.timestamp, 'time')}
                              </span>
                              <span>
                                {event.entityType}#{event.entityId}
                              </span>
                            </div>
                          </div>

                          <Button variant="ghost" size="sm">
                            <Eye className="h-4 w-4" />
                          </Button>
                        </div>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="flex items-center justify-center h-full text-gray-500">
                    <div className="text-center">
                      <Database className="h-8 w-8 mx-auto mb-3 text-gray-400" />
                      <p>No event data yet</p>
                      <p className="text-sm mt-1">Events will appear here in real time as they occur</p>
                    </div>
                  </div>
                )}
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Event detail panel */}
        <div className="col-span-4">
          <Card className="sticky top-6">
            <CardHeader>
              <CardTitle>Event Details</CardTitle>
            </CardHeader>
            <CardContent>
              {selectedEvent ? (
                <EventDetailView event={selectedEvent} />
              ) : (
                <div className="text-center text-gray-500 py-8">
                  <Eye className="h-8 w-8 mx-auto mb-3 text-gray-400" />
                  <p>Select an event to view details</p>
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}

// Event detail view component
interface EventDetailViewProps {
  event: StreamEvent
}

function EventDetailView({ event }: EventDetailViewProps) {
  return (
    <div className="space-y-4">
      {/* Basic information */}
      <div>
        <h4 className="font-medium text-gray-900 mb-2">Basic Information</h4>
        <div className="space-y-2 text-sm">
          <div className="flex justify-between">
            <span className="text-gray-600">Event ID</span>
            <span className="font-mono text-xs">{event.id}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">Type</span>
            <Badge className="text-xs">{event.type}</Badge>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">Time</span>
            <span>{formatDate(event.timestamp, 'time')}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">User</span>
            <span>{event.userName}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">Entity</span>
            <span>{event.entityType}#{event.entityId}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">Version</span>
            <span>v{event.aggregateVersion}</span>
          </div>
        </div>
      </div>

      {/* Payload data */}
      <div>
        <h4 className="font-medium text-gray-900 mb-2">Payload Data</h4>
        <pre className="text-xs bg-gray-50 p-3 rounded border overflow-auto max-h-32">
          {JSON.stringify(event.payload, null, 2)}
        </pre>
      </div>

      {/* Metadata */}
      <div>
        <h4 className="font-medium text-gray-900 mb-2">Metadata</h4>
        <div className="space-y-2 text-sm">
          {event.metadata.correlationId && (
            <div className="flex justify-between">
              <span className="text-gray-600">Correlation ID</span>
              <span className="font-mono text-xs">{event.metadata.correlationId}</span>
            </div>
          )}
          {event.metadata.parentEventId && (
            <div className="flex justify-between">
              <span className="text-gray-600">Parent Event</span>
              <span className="font-mono text-xs">{event.metadata.parentEventId}</span>
            </div>
          )}
          {event.metadata.ip && (
            <div className="flex justify-between">
              <span className="text-gray-600">IP Address</span>
              <span className="font-mono text-xs">{event.metadata.ip}</span>
            </div>
          )}
        </div>
      </div>

      {/* Action buttons */}
      <div className="pt-4 border-t border-gray-200">
        <div className="space-y-2">
          <Button variant="secondary" size="sm" className="w-full">
            View Related Events
          </Button>
          <Button variant="secondary" size="sm" className="w-full">
            Time Travel to This Moment
          </Button>
          <Button variant="secondary" size="sm" className="w-full">
            Export Event Data
          </Button>
        </div>
      </div>
    </div>
  )
}

// Mock API function
async function fetchEventStream(
  timeRange: string,
  filters: Partial<EventFilter>
): Promise<StreamEvent[]> {
  // Simulate an API call
  await new Promise(resolve => setTimeout(resolve, 500))

  const mockEvents: StreamEvent[] = [
    {
      id: 'evt-001',
      type: 'create',
      timestamp: new Date(Date.now() - 300000),
      entityType: 'Contact',
      entityId: 'contact-123',
      userId: 'user-001',
      userName: 'John Smith',
      action: 'Created contact',
      payload: {
        firstName: 'John',
        lastName: 'Smith',
        email: 'john.smith@example.com'
      },
      metadata: {
        ip: '192.168.1.100',
        correlationId: 'corr-001'
      },
      aggregateVersion: 1
    },
    {
      id: 'evt-002',
      type: 'update',
      timestamp: new Date(Date.now() - 240000),
      entityType: 'Contact',
      entityId: 'contact-123',
      userId: 'user-001',
      userName: 'John Smith',
      action: 'Updated contact information',
      payload: {
        phone: '+1 555 0100'
      },
      metadata: {
        ip: '192.168.1.100',
        correlationId: 'corr-001',
        parentEventId: 'evt-001'
      },
      aggregateVersion: 2
    },
    {
      id: 'evt-003',
      type: 'workflow',
      timestamp: new Date(Date.now() - 180000),
      entityType: 'Contact',
      entityId: 'contact-123',
      userId: 'system',
      userName: 'System',
      action: 'Triggered welcome email workflow',
      payload: {
        workflowId: 'wf-welcome-email',
        status: 'started'
      },
      metadata: {
        correlationId: 'corr-002'
      },
      aggregateVersion: 3
    },
    {
      id: 'evt-004',
      type: 'login',
      timestamp: new Date(Date.now() - 120000),
      entityType: 'User',
      entityId: 'user-002',
      userId: 'user-002',
      userName: 'Jane Doe',
      action: 'User logged in',
      payload: {
        loginMethod: 'password',
        sessionId: 'sess-456'
      },
      metadata: {
        ip: '192.168.1.101',
        userAgent: 'Mozilla/5.0...'
      },
      aggregateVersion: 1
    }
  ]

  // Apply filters
  let filtered = mockEvents

  if (filters.searchTerm) {
    filtered = filtered.filter(event => 
      event.action.toLowerCase().includes(filters.searchTerm!.toLowerCase()) ||
      event.userName.toLowerCase().includes(filters.searchTerm!.toLowerCase())
    )
  }

  // Sort by time (most recent last)
  return filtered.sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime())
}
