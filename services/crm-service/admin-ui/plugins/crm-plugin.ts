/**
 * CRM Component Plug      props: (match?: RegExpMatchArray | undefined): Record<string, any> => {
        if (!match) return {};
        return { contactId: match[1] };
      },n
 *
 * This plugin registers CRM-specific components and routes
 * for the Atomo Admin UI.
 */

import { lazy } from 'react'
import { ComponentPlugin, componentPluginManager } from './component-plugins'

// CRM Component Plugin definition
export const crmComponentPlugin: ComponentPlugin = {
  name: 'crm',
  components: {
    // Components will be loaded dynamically from CRM service
  },
  routes: [
    {
      pattern: /^\/deals\/board$/,
      component: lazy(() => import('../components/DealsKanban').then(module => ({ default: module.DealsKanban }))),
      props: () => ({})
    },
    {
      pattern: /^\/contacts\/([^\/]+)\/timeline$/,
      component: lazy(() => import('../components/ContactTimeline').then(module => ({ default: module.ContactTimeline }))),
      props: (match?: RegExpMatchArray | undefined): Record<string, any> => {
        if (!match) return {};
        return { contactId: match[1] };
      }
    }
  ],
  init: () => {
    console.log('CRM Component Plugin initialized')
  }
}

// Initialize CRM plugin
export function initCrmPlugin() {
  componentPluginManager.registerPlugin(crmComponentPlugin)
}
