/**
 * Settings View
 *
 * Real, read-only operational settings: the signed-in identity, the API endpoint
 * the admin is talking to, the running server's build (GET /version), and the
 * platform configuration reported by the schema — plus sign-out. This replaces the
 * old dead `/settings` link that silently fell back to the dashboard.
 */

import React from 'react'
import { useQuery } from '@tanstack/react-query'
import { SchemaMetadata } from '../../lib/types'
import { apiClient } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Server, ShieldCheck, LogOut } from 'lucide-react'

interface SettingsProps {
  schema: SchemaMetadata
}

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between py-2 border-b border-gray-100 last:border-0">
      <span className="text-sm text-gray-600">{label}</span>
      <span className="text-sm font-medium text-gray-900 text-right break-all">{value}</span>
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
      <div>
        <h1 className="text-3xl font-bold text-gray-900">Settings</h1>
        <p className="text-gray-600 mt-2">Connection, server build, and platform configuration.</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <ShieldCheck className="h-5 w-5" /> Account
          </CardTitle>
          <CardDescription>The currently signed-in user.</CardDescription>
        </CardHeader>
        <CardContent>
          <Row label="Name" value={fullName || '—'} />
          <Row label="Email" value={me?.email ?? '—'} />
          <Row label="Role" value={me?.role ?? '—'} />
          <div className="pt-4">
            <Button variant="ghost" size="sm" onClick={handleSignOut}>
              <LogOut className="h-4 w-4 mr-2" /> Sign out
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Server className="h-5 w-5" /> Server
          </CardTitle>
          <CardDescription>The Atomo server this admin is connected to.</CardDescription>
        </CardHeader>
        <CardContent>
          <Row label="API endpoint" value={apiBase} />
          <Row label="Version" value={version?.version || '—'} />
          <Row label="Commit" value={version?.commit && version.commit !== 'unknown' ? version.commit : '—'} />
          <Row label="Build time" value={version?.buildTime && version.buildTime !== 'unknown' ? version.buildTime : '—'} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Platform configuration</CardTitle>
          <CardDescription>Reported by the loaded schema (read-only).</CardDescription>
        </CardHeader>
        <CardContent>
          <Row label="Models" value={Object.keys(schema.models).length} />
          <Row label="Audit log" value={schema.config.auditLog ? 'Enabled' : 'Disabled'} />
          <Row label="Soft deletes" value={schema.config.softDeletes ? 'Enabled' : 'Disabled'} />
          <Row label="Default page size" value={schema.config.defaultPageSize || 20} />
        </CardContent>
      </Card>
    </div>
  )
}
