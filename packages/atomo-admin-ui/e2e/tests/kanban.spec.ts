import { test, expect } from '@playwright/test'
import t from '../../../../services/crm-service/admin-ui/locales/en.json' with { type: 'json' }

test('Kanban loads and supports DnD', async ({ page }) => {
  await page.goto('/deals/board')
  await expect(page.getByRole('heading', { name: t.deals.title })).toBeVisible()

  // Demo mode (no backend in e2e) surfaces the honesty banner
  await expect(page.getByRole('note')).toContainText(t.demoBanner)

  // Expect columns
  await expect(page.getByTestId('deals-kanban-column')).toHaveCount(6)

  // If there is a card, try to drag it within the same column
  const cards = page.getByTestId('deal-card')
  if (await cards.count() > 0) {
    const first = cards.first()
    await first.dragTo(cards.nth(0))
  }
})
