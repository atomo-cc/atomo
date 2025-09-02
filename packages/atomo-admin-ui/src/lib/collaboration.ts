/**
 * Real-time Collaboration System
 * 
 * 提供实时协作功能的基础架构，包括：
 * - WebSocket连接管理
 * - 用户状态同步
 * - 冲突解决机制
 * - 协作事件处理
 */

import { EventEmitter } from 'events'

export interface CollaborationUser {
  id: string
  name: string
  avatar?: string
  color: string
  cursor?: {
    x: number
    y: number
    element?: string
  }
  selection?: {
    start: number
    end: number
    element?: string
  }
  lastSeen: Date
}

export interface CollaborationEvent {
  type: 'user-join' | 'user-leave' | 'cursor-move' | 'selection-change' | 'data-change' | 'typing'
  userId: string
  data: any
  timestamp: Date
}

export interface CollaborationState {
  users: Map<string, CollaborationUser>
  currentUser: CollaborationUser
  roomId: string
  isConnected: boolean
}

export class CollaborationManager extends EventEmitter {
  private ws: WebSocket | null = null
  private state: CollaborationState
  private reconnectAttempts = 0
  private maxReconnectAttempts = 5
  private heartbeatInterval: NodeJS.Timeout | null = null

  constructor(roomId: string, currentUser: Omit<CollaborationUser, 'lastSeen'>) {
    super()
    
    this.state = {
      users: new Map(),
      currentUser: {
        ...currentUser,
        lastSeen: new Date()
      },
      roomId,
      isConnected: false
    }
  }

  /**
   * 连接到协作服务
   */
  async connect(wsUrl?: string): Promise<void> {
    const url = wsUrl || `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}/api/collaboration`
    
    try {
      this.ws = new WebSocket(`${url}?room=${this.state.roomId}&user=${this.state.currentUser.id}`)
      
      this.ws.onopen = this.handleOpen.bind(this)
      this.ws.onmessage = this.handleMessage.bind(this)
      this.ws.onclose = this.handleClose.bind(this)
      this.ws.onerror = this.handleError.bind(this)
      
    } catch (error) {
      console.error('协作连接失败:', error)
      this.emit('error', error)
    }
  }

