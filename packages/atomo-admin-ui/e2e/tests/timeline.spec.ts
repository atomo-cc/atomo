import { test, expect } from '@playwright/test'

test('Contact timeline shows and can add note', async ({ page }) => {
  await page.goto('/contacts/contact_ava/timeline')
  await expect(page.getByText('联系人时间线')).toBeVisible()

  const textarea = page.locator('textarea').first()
  await textarea.fill('E2E note')
  await page.getByRole('button', { name: '添加备注' }).click()
  await expect(page.getByText('E2E note')).toBeVisible({ timeout: 10_000 })
})
