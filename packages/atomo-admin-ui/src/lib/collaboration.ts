/**
 * Real-time Collaboration System
 *
 * Foundational infrastructure for real-time collaboration, including:
 * - WebSocket connection management
 * - User presence synchronization
 * - Conflict resolution
 * - Collaboration event handling
 */

import React from 'react'

type EventHandler = (...args: any[]) => void

class BrowserEventEmitter {
  private listeners = new Map<string, Set<EventHandler>>()

  on(event: string, handler: EventHandler): void {
    const handlers = this.listeners.get(event) ?? new Set<EventHandler>()
    handlers.add(handler)
    this.listeners.set(event, handlers)
  }

  emit(event: string, ...args: any[]): void {
    this.listeners.get(event)?.forEach((handler) => handler(...args))
  }

  removeAllListeners(): void {
    this.listeners.clear()
  }
}

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

export class CollaborationManager extends BrowserEventEmitter {
  private ws: WebSocket | null = null
  private state: CollaborationState
  private reconnectAttempts = 0
  private maxReconnectAttempts = 5
  private heartbeatInterval: ReturnType<typeof setInterval> | null = null

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
   * Connect to the collaboration service.
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
      console.error('Collaboration connection failed:', error)
      this.emit('error', error)
    }
  }

  /**
   * Disconnect.
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
   * Send a collaboration event.
   */
  sendEvent(event: Omit<CollaborationEvent, 'userId' | 'timestamp'>): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      console.warn('WebSocket is not connected; cannot send event')
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
   * Update the cursor position.
   */
  updateCursor(x: number, y: number, element?: string): void {
    this.state.currentUser.cursor = { x, y, element }
    
    this.sendEvent({
      type: 'cursor-move',
      data: { x, y, element }
    })
  }

  /**
   * Update the selection range.
   */
  updateSelection(start: number, end: number, element?: string): void {
    this.state.currentUser.selection = { start, end, element }
    
    this.sendEvent({
      type: 'selection-change',
      data: { start, end, element }
    })
  }

  /**
   * Send a data change.
   */
  sendDataChange(path: string, operation: 'create' | 'update' | 'delete', data: any): void {
    this.sendEvent({
      type: 'data-change',
      data: { path, operation, data }
    })
  }

  /**
   * Send typing status.
   */
  sendTyping(element: string, isTyping: boolean): void {
    this.sendEvent({
      type: 'typing',
      data: { element, isTyping }
    })
  }

  /**
   * Get the current state.
   */
  getState(): CollaborationState {
    return { ...this.state }
  }

  /**
   * Get the list of online users.
   */
  getOnlineUsers(): CollaborationUser[] {
    return Array.from(this.state.users.values())
  }

  private handleOpen(): void {
    console.log('Collaboration connection established')
    this.state.isConnected = true
    this.reconnectAttempts = 0
    
    // Send the user-join event
    this.sendEvent({
      type: 'user-join',
      data: this.state.currentUser
    })

    // Start the heartbeat
    this.startHeartbeat()
    
    this.emit('connected')
  }

  private handleMessage(event: MessageEvent): void {
    try {
      const collaborationEvent: CollaborationEvent = JSON.parse(event.data)
      this.processEvent(collaborationEvent)
    } catch (error) {
      console.error('Failed to parse collaboration message:', error)
    }
  }

  private handleClose(): void {
    console.log('Collaboration connection closed')
    this.state.isConnected = false
    
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval)
      this.heartbeatInterval = null
    }

    // Attempt to reconnect
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++
      console.log(`Attempting to reconnect (${this.reconnectAttempts}/${this.maxReconnectAttempts})`)
      setTimeout(() => this.connect(), 2000 * this.reconnectAttempts)
    }

    this.emit('disconnected')
  }

  private handleError(error: Event): void {
    console.error('Collaboration connection error:', error)
    this.emit('error', error)
  }

  private processEvent(event: CollaborationEvent): void {
    // Ignore events sent by ourselves
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
    }, 30000) // 30-second heartbeat
  }
}

/**
 * User color generator.
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
 * Collaboration hook.
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

    // Connect to the collaboration service
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
