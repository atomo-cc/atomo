/**
 * Service Plugin Demo - 服务插件演示组件
 *
 * 展示从服务动态加载的插件组件
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
            插件信息
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="text-sm font-medium text-gray-700">服务名称</label>
              <p className="text-sm text-gray-900">{serviceName || 'CRM Service'}</p>
            </div>
            <div>
              <label className="text-sm font-medium text-gray-700">插件类型</label>
              <p className="text-sm text-gray-900">Admin UI Extension</p>
            </div>
            {contactId && (
              <div>
                <label className="text-sm font-medium text-gray-700">联系人ID</label>
                <p className="text-sm text-gray-900">{contactId}</p>
              </div>
            )}
            <div>
              <label className="text-sm font-medium text-gray-700">加载方式</label>
              <p className="text-sm text-gray-900">动态加载</p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Demo Content */}
      <Card>
        <CardHeader>
          <CardTitle>插件内容演示</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="bg-gradient-to-r from-blue-50 to-indigo-50 rounded-lg p-6">
            <div className="text-center">
              <div className="w-16 h-16 bg-blue-100 rounded-full flex items-center justify-center mx-auto mb-4">
                <ExternalLink className="w-8 h-8 text-blue-600" />
              </div>
              <h3 className="text-lg font-semibold text-gray-900 mb-2">
                服务插件已成功加载
              </h3>
              <p className="text-gray-600 mb-4">
                这个组件是从 {serviceName || 'CRM'} 服务动态加载的插件演示。
                在生产环境中，这里会显示实际的业务组件。
              </p>
              <div className="flex justify-center gap-2">
                <Badge variant="outline">React 18</Badge>
                <Badge variant="outline">TypeScript</Badge>
                <Badge variant="outline">动态加载</Badge>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Architecture Info */}
      <Card>
        <CardHeader>
          <CardTitle>架构说明</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="prose prose-sm max-w-none">
            <p>
              这个演示展示了Atomo平台的纯架构设计：
            </p>
            <ul className="list-disc list-inside space-y-1 mt-2">
              <li><strong>平台Admin UI</strong>：保持业务无关，只提供通用基础设施</li>
              <li><strong>服务级插件</strong>：每个业务服务可以注册自己的Admin UI组件</li>
              <li><strong>动态加载</strong>：插件在运行时动态加载，无需重新构建平台</li>
              <li><strong>类型安全</strong>：完整的TypeScript支持，确保编译时检查</li>
            </ul>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
