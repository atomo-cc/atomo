/**
 * Service Plugin Loader
 *
 * This module handles loading plugins from different Atomo services
 * dynamically at runtime, enabling service-level Admin UI extensions.
 */

import React from 'react'
import { ComponentPlugin, componentPluginManager } from './component-plugins'

// Service plugin configuration
export interface ServicePluginConfig {
  serviceName: string
  pluginUrl: string
  enabled: boolean
}

// Default service plugin configurations
const DEFAULT_SERVICE_PLUGINS: ServicePluginConfig[] = [
  {
    serviceName: 'crm',
    pluginUrl: '/api/services/crm/admin/plugin',
    enabled: true
  }
  // Add more services here as they implement Admin UI plugins
]

/**
 * Load a plugin for a service.
 *
 * The core admin is **service-agnostic**: it has no build-time import of any
 * service's source, so it builds and bundles on its own. Service-specific Admin
 * UI (e.g. a Deal Kanban) ships as a **runtime plugin** served at `pluginUrl`;
 * the in-package demo components stand in until a real plugin is registered.
 */
async function loadServicePlugin(config: ServicePluginConfig): Promise<ComponentPlugin | null> {
  return loadServicePluginDemo(config)
}

/**
 * Load a plugin with demo components (fallback)
 */
async function loadServicePluginDemo(config: ServicePluginConfig): Promise<ComponentPlugin | null> {
  try {
    console.log(`Loading demo plugin for service: ${config.serviceName}`)

    if (config.serviceName === 'crm') {
      return {
        name: 'crm-demo',
        components: {},
        routes: [
          {
            pattern: /^\/deals\/board$/,
            component: React.lazy(() => import('../components/views/ServicePluginDemo').then(module => ({ default: module.ServicePluginDemo }))),
            props: () => ({ title: 'CRM Deals Kanban', description: 'Service-level CRM plugin loaded' })
          },
          {
            pattern: /^\/contacts\/([^\/]+)\/timeline$/,
            component: React.lazy(() => import('../components/views/ServicePluginDemo').then(module => ({ default: module.ServicePluginDemo }))),
            props: (match: RegExpMatchArray) => ({
              title: 'Contact Timeline',
              description: `Timeline for contact ${match[1]} (loaded from CRM service)`,
              contactId: match[1]
            })
          }
        ],
        init: () => {
          console.log('CRM demo plugin initialized')
        }
      }
    }

    return null
  } catch (error) {
    console.warn(`Failed to load demo plugin for service ${config.serviceName}:`, error)
    return null
  }
}

/**
 * Initialize service plugins
 */
export async function initializeServicePlugins(): Promise<void> {
  console.log('Initializing service plugins...')

  const loadPromises = DEFAULT_SERVICE_PLUGINS
    .filter(config => config.enabled)
    .map(config => loadServicePlugin(config))

  try {
    const plugins = await Promise.all(loadPromises)
    const validPlugins = plugins.filter((plugin): plugin is ComponentPlugin => plugin !== null)

    // Register all loaded plugins
    validPlugins.forEach(plugin => {
      console.log(`Registering plugin: ${plugin.name}`)
      componentPluginManager.registerPlugin(plugin)
    })

    console.log(`Successfully loaded ${validPlugins.length} service plugins`)
  } catch (error) {
    console.error('Error loading service plugins:', error)
  }
}

/**
 * Get available service plugins
 */
export function getAvailableServicePlugins(): ServicePluginConfig[] {
  return DEFAULT_SERVICE_PLUGINS.filter(config => config.enabled)
}

/**
 * Enable or disable a service plugin
 */
export function setServicePluginEnabled(serviceName: string, enabled: boolean): void {
  const config = DEFAULT_SERVICE_PLUGINS.find(p => p.serviceName === serviceName)
  if (config) {
    config.enabled = enabled
    console.log(`${enabled ? 'Enabled' : 'Disabled'} plugin for service: ${serviceName}`)
  }
}
