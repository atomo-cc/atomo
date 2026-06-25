import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
//
// `base` is set on the CLI at build time (e.g. `vite build --base=/admin/`) so the
// same config serves both root (dev) and the /admin subpath (production image).
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
  },
  server: {
    port: 5173,
    host: true,
    proxy: {
      // Proxy all API requests to atomo-server. Vite serves its own assets
      // (/@vite, /src, /node_modules, *.hot-update.*) directly; everything
      // else is an API call that should hit the backend.
      '/api': 'http://localhost:3000',
      '/auth': 'http://localhost:3000',
      '/graphql': 'http://localhost:3000',
      '/schema.ts': 'http://localhost:3000',
      '/meta': 'http://localhost:3000',
      '/health': 'http://localhost:3000',
      '/media': 'http://localhost:3000',
      '/jobs': 'http://localhost:3000',
      '/workflows': 'http://localhost:3000',
      '/audit': 'http://localhost:3000',
      '/oauth': 'http://localhost:3000',
      '/version': 'http://localhost:3000',
      '/info': 'http://localhost:3000',
      '/ready': 'http://localhost:3000',
      '/metrics': 'http://localhost:3000',
      '/ws': { target: 'ws://localhost:3000', ws: true },
    },
  },
})
