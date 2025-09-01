/**
 * Dashboard View - 仪表盘视图
 * 
 * 展示系统概览和快速操作
 */

import React from 'react'
import { SchemaMetadata } from '../../lib/types'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { useNavigate } from 'react-router-dom'
import { 
  Users,
  Building2,
  DollarSign,
  Activity,
  Plus,
  TrendingUp
} from 'lucide-react'

interface DashboardProps {
  schema: SchemaMetadata
}

export function Dashboard({ schema }: DashboardProps) {
  const navigate = useNavigate()
  
  // 从 schema 中提取模型列表
  const models = Object.keys(schema.models)
  
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

  return (
    <div className="p-6 space-y-6">
      {/* 标题区域 */}
      <div>
        <h1 className="text-3xl font-bold text-gray-900">仪表盘</h1>
        <p className="text-gray-600 mt-2">欢迎使用 Atomo Admin UI</p>
      </div>

      {/* 统计卡片区域 */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        {models.map((modelName) => {
          const IconComponent = getModelIcon(modelName)
          const modelMeta = schema.models[modelName]
          
          return (
            <Card key={modelName} className="hover:shadow-md transition-shadow">
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">
                  {modelMeta.ui.displayField || modelName}
                </CardTitle>
                <IconComponent className="h-4 w-4 text-gray-600" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">--</div>
                <p className="text-xs text-gray-600">
                  总计 {modelName.toLowerCase()} 数量
                </p>
                <div className="mt-4">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => navigate(`/entities/${modelName}`)}
                    className="w-full"
                  >
                    查看全部
                  </Button>
                </div>
              </CardContent>
            </Card>
          )
        })}
      </div>

      {/* 快速操作区域 */}
      <div>
        <h2 className="text-xl font-semibold text-gray-900 mb-4">快速操作</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {models.map((modelName) => {
            const modelMeta = schema.models[modelName]
            const IconComponent = getModelIcon(modelName)
            
            return (
              <Card key={`action-${modelName}`} className="hover:shadow-md transition-shadow">
                <CardContent className="p-4">
                  <div className="flex items-center space-x-3">
                    <div className="p-2 bg-primary-100 rounded-lg">
                      <IconComponent className="h-5 w-5 text-primary-600" />
                    </div>
                    <div className="flex-1">
                      <h3 className="font-medium text-gray-900">
                        新建 {modelMeta.ui.displayField || modelName}
                      </h3>
                      <p className="text-sm text-gray-600">
                        快速创建新的 {modelName.toLowerCase()}
                      </p>
                    </div>
                    <Button
                      size="sm"
                      onClick={() => navigate(`/entities/${modelName}/new`)}
                    >
                      <Plus className="h-4 w-4" />
                    </Button>
                  </div>
                </CardContent>
              </Card>
            )
          })}
        </div>
      </div>

      {/* 系统信息 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <TrendingUp className="h-5 w-5" />
            系统信息
          </CardTitle>
          <CardDescription>
            当前连接的 Atomo 服务状态
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div>
              <div className="font-medium text-gray-900">模型数量</div>
              <div className="text-gray-600">{models.length}</div>
            </div>
            <div>
              <div className="font-medium text-gray-900">审计日志</div>
              <div className="text-gray-600">
                {schema.config.auditLog ? '已启用' : '已禁用'}
              </div>
            </div>
            <div>
              <div className="font-medium text-gray-900">软删除</div>
              <div className="text-gray-600">
                {schema.config.softDeletes ? '已启用' : '已禁用'}
              </div>
            </div>
            <div>
              <div className="font-medium text-gray-900">分页大小</div>
              <div className="text-gray-600">
                {schema.config.defaultPageSize || 20}
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
