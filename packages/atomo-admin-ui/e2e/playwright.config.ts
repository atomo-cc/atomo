import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './tests',
  timeout: 30_000,
  retries: 0,
  use: {
    baseURL: process.env.E2E_BASE_URL || 'http://localhost:5173',
    headless: true,
  },
  webServer: process.env.E2E_BASE_URL
    ? undefined
    : {
        command: 'pnpm dev',
        url: 'http://localhost:5173',
        reuseExistingServer: !process.env.CI,
        timeout: 120_000,
      },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
})
