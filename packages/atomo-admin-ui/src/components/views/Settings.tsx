/**
 * Settings View — Dashin Settings Management
 */

import React from 'react'
import { useQuery } from '@tanstack/react-query'
import { SchemaMetadata } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Server, ShieldCheck, LogOut, Sliders } from 'lucide-react'

interface SettingsProps {
  schema: SchemaMetadata
}

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between py-2.5 border-b border-bn-border/60 last:border-0">
      <span className="text-xs text-icon-muted font-medium">{label}</span>
      <span className="text-xs font-semibold text-foreground text-right break-all">{value}</span>
    </div>
  )
}

export function Settings({ schema }: SettingsProps) {
  const { data: me } = useQuery({
    queryKey: ['auth-me'],
    queryFn: () => apiClient.getCurrentUser(),
    staleTime: 60_000,
  })
  const { data: version } = useQuery({
    queryKey: ['server-version'],
    queryFn: () => apiClient.getVersion(),
    staleTime: 5 * 60_000,
    retry: false,
  })

  const apiBase = apiClient.getBaseUrl() || `${window.location.origin} (same-origin)`
  const fullName = me ? [me.first_name, me.last_name].filter(Boolean).join(' ').trim() : ''

  const handleSignOut = () => {
    apiClient.logout()
    window.location.href = `${(import.meta as any).env.BASE_URL}login`
  }

  return (
    <div className="p-6 space-y-6 max-w-3xl">
      {/* Header */}
      <div className="flex items-center gap-3">
        <div className="w-9 h-9 rounded-bn bg-primary/10 flex items-center justify-center text-primary">
          <Sliders className="h-5 w-5" />
        </div>
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Settings</h1>
          <p className="text-xs text-icon-muted mt-0.5">Connection runtime, server build metadata, and platform parameters</p>
        </div>
      </div>

      {/* Account card */}
      <Card>
        <CardHeader className="py-4 border-b border-bn-border/60">
          <CardTitle className="flex items-center gap-2">
            <ShieldCheck className="h-4 w-4 text-primary" /> Current Session
          </CardTitle>
          <CardDescription>Authentication identity and authorization role</CardDescription>
        </CardHeader>
        <CardContent className="p-5">
          <Row label="Full Name" value={fullName || '—'} />
          <Row label="Email Address" value={me?.email ?? '—'} />
          <Row label="Assigned Role" value={<Badge variant="secondary">{me?.role ?? '—'}</Badge>} />
          <div className="pt-4 flex justify-end">
            <Button variant="danger" size="sm" onClick={handleSignOut}>
              <LogOut className="h-3.5 w-3.5 mr-1.5" /> Sign out
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Server metadata card */}
      <Card>
        <CardHeader className="py-4 border-b border-bn-border/60">
          <CardTitle className="flex items-center gap-2">
            <Server className="h-4 w-4 text-primary" /> Backend Server
          </CardTitle>
          <CardDescription>The active Atomo server endpoint this admin console is connected to</CardDescription>
        </CardHeader>
        <CardContent className="p-5">
          <Row label="API Gateway URL" value={<span className="font-mono text-xs text-primary">{apiBase}</span>} />
          <Row label="Engine Version" value={version?.version || '—'} />
          <Row label="Git Commit" value={version?.commit && version.commit !== 'unknown' ? <span className="font-mono">{version.commit.slice(0, 12)}</span> : '—'} />
          <Row label="Build Timestamp" value={version?.buildTime && version.buildTime !== 'unknown' ? version.buildTime : '—'} />
        </CardContent>
      </Card>

      {/* Platform configuration card */}
      <Card>
        <CardHeader className="py-4 border-b border-bn-border/60">
          <CardTitle>Platform Configuration</CardTitle>
          <CardDescription>Introspected runtime capabilities reported by the loaded schema</CardDescription>
        </CardHeader>
        <CardContent className="p-5">
          <Row label="Active Schema Models" value={Object.keys(schema.models).length} />
          <Row label="Audit Trail" value={<Badge variant={schema.config.auditLog ? 'success' : 'secondary'}>{schema.config.auditLog ? 'Enabled' : 'Disabled'}</Badge>} />
          <Row label="Soft Deletes" value={<Badge variant={schema.config.softDeletes ? 'success' : 'secondary'}>{schema.config.softDeletes ? 'Enabled' : 'Disabled'}</Badge>} />
          <Row label="Default Query Page Size" value={schema.config.defaultPageSize || 20} />
        </CardContent>
      </Card>
    </div>
  )
}
