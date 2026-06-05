/**
 * Service Plugin Loader
 *
 * The admin core is service-agnostic — it has no build-time dependency on any
 * service's source. Service-specific Admin UI (e.g. a CRM Deal Kanban) ships as a
 * RUNTIME component plugin served by the service and registered here via
 * `componentPluginManager`. Until a real plugin is served, nothing is registered
 * and the corresponding routes return a 404 — there are no in-package demo stand-ins.
 */

import { componentPluginManager, ComponentPlugin } from './component-plugins'

export interface ServicePluginConfig {
  serviceName: string
  /** URL the service serves its Admin UI plugin module from. */
  pluginUrl: string
  enabled: boolean
}

// Services that may serve an Admin UI plugin. Empty by default; populate this (or
// fetch it from the server) once a service actually ships a plugin.
const SERVICE_PLUGINS: ServicePluginConfig[] = []

/**
 * Load a service's runtime Admin UI plugin from its `pluginUrl`. Returns null when
 * the service serves no plugin. (Remote-module loading is intentionally a no-op
 * stub for now — extend this to import the served module and return its
 * `ComponentPlugin`.)
 */
async function loadServicePlugin(_config: ServicePluginConfig): Promise<ComponentPlugin | null> {
  return null
}

/** Load each enabled service's plugin and register it with the component-plugin manager. */
export async function initializeServicePlugins(): Promise<void> {
  const loaded = await Promise.all(
    SERVICE_PLUGINS.filter((c) => c.enabled).map((c) => loadServicePlugin(c)),
  )
  loaded
    .filter((p): p is ComponentPlugin => p !== null)
    .forEach((p) => componentPluginManager.registerPlugin(p))
}
