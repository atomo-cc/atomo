import type { LeadSource } from '../generated/types'

export function scoreLead(input: { email: string; source: LeadSource; companyId: string | null }): number {
  const domain = input.email.split('@')[1]?.toLowerCase() ?? ''
  let score = 20

  if (input.source === 'referral') score += 25
  if (input.source === 'event') score += 15
  if (input.companyId) score += 20
  if (!['gmail.com', 'yahoo.com', 'outlook.com', 'icloud.com'].includes(domain)) score += 20

  return Math.max(0, Math.min(100, score))
}
