/**
 * CRM Component Plugin
 *
 * This plugin registers CRM-specific components and routes
 * for the Atomo Admin UI.
 */

import React from 'react'
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
      component: React.lazy(() => import('../components/views/DealsKanban').then(module => ({ default: module.DealsKanban }))),
      props: () => ({})
    },
    {
      pattern: /^\/contacts\/([^\/]+)\/timeline$/,
      component: React.lazy(() => import('../components/views/ContactTimeline').then(module => ({ default: module.ContactTimeline }))),
      props: (match: RegExpMatchArray) => ({ contactId: match[1] })
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
