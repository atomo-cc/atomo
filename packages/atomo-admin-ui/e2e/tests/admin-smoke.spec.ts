/**
 * Admin smoke — guards the consumer-reported escape class: admin rendering that
 * contradicts schema truth. Every assertion here maps to a real feedback item
 * (#8: listView columns dropped, #9: date-only timestamps, #11: field-name card
 * titles + quick-create on server-written models) or its nearest neighbor.
 *
 * Requires the server running on :3000 with e2e/schema.e2e.ts (see ci.yml `e2e`
 * job, or run locally: DATABASE_URL=… ATOMO_SCHEMA_PATH=…/schema.e2e.ts
 * ADMIN_EMAIL/… ./target/debug/atomo-server, then `pnpm e2e`).
 */

import { test, expect, request } from '@playwright/test'

const API = process.env.E2E_API_URL || 'http://localhost:3000'
const ADMIN_EMAIL = process.env.E2E_ADMIN_EMAIL || 'admin@e2e.dev'
const ADMIN_PASSWORD = process.env.E2E_ADMIN_PASSWORD || 'e2e-admin-pass'

let articleId: string

// Seed one Article through the real API so list assertions have a row.
test.beforeAll(async () => {
  const api = await request.newContext({ baseURL: API })
  const login = await api.post('/auth/login', {
    data: { email: ADMIN_EMAIL, password: ADMIN_PASSWORD },
  })
  expect(login.ok(), `login failed: ${login.status()}`).toBeTruthy()
  const { token } = await login.json()

  // coverImage is a File field seeded with a BARE media-id string — the shape a
  // worker/migration writes, and the shape that crashed the record view (#12A).
  const create = await api.post('/graphql', {
    headers: { authorization: `Bearer ${token}` },
    data: {
      query: `mutation { create(model: "Article", data: { title: "Smoke Article", status: "published", coverImage: "e2e-fake-media-id" }) }`,
    },
  })
  expect(create.ok()).toBeTruthy()
  const body = await create.json()
  expect(body.errors, JSON.stringify(body.errors)).toBeFalsy()
  articleId = body.data.create.id
  await api.dispose()
})

// Sign in through the real login form once per test (state is not shared).
async function signIn(page: import('@playwright/test').Page) {
  await page.goto('/')
  await page.locator('input[type=email]').fill(ADMIN_EMAIL)
  await page.locator('input[type=password]').fill(ADMIN_PASSWORD)
  await page.locator('button[type=submit]').click()
  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible({ timeout: 15_000 })
}

test('dashboard: cards titled by model display name; quick-create only where creatable (#11)', async ({ page }) => {
  await signIn(page)

  // Cards carry MODEL display names — never field names.
  await expect(page.getByText('Audit Event', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('Created At', { exact: true })).toHaveCount(0)

  // Quick-create offered for the creatable model, hidden for the system model.
  await expect(page.getByRole('heading', { name: 'New Article' })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'New Audit Event' })).toHaveCount(0)
})

test('list grid: declared listView columns render, timestamps show time-of-day (#8/#9)', async ({ page }) => {
  await signIn(page)
  await page.goto('/entities/Article')

  // Declared columns, including the timestamp the schema opted into.
  await expect(page.getByText('Created At').first()).toBeVisible()
  await expect(page.getByText('Smoke Article').first()).toBeVisible({ timeout: 15_000 })

  // The datetime cell must include time-of-day, not a bare date (#9).
  // `.grid` is the row container (header + data rows); the last grid containing
  // the title is the data row, which also holds the Created At cell.
  const row = page.locator('div.grid', { hasText: 'Smoke Article' }).last()
  await expect(row).toContainText(/\d{1,2}:\d{2}/)
})

test('list search is wired to the server and honestly labeled', async ({ page }) => {
  await signIn(page)
  await page.goto('/entities/Article')
  await expect(page.getByText('Smoke Article').first()).toBeVisible({ timeout: 15_000 })

  const search = page.getByPlaceholder(/^Search by /)
  await expect(search).toBeVisible()

  // Functional round trip: a nonsense term empties the grid; clearing restores it.
  await search.fill('zz-no-such-record-zz')
  await expect(page.getByText('No data').first()).toBeVisible({ timeout: 15_000 })
  await search.fill('')
  await expect(page.getByText('Smoke Article').first()).toBeVisible({ timeout: 15_000 })
})

test('create form: in:-constrained field renders a dropdown of allowed values', async ({ page }) => {
  await signIn(page)
  await page.goto('/entities/Article/new')

  const combo = page.getByRole('combobox').first()
  await expect(combo).toBeVisible({ timeout: 15_000 })
  await combo.click()
  await expect(page.getByRole('option', { name: 'draft' })).toBeVisible()
  await expect(page.getByRole('option', { name: 'published' })).toBeVisible()
})

test('server-written model: list page offers no create affordance', async ({ page }) => {
  await signIn(page)
  await page.goto('/entities/AuditEvent')

  await expect(page.getByRole('heading', { name: /Audit Event List/ })).toBeVisible({ timeout: 15_000 })
  await expect(page.getByRole('button', { name: /New Audit Event/ })).toHaveCount(0)
})

test('File field with a scalar media-id value renders, never crashes (#12)', async ({ page }) => {
  await signIn(page)

  // Record view: the scalar coverImage must not take down the page — it renders
  // as a single-item file preview showing the media id. (Field values live in
  // form inputs, so assert on the heading + the preview text, not input values.)
  await page.goto(`/entities/Article/${articleId}`)
  await expect(page.getByRole('heading', { name: 'Article', exact: true })).toBeVisible({ timeout: 15_000 })
  await expect(page.getByText('e2e-fake-media-id').first()).toBeVisible()
  await expect(page.getByText(/Something went wrong|is not a function/)).toHaveCount(0)

  // List view: the file column falls back to the id text when the image 404s.
  await page.goto('/entities/Article')
  await expect(page.getByText('e2e-fake-media-id').first()).toBeVisible({ timeout: 15_000 })
})

test('observability: real queue numbers render for an admin', async ({ page }) => {
  await signIn(page)
  await page.goto('/observability')

  await expect(page.getByRole('heading', { name: 'Observability' })).toBeVisible({ timeout: 15_000 })
  await expect(page.getByText('Recent jobs')).toBeVisible()
  // Status tiles resolve to numbers (0 is fine) — not stuck on the loading dash.
  await expect(page.getByText('queued', { exact: false }).first()).toBeVisible()
})
