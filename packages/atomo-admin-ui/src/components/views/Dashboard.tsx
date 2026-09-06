import React from 'react'
import { useQuery } from '@tanstack/react-query'
import { Link } from 'react-router-dom'
import { 
  Sparkles, 
  Receipt, 
  Crown, 
  Timer, 
  BarChart3, 
  ArrowRight, 
  Activity, 
  CheckCircle2, 
  Layers, 
  Plus,
  ShieldCheck,
  Server
} from 'lucide-react'

import { SchemaMetadata } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { getFieldLabel } from '../../lib/utils'

interface DashboardProps {
  schema: SchemaMetadata
}

export function Dashboard({ schema }: DashboardProps) {
  const models = schema ? Object.keys(schema.models) : []

  // Fetch count for each model
  const { data: counts, isLoading } = useQuery({
    queryKey: ['dashboard-counts', models],
    queryFn: async () => {
      const entries = await Promise.all(
        models.map(async (m) => {
          try {
            const res = await apiClient.listEntities(m, { limit: 1 })
            return [m, res.total] as const
          } catch {
            return [m, 0] as const
          }
        })
      )
      return Object.fromEntries(entries) as Record<string, number>
    },
    staleTime: 30_000,
  })

  // Specific high-level stats for PhotoEasy
  const genCount = counts?.GenerationJob ?? 0
  const ledgerCount = counts?.CreditLedger ?? 0
  const subCount = counts?.Subscription ?? 0
  const trialCount = counts?.TrialUsage ?? 0

  const kpis = [
    {
      title: 'Generation Jobs',
      value: genCount,
      desc: 'Total AI generation requests',
      icon: Sparkles,
      href: '/entities/GenerationJob',
      color: 'from-blue-500 to-indigo-600',
    },
    {
      title: 'Credit Ledger',
      value: ledgerCount,
      desc: 'Immutable financial audits',
      icon: Receipt,
      href: '/entities/CreditLedger',
      color: 'from-emerald-500 to-teal-600',
    },
    {
      title: 'Subscriptions',
      value: subCount,
      desc: 'Active user subscription plans',
      icon: Crown,
      href: '/entities/Subscription',
      color: 'from-amber-500 to-orange-600',
    },
    {
      title: 'Trial Usage',
      value: trialCount,
      desc: 'Anti-abuse free quota trackers',
      icon: Timer,
      href: '/entities/TrialUsage',
      color: 'from-purple-500 to-pink-600',
    },
  ]

  return (
    <div className="p-4 sm:p-6 lg:p-8 space-y-8 max-w-7xl mx-auto animate-fade-in">
      {/* Welcome Banner */}
      <div className="relative overflow-hidden rounded-bn bg-primary-gradient p-6 sm:p-8 text-white shadow-bn">
        <div className="relative z-10 max-w-2xl">
          <div className="inline-flex items-center space-x-2 px-3 py-1 rounded-full bg-white/20 backdrop-blur-sm text-xs font-semibold mb-3">
            <Sparkles className="w-3.5 h-3.5" />
            <span>Dashin x Atomo Framework</span>
          </div>
          <h1 className="text-2xl sm:text-3xl font-bold tracking-tight">
            PhotoEasy Cloud Admin
          </h1>
          <p className="mt-2 text-sm sm:text-base text-white/80 leading-relaxed">
            Zero-configuration, schema-driven administration for PhotoEasy GPT-image billing, credits ledger, and async generation tasks.
          </p>
        </div>
      </div>

      {/* KPI Stat Band */}
      <div>
        <h2 className="text-sm font-semibold uppercase tracking-wider text-icon-muted mb-4">
          Key System Metrics
        </h2>
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 sm:gap-6">
          {kpis.map((kpi) => {
            const Icon = kpi.icon
            return (
              <Link
                key={kpi.title}
                to={kpi.href}
                className="group bg-content-box p-5 rounded-bn border border-bn-border shadow-bn hover:shadow-lg transition-all duration-200 flex flex-col justify-between"
              >
                <div className="flex items-center justify-between mb-4">
                  <div className={`w-10 h-10 rounded-bn bg-gradient-to-tr ${kpi.color} flex items-center justify-center text-white shadow-md group-hover:scale-110 transition-transform`}>
                    <Icon className="w-5 h-5" />
                  </div>
                  <ArrowRight className="w-4 h-4 text-icon-muted group-hover:text-primary group-hover:translate-x-1 transition-all" />
                </div>
                <div>
                  <div className="text-2xl font-bold text-foreground tracking-tight">
                    {isLoading ? '…' : kpi.value}
                  </div>
                  <div className="text-xs font-semibold text-foreground mt-0.5">{kpi.title}</div>
                  <div className="text-[11px] text-icon-muted mt-1">{kpi.desc}</div>
                </div>
              </Link>
            )
          })}
        </div>
      </div>

      {/* Model Registry Quick-Launch */}
      <div className="bg-content-box rounded-bn border border-bn-border shadow-bn p-6">
        <div className="flex items-center justify-between mb-5">
          <div>
            <h2 className="text-base font-bold text-foreground">Introspected Schemas</h2>
            <p className="text-xs text-icon-muted mt-0.5">
              Live models discovered from backend <code className="font-mono text-[11px]">/meta/schema</code>
            </p>
          </div>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {models.map((modelName) => {
            const meta = schema.models[modelName]
            const count = counts?.[modelName] ?? 0
            const isReadOnly = meta?.access?.update === 'never' || meta?.access?.update === 'system'

            return (
              <Link
                key={modelName}
                to={`/entities/${modelName}`}
                className="p-4 rounded-bn border border-bn-border bg-content-bg/40 hover:bg-content-bg hover:border-primary/40 transition-all group flex items-center justify-between"
              >
                <div className="flex items-center space-x-3 truncate">
                  <div className="w-8 h-8 rounded-bn bg-content-box border border-bn-border flex items-center justify-center text-primary shadow-sm flex-shrink-0">
                    <Layers className="w-4 h-4" />
                  </div>
                  <div className="truncate">
                    <div className="font-semibold text-sm text-foreground group-hover:text-primary transition-colors truncate">
                      {getFieldLabel(modelName, meta?.tableName)}
                    </div>
                    <div className="text-[11px] text-icon-muted font-mono truncate">
                      {meta.tableName || modelName}
                    </div>
                  </div>
                </div>

                <div className="flex items-center space-x-2 flex-shrink-0">
                  {isReadOnly && (
                    <span className="px-1.5 py-0.5 text-[10px] font-medium rounded bg-amber-500/10 text-amber-600 dark:text-amber-400 border border-amber-500/20">
                      Audit
                    </span>
                  )}
                  <span className="px-2 py-0.5 text-xs font-semibold rounded-full bg-primary/10 text-primary">
                    {count}
                  </span>
                </div>
              </Link>
            )
          })}
        </div>
      </div>

      {/* System Status Footprint */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <div className="bg-content-box rounded-bn border border-bn-border shadow-bn p-5 flex items-center space-x-4">
          <div className="w-10 h-10 rounded-bn bg-success/10 text-success border border-success/20 flex items-center justify-center flex-shrink-0">
            <CheckCircle2 className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-foreground">Postgres 16 CQRS Projections</h3>
            <p className="text-xs text-icon-muted mt-0.5">
              Read-models synchronized in real-time via event-sourced stream projectors.
            </p>
          </div>
        </div>

        <div className="bg-content-box rounded-bn border border-bn-border shadow-bn p-5 flex items-center space-x-4">
          <div className="w-10 h-10 rounded-bn bg-primary/10 text-primary border border-primary/20 flex items-center justify-center flex-shrink-0">
            <Server className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-foreground">Atomo Server v0.6.4 Running</h3>
            <p className="text-xs text-icon-muted mt-0.5">
              Axum Rust engine hosting GraphQL & serving embedded Dashin Admin SPA.
            </p>
          </div>
        </div>
      </div>
    </div>
  )
}
