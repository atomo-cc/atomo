/**
 * Collaboration Indicator - 实时协作指示器
 * 
 * 显示在线用户、协作状态和用户活动
 */

import React from 'react'
import { Users, Wifi, WifiOff, Circle } from 'lucide-react'
import { CollaborationUser } from '../../lib/collaboration'
import { Badge } from '../ui/Badge'
import { Card, CardContent } from '../ui/Card'
import { cn } from '../../lib/utils'

interface CollaborationIndicatorProps {
  onlineUsers: CollaborationUser[]
  isConnected: boolean
  currentUserId: string
  className?: string
}

export function CollaborationIndicator({ 
  onlineUsers, 
  isConnected, 
  currentUserId,
  className 
}: CollaborationIndicatorProps) {
  const otherUsers = onlineUsers.filter(user => user.id !== currentUserId)

  return (
    <div className={cn('flex items-center gap-3', className)}>
      {/* 连接状态 */}
      <div className="flex items-center gap-1">
        {isConnected ? (
          <Wifi className="h-4 w-4 text-green-600" />
        ) : (
          <WifiOff className="h-4 w-4 text-red-600" />
        )}
        <span className={cn(
          'text-xs font-medium',
          isConnected ? 'text-green-600' : 'text-red-600'
        )}>
          {isConnected ? '已连接' : '已断开'}
        </span>
      </div>

      {/* 在线用户 */}
      {isConnected && otherUsers.length > 0 && (
        <div className="flex items-center gap-2">
          <Users className="h-4 w-4 text-gray-600" />
          <div className="flex items-center gap-1">
            {otherUsers.slice(0, 3).map((user) => (
              <UserAvatar key={user.id} user={user} />
            ))}
            {otherUsers.length > 3 && (
              <Badge variant="secondary" className="text-xs h-6 w-6 p-0 flex items-center justify-center rounded-full">
                +{otherUsers.length - 3}
              </Badge>
            )}
          </div>
        </div>
      )}

      {/* 无其他用户时的提示 */}
      {isConnected && otherUsers.length === 0 && (
        <span className="text-xs text-gray-500">只有你在线</span>
      )}
    </div>
  )
}

interface UserAvatarProps {
  user: CollaborationUser
  size?: 'sm' | 'md' | 'lg'
}

function UserAvatar({ user, size = 'sm' }: UserAvatarProps) {
  const sizeClasses = {
    sm: 'h-6 w-6 text-xs',
    md: 'h-8 w-8 text-sm',
    lg: 'h-10 w-10 text-base'
  }

  return (
    <div
      className={cn(
        'rounded-full flex items-center justify-center font-medium text-white relative',
        sizeClasses[size]
      )}
      style={{ backgroundColor: user.color }}
      title={user.name}
    >
      {user.avatar ? (
        <img 
          src={user.avatar} 
          alt={user.name}
          className="w-full h-full rounded-full object-cover"
        />
      ) : (
        user.name.charAt(0).toUpperCase()
      )}
      
      {/* 在线状态指示器 */}
      <Circle 
        className="absolute -bottom-0.5 -right-0.5 h-2 w-2 fill-green-500 text-green-500 border border-white rounded-full" 
      />
    </div>
  )
}

/**
 * 协作光标组件
 */
interface CollaborationCursorProps {
  user: CollaborationUser
  x: number
  y: number
}

export function CollaborationCursor({ user, x, y }: CollaborationCursorProps) {
  return (
    <div
      className="fixed pointer-events-none z-50"
      style={{ left: x, top: y }}
    >
      <div className="relative">
        {/* 光标 */}
        <svg 
          width="16" 
          height="16" 
          viewBox="0 0 16 16" 
          className="relative z-10"
        >
          <path
            d="M0 0L0 14L4 10L8 16L10 15L6 9L14 9L0 0Z"
            fill={user.color}
            stroke="white"
            strokeWidth="1"
          />
        </svg>
        
        {/* 用户名标签 */}
        <div 
          className="absolute top-4 left-2 px-2 py-1 rounded text-white text-xs font-medium whitespace-nowrap"
          style={{ backgroundColor: user.color }}
        >
          {user.name}
        </div>
      </div>
    </div>
  )
}

/**
 * 协作选择范围组件
 */
interface CollaborationSelectionProps {
  user: CollaborationUser
  element: HTMLElement
  start: number
  end: number
}

export function CollaborationSelection({ user, element, start, end }: CollaborationSelectionProps) {
  React.useEffect(() => {
    if (!element) return

    // 创建选择范围高亮
    const range = document.createRange()
    const selection = window.getSelection()
    
    try {
      range.setStart(element.firstChild || element, start)
      range.setEnd(element.firstChild || element, end)
      
      // 创建高亮元素
      const highlight = document.createElement('span')
      highlight.className = 'collaboration-selection'
      highlight.style.backgroundColor = user.color + '30' // 30% 透明度
      highlight.style.position = 'relative'
      
      range.surroundContents(highlight)
      
    } catch (error) {
      console.warn('无法创建协作选择高亮:', error)
    }

    return () => {
      // 清理高亮
      const highlights = element.querySelectorAll('.collaboration-selection')
      highlights.forEach(h => {
        const parent = h.parentNode
        if (parent) {
          parent.insertBefore(h.firstChild || document.createTextNode(''), h)
          parent.removeChild(h)
        }
      })
    }
  }, [user, element, start, end])

  return null
}

/**
 * 输入状态指示器
 */
interface TypingIndicatorProps {
  users: CollaborationUser[]
  element?: string
}

export function TypingIndicator({ users, element }: TypingIndicatorProps) {
  if (users.length === 0) return null

  return (
    <div className="flex items-center gap-2 text-xs text-gray-500">
      <div className="flex items-center gap-1">
        {users.slice(0, 2).map(user => (
          <Circle 
            key={user.id}
            className="h-1 w-1 animate-pulse"
            style={{ color: user.color }}
          />
        ))}
      </div>
      <span>
        {users.length === 1 
          ? `${users[0].name} 正在输入...`
          : users.length === 2
          ? `${users[0].name} 和 ${users[1].name} 正在输入...`
          : `${users[0].name} 等 ${users.length} 人正在输入...`
        }
      </span>
    </div>
  )
}
