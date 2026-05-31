import { defineConfig } from 'vitest/config'

// Unit tests live in src/**; Playwright e2e specs (e2e/**) are run separately.
export default defineConfig({
  test: {
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['e2e/**', 'node_modules/**'],
  },
})
