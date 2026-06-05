/**
 * Service Plugin Demo
 *
 * Showcases a plugin component dynamically loaded from a service.
 */

import React from 'react'
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Zap, ExternalLink, Info } from 'lucide-react'

interface ServicePluginDemoProps {
  title: string
  description: string
  serviceName?: string
  contactId?: string
}

export function ServicePluginDemo({ title, description, serviceName, contactId }: ServicePluginDemoProps) {
  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900">{title}</h1>
          <p className="text-gray-600 mt-1">{description}</p>
        </div>
        <Badge variant="secondary" className="flex items-center gap-1">
          <Zap className="w-3 h-3" />
          Service Plugin
        </Badge>
      </div>

      {/* Plugin Info Card */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Info className="w-5 h-5" />
            Plugin Information
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="text-sm font-medium text-gray-700">Service Name</label>
              <p className="text-sm text-gray-900">{serviceName || 'CRM Service'}</p>
            </div>
            <div>
              <label className="text-sm font-medium text-gray-700">Plugin Type</label>
              <p className="text-sm text-gray-900">Admin UI Extension</p>
            </div>
            {contactId && (
              <div>
                <label className="text-sm font-medium text-gray-700">Contact ID</label>
                <p className="text-sm text-gray-900">{contactId}</p>
              </div>
            )}
            <div>
              <label className="text-sm font-medium text-gray-700">Loading Method</label>
              <p className="text-sm text-gray-900">Dynamic Loading</p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Demo Content */}
      <Card>
        <CardHeader>
          <CardTitle>Plugin Content Demo</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="bg-gradient-to-r from-blue-50 to-indigo-50 rounded-lg p-6">
            <div className="text-center">
              <div className="w-16 h-16 bg-blue-100 rounded-full flex items-center justify-center mx-auto mb-4">
                <ExternalLink className="w-8 h-8 text-blue-600" />
              </div>
              <h3 className="text-lg font-semibold text-gray-900 mb-2">
                Service plugin loaded successfully
              </h3>
              <p className="text-gray-600 mb-4">
                This component is a plugin demo dynamically loaded from the {serviceName || 'CRM'} service.
                In production, the actual business component would appear here.
              </p>
              <div className="flex justify-center gap-2">
                <Badge variant="outline">React 18</Badge>
                <Badge variant="outline">TypeScript</Badge>
                <Badge variant="outline">Dynamic Loading</Badge>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Architecture Info */}
      <Card>
        <CardHeader>
          <CardTitle>Architecture Overview</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="prose prose-sm max-w-none">
            <p>
              This demo illustrates the clean architecture of the Atomo platform:
            </p>
            <ul className="list-disc list-inside space-y-1 mt-2">
              <li><strong>Platform Admin UI</strong>: stays business-agnostic and provides only generic infrastructure</li>
              <li><strong>Service-level plugins</strong>: each business service can register its own Admin UI components</li>
              <li><strong>Dynamic loading</strong>: plugins load at runtime with no need to rebuild the platform</li>
              <li><strong>Type safety</strong>: full TypeScript support ensures compile-time checks</li>
            </ul>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
