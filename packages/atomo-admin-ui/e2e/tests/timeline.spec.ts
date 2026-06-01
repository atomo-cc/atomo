import { test, expect } from '@playwright/test'
import t from '../../../../services/crm-service/admin-ui/locales/en.json' with { type: 'json' }

test('Contact timeline shows and can add note', async ({ page }) => {
  await page.goto('/contacts/contact_ava/timeline')
  await expect(page.getByText(t.timeline.title)).toBeVisible()

  const textarea = page.locator('textarea').first()
  await textarea.fill('E2E note')
  await page.getByRole('button', { name: t.timeline.addNote }).click()
  await expect(page.getByText('E2E note')).toBeVisible({ timeout: 10_000 })
})
