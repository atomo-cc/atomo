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
    // Split stable third-party deps into their own cacheable chunks so the app
    // bundle stays small and vendor code isn't re-downloaded on every app change.
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules')) {
            if (/[\\/]node_modules[\\/](react|react-dom|react-router|scheduler)[\\/]/.test(id)) {
              return 'react-vendor'
            }
            return 'vendor'
          }
        },
      },
    },
  },
  server: {
    port: 5173,
    host: true,
  },
})
