/**
 * Company Data Enrichment Plugin
 * 
 * This WASM plugin automatically enriches company data by calling external APIs
 * when a company is created or its website is updated.
 */

import { onEvent, fetch, logger } from '@atomo/plugin-sdk';

// Listen for company creation and updates
onEvent('Company.Created', enrichCompanyData);
onEvent('Company.Updated', async (event) => {
  // Only enrich if website was changed
  if (event.changes.includes('website')) {
    await enrichCompanyData(event);
  }
});

async function enrichCompanyData(event: any) {
  const company = event.payload;
  const { id, name, website } = company;
  
  try {
    logger.info(`Enriching data for company: ${name}`);
    
    // Skip if no website provided
    if (!website) {
      logger.debug(`No website provided for company ${name}, skipping enrichment`);
      return;
    }
    
    // Call company enrichment API (example: Clearbit, HunterIO, etc.)
    const enrichmentData = await fetchCompanyInfo(website);
    
    if (enrichmentData) {
      // Update company with enriched data
      await updateCompany(id, {
        industry: enrichmentData.industry || company.industry,
        size: enrichmentData.size || company.size,
        address: enrichmentData.address || company.address,
        // Add enriched data to notes
        notes: [
          ...company.notes,
          {
            type: 'paragraph',
            content: `Auto-enriched data: ${JSON.stringify(enrichmentData, null, 2)}`
          }
        ]
      });
      
      logger.info(`Successfully enriched company ${name} with external data`);
    }
    
  } catch (error) {
    logger.error(`Failed to enrich company ${name}:`, error);
    // Don't throw error - enrichment failure shouldn't block company creation
  }
}

async function fetchCompanyInfo(website: string) {
  try {
    // Example API call - replace with your preferred data provider
    const response = await fetch(`https://api.clearbit.com/v2/companies/find?domain=${extractDomain(website)}`, {
      headers: {
        'Authorization': `Bearer ${process.env.CLEARBIT_API_KEY}`
      }
    });
    
    if (!response.ok) {
      throw new Error(`API call failed: ${response.status}`);
    }
    
    const data = await response.json();
    
    return {
      industry: data.category?.industry,
      size: mapCompanySize(data.metrics?.employees),
      address: formatAddress(data.geo),
      description: data.description,
      foundedYear: data.foundedYear,
      techStack: data.tech?.join(', ')
    };
    
  } catch (error) {
    logger.warn(`External API call failed for ${website}:`, error.message);
    return null;
  }
}

function extractDomain(website: string): string {
  try {
    const url = new URL(website.startsWith('http') ? website : `https://${website}`);
    return url.hostname;
  } catch {
    return website;
  }
}

function mapCompanySize(employeeCount: number): string {
  if (!employeeCount) return 'unknown';
  if (employeeCount < 10) return 'startup';
  if (employeeCount < 50) return 'small';
  if (employeeCount < 250) return 'medium';
  if (employeeCount < 1000) return 'large';
  return 'enterprise';
}

function formatAddress(geo: any): string {
  if (!geo) return '';
  const parts = [geo.streetName, geo.city, geo.state, geo.country].filter(Boolean);
  return parts.join(', ');
}

// Helper function to update company (provided by Atomo runtime)
declare function updateCompany(id: string, updates: any): Promise<void>;
