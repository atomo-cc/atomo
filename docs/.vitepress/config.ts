import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Atomo',
  description: 'The Next-Generation Content Core - Developer Documentation',

  head: [
    ['link', { rel: 'icon', href: '/favicon.ico' }],
    ['meta', { name: 'theme-color', content: '#3c82f6' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:locale', content: 'en' }],
    ['meta', { property: 'og:title', content: 'Atomo | Next-Generation Content Core' }],
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
      { text: 'Playground', link: 'https://playground.atomo.cc' },
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
            { text: 'Real-time Collaboration', link: '/guide/collaboration' },
            { text: 'Dev Runtime & Workspace', link: '/guide/dev-runtime' },
            { text: 'Server Routes in Dev', link: '/guide/server-routes-dev' },
            { text: 'Modeling & Access', link: '/guide/modeling' },
            { text: 'Configuration', link: '/guide/configuration' },
            { text: 'WASM Plugins', link: '/guide/plugins' },
            { text: 'Testing', link: '/guide/testing' },
            { text: 'Local-First Architecture', link: '/guide/local-first' }
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
            { text: 'Multi-tenant Setup', link: '/guide/advanced/multi-tenant' },
            { text: 'Access & Hooks', link: '/guide/advanced/access-hooks' },
            { text: 'AI & Vector Search', link: '/guide/advanced/ai-vector' },
            { text: 'Security & Auth', link: '/guide/advanced/security' },
            { text: 'Production Readiness', link: '/guide/advanced/production-readiness' },
            { text: 'Performance Tuning', link: '/guide/advanced/performance' },
            { text: 'Proposal: Scripting Plugins', link: '/guide/advanced/scripting-plugins-proposal' },
            { text: 'Proposal: Plugin Marketplace', link: '/guide/advanced/plugin-marketplace-proposal' },
            { text: 'Proposal: Workflow Designer', link: '/guide/advanced/workflow-designer-proposal' }
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
            { text: 'Release Checklist', link: '/services/crm/release-checklist' }
          ]
        }
      ],
      '/examples/': [
        {
          text: '🎯 Examples',
          items: [
            { text: 'Overview', link: '/examples/' },
            { text: 'CRM System', link: '/examples/crm' },
            { text: 'Content Management', link: '/examples/cms' },
            { text: 'Collaboration Tools', link: '/examples/collaboration' },
            { text: 'E-commerce', link: '/examples/ecommerce' },
            { text: 'Analytics Dashboard', link: '/examples/analytics' }
          ]
        }
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/Chris533/atomo' },
      { icon: 'discord', link: 'https://discord.gg/atomo' }
    ],

    footer: {
      message: 'Released under the AGPL-3.0 License.',
      copyright: 'Copyright © 2024 Atomo Team'
    },

    search: {
      provider: 'local'
    },

    editLink: {
      pattern: 'https://github.com/Chris533/atomo/edit/main/docs/:path',
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
