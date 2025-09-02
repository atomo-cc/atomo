/**
 * Notification System - 通知系统
 * 
 * 提供实时通知和提醒功能，包括：
 * - 实时推送通知
 * - 通知中心管理
 * - 通知偏好设置
 * - 桌面和移动端推送
 */

import React, { useState, useEffect } from 'react'
import { 
  Bell, 
  X, 
  Check, 
  AlertTriangle, 
  Info, 
  CheckCircle,
  Clock,
  Settings,
  Filter,
  Trash2,
  Volume2,
  VolumeX
} from 'lucide-react'

import { Button } from '../ui/Button'
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Switch } from '../ui/Switch'
import { formatDate, cn } from '../../lib/utils'

export interface Notification {
  id: string
  type: 'info' | 'warning' | 'error' | 'success'
  title: string
  message: string
  timestamp: Date
  read: boolean
  category: string
  priority: 'low' | 'medium' | 'high' | 'urgent'
  actionUrl?: string
  metadata?: Record<string, any>
}

interface NotificationPreferences {
  desktop: boolean
  sound: boolean
  email: boolean
  categories: Record<string, boolean>
  quietHours: {
    enabled: boolean
    start: string
    end: string
  }
}

interface NotificationSystemProps {
  isOpen: boolean
  onClose: () => void
}

