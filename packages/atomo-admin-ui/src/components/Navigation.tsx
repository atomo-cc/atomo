import React from 'react'
import { useQuery } from '@tanstack/react-query'
import { Link, useLocation } from 'react-router-dom'
import { 
  LayoutDashboard, 
  Sparkles,
  Receipt,
  Wallet,
  Crown,
  Timer,
  BarChart3,
  Users, 
  Building2, 
  DollarSign, 
  Activity,
  Workflow,
  Trash2,
  Settings,
  HelpCircle,
  ShieldCheck,
  Layers,
  X
} from 'lucide-react'

import { apiClient, AuthUser } from '../lib/api'
import { getFieldLabel } from '../lib/utils'

interface NavigationProps {
  user?: AuthUser | null
  isMobileOpen?: boolean
  onCloseMobileMenu?: () => void
}

export function Navigation({ user, isMobileOpen, onCloseMobileMenu }: NavigationProps) {
  const location = useLocation()

  // Load schema metadata
  const { data: schema } = useQuery({
    queryKey: ['schema-metadata'],
    queryFn: () => apiClient.getSchemaMetadata(),
    staleTime: 5 * 60 * 1000,
  })

  const models = schema?.models ? Object.keys(schema.models) : []

  // Semantic icon assignment based on model name
  const getModelIcon = (modelName: string) => {
    const lower = modelName.toLowerCase()
    if (lower.includes('generation') || lower.includes('job') || lower.includes('image')) return Sparkles
    if (lower.includes('ledger') || lower.includes('transaction')) return Receipt
    if (lower.includes('balance') || lower.includes('credit')) return Wallet
    if (lower.includes('subscription') || lower.includes('plan')) return Crown
    if (lower.includes('trial') || lower.includes('usage')) return Timer
    if (lower.includes('event') || lower.includes('telemetry') || lower.includes('metric')) return BarChart3
    if (lower.includes('user') || lower.includes('account') || lower.includes('customer')) return Users
    if (lower.includes('company') || lower.includes('org')) return Building2
    if (lower.includes('deal') || lower.includes('order')) return DollarSign
    return Layers
  }

  // Active route checking
  const isActive = (path: string) => {
    const base = (import.meta as any).env.BASE_URL || '/'
    const cleanPath = location.pathname.replace(new RegExp(`^${base}`), '/')
    if (path === '/') {
      return cleanPath === '/' || cleanPath === '/dashboard'
    }
    return cleanPath.startsWith(path)
  }

  const linkClass = (active: boolean) =>
    `flex items-center space-x-3 px-3.5 py-2.5 rounded-bn text-sm font-medium transition-all duration-150 ${
      active
        ? 'bg-primary/10 text-primary shadow-sm font-semibold'
        : 'text-icon-muted hover:text-foreground hover:bg-content-bg'
    }`

  return (
    <>
      {/* Mobile Backdrop */}
      {isMobileOpen && (
        <div
          className="fixed inset-0 bg-black/40 backdrop-blur-sm z-40 lg:hidden"
          onClick={onCloseMobileMenu}
        />
      )}

      <aside
        className={`fixed inset-y-0 left-0 z-50 w-64 bg-sidebar border-r border-bn-border flex flex-col transition-transform duration-300 ease-in-out lg:translate-x-0 ${
          isMobileOpen ? 'translate-x-0' : '-translate-x-full'
        }`}
      >
        {/* Dashin Brand Header */}
        <div className="h-16 px-5 border-b border-bn-border flex items-center justify-between">
          <Link to="/" className="flex items-center space-x-3 group" onClick={onCloseMobileMenu}>
            <div className="w-9 h-9 rounded-bn bg-primary-gradient flex items-center justify-center text-white shadow-bn group-hover:scale-105 transition-transform">
              <Sparkles className="w-5 h-5" />
            </div>
            <div>
              <div className="font-bold text-foreground text-sm tracking-tight flex items-center space-x-1.5">
                <span>{(schema?.config as any)?.title || (schema?.config as any)?.name || 'Dashin Admin'}</span>
              </div>
              <div className="text-[10px] text-icon-muted font-medium">
                {(schema?.config as any)?.description || 'Schema Management'}
              </div>
            </div>
          </Link>

          {onCloseMobileMenu && (
            <button
              onClick={onCloseMobileMenu}
              className="lg:hidden p-1.5 rounded-bn text-icon-muted hover:text-foreground"
            >
              <X className="w-5 h-5" />
            </button>
          )}
        </div>

        {/* Navigation Content */}
        <div className="flex-1 overflow-y-auto px-3 py-4 space-y-6">
          {/* Main Dashboard */}
          <div>
            <Link
              to="/"
              onClick={onCloseMobileMenu}
              className={linkClass(isActive('/'))}
            >
              <LayoutDashboard className="w-4 h-4" />
              <span>Dashboard</span>
            </Link>
          </div>

          {/* Introspected Models Group */}
          <div>
            <div className="px-3 mb-2 text-[11px] font-bold uppercase tracking-wider text-icon-muted">
              Data Models
            </div>
            <div className="space-y-1">
              {models.map((m) => {
                const Icon = getModelIcon(m)
                const active = isActive(`/entities/${m}`)
                const meta = schema?.models[m]
                const label = meta ? getFieldLabel(m, meta?.tableName) : m
                return (
                  <Link
                    key={m}
                    to={`/entities/${m}`}
                    onClick={onCloseMobileMenu}
                    className={linkClass(active)}
                  >
                    <Icon className="w-4 h-4 flex-shrink-0" />
                    <span className="truncate flex-1">{label}</span>
                  </Link>
                )
              })}
            </div>
          </div>

          {/* Platform Tools */}
          <div>
            <div className="px-3 mb-2 text-[11px] font-bold uppercase tracking-wider text-icon-muted">
              Platform
            </div>
            <div className="space-y-1">
              <Link
                to="/workflows"
                onClick={onCloseMobileMenu}
                className={linkClass(isActive('/workflows'))}
              >
                <Workflow className="w-4 h-4" />
                <span>Workflows</span>
              </Link>
              <Link
                to="/observability"
                onClick={onCloseMobileMenu}
                className={linkClass(isActive('/observability'))}
              >
                <Activity className="w-4 h-4" />
                <span>Observability</span>
              </Link>
              <Link
                to="/trash"
                onClick={onCloseMobileMenu}
                className={linkClass(isActive('/trash'))}
              >
                <Trash2 className="w-4 h-4" />
                <span>Trash</span>
              </Link>
            </div>
          </div>

          {/* System Settings */}
          <div>
            <div className="px-3 mb-2 text-[11px] font-bold uppercase tracking-wider text-icon-muted">
              System
            </div>
            <div className="space-y-1">
              <Link
                to="/settings"
                onClick={onCloseMobileMenu}
                className={linkClass(isActive('/settings'))}
              >
                <Settings className="w-4 h-4" />
                <span>Settings</span>
              </Link>
              <Link
                to="/help"
                onClick={onCloseMobileMenu}
                className={linkClass(isActive('/help'))}
              >
                <HelpCircle className="w-4 h-4" />
                <span>Documentation</span>
              </Link>
            </div>
          </div>
        </div>

        {/* Sidebar Footer */}
        <div className="p-3 border-t border-bn-border bg-content-bg/50">
          <div className="flex items-center space-x-3 px-2 py-1.5">
            <ShieldCheck className="w-4 h-4 text-success" />
            <div className="text-xs">
              <span className="font-semibold text-foreground">Secure Core</span>
              <span className="text-[10px] text-icon-muted block">Schema-Driven Admin</span>
            </div>
          </div>
        </div>
      </aside>
    </>
  )
}
