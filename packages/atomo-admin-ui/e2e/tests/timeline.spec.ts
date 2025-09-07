import { test, expect } from '@playwright/test'

test('Contact timeline shows and can add note', async ({ page }) => {
  // This assumes there is at least one contact; navigate via list
  await page.goto('/entities/Contact')
  const firstRow = page.locator('table tr').nth(1)
  if (await firstRow.count() === 0) test.skip()

  const idCell = firstRow.locator('td').first()
  const idText = await idCell.textContent()
  if (!idText) test.skip()

  await page.goto(`/contacts/${idText}/timeline`)
  await expect(page.getByText('联系人时间线')).toBeVisible()

  const textarea = page.locator('textarea').first()
  await textarea.fill('E2E note')
  await page.getByRole('button', { name: '添加备注' }).click()
  await expect(page.getByText('E2E note')).toBeVisible({ timeout: 10_000 })
})

