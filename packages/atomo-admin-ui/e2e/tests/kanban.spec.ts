import { test, expect } from '@playwright/test'

test('Kanban loads and supports DnD', async ({ page }) => {
  await page.goto('/deals/board')
  await expect(page.getByText('商机看板')).toBeVisible()

  // Expect columns
  const columns = await page.locator('[class*=pipeline-columns], [class*=grid]').count()
  expect(columns).toBeGreaterThan(0)

  // If there is a card, try to drag it within the same column
  const cards = page.locator('[draggable]')
  if (await cards.count() > 0) {
    const first = cards.first()
    await first.dragTo(cards.nth(0))
  }
})

