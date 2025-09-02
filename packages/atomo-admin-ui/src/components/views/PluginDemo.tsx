/**
 * Plugin Demo - WASM插件系统演示页面
 * 
 * 展示WASM插件系统的功能和用法
 */

import React, { useState } from 'react'
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { PluginManager, WasmPluginConfig } from '../plugins/WasmPluginSystem'
import { Code, Zap, Book, Download } from 'lucide-react'

export function PluginDemo() {
  // 模拟插件配置
  const [demoPlugins] = useState<WasmPluginConfig[]>([
    {
      id: 'text-counter',
      name: '文本计数器',
      version: '1.0.0',
      isDevelopment: true,
      jsUrl: '/demo-plugins/text-counter.js',
      metadata: {
        description: '实时统计文本字符数、词数的插件组件',
        author: 'Atomo Team',
        permissions: ['read-input']
      }
    },
    {
      id: 'color-picker',
      name: '颜色选择器',
      version: '1.2.0',
      isDevelopment: true,
      jsUrl: '/demo-plugins/color-picker.js',
      metadata: {
        description: '高级颜色选择器，支持RGB、HSL、HEX格式',
        author: 'Community',
        permissions: ['read-input', 'write-output']
      }
    },
    {
      id: 'chart-widget',
      name: '图表组件',
      version: '2.0.0',
      isDevelopment: false,
      wasmUrl: '/demo-plugins/chart-widget.wasm',
      metadata: {
        description: '高性能图表渲染组件，基于WASM实现',
        author: 'Atomo Labs',
        permissions: ['read-data', 'render-canvas']
      }
    }
  ])

  const handlePluginUpdate = (pluginId: string) => {
    console.log('热重载插件:', pluginId)
    // 在实际实现中，这里会触发插件的重新加载
  }

  return (
    <div className="p-6 space-y-6">
      {/* 页面标题 */}
      <div>
        <h1 className="text-3xl font-bold text-gray-900 flex items-center gap-3">
          <Zap className="h-8 w-8 text-primary-600" />
          WASM 插件系统
        </h1>
        <p className="text-gray-600 mt-2">
          下一代可扩展UI组件系统，支持安全的WASM插件和开发模式HMR
        </p>
      </div>

      {/* 功能特性 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Book className="h-5 w-5" />
            系统特性
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid md:grid-cols-2 gap-4">
            <div className="space-y-3">
              <h3 className="font-medium text-gray-900">🔒 安全沙箱</h3>
              <ul className="text-sm text-gray-600 space-y-1">
                <li>• Web Worker 隔离执行</li>
                <li>• 受限的API访问权限</li>
                <li>• 内存安全保证</li>
                <li>• 跨站脚本防护</li>
              </ul>
            </div>
            
            <div className="space-y-3">
              <h3 className="font-medium text-gray-900">⚡ 高性能</h3>
              <ul className="text-sm text-gray-600 space-y-1">
                <li>• WASM 近原生性能</li>
                <li>• 虚拟DOM优化渲染</li>
                <li>• 异步非阻塞加载</li>
                <li>• 内存使用优化</li>
              </ul>
            </div>
            
            <div className="space-y-3">
              <h3 className="font-medium text-gray-900">🛠️ 开发友好</h3>
              <ul className="text-sm text-gray-600 space-y-1">
                <li>• 开发模式HMR支持</li>
                <li>• 实时日志和调试</li>
                <li>• TypeScript类型安全</li>
                <li>• 插件生命周期管理</li>
              </ul>
            </div>
            
            <div className="space-y-3">
              <h3 className="font-medium text-gray-900">🔌 可扩展</h3>
              <ul className="text-sm text-gray-600 space-y-1">
                <li>• 多语言支持（Rust, Go, C++）</li>
                <li>• 标准化插件接口</li>
                <li>• 版本管理和更新</li>
                <li>• 插件市场生态</li>
              </ul>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* 插件管理器 */}
      <PluginManager 
        plugins={demoPlugins} 
        onPluginUpdate={handlePluginUpdate}
      />

      {/* 开发指南 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Code className="h-5 w-5" />
            开发指南
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="bg-gray-50 p-4 rounded-md">
            <h4 className="font-medium mb-2">1. 创建插件项目</h4>
            <pre className="text-sm bg-gray-900 text-green-400 p-3 rounded overflow-x-auto">
              <code>{`# 使用 Atomo CLI 创建插件
atomo plugin create my-field-widget

# 进入插件目录
cd my-field-widget

# 开发模式运行
atomo plugin dev`}</code>
            </pre>
          </div>

          <div className="bg-gray-50 p-4 rounded-md">
            <h4 className="font-medium mb-2">2. 插件代码示例 (TypeScript/AssemblyScript)</h4>
            <pre className="text-sm bg-gray-900 text-blue-400 p-3 rounded overflow-x-auto">
              <code>{`export function render(props: any) {
  return {
    type: 'div',
    props: { className: 'custom-widget' },
    children: [
      {
        type: 'input',
        props: {
          value: props.value,
          onChange: 'handleChange',
          placeholder: props.placeholder
        }
      }
    ]
  }
}

export function handleChange(event: any) {
  // 触发值变化事件
  sendEvent('change', { value: event.target.value })
}`}</code>
            </pre>
          </div>

          <div className="bg-gray-50 p-4 rounded-md">
            <h4 className="font-medium mb-2">3. 在表单中使用</h4>
            <pre className="text-sm bg-gray-900 text-yellow-400 p-3 rounded overflow-x-auto">
              <code>{`// schema.ts
export interface Contact {
  customField: {
    type: 'custom',
    component: 'wasm:my-field-widget',
    ui: {
      label: '自定义字段',
      placeholder: '请输入...'
    }
  }
}`}</code>
            </pre>
          </div>

          <div className="flex gap-3">
            <Button variant="secondary">
              <Download className="h-4 w-4 mr-2" />
              下载SDK
            </Button>
            <Button variant="secondary">
              <Book className="h-4 w-4 mr-2" />
              查看文档
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* 状态统计 */}
      <div className="grid md:grid-cols-4 gap-4">
        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-primary-600">
              {demoPlugins.length}
            </div>
            <div className="text-sm text-gray-600">已安装插件</div>
          </CardContent>
        </Card>
        
        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-green-600">
              {demoPlugins.filter(p => p.isDevelopment).length}
            </div>
            <div className="text-sm text-gray-600">开发模式</div>
          </CardContent>
        </Card>
        
        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-blue-600">
              {demoPlugins.filter(p => p.wasmUrl).length}
            </div>
            <div className="text-sm text-gray-600">WASM 插件</div>
          </CardContent>
        </Card>
        
        <Card>
          <CardContent className="p-4 text-center">
            <div className="text-2xl font-bold text-purple-600">
              100%
            </div>
            <div className="text-sm text-gray-600">沙箱安全</div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
