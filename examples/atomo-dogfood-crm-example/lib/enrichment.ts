export function domainFromWebsite(website: string | null): string | null {
  if (!website) return null
  try {
    return new URL(website).hostname.replace(/^www\./, '')
  } catch {
    return null
  }
}

export async function inferIndustry(companyName: string, domain: string | null): Promise<string | null> {
  if (domain?.includes('health')) return 'Healthcare'
  if (domain?.includes('bank') || companyName.toLowerCase().includes('capital')) return 'Finance'
  if (domain?.includes('edu')) return 'Education'
  return 'Software'
}
