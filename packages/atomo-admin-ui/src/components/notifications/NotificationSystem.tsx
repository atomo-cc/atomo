/**
 * Notification System - Notification system
 *
 * Provides real-time notifications and reminders, including:
 * - Real-time push notifications
 * - Notification center management
 * - Notification preferences
 * - Desktop and mobile push
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

  // Simulate receiving real-time notifications
  useEffect(() => {
    // Load initial notifications
    setNotifications(generateMockNotifications())

    // Simulate real-time notifications
    const interval = setInterval(() => {
      if (Math.random() < 0.3) { // 30% chance of receiving a new notification
        const newNotification = generateRandomNotification()
        setNotifications(prev => [newNotification, ...prev])

        // Show desktop notification
        if (preferences.desktop && 'Notification' in window) {
          showDesktopNotification(newNotification)
        }

        // Play sound
        if (preferences.sound) {
          playNotificationSound()
        }
      }
    }, 10000) // Check every 10 seconds

    return () => clearInterval(interval)
  }, [preferences.desktop, preferences.sound])

  // Request desktop notification permission
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
              <CardTitle>Notification Center</CardTitle>
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

          {/* Tab switcher */}
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
              Notifications
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
              Settings
            </button>
          </div>
        </CardHeader>

        <CardContent className="p-0 h-full overflow-auto">
          {activeTab === 'notifications' && (
            <div className="h-full flex flex-col">
              {/* Filter and action bar */}
              <div className="p-4 border-b bg-gray-50">
                <div className="flex items-center justify-between mb-3">
                  <select
                    value={filter}
                    onChange={(e) => setFilter(e.target.value)}
                    className="px-3 py-1 border border-gray-300 rounded text-sm"
                  >
                    <option value="all">All Notifications</option>
                    <option value="unread">Unread</option>
                    <option value="system">System</option>
                    <option value="workflow">Workflow</option>
                    <option value="task">Task</option>
                    <option value="security">Security</option>
                  </select>

                  <div className="flex gap-2">
                    {unreadCount > 0 && (
                      <Button variant="ghost" size="sm" onClick={markAllAsRead}>
                        <Check className="h-4 w-4 mr-1" />
                        Mark all read
                      </Button>
                    )}
                    <Button variant="ghost" size="sm" onClick={clearAllNotifications}>
                      <Trash2 className="h-4 w-4 mr-1" />
                      Clear all
                    </Button>
                  </div>
                </div>
              </div>

              {/* Notification list */}
              <div className="flex-1 overflow-y-auto">
                {filteredNotifications.length === 0 ? (
                  <div className="flex items-center justify-center h-full text-gray-500">
                    <div className="text-center">
                      <Bell className="h-8 w-8 mx-auto mb-3 text-gray-400" />
                      <p>No notifications</p>
                      <p className="text-sm mt-1">New notifications will appear here</p>
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
                                    Urgent
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

// Notification settings component
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
      {/* Basic settings */}
      <div>
        <h3 className="font-medium text-gray-900 mb-3">Notification Methods</h3>
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Bell className="h-4 w-4 text-gray-600" />
              <span className="text-sm">Desktop notifications</span>
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
              <span className="text-sm">Sound alerts</span>
            </div>
            <Switch
              checked={preferences.sound}
              onCheckedChange={(checked) => updatePreference('sound', checked)}
            />
          </div>

          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className="text-sm">Email notifications</span>
            </div>
            <Switch
              checked={preferences.email}
              onCheckedChange={(checked) => updatePreference('email', checked)}
            />
          </div>
        </div>
      </div>

      {/* Notification categories */}
      <div>
        <h3 className="font-medium text-gray-900 mb-3">Notification Categories</h3>
        <div className="space-y-3">
          {Object.entries(preferences.categories).map(([category, enabled]) => (
            <div key={category} className="flex items-center justify-between">
              <span className="text-sm capitalize">
                {category === 'system' && 'System notifications'}
                {category === 'workflow' && 'Workflow'}
                {category === 'task' && 'Task'}
                {category === 'security' && 'Security'}
                {category === 'update' && 'Update'}
              </span>
              <Switch
                checked={enabled}
                onCheckedChange={(checked) => updateCategoryPreference(category, checked)}
              />
            </div>
          ))}
        </div>
      </div>

      {/* Do not disturb hours */}
      <div>
        <h3 className="font-medium text-gray-900 mb-3">Do Not Disturb</h3>
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm">Enable Do Not Disturb</span>
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
                <label className="text-xs text-gray-600">Start time</label>
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
                <label className="text-xs text-gray-600">End time</label>
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

      {/* Test notification */}
      <div>
        <h3 className="font-medium text-gray-900 mb-3">Test</h3>
        <Button
          variant="secondary"
          onClick={() => {
            const testNotification: Notification = {
              id: `test-${Date.now()}`,
              type: 'info',
              title: 'Test Notification',
              message: 'This is a test notification to verify your notification settings',
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
          Send test notification
        </Button>
      </div>
    </div>
  )
}

// Helper functions
function generateMockNotifications(): Notification[] {
  return [
    {
      id: '1',
      type: 'success',
      title: 'Workflow Completed',
      message: 'The customer welcome email workflow ran successfully',
      timestamp: new Date(Date.now() - 300000),
      read: false,
      category: 'workflow',
      priority: 'medium'
    },
    {
      id: '2',
      type: 'warning',
      title: 'Task Due Soon',
      message: 'Prepare customer demo slides is due in 1 day',
      timestamp: new Date(Date.now() - 600000),
      read: false,
      category: 'task',
      priority: 'high'
    },
    {
      id: '3',
      type: 'info',
      title: 'System Maintenance Notice',
      message: 'The system will undergo maintenance tonight from 23:00 to 01:00, which may affect service',
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
    { title: 'New Deal Created', message: 'A user created a new deal record' },
    { title: 'Data Sync Completed', message: 'Customer data sync completed successfully' },
    { title: 'Login Anomaly', message: 'Unusual login activity detected, please review your security' },
    { title: 'Backup Completed', message: 'Database backup completed successfully' }
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
  // Simple notification sound (using the Web Audio API)
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
