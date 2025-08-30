/**
 * Atomo CRM Service Configuration
 * 
 * This file configures an Atomo instance specifically for CRM functionality.
 * The Atomo platform will read this configuration and automatically generate:
 * - GraphQL API endpoints
 * - Database schema and migrations  
 * - Admin UI interfaces
 * - Real-time subscriptions
 */

import { defineConfig } from '@atomo/core';

export default defineConfig({
  // Service metadata
  name: 'crm-service',
  version: '1.0.0',
  description: 'Customer Relationship Management service built on Atomo platform',
  
  // Database configuration
  database: {
    // Atomo will automatically create and manage these tables
    autoMigration: true,
    // Enable audit logging for all CRM operations
    auditLog: true,
    // Enable event sourcing for future migration to Phase 2
    eventSourcing: {
      enabled: false, // Phase 1: CRUD with audit
      streamPrefix: 'crm'
    }
  },
  
  // GraphQL API configuration
  graphql: {
    // Enable GraphQL playground in development
    playground: true,
    // Enable introspection
    introspection: true,
    // Custom resolvers (if needed)
    customResolvers: './plugins/resolvers',
  },
  
  // Admin UI configuration
  adminUI: {
    // Enable the admin interface
    enabled: true,
    // Custom theme and components
    customization: './admin',
    // Navigation structure will be auto-generated from schema
    autoNavigation: true,
  },
  
  // Plugin system configuration
  plugins: {
    // Directory containing WASM plugins
    directory: './plugins',
    // Enable hot-reloading in development
    hotReload: true,
  },
  
  // Workflow engine configuration (future feature)
  workflows: {
    directory: './workflows',
    enabled: true,
  },
  
  // Security and authentication
  auth: {
    // For now, use simple API key auth
    // In production, integrate with your identity provider
    mode: 'api_key',
    // Admin users who can access the backend
    adminUsers: [
      'admin@company.com'
    ]
  },
  
  // Performance and caching
  cache: {
    enabled: true,
    ttl: 300, // 5 minutes
  },
  
  // Monitoring and observability
  monitoring: {
    metrics: true,
    tracing: true,
    healthChecks: true,
  }
});
