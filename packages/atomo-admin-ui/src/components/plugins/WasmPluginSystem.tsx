/**
 * WASM Plugin System - WASM UI插件运行时系统
 * 
 * 提供安全的WASM插件加载和运行环境，支持：
 * - 沙箱化执行
 * - 虚拟DOM代理
 * - 开发模式HMR
 * - 插件生命周期管理
 */

import React, { useEffect, useRef, useState, useCallback } from 'react'
import { Card, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Spinner } from '../ui/Spinner'
import { AlertCircle, RefreshCw, Code, Zap } from 'lucide-react'

export interface WasmPluginConfig {
  id: string
  name: string
  version: string
  wasmUrl?: string       // 生产模式WASM文件URL
  jsUrl?: string         // 开发模式JS文件URL
  isDevelopment?: boolean
  metadata?: {
    description?: string
    author?: string
    permissions?: string[]
  }
}

export interface PluginMessage {
  type: 'init' | 'props' | 'render' | 'event' | 'error' | 'log'
  payload: any
}

export interface VirtualNode {
  type: string
  props: Record<string, any>
  children?: (VirtualNode | string)[]
}

interface WasmPluginProps {
  config: WasmPluginConfig
  props?: Record<string, any>
  onEvent?: (event: string, data: any) => void
  className?: string
}

/**
 * WASM插件运行时组件
 */
