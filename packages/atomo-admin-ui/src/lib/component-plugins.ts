/**
 * Component Plugin System for Atomo Admin UI
 *
 * This system allows business domains to register custom components
 * without modifying the core Admin UI.
 */

import React from 'react'

// Component registry type
export interface ComponentRegistry {
  [componentName: string]: React.ComponentType<any>
}

// Route handler type
export interface RouteHandler {
  pattern: RegExp
  component: React.ComponentType<any>
  props?: (match: RegExpMatchArray) => any
}

// Plugin definition
export interface ComponentPlugin {
  name: string
  components: ComponentRegistry
  routes: RouteHandler[]
  init?: () => void
}

// Global component registry
class ComponentPluginManager {
  private plugins: Map<string, ComponentPlugin> = new Map()
  private componentRegistry: ComponentRegistry = {}
  private routeHandlers: RouteHandler[] = []

  registerPlugin(plugin: ComponentPlugin) {
    this.plugins.set(plugin.name, plugin)

    // Register components
    Object.assign(this.componentRegistry, plugin.components)

    // Register routes
    this.routeHandlers.push(...plugin.routes)

    // Initialize plugin if needed
    if (plugin.init) {
      plugin.init()
    }
  }

  getComponent(name: string): React.ComponentType<any> | null {
    return this.componentRegistry[name] || null
  }

  getRouteHandler(path: string): { component: React.ComponentType<any>, props?: any } | null {
    for (const handler of this.routeHandlers) {
      const match = path.match(handler.pattern)
      if (match) {
        return {
          component: handler.component,
          props: handler.props ? handler.props(match) : undefined
        }
      }
    }
    return null
  }

  getAllPlugins(): ComponentPlugin[] {
    return Array.from(this.plugins.values())
  }
}

// Global instance
export const componentPluginManager = new ComponentPluginManager()

// Helper hook for using component plugins
export function useComponentPlugin() {
  return {
    getComponent: (name: string) => componentPluginManager.getComponent(name),
    getRouteHandler: (path: string) => componentPluginManager.getRouteHandler(path),
    registerPlugin: (plugin: ComponentPlugin) => componentPluginManager.registerPlugin(plugin)
  }
}