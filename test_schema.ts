// Test schema to verify CRM generation
export interface Contact {
  id: string;
  name: string;
  email: string;
  phone?: string;
  companyId?: string;
}

export interface Company {
  id: string;
  name: string;
  domain?: string;
  website?: string;
  industry?: string;
}
