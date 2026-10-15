import { defineConfig, devices } from '@playwright/test'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const packageDir = fileURLToPath(new URL('..', import.meta.url))
const production = process.env.E2E_PRODUCTION === '1'
const port = Number(process.env.E2E_FRONTEND_PORT || (production ? '4173' : '5173'))
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('Invalid E2E_FRONTEND_PORT')
const baseURL = process.env.E2E_BASE_URL || `http://127.0.0.1:${port}`

export default defineConfig({
  testDir: './tests',
  timeout: 30_000,
  globalTimeout: 300_000,
  retries: 0,
  // Match CI's package-root artifact path; absolute local overrides stay absolute.
  outputDir: resolve(packageDir, process.env.E2E_OUTPUT_DIR || 'test-results'),
  use: {
    baseURL,
    headless: true,
    screenshot: 'on',
  },
  webServer: process.env.E2E_BASE_URL
    ? undefined
    : {
        command: `"${process.execPath}" node_modules/vite/bin/vite.js ${production ? 'preview' : ''} --host 127.0.0.1 --port ${port} --strictPort`,
        // Relative to this config file — the admin package root, where `pnpm dev` exists.
        cwd: '..',
        url: baseURL,
        reuseExistingServer: !production && !process.env.CI,
        timeout: 120_000,
      },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
})
