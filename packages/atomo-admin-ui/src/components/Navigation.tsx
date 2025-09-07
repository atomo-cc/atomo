/**
 * Navigation - 动态导航组件
 * 
 * 根据 Schema 元数据自动生成导航菜单
 */

import React, { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Link, useLocation } from 'react-router-dom'
import { 
  Home, 
  Users, 
  Building2, 
  DollarSign, 
  Activity,
  Menu,
  X,
  ChevronDown,
  Settings,
  HelpCircle
} from 'lucide-react'

import { apiClient } from '../lib/api'
import { cn, getFieldLabel } from '../lib/utils'
import { Button } from './ui/Button'

export function Navigation() {
  const location = useLocation()
  const [isMobileOpen, setIsMobileOpen] = useState(false)

  // 加载 Schema 元数据用于生成导航
  const { data: schema, isLoading } = useQuery({
    queryKey: ['schema-metadata'],
    queryFn: () => apiClient.getSchemaMetadata(),
    staleTime: 5 * 60 * 1000,
  })

  // 图标映射
  const getModelIcon = (modelName: string) => {
    const iconMap: Record<string, React.ComponentType<any>> = {
      contact: Users,
      company: Building2,
      deal: DollarSign,
      user: Users,
      default: Activity
    }
    
    return iconMap[modelName.toLowerCase()] || iconMap.default
  }

  // 检查当前路径是否激活
  const isActive = (path: string) => {
    if (path === '/' || path === '/dashboard') {
      return location.pathname === '/' || location.pathname === '/dashboard'
    }
    return location.pathname.startsWith(path)
  }

  let navigationItems = [
    {
      name: '仪表盘',
      href: '/',
      icon: Home,
      active: isActive('/')
    },
    ...(schema ? Object.keys(schema.models).map(modelName => ({
      name: getFieldLabel(modelName),
      href: `/entities/${modelName}`,
      icon: getModelIcon(modelName),
      active: isActive(`/entities/${modelName}`)
    })) : [])
  ]

  // Add custom CRM links when relevant models exist
  if (schema && schema.models['Deal']) {
    navigationItems = [
      ...navigationItems,
      {
        name: '商机看板',
        href: '/deals/board',
        icon: DollarSign,
        active: isActive('/deals/board')
      }
    ]
  }

  const sidebarContent = (
    <div className="flex flex-col h-full">
      {/* Logo */}
      <div className="flex items-center h-16 px-6 border-b border-gray-200">
        <div className="flex items-center">
          <div className="w-8 h-8 bg-primary-600 rounded-lg flex items-center justify-center">
            <span className="text-white font-bold text-lg">A</span>
          </div>
          <span className="ml-3 text-xl font-semibold text-gray-900">
            Atomo Admin
          </span>
        </div>
      </div>

      {/* 导航菜单 */}
      <nav className="flex-1 px-4 py-6 space-y-2">
        {isLoading ? (
          <div className="space-y-2">
            {[1, 2, 3].map(i => (
              <div key={i} className="h-10 bg-gray-200 rounded animate-pulse" />
            ))}
          </div>
        ) : (
          navigationItems.map((item) => {
            const IconComponent = item.icon
            return (
              <Link
                key={item.href}
                to={item.href}
                onClick={() => setIsMobileOpen(false)}
                className={cn(
                  'flex items-center px-3 py-2 text-sm font-medium rounded-md transition-colors',
                  item.active
                    ? 'bg-primary-100 text-primary-700 border-r-2 border-primary-700'
                    : 'text-gray-600 hover:bg-gray-100 hover:text-gray-900'
                )}
              >
                <IconComponent className="mr-3 h-5 w-5" />
                {item.name}
              </Link>
            )
          })
        )}
      </nav>

      {/* 底部菜单 */}
      <div className="border-t border-gray-200 p-4 space-y-2">
        <Link
          to="/settings"
          className="flex items-center px-3 py-2 text-sm font-medium text-gray-600 rounded-md hover:bg-gray-100 hover:text-gray-900"
        >
          <Settings className="mr-3 h-5 w-5" />
          设置
        </Link>
        
        <Link
          to="/help"
          className="flex items-center px-3 py-2 text-sm font-medium text-gray-600 rounded-md hover:bg-gray-100 hover:text-gray-900"
        >
          <HelpCircle className="mr-3 h-5 w-5" />
          帮助
        </Link>
      </div>

      {/* 用户信息 */}
      <div className="border-t border-gray-200 p-4">
        <div className="flex items-center">
          <div className="w-8 h-8 bg-gray-300 rounded-full flex items-center justify-center">
            <span className="text-gray-600 text-sm font-medium">A</span>
          </div>
          <div className="ml-3">
            <p className="text-sm font-medium text-gray-900">管理员</p>
            <p className="text-xs text-gray-500">admin@atomo.dev</p>
          </div>
        </div>
      </div>
    </div>
  )

  return (
    <>
      {/* 移动端菜单按钮 */}
      <div className="lg:hidden">
        <div className="flex items-center justify-between h-16 px-4 bg-white border-b border-gray-200">
          <div className="flex items-center">
            <div className="w-8 h-8 bg-primary-600 rounded-lg flex items-center justify-center">
              <span className="text-white font-bold text-lg">A</span>
            </div>
            <span className="ml-3 text-xl font-semibold text-gray-900">
              Atomo Admin
            </span>
          </div>
          
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setIsMobileOpen(true)}
          >
            <Menu className="h-6 w-6" />
          </Button>
        </div>
      </div>

      {/* 桌面端侧边栏 */}
      <div className="hidden lg:fixed lg:inset-y-0 lg:flex lg:w-64 lg:flex-col">
        <div className="flex-1 flex flex-col min-h-0 bg-white border-r border-gray-200">
          {sidebarContent}
        </div>
      </div>

      {/* 移动端侧边栏覆盖层 */}
      {isMobileOpen && (
        <div className="fixed inset-0 flex z-40 lg:hidden">
          {/* 背景遮罩 */}
          <div
            className="fixed inset-0 bg-gray-600 bg-opacity-75"
            onClick={() => setIsMobileOpen(false)}
          />
          
          {/* 侧边栏 */}
          <div className="relative flex-1 flex flex-col max-w-xs w-full bg-white">
            <div className="absolute top-0 right-0 -mr-12 pt-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setIsMobileOpen(false)}
                className="text-white hover:text-white"
              >
                <X className="h-6 w-6" />
              </Button>
            </div>
            
            {sidebarContent}
          </div>
        </div>
      )}
    </>
  )
}