export function NotificationSystem({ isOpen, onClose }: NotificationSystemProps) {
  const [notifications, setNotifications] = useState<Notification[]>([])
  const [preferences, setPreferences] = useState<NotificationPreferences>({
    desktop: true,
    sound: true,
    email: false,
    categories: {
      system: true,
      workflow: true,
      task: true,
      security: true,
      update: false
    },
    quietHours: {
      enabled: false,
      start: '22:00',
      end: '08:00'
    }
  })
  const [filter, setFilter] = useState<string>('all')
  const [activeTab, setActiveTab] = useState<'notifications' | 'settings'>('notifications')

  // 模拟实时通知接收
  useEffect(() => {
    // 加载初始通知
    setNotifications(generateMockNotifications())

    // 模拟实时通知
    const interval = setInterval(() => {
      if (Math.random() < 0.3) { // 30% 概率收到新通知
        const newNotification = generateRandomNotification()
        setNotifications(prev => [newNotification, ...prev])
        
        // 显示桌面通知
        if (preferences.desktop && 'Notification' in window) {
          showDesktopNotification(newNotification)
        }
        
        // 播放声音
        if (preferences.sound) {
          playNotificationSound()
        }
      }
    }, 10000) // 每10秒检查一次

    return () => clearInterval(interval)
  }, [preferences.desktop, preferences.sound])

  // 请求桌面通知权限
  useEffect(() => {
    if ('Notification' in window && Notification.permission === 'default') {
      Notification.requestPermission()
    }
  }, [])

  const filteredNotifications = notifications.filter(notification => {
    if (filter === 'all') return true
    if (filter === 'unread') return !notification.read
    return notification.category === filter
  })

  const unreadCount = notifications.filter(n => !n.read).length

  const markAsRead = (notificationId: string) => {
    setNotifications(prev => 
      prev.map(n => n.id === notificationId ? { ...n, read: true } : n)
    )
  }

  const markAllAsRead = () => {
    setNotifications(prev => prev.map(n => ({ ...n, read: true })))
  }

  const deleteNotification = (notificationId: string) => {
    setNotifications(prev => prev.filter(n => n.id !== notificationId))
  }

  const clearAllNotifications = () => {
    setNotifications([])
  }

  const getNotificationIcon = (type: Notification['type']) => {
    switch (type) {
      case 'success': return <CheckCircle className="h-5 w-5 text-green-600" />
      case 'warning': return <AlertTriangle className="h-5 w-5 text-yellow-600" />
      case 'error': return <AlertTriangle className="h-5 w-5 text-red-600" />
      default: return <Info className="h-5 w-5 text-blue-600" />
    }
  }

  const getPriorityColor = (priority: Notification['priority']) => {
    switch (priority) {
      case 'urgent': return 'border-l-red-500 bg-red-50'
      case 'high': return 'border-l-orange-500 bg-orange-50'
      case 'medium': return 'border-l-yellow-500 bg-yellow-50'
      default: return 'border-l-blue-500 bg-blue-50'
    }
  }

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-end bg-black bg-opacity-50">
      <Card className="w-full max-w-md h-full overflow-hidden m-0 rounded-none">
        <CardHeader className="border-b">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Bell className="h-5 w-5" />
              <CardTitle>通知中心</CardTitle>
              {unreadCount > 0 && (
                <Badge variant="danger" className="text-xs">
                  {unreadCount}
                </Badge>
              )}
            </div>
            <Button variant="ghost" size="sm" onClick={onClose}>
              <X className="h-4 w-4" />
            </Button>
          </div>

          {/* 标签页切换 */}
          <div className="flex gap-1 bg-gray-100 p-1 rounded-md">
            <button
              onClick={() => setActiveTab('notifications')}
              className={cn(
                'flex-1 px-3 py-2 text-sm font-medium rounded-sm transition-colors',
                activeTab === 'notifications'
                  ? 'bg-white text-gray-900 shadow-sm'
                  : 'text-gray-600 hover:text-gray-900'
              )}
            >
              通知
            </button>
            <button
              onClick={() => setActiveTab('settings')}
              className={cn(
                'flex-1 px-3 py-2 text-sm font-medium rounded-sm transition-colors',
                activeTab === 'settings'
                  ? 'bg-white text-gray-900 shadow-sm'
                  : 'text-gray-600 hover:text-gray-900'
              )}
            >
              设置
            </button>
          </div>
        </CardHeader>

        <CardContent className="p-0 h-full overflow-auto">
          {activeTab === 'notifications' && (
            <div className="h-full flex flex-col">
              {/* 筛选和操作栏 */}
              <div className="p-4 border-b bg-gray-50">
                <div className="flex items-center justify-between mb-3">
                  <select
                    value={filter}
                    onChange={(e) => setFilter(e.target.value)}
                    className="px-3 py-1 border border-gray-300 rounded text-sm"
                  >
                    <option value="all">全部通知</option>
                    <option value="unread">未读</option>
                    <option value="system">系统</option>
                    <option value="workflow">工作流</option>
                    <option value="task">任务</option>
                    <option value="security">安全</option>
                  </select>

                  <div className="flex gap-2">
                    {unreadCount > 0 && (
                      <Button variant="ghost" size="sm" onClick={markAllAsRead}>
                        <Check className="h-4 w-4 mr-1" />
                        全部已读
                      </Button>
                    )}
                    <Button variant="ghost" size="sm" onClick={clearAllNotifications}>
                      <Trash2 className="h-4 w-4 mr-1" />
                      清空
                    </Button>
                  </div>
                </div>
              </div>

              {/* 通知列表 */}
              <div className="flex-1 overflow-y-auto">
                {filteredNotifications.length === 0 ? (
                  <div className="flex items-center justify-center h-full text-gray-500">
                    <div className="text-center">
                      <Bell className="h-8 w-8 mx-auto mb-3 text-gray-400" />
                      <p>暂无通知</p>
                      <p className="text-sm mt-1">当有新通知时会显示在这里</p>
                    </div>
                  </div>
                ) : (
                  <div className="space-y-1">
                    {filteredNotifications.map((notification) => (
                      <div
                        key={notification.id}
                        className={cn(
                          'p-4 border-l-4 hover:bg-gray-50 cursor-pointer transition-colors',
                          !notification.read && 'bg-blue-50 border-l-blue-500',
                          notification.read && 'border-l-gray-200',
                          notification.priority === 'urgent' && getPriorityColor('urgent'),
                          notification.priority === 'high' && getPriorityColor('high')
                        )}
                        onClick={() => markAsRead(notification.id)}
                      >
                        <div className="flex items-start gap-3">
                          {getNotificationIcon(notification.type)}
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2 mb-1">
                              <h4 className="font-medium text-gray-900 text-sm">
                                {notification.title}
                              </h4>
                              {!notification.read && (
                                <div className="w-2 h-2 bg-blue-600 rounded-full" />
                              )}
                            </div>
                            <p className="text-sm text-gray-600 mb-2">
                              {notification.message}
                            </p>
                            <div className="flex items-center justify-between">
                              <div className="flex items-center gap-2 text-xs text-gray-500">
                                <Clock className="h-3 w-3" />
                                <span>{formatDate(notification.timestamp, 'time')}</span>
                                <Badge variant="secondary" className="text-xs">
                                  {notification.category}
                                </Badge>
                                {notification.priority === 'urgent' && (
                                  <Badge variant="danger" className="text-xs">
                                    紧急
                                  </Badge>
                                )}
                              </div>
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={(e) => {
                                  e.stopPropagation()
                                  deleteNotification(notification.id)
                                }}
                                className="h-6 w-6 p-0"
                              >
                                <X className="h-3 w-3" />
                              </Button>
                            </div>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {activeTab === 'settings' && (
            <div className="p-4 space-y-6">
              <NotificationSettings 
                preferences={preferences}
                onPreferencesChange={setPreferences}
              />
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

// 通知设置组件
interface NotificationSettingsProps {
  preferences: NotificationPreferences
  onPreferencesChange: (preferences: NotificationPreferences) => void
}

function NotificationSettings({ preferences, onPreferencesChange }: NotificationSettingsProps) {
  const updatePreference = (key: keyof NotificationPreferences, value: any) => {
    onPreferencesChange({
      ...preferences,
      [key]: value
    })
  }

  const updateCategoryPreference = (category: string, enabled: boolean) => {
    onPreferencesChange({
      ...preferences,
      categories: {
        ...preferences.categories,
        [category]: enabled
      }
    })
  }

  return (
    <div className="space-y-6">
      {/* 基本设置 */}
      <div>
        <h3 className="font-medium text-gray-900 mb-3">通知方式</h3>
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Bell className="h-4 w-4 text-gray-600" />
              <span className="text-sm">桌面通知</span>
            </div>
            <Switch
              checked={preferences.desktop}
              onCheckedChange={(checked) => updatePreference('desktop', checked)}
            />
          </div>

          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              {preferences.sound ? (
                <Volume2 className="h-4 w-4 text-gray-600" />
              ) : (
                <VolumeX className="h-4 w-4 text-gray-600" />
              )}
              <span className="text-sm">声音提醒</span>
            </div>
            <Switch
              checked={preferences.sound}
              onCheckedChange={(checked) => updatePreference('sound', checked)}
            />
          </div>

          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className="text-sm">邮件通知</span>
            </div>
            <Switch
              checked={preferences.email}
              onCheckedChange={(checked) => updatePreference('email', checked)}
            />
          </div>
        </div>
      </div>

      {/* 通知类别 */}
      <div>
        <h3 className="font-medium text-gray-900 mb-3">通知类别</h3>
        <div className="space-y-3">
          {Object.entries(preferences.categories).map(([category, enabled]) => (
            <div key={category} className="flex items-center justify-between">
              <span className="text-sm capitalize">
                {category === 'system' && '系统通知'}
                {category === 'workflow' && '工作流'}
                {category === 'task' && '任务'}
                {category === 'security' && '安全'}
                {category === 'update' && '更新'}
              </span>
              <Switch
                checked={enabled}
                onCheckedChange={(checked) => updateCategoryPreference(category, checked)}
              />
            </div>
          ))}
        </div>
      </div>

      {/* 免打扰时间 */}
      <div>
        <h3 className="font-medium text-gray-900 mb-3">免打扰时间</h3>
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm">启用免打扰</span>
            <Switch
              checked={preferences.quietHours.enabled}
              onCheckedChange={(checked) => 
                updatePreference('quietHours', {
                  ...preferences.quietHours,
                  enabled: checked
                })
              }
            />
          </div>

          {preferences.quietHours.enabled && (
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="text-xs text-gray-600">开始时间</label>
                <input
                  type="time"
                  value={preferences.quietHours.start}
                  onChange={(e) => 
                    updatePreference('quietHours', {
                      ...preferences.quietHours,
                      start: e.target.value
                    })
                  }
                  className="w-full mt-1 px-2 py-1 border border-gray-300 rounded text-sm"
                />
              </div>
              <div>
                <label className="text-xs text-gray-600">结束时间</label>
                <input
                  type="time"
                  value={preferences.quietHours.end}
                  onChange={(e) => 
                    updatePreference('quietHours', {
                      ...preferences.quietHours,
                      end: e.target.value
                    })
                  }
                  className="w-full mt-1 px-2 py-1 border border-gray-300 rounded text-sm"
                />
              </div>
            </div>
          )}
        </div>
      </div>

      {/* 测试通知 */}
      <div>
        <h3 className="font-medium text-gray-900 mb-3">测试</h3>
        <Button
          variant="secondary"
          onClick={() => {
            const testNotification: Notification = {
              id: `test-${Date.now()}`,
              type: 'info',
              title: '测试通知',
              message: '这是一条测试通知，用于验证您的通知设置',
              timestamp: new Date(),
              read: false,
              category: 'system',
              priority: 'medium'
            }
            
            if (preferences.desktop && 'Notification' in window) {
              showDesktopNotification(testNotification)
            }
            
            if (preferences.sound) {
              playNotificationSound()
            }
          }}
        >
          <Bell className="h-4 w-4 mr-2" />
          发送测试通知
        </Button>
      </div>
    </div>
  )
}

// 辅助函数
function generateMockNotifications(): Notification[] {
  return [
    {
      id: '1',
      type: 'success',
      title: '工作流完成',
      message: '客户欢迎邮件工作流已成功执行',
      timestamp: new Date(Date.now() - 300000),
      read: false,
      category: 'workflow',
      priority: 'medium'
    },
    {
      id: '2',
      type: 'warning',
      title: '任务即将到期',
      message: '准备客户演示PPT将在1天后到期',
      timestamp: new Date(Date.now() - 600000),
      read: false,
      category: 'task',
      priority: 'high'
    },
    {
      id: '3',
      type: 'info',
      title: '系统维护通知',
      message: '系统将在今晚23:00-01:00进行维护，期间可能影响服务',
      timestamp: new Date(Date.now() - 900000),
      read: true,
      category: 'system',
      priority: 'medium'
    }
  ]
}

function generateRandomNotification(): Notification {
  const types: Notification['type'][] = ['info', 'warning', 'error', 'success']
  const categories = ['system', 'workflow', 'task', 'security']
  const priorities: Notification['priority'][] = ['low', 'medium', 'high', 'urgent']
  
  const templates = [
    { title: '新交易创建', message: '用户创建了一个新的交易记录' },
    { title: '数据同步完成', message: '客户数据同步已成功完成' },
    { title: '登录异常', message: '检测到异常登录行为，请注意安全' },
    { title: '备份完成', message: '数据库备份已成功完成' }
  ]
  
  const template = templates[Math.floor(Math.random() * templates.length)]
  
  return {
    id: `notif-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
    type: types[Math.floor(Math.random() * types.length)],
    title: template.title,
    message: template.message,
    timestamp: new Date(),
    read: false,
    category: categories[Math.floor(Math.random() * categories.length)],
    priority: priorities[Math.floor(Math.random() * priorities.length)]
  }
}

function showDesktopNotification(notification: Notification) {
  if ('Notification' in window && Notification.permission === 'granted') {
    new Notification(notification.title, {
      body: notification.message,
      icon: '/favicon.ico',
      tag: notification.id
    })
  }
}

function playNotificationSound() {
  // 简单的通知音效（使用Web Audio API）
  const audioContext = new (window.AudioContext || (window as any).webkitAudioContext)()
  const oscillator = audioContext.createOscillator()
  const gainNode = audioContext.createGain()
  
  oscillator.connect(gainNode)
  gainNode.connect(audioContext.destination)
  
  oscillator.frequency.setValueAtTime(800, audioContext.currentTime)
  oscillator.frequency.setValueAtTime(600, audioContext.currentTime + 0.1)
  
  gainNode.gain.setValueAtTime(0.3, audioContext.currentTime)
  gainNode.gain.exponentialRampToValueAtTime(0.01, audioContext.currentTime + 0.5)
  
  oscillator.start(audioContext.currentTime)
  oscillator.stop(audioContext.currentTime + 0.5)
}