  /**
   * 断开连接
   */
  disconnect(): void {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval)
      this.heartbeatInterval = null
    }

    if (this.ws) {
      this.ws.close()
      this.ws = null
    }

    this.state.isConnected = false
    this.state.users.clear()
    this.emit('disconnected')
  }

  /**
   * 发送协作事件
   */
  sendEvent(event: Omit<CollaborationEvent, 'userId' | 'timestamp'>): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      console.warn('WebSocket 未连接，无法发送事件')
      return
    }

    const fullEvent: CollaborationEvent = {
      ...event,
      userId: this.state.currentUser.id,
      timestamp: new Date()
    }

    this.ws.send(JSON.stringify(fullEvent))
  }

  /**
   * 更新光标位置
   */
  updateCursor(x: number, y: number, element?: string): void {
    this.state.currentUser.cursor = { x, y, element }
    
    this.sendEvent({
      type: 'cursor-move',
      data: { x, y, element }
    })
  }

  /**
   * 更新选择范围
   */
  updateSelection(start: number, end: number, element?: string): void {
    this.state.currentUser.selection = { start, end, element }
    
    this.sendEvent({
      type: 'selection-change',
      data: { start, end, element }
    })
  }

  /**
   * 发送数据变更
   */
  sendDataChange(path: string, operation: 'create' | 'update' | 'delete', data: any): void {
    this.sendEvent({
      type: 'data-change',
      data: { path, operation, data }
    })
  }

  /**
   * 发送输入状态
   */
  sendTyping(element: string, isTyping: boolean): void {
    this.sendEvent({
      type: 'typing',
      data: { element, isTyping }
    })
  }

  /**
   * 获取当前状态
   */
  getState(): CollaborationState {
    return { ...this.state }
  }

  /**
   * 获取在线用户列表
   */
  getOnlineUsers(): CollaborationUser[] {
    return Array.from(this.state.users.values())
  }

  private handleOpen(): void {
    console.log('协作连接已建立')
    this.state.isConnected = true
    this.reconnectAttempts = 0
    
    // 发送用户加入事件
    this.sendEvent({
      type: 'user-join',
      data: this.state.currentUser
    })

    // 启动心跳
    this.startHeartbeat()
    
    this.emit('connected')
  }

  private handleMessage(event: MessageEvent): void {
    try {
      const collaborationEvent: CollaborationEvent = JSON.parse(event.data)
      this.processEvent(collaborationEvent)
    } catch (error) {
      console.error('解析协作消息失败:', error)
    }
  }

  private handleClose(): void {
    console.log('协作连接已断开')
    this.state.isConnected = false
    
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval)
      this.heartbeatInterval = null
    }

    // 尝试重连
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++
      console.log(`尝试重连 (${this.reconnectAttempts}/${this.maxReconnectAttempts})`)
      setTimeout(() => this.connect(), 2000 * this.reconnectAttempts)
    }

    this.emit('disconnected')
  }

  private handleError(error: Event): void {
    console.error('协作连接错误:', error)
    this.emit('error', error)
  }

  private processEvent(event: CollaborationEvent): void {
    // 忽略自己发送的事件
    if (event.userId === this.state.currentUser.id) {
      return
    }

    switch (event.type) {
      case 'user-join':
        this.state.users.set(event.userId, {
          ...event.data,
          lastSeen: new Date(event.timestamp)
        })
        this.emit('user-join', event.data)
        break

      case 'user-leave':
        this.state.users.delete(event.userId)
        this.emit('user-leave', event.userId)
        break

      case 'cursor-move':
        const user = this.state.users.get(event.userId)
        if (user) {
          user.cursor = event.data
          user.lastSeen = new Date(event.timestamp)
          this.emit('cursor-move', event.userId, event.data)
        }
        break

      case 'selection-change':
        const userWithSelection = this.state.users.get(event.userId)
        if (userWithSelection) {
          userWithSelection.selection = event.data
          userWithSelection.lastSeen = new Date(event.timestamp)
          this.emit('selection-change', event.userId, event.data)
        }
        break

      case 'data-change':
        this.emit('data-change', event.userId, event.data)
        break

      case 'typing':
        this.emit('typing', event.userId, event.data)
        break
    }
  }

  private startHeartbeat(): void {
    this.heartbeatInterval = setInterval(() => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: 'ping' }))
      }
    }, 30000) // 30秒心跳
  }
}

/**
 * 用户颜色生成器
 */
export function generateUserColor(userId: string): string {
  const colors = [
    '#FF6B6B', '#4ECDC4', '#45B7D1', '#96CEB4', '#FFEAA7',
    '#DDA0DD', '#98D8C8', '#F7DC6F', '#BB8FCE', '#85C1E9'
  ]
  
  let hash = 0
  for (let i = 0; i < userId.length; i++) {
    hash = userId.charCodeAt(i) + ((hash << 5) - hash)
  }
  
  return colors[Math.abs(hash) % colors.length]
}

/**
 * 协作钩子
 */
export function useCollaboration(roomId: string, currentUser: Omit<CollaborationUser, 'lastSeen' | 'color'>) {
  const [manager] = React.useState(() => {
    const userWithColor = {
      ...currentUser,
      color: generateUserColor(currentUser.id)
    }
    return new CollaborationManager(roomId, userWithColor)
  })

  const [state, setState] = React.useState(manager.getState())
  const [onlineUsers, setOnlineUsers] = React.useState<CollaborationUser[]>([])

  React.useEffect(() => {
    const updateState = () => {
      setState(manager.getState())
      setOnlineUsers(manager.getOnlineUsers())
    }

    manager.on('connected', updateState)
    manager.on('disconnected', updateState)
    manager.on('user-join', updateState)
    manager.on('user-leave', updateState)
    manager.on('cursor-move', updateState)
    manager.on('selection-change', updateState)

    // 连接到协作服务
    manager.connect()

    return () => {
      manager.removeAllListeners()
      manager.disconnect()
    }
  }, [manager])

  return {
    manager,
    state,
    onlineUsers,
    isConnected: state.isConnected
  }
}

// 导入React（用于钩子）
import React from 'react'