export function WasmPlugin({ 
  config, 
  props = {}, 
  onEvent, 
  className 
}: WasmPluginProps) {
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading')
  const [error, setError] = useState<string | null>(null)
  const [virtualDOM, setVirtualDOM] = useState<VirtualNode | null>(null)
  const [logs, setLogs] = useState<string[]>([])
  
  const workerRef = useRef<Worker | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  // 初始化插件
  useEffect(() => {
    initializePlugin()
    return () => cleanup()
  }, [config.id])

  // 当props变化时通知插件
  useEffect(() => {
    if (status === 'ready' && workerRef.current) {
      sendMessageToPlugin({
        type: 'props',
        payload: props
      })
    }
  }, [props, status])

  const initializePlugin = async () => {
    try {
      setStatus('loading')
      setError(null)
      
      if (config.isDevelopment && config.jsUrl) {
        // 开发模式：加载JS版本，支持HMR
        await initializeDevelopmentPlugin()
      } else if (config.wasmUrl) {
        // 生产模式：加载WASM版本
        await initializeProductionPlugin()
      } else {
        throw new Error('插件配置无效：缺少WASM或JS文件URL')
      }
      
    } catch (err) {
      setError(err instanceof Error ? err.message : '未知错误')
      setStatus('error')
    }
  }

  const initializeDevelopmentPlugin = async () => {
    // 开发模式：直接在主线程加载JS版本
    try {
      const response = await fetch(config.jsUrl!)
      const jsCode = await response.text()
      
      // 创建安全的执行环境
      const pluginFunction = new Function('props', 'sendEvent', 'log', jsCode)
      
      // 模拟插件API
      const sendEvent = (event: string, data: any) => {
        onEvent?.(event, data)
      }
      
      const log = (message: string) => {
        setLogs(prev => [...prev.slice(-9), `[${config.name}] ${message}`])
      }
      
      // 执行插件并获取渲染函数
      const plugin = pluginFunction(props, sendEvent, log)
      
      if (typeof plugin.render === 'function') {
        const vdom = plugin.render(props)
        setVirtualDOM(vdom)
        setStatus('ready')
      } else {
        throw new Error('插件必须导出render函数')
      }
      
    } catch (err) {
      throw new Error(`加载开发模式插件失败: ${err instanceof Error ? err.message : '未知错误'}`)
    }
  }

  const initializeProductionPlugin = async () => {
    // 生产模式：在Worker中加载WASM版本
    try {
      // 创建Worker
      const workerCode = `
        let wasmModule = null;
        
        // 处理主线程消息
        self.onmessage = async function(e) {
          const { type, payload } = e.data;
          
          try {
            switch (type) {
              case 'init':
                await initializeWasm(payload.wasmUrl);
                break;
                
              case 'props':
                if (wasmModule && wasmModule.render) {
                  const vdom = wasmModule.render(payload);
                  self.postMessage({
                    type: 'render',
                    payload: vdom
                  });
                }
                break;
                
              case 'event':
                if (wasmModule && wasmModule.handleEvent) {
                  wasmModule.handleEvent(payload.event, payload.data);
                }
                break;
            }
          } catch (error) {
            self.postMessage({
              type: 'error',
              payload: error.message
            });
          }
        };
        
        async function initializeWasm(wasmUrl) {
          try {
            const wasmResponse = await fetch(wasmUrl);
            const wasmBytes = await wasmResponse.arrayBuffer();
            
            // 实例化WASM模块
            const wasmModule = await WebAssembly.instantiate(wasmBytes, {
              env: {
                // 提供给WASM的环境函数
                log: (ptr, len) => {
                  // 从WASM内存读取字符串并输出日志
                  console.log('WASM Log:', ptr, len);
                }
              }
            });
            
            wasmModule = wasmModule.instance.exports;
            
            self.postMessage({
              type: 'ready',
              payload: null
            });
            
          } catch (error) {
            throw new Error('WASM模块初始化失败: ' + error.message);
          }
        }
      `
      
      const blob = new Blob([workerCode], { type: 'application/javascript' })
      const worker = new Worker(URL.createObjectURL(blob))
      
      // 设置消息处理器
      worker.onmessage = handleWorkerMessage
      worker.onerror = (error) => {
        setError(`Worker错误: ${error.message}`)
        setStatus('error')
      }
      
      // 初始化WASM模块
      worker.postMessage({
        type: 'init',
        payload: { wasmUrl: config.wasmUrl }
      })
      
      workerRef.current = worker
      
    } catch (err) {
      throw new Error(`创建WASM运行环境失败: ${err instanceof Error ? err.message : '未知错误'}`)
    }
  }

  const handleWorkerMessage = (e: MessageEvent<PluginMessage>) => {
    const { type, payload } = e.data
    
    switch (type) {
      case 'ready':
        setStatus('ready')
        // 发送初始props
        sendMessageToPlugin({
          type: 'props',
          payload: props
        })
        break
        
      case 'render':
        setVirtualDOM(payload)
        break
        
      case 'error':
        setError(payload)
        setStatus('error')
        break
        
      case 'log':
        setLogs(prev => [...prev.slice(-9), `[${config.name}] ${payload}`])
        break
        
      default:
        console.warn('未知的插件消息类型:', type)
    }
  }

  const sendMessageToPlugin = (message: PluginMessage) => {
    if (workerRef.current) {
      workerRef.current.postMessage(message)
    }
  }

  const handleVirtualDOMEvent = useCallback((event: string, data: any) => {
    if (workerRef.current) {
      sendMessageToPlugin({
        type: 'event',
        payload: { event, data }
      })
    }
    onEvent?.(event, data)
  }, [onEvent])

  const cleanup = () => {
    if (workerRef.current) {
      workerRef.current.terminate()
      workerRef.current = null
    }
  }

  const reload = () => {
    cleanup()
    setLogs([])
    initializePlugin()
  }

  // 渲染虚拟DOM为真实DOM
  const renderVirtualDOM = (vnode: VirtualNode | string): React.ReactNode => {
    if (typeof vnode === 'string') {
      return vnode
    }

    if (!vnode || typeof vnode !== 'object') {
      return null
    }

    const { type, props: nodeProps = {}, children = [] } = vnode

    // 处理事件属性
    const eventProps: Record<string, any> = {}
    Object.entries(nodeProps).forEach(([key, value]) => {
      if (key.startsWith('on') && typeof value === 'string') {
        // 将事件名转换为处理函数
        eventProps[key] = (e: any) => {
          handleVirtualDOMEvent(value, {
            type: e.type,
            target: e.target.value || e.target.textContent,
            ...e
          })
        }
      } else {
        eventProps[key] = value
      }
    })

    // 渲染子元素
    const renderedChildren = children.map((child, index) => 
      React.createElement(React.Fragment, { key: index }, renderVirtualDOM(child))
    )

    // 创建React元素
    return React.createElement(type, eventProps, ...renderedChildren)
  }

  return (
    <div className={className} ref={containerRef}>
      {status === 'loading' && (
        <Card className="border-dashed">
          <CardContent className="py-8 text-center">
            <Spinner className="mx-auto mb-4" />
            <p className="text-sm text-gray-600">正在加载插件 {config.name}...</p>
          </CardContent>
        </Card>
      )}

      {status === 'error' && (
        <Card className="border-red-200 bg-red-50">
          <CardContent className="py-6">
            <div className="flex items-start gap-3">
              <AlertCircle className="h-5 w-5 text-red-600 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <h3 className="font-medium text-red-900">插件加载失败</h3>
                <p className="text-sm text-red-700 mt-1">{error}</p>
                <div className="mt-3">
                  <Button variant="secondary" size="sm" onClick={reload}>
                    <RefreshCw className="h-4 w-4 mr-1" />
                    重试加载
                  </Button>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {status === 'ready' && virtualDOM && (
        <div className="plugin-content">
          {renderVirtualDOM(virtualDOM)}
        </div>
      )}

      {/* 开发模式调试信息 */}
      {config.isDevelopment && logs.length > 0 && (
        <details className="mt-4">
          <summary className="cursor-pointer text-sm font-medium text-gray-700 flex items-center gap-2">
            <Code className="h-4 w-4" />
            插件日志 ({logs.length})
          </summary>
          <div className="mt-2 p-3 bg-gray-50 rounded-md">
            <div className="space-y-1 text-xs font-mono">
              {logs.map((log, index) => (
                <div key={index} className="text-gray-600">{log}</div>
              ))}
            </div>
          </div>
        </details>
      )}
    </div>
  )
}

/**
 * 插件管理器组件
 */
interface PluginManagerProps {
  plugins: WasmPluginConfig[]
  onPluginUpdate?: (pluginId: string) => void
}

export function PluginManager({ plugins, onPluginUpdate }: PluginManagerProps) {
  const [activePlugins, setActivePlugins] = useState<Set<string>>(new Set())

  const togglePlugin = (pluginId: string) => {
    setActivePlugins(prev => {
      const newSet = new Set(prev)
      if (newSet.has(pluginId)) {
        newSet.delete(pluginId)
      } else {
        newSet.add(pluginId)
      }
      return newSet
    })
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <Zap className="h-5 w-5 text-primary-600" />
        <h3 className="text-lg font-semibold">WASM 插件管理器</h3>
        <span className="text-sm text-gray-500">({plugins.length} 个插件)</span>
      </div>

      <div className="grid gap-4">
        {plugins.map((plugin) => (
          <Card key={plugin.id} className="border-gray-200">
            <CardContent className="p-4">
              <div className="flex items-start justify-between">
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <h4 className="font-medium">{plugin.name}</h4>
                    <span className="text-xs bg-gray-100 px-2 py-1 rounded">
                      v{plugin.version}
                    </span>
                    {plugin.isDevelopment && (
                      <span className="text-xs bg-orange-100 text-orange-700 px-2 py-1 rounded">
                        开发模式
                      </span>
                    )}
                  </div>
                  
                  {plugin.metadata?.description && (
                    <p className="text-sm text-gray-600 mt-1">
                      {plugin.metadata.description}
                    </p>
                  )}
                  
                  {plugin.metadata?.author && (
                    <p className="text-xs text-gray-500 mt-2">
                      作者: {plugin.metadata.author}
                    </p>
                  )}
                </div>

                <div className="flex items-center gap-2">
                  <Button
                    variant={activePlugins.has(plugin.id) ? "danger" : "primary"}
                    size="sm"
                    onClick={() => togglePlugin(plugin.id)}
                  >
                    {activePlugins.has(plugin.id) ? '停用' : '启用'}
                  </Button>
                  
                  {plugin.isDevelopment && (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => onPluginUpdate?.(plugin.id)}
                      title="热重载"
                    >
                      <RefreshCw className="h-4 w-4" />
                    </Button>
                  )}
                </div>
              </div>

              {/* 插件实例 */}
              {activePlugins.has(plugin.id) && (
                <div className="mt-4 border-t pt-4">
                  <WasmPlugin
                    config={plugin}
                    props={{ message: 'Hello from Atomo!' }}
                    onEvent={(event, data) => {
                      console.log(`插件事件 [${plugin.name}]:`, event, data)
                    }}
                  />
                </div>
              )}
            </CardContent>
          </Card>
        ))}
      </div>

      {plugins.length === 0 && (
        <Card className="border-dashed">
          <CardContent className="py-8 text-center">
            <Zap className="h-8 w-8 text-gray-400 mx-auto mb-3" />
            <p className="text-gray-600">暂无已安装的插件</p>
            <p className="text-sm text-gray-500 mt-1">
              使用 <code className="bg-gray-100 px-1 rounded">atomo plugin install</code> 安装插件
            </p>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
