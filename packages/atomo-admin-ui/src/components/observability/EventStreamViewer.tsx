/**
 * Event Stream Viewer - 事件河流可视化组件
 * 
 * 实现Atomo"事件的河流"哲学的核心可视化，提供：
 * - 实时事件流展示
 * - 时间旅行功能
 * - 事件关联分析
 * - 审计追踪能力
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

  // 事件流查询
  const { data: events, isLoading, refetch } = useQuery({
    queryKey: ['event-stream', timeRange, filters],
    queryFn: () => fetchEventStream(timeRange, filters),
    refetchInterval: isPlaying ? 3000 / playbackSpeed : false, // 根据播放速度调整刷新间隔
  })

  // 时间旅行控制
  useEffect(() => {
    if (isPlaying) {
      const interval = setInterval(() => {
        setCurrentTime(new Date())
      }, 1000 / playbackSpeed)
      return () => clearInterval(interval)
    }
  }, [isPlaying, playbackSpeed])

  // 自动滚动到最新事件
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

  // 统计信息
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
      {/* 事件流统计 */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-blue-600">
              {stats?.total || 0}
            </div>
            <div className="text-sm text-gray-600">总事件数</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-green-600">
              {stats?.uniqueUsers || 0}
            </div>
            <div className="text-sm text-gray-600">活跃用户</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-purple-600">
              {Object.keys(stats?.byType || {}).length}
            </div>
            <div className="text-sm text-gray-600">事件类型</div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-orange-600">
              {stats?.timeSpan ? Math.round(stats.timeSpan / 1000 / 60) : 0}
            </div>
            <div className="text-sm text-gray-600">时间跨度(分钟)</div>
          </CardContent>
        </Card>
      </div>

      {/* 时间旅行控制面板 */}
      <Card>
        <CardContent className="p-4">
          <div className="flex items-center justify-between">
            {/* 播放控制 */}
            <div className="flex items-center gap-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {/* 跳到开始 */}}
              >
                <SkipBack className="h-4 w-4" />
              </Button>

              <Button
                variant="ghost"
                size="sm"
                onClick={() => {/* 倒退 */}}
              >
                <Rewind className="h-4 w-4" />
              </Button>

              <Button
                variant={isPlaying ? "secondary" : "primary"}
                size="sm"
                onClick={() => setIsPlaying(!isPlaying)}
              >
                {isPlaying ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
                {isPlaying ? '暂停' : '播放'}
              </Button>

              <Button
                variant="ghost"
                size="sm"
                onClick={() => {/* 快进 */}}
              >
                <FastForward className="h-4 w-4" />
              </Button>

              <Button
                variant="ghost"
                size="sm"
                onClick={() => {/* 跳到最新 */}}
              >
                <SkipForward className="h-4 w-4" />
              </Button>

              {/* 播放速度 */}
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

            {/* 当前时间 */}
            <div className="flex items-center gap-2 text-sm text-gray-600">
              <Clock className="h-4 w-4" />
              <span>{formatDate(currentTime, 'time')}</span>
            </div>

            {/* 筛选控制 */}
            <div className="flex items-center gap-2">
              <div className="relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-gray-400" />
                <Input
                  placeholder="搜索事件..."
                  value={filters.searchTerm || ''}
                  onChange={(e) => setFilters(prev => ({ ...prev, searchTerm: e.target.value }))}
                  className="pl-9 w-48"
                />
              </div>

              <Button variant="secondary" size="sm">
                <Filter className="h-4 w-4 mr-1" />
                筛选
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* 事件流时间轴 */}
      <div className="grid grid-cols-12 gap-6">
        {/* 事件列表 */}
        <div className="col-span-8">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Database className="h-5 w-5" />
                事件河流
                {isPlaying && (
                  <Badge variant="secondary" className="animate-pulse">
                    实时
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
                    {/* 时间线 */}
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
                        {/* 时间线节点 */}
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
                      <p>暂无事件数据</p>
                      <p className="text-sm mt-1">事件将在产生时实时显示在这里</p>
                    </div>
                  </div>
                )}
              </div>
            </CardContent>
          </Card>
        </div>

        {/* 事件详情面板 */}
        <div className="col-span-4">
          <Card className="sticky top-6">
            <CardHeader>
              <CardTitle>事件详情</CardTitle>
            </CardHeader>
            <CardContent>
              {selectedEvent ? (
                <EventDetailView event={selectedEvent} />
              ) : (
                <div className="text-center text-gray-500 py-8">
                  <Eye className="h-8 w-8 mx-auto mb-3 text-gray-400" />
                  <p>选择一个事件查看详情</p>
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}

// 事件详情视图组件
interface EventDetailViewProps {
  event: StreamEvent
}

function EventDetailView({ event }: EventDetailViewProps) {
  return (
    <div className="space-y-4">
      {/* 基本信息 */}
      <div>
        <h4 className="font-medium text-gray-900 mb-2">基本信息</h4>
        <div className="space-y-2 text-sm">
          <div className="flex justify-between">
            <span className="text-gray-600">事件ID</span>
            <span className="font-mono text-xs">{event.id}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">类型</span>
            <Badge className="text-xs">{event.type}</Badge>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">时间</span>
            <span>{formatDate(event.timestamp, 'time')}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">用户</span>
            <span>{event.userName}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">实体</span>
            <span>{event.entityType}#{event.entityId}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">版本</span>
            <span>v{event.aggregateVersion}</span>
          </div>
        </div>
      </div>

      {/* 载荷数据 */}
      <div>
        <h4 className="font-medium text-gray-900 mb-2">载荷数据</h4>
        <pre className="text-xs bg-gray-50 p-3 rounded border overflow-auto max-h-32">
          {JSON.stringify(event.payload, null, 2)}
        </pre>
      </div>

      {/* 元数据 */}
      <div>
        <h4 className="font-medium text-gray-900 mb-2">元数据</h4>
        <div className="space-y-2 text-sm">
          {event.metadata.correlationId && (
            <div className="flex justify-between">
              <span className="text-gray-600">关联ID</span>
              <span className="font-mono text-xs">{event.metadata.correlationId}</span>
            </div>
          )}
          {event.metadata.parentEventId && (
            <div className="flex justify-between">
              <span className="text-gray-600">父事件</span>
              <span className="font-mono text-xs">{event.metadata.parentEventId}</span>
            </div>
          )}
          {event.metadata.ip && (
            <div className="flex justify-between">
              <span className="text-gray-600">IP地址</span>
              <span className="font-mono text-xs">{event.metadata.ip}</span>
            </div>
          )}
        </div>
      </div>

      {/* 操作按钮 */}
      <div className="pt-4 border-t border-gray-200">
        <div className="space-y-2">
          <Button variant="secondary" size="sm" className="w-full">
            查看相关事件
          </Button>
          <Button variant="secondary" size="sm" className="w-full">
            时间旅行到此刻
          </Button>
          <Button variant="secondary" size="sm" className="w-full">
            导出事件数据
          </Button>
        </div>
      </div>
    </div>
  )
}

// 模拟API函数
async function fetchEventStream(
  timeRange: string, 
  filters: Partial<EventFilter>
): Promise<StreamEvent[]> {
  // 模拟API调用
  await new Promise(resolve => setTimeout(resolve, 500))

  const mockEvents: StreamEvent[] = [
    {
      id: 'evt-001',
      type: 'create',
      timestamp: new Date(Date.now() - 300000),
      entityType: 'Contact',
      entityId: 'contact-123',
      userId: 'user-001',
      userName: '张三',
      action: '创建联系人',
      payload: {
        firstName: '张',
        lastName: '三',
        email: 'zhangsan@example.com'
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
      userName: '张三',
      action: '更新联系人信息',
      payload: {
        phone: '+86 138 0013 8000'
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
      userName: '系统',
      action: '触发欢迎邮件工作流',
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
      userName: '李四',
      action: '用户登录',
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

  // 应用筛选
  let filtered = mockEvents

  if (filters.searchTerm) {
    filtered = filtered.filter(event => 
      event.action.toLowerCase().includes(filters.searchTerm!.toLowerCase()) ||
      event.userName.toLowerCase().includes(filters.searchTerm!.toLowerCase())
    )
  }

  // 按时间排序（最新的在后面）
  return filtered.sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime())
}
