export async function notifySales(input: { title: string; value: number; tenantId: string }) {
  console.log('[slack.sales]', input)
}
