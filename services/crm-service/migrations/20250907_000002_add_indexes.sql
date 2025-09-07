-- Additional helpful indexes for CRM

-- Contacts
CREATE INDEX IF NOT EXISTS idx_contact_email ON contact (email);
CREATE INDEX IF NOT EXISTS idx_contact_company_id ON contact (company_id);

-- Deals
CREATE INDEX IF NOT EXISTS idx_deal_contact_id ON deal (contact_id);
CREATE INDEX IF NOT EXISTS idx_deal_company_id ON deal (company_id);
CREATE INDEX IF NOT EXISTS idx_deal_updated_at ON deal (updated_at DESC);

-- Companies
CREATE INDEX IF NOT EXISTS idx_company_name ON company (name);

