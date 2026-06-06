import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Atomo',
  // Project Pages are served under /atomo/, so assets must resolve against this base.
  base: '/atomo/',
  // Localhost URLs are legitimate examples in the docs (dev server, /admin) — don't
  // fail the build treating them as dead links.
  ignoreDeadLinks: [/^https?:\/\/localhost/, /^https?:\/\/127\.0\.0\.1/],
  description: 'Atomo Content Core — open-source, self-hostable, event-sourced backend for content-driven apps (GraphQL API, auth, realtime, admin UI). Documentation.',

  head: [
    ['link', { rel: 'icon', href: '/favicon.ico' }],
    ['meta', { name: 'theme-color', content: '#3c82f6' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:locale', content: 'en' }],
    ['meta', { property: 'og:title', content: 'Atomo Content Core — self-hostable event-sourced backend' }],
    ['meta', { property: 'og:site_name', content: 'Atomo Docs' }],
    ['meta', { property: 'og:url', content: 'https://docs.atomo.cc/' }],
  ],

  themeConfig: {
    logo: '/logo.svg',
    outline: 'deep',

    nav: [
      { text: 'Vision', link: '/vision' },
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'API', link: '/api/' },
      { text: 'Examples', link: '/examples/' },
      { text: 'Services', link: '/services/' },
      { text: '中文', items: [
        { text: '愿景', link: '/zh/vision' },
        { text: '路线图', link: '/zh/roadmap' }
      ]},
      {
        text: 'v0.1.0',
        items: [
          { text: 'Changelog', link: '/changelog' },
          { text: 'Roadmap', link: '/roadmap' }
        ]
      }
    ],

    sidebar: {
      '/guide/': [
        {
          text: '🚀 Getting Started',
          items: [
            { text: 'Introduction', link: '/guide/introduction' },
            { text: 'Quick Start', link: '/guide/getting-started' },
            { text: 'Installation', link: '/guide/installation' },
            { text: 'Your First Project', link: '/guide/first-project' },
            { text: 'Architecture Overview', link: '/guide/architecture' }
          ]
        },
        {
          text: '🏗️ Core Concepts',
          items: [
            { text: 'Event Sourcing', link: '/guide/event-sourcing' },
            { text: 'Schema-Driven Development', link: '/guide/schema-driven' },
            { text: 'Modeling & Access', link: '/guide/modeling' },
            { text: 'Subscriptions', link: '/guide/subscriptions' },
            { text: 'Soft Delete & Trash', link: '/guide/soft-delete' },
            { text: 'Caching', link: '/guide/caching' },
            { text: 'Dev Runtime & Workspace', link: '/guide/dev-runtime' },
            { text: 'Server Routes in Dev', link: '/guide/server-routes-dev' },
            { text: 'Configuration', link: '/guide/configuration' },
            { text: 'WASM & JS Plugins', link: '/guide/plugins' },
            { text: 'Testing', link: '/guide/testing' },
            { text: 'Real-time Collaboration (planned)', link: '/guide/collaboration' },
            { text: 'Local-First (planned)', link: '/guide/local-first' }
          ]
        },
        {
          text: '📘 Tutorials',
          items: [
            { text: 'Building a CRM', link: '/guide/tutorials/crm' },
            { text: 'Custom Content Types', link: '/guide/tutorials/content-types' },
            { text: 'Plugin Development', link: '/guide/tutorials/plugin-dev' },
            { text: 'Deployment Guide', link: '/guide/tutorials/deployment' }
          ]
        },
        {
          text: '🔧 Advanced',
          items: [
            { text: 'Custom Event Stores', link: '/guide/advanced/event-stores' },
            { text: 'Access Control (RBAC)', link: '/guide/advanced/access-hooks' },
            { text: 'Validation Rules', link: '/guide/advanced/validation' },
            { text: 'Multi-tenant', link: '/guide/advanced/multi-tenant' },
            { text: 'AI & Vector Search', link: '/guide/advanced/ai-vector' },
            { text: 'Security & Auth', link: '/guide/advanced/security' },
            { text: 'Production Readiness', link: '/guide/advanced/production-readiness' },
            { text: 'Performance Tuning', link: '/guide/advanced/performance' },
            { text: 'Writing Plugins', link: '/guide/advanced/writing-plugins' },
            { text: 'Proposal: Scripting Plugins', link: '/guide/advanced/scripting-plugins-proposal' },
            { text: 'Proposal: Plugin Marketplace', link: '/guide/advanced/plugin-marketplace-proposal' },
            { text: 'Proposal: Workflow Designer', link: '/guide/advanced/workflow-designer-proposal' },
            { text: 'Proposal: Realtime Channels & Presence', link: '/guide/advanced/realtime-channels-proposal' },
            { text: 'Proposal: Custom HTTP Routes', link: '/guide/advanced/custom-routes-proposal' },
            { text: 'Design: Custom Routes Phase 3', link: '/guide/advanced/custom-routes-phase3-design' },
            { text: 'Design: Multi-Project Platform', link: '/guide/advanced/multi-project-design' },
            { text: 'Proposal: Schema Constraints', link: '/guide/advanced/schema-constraints-proposal' },
            { text: 'Plan: CRM Conformance Suite', link: '/guide/advanced/crm-conformance-plan' }
          ]
        }
      ],
      '/api/': [
        {
          text: '📚 API Reference',
          items: [
            { text: 'Overview', link: '/api/' },
            { text: 'CLI Commands', link: '/api/cli' },
            { text: 'GraphQL Schema', link: '/api/graphql' },
            { text: 'Platform GraphQL', link: '/api/platform' },
            { text: 'Auth (REST)', link: '/api/auth' },
            { text: 'Audit (REST)', link: '/api/audit' },
            { text: 'Workflows (REST)', link: '/api/workflows' },
            { text: 'Projections (REST)', link: '/api/projections' },
            { text: 'Schema Metadata', link: '/api/metadata' },
            { text: 'Content Blocks', link: '/api/content-blocks' },
            { text: 'TypeScript SDK', link: '/api/typescript-sdk' },
            { text: 'Rust APIs', link: '/api/rust' },
            { text: 'Plugin APIs', link: '/api/plugins' }
          ]
        }
      ],
      '/services/': [
        {
          text: '🧩 CRM Service',
          items: [
            { text: 'Refinement & Dogfooding Roadmap', link: '/services/crm/roadmap' },
            { text: 'Release Checklist', link: '/services/crm/release-checklist' }
          ]
        }
      ],
      '/examples/': [
        {
          text: '🎯 Examples',
          items: [
            { text: 'Overview', link: '/examples/' },
            { text: 'CRM (flagship service)', link: '/services/' }
          ]
        }
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/atomo-cc/atomo' }
    ],

    footer: {
      message: 'Released under the AGPL-3.0 License.',
      copyright: 'Copyright © 2024 Atomo Team'
    },

    search: {
      provider: 'local'
    },

    editLink: {
      pattern: 'https://github.com/atomo-cc/atomo/edit/main/docs/:path',
      text: 'Edit this page on GitHub'
    }
  },

  markdown: {
    theme: {
      light: 'github-light',
      dark: 'github-dark'
    },
    lineNumbers: true
  }
})
