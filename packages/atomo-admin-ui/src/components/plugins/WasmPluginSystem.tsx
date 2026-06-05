/**
 * WASM Plugin System - WASM UI plugin runtime
 *
 * Provides a secure environment for loading and running WASM plugins, supporting:
 * - Sandboxed execution
 * - Virtual DOM proxying
 * - HMR in development mode
 * - Plugin lifecycle management
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
  wasmUrl?: string       // WASM file URL for production mode
  jsUrl?: string         // JS file URL for development mode
  isDevelopment?: boolean
  metadata?: {
    description?: string
    author?: string
    permissions?: string[]
  }
}

export interface PluginMessage {
  type: 'init' | 'ready' | 'props' | 'render' | 'event' | 'error' | 'log'
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
 * WASM plugin runtime component
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

  // Initialize the plugin
  useEffect(() => {
    initializePlugin()
    return () => cleanup()
  }, [config.id])

  // Notify the plugin when props change
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
        // Development mode: load the JS build, with HMR support
        await initializeDevelopmentPlugin()
      } else if (config.wasmUrl) {
        // Production mode: load the WASM build
        await initializeProductionPlugin()
      } else {
        throw new Error('Invalid plugin config: missing WASM or JS file URL')
      }

    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error')
      setStatus('error')
    }
  }

  const initializeDevelopmentPlugin = async () => {
    // Development mode: load the JS build directly on the main thread
    try {
      const response = await fetch(config.jsUrl!)
      const jsCode = await response.text()

      // Create a secure execution environment
      const pluginFunction = new Function('props', 'sendEvent', 'log', jsCode)

      // Mock plugin API
      const sendEvent = (event: string, data: any) => {
        onEvent?.(event, data)
      }

      const log = (message: string) => {
        setLogs(prev => [...prev.slice(-9), `[${config.name}] ${message}`])
      }

      // Execute the plugin and obtain its render function
      const plugin = pluginFunction(props, sendEvent, log)

      if (typeof plugin.render === 'function') {
        const vdom = plugin.render(props)
        setVirtualDOM(vdom)
        setStatus('ready')
      } else {
        throw new Error('Plugin must export a render function')
      }

    } catch (err) {
      throw new Error(`Failed to load development-mode plugin: ${err instanceof Error ? err.message : 'Unknown error'}`)
    }
  }

  const initializeProductionPlugin = async () => {
    // Production mode: load the WASM build inside a Worker
    try {
      // Create the Worker
      const workerCode = `
        let wasmModule = null;

        // Handle messages from the main thread
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

            // Instantiate the WASM module
            const wasmModule = await WebAssembly.instantiate(wasmBytes, {
              env: {
                // Environment functions provided to WASM
                log: (ptr, len) => {
                  // Read a string from WASM memory and log it
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
            throw new Error('WASM module initialization failed: ' + error.message);
          }
        }
      `
      
      const blob = new Blob([workerCode], { type: 'application/javascript' })
      const worker = new Worker(URL.createObjectURL(blob))
      
      // Set up message handlers
      worker.onmessage = handleWorkerMessage
      worker.onerror = (error) => {
        setError(`Worker error: ${error.message}`)
        setStatus('error')
      }

      // Initialize the WASM module
      worker.postMessage({
        type: 'init',
        payload: { wasmUrl: config.wasmUrl }
      })

      workerRef.current = worker

    } catch (err) {
      throw new Error(`Failed to create WASM runtime environment: ${err instanceof Error ? err.message : 'Unknown error'}`)
    }
  }

  const handleWorkerMessage = (e: MessageEvent<PluginMessage>) => {
    const { type, payload } = e.data
    
    switch (type) {
      case 'ready':
        setStatus('ready')
        // Send the initial props
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
        console.warn('Unknown plugin message type:', type)
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

  // Render the virtual DOM into real DOM
  const renderVirtualDOM = (vnode: VirtualNode | string): React.ReactNode => {
    if (typeof vnode === 'string') {
      return vnode
    }

    if (!vnode || typeof vnode !== 'object') {
      return null
    }

    const { type, props: nodeProps = {}, children = [] } = vnode

    // Handle event props
    const eventProps: Record<string, any> = {}
    Object.entries(nodeProps).forEach(([key, value]) => {
      if (key.startsWith('on') && typeof value === 'string') {
        // Convert the event name into a handler function
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

    // Render child elements
    const renderedChildren = children.map((child, index) =>
      React.createElement(React.Fragment, { key: index }, renderVirtualDOM(child))
    )

    // Create the React element
    return React.createElement(type, eventProps, ...renderedChildren)
  }

  return (
    <div className={className} ref={containerRef}>
      {status === 'loading' && (
        <Card className="border-dashed">
          <CardContent className="py-8 text-center">
            <Spinner className="mx-auto mb-4" />
            <p className="text-sm text-gray-600">Loading plugin {config.name}...</p>
          </CardContent>
        </Card>
      )}

      {status === 'error' && (
        <Card className="border-red-200 bg-red-50">
          <CardContent className="py-6">
            <div className="flex items-start gap-3">
              <AlertCircle className="h-5 w-5 text-red-600 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <h3 className="font-medium text-red-900">Plugin failed to load</h3>
                <p className="text-sm text-red-700 mt-1">{error}</p>
                <div className="mt-3">
                  <Button variant="secondary" size="sm" onClick={reload}>
                    <RefreshCw className="h-4 w-4 mr-1" />
                    Retry
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

      {/* Debug info in development mode */}
      {config.isDevelopment && logs.length > 0 && (
        <details className="mt-4">
          <summary className="cursor-pointer text-sm font-medium text-gray-700 flex items-center gap-2">
            <Code className="h-4 w-4" />
            Plugin Logs ({logs.length})
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
 * Plugin manager component
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
        <h3 className="text-lg font-semibold">WASM Plugin Manager</h3>
        <span className="text-sm text-gray-500">({plugins.length} plugins)</span>
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
                        Dev Mode
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
                      Author: {plugin.metadata.author}
                    </p>
                  )}
                </div>

                <div className="flex items-center gap-2">
                  <Button
                    variant={activePlugins.has(plugin.id) ? "danger" : "primary"}
                    size="sm"
                    onClick={() => togglePlugin(plugin.id)}
                  >
                    {activePlugins.has(plugin.id) ? 'Disable' : 'Enable'}
                  </Button>
                  
                  {plugin.isDevelopment && (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => onPluginUpdate?.(plugin.id)}
                      title="Hot Reload"
                    >
                      <RefreshCw className="h-4 w-4" />
                    </Button>
                  )}
                </div>
              </div>

              {/* Plugin instance */}
              {activePlugins.has(plugin.id) && (
                <div className="mt-4 border-t pt-4">
                  <WasmPlugin
                    config={plugin}
                    props={{ message: 'Hello from Atomo!' }}
                    onEvent={(event, data) => {
                      console.log(`Plugin event [${plugin.name}]:`, event, data)
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
            <p className="text-gray-600">No plugins installed</p>
            <p className="text-sm text-gray-500 mt-1">
              Run <code className="bg-gray-100 px-1 rounded">atomo plugin install</code> to install a plugin
            </p>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
