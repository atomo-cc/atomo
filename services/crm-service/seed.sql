-- Seed data for CRM demo

-- Companies
INSERT INTO company (name, website, industry, notes) VALUES
  ('Acme Corp', 'https://acme.example', 'Manufacturing', '[]'::jsonb),
  ('Globex Inc', 'https://globex.example', 'Technology', '[]'::jsonb),
  ('Initech', 'https://initech.example', 'Software', '[]'::jsonb);

-- Contacts
INSERT INTO contact (first_name, last_name, email, phone, company_id, tags, notes)
SELECT 'John', 'Doe', 'john.doe@acme.example', '123-456-7890', c.id, '[]'::jsonb, '[]'::jsonb FROM company c WHERE c.name = 'Acme Corp';
INSERT INTO contact (first_name, last_name, email, phone, company_id, tags, notes)
SELECT 'Jane', 'Smith', 'jane.smith@globex.example', '222-333-4444', c.id, '[]'::jsonb, '[]'::jsonb FROM company c WHERE c.name = 'Globex Inc';
INSERT INTO contact (first_name, last_name, email, phone, company_id, tags, notes)
SELECT 'Peter', 'Gibbons', 'peter@initech.example', '555-123-4567', c.id, '[]'::jsonb, '[]'::jsonb FROM company c WHERE c.name = 'Initech';

-- Deals (20 across stages)
WITH contacts AS (
  SELECT id AS contact_id FROM contact LIMIT 3
)
INSERT INTO deal (title, value, stage, contact_id, company_id, description, expected_close_date, position)
SELECT 
  d.title,
  d.value,
  d.stage,
  c.contact_id,
  (SELECT company_id FROM contact WHERE id = c.contact_id),
  '[]'::jsonb,
  NOW() + (d.days_offset || ' days')::interval,
  d.pos
FROM contacts c,
  (VALUES 
    ('Initial outreach', 5000, 'lead', 7, 0),
    ('Qualification call', 8000, 'qualified', 10, 1),
    ('Proposal draft', 12000, 'proposal', 14, 0),
    ('Negotiation round 1', 20000, 'negotiation', 21, 2),
    ('Negotiation round 2', 25000, 'negotiation', 28, 3),
    ('Pilot project', 15000, 'proposal', 18, 1),
    ('Discount discussion', 9000, 'negotiation', 12, 4),
    ('Legal review', 18000, 'proposal', 16, 2),
    ('SOW finalized', 30000, 'proposal', 20, 3),
    ('Closed won - Q3 deal', 45000, 'won', 5, 0),
    ('Renewal - Year 2', 32000, 'won', 60, 1),
    ('Churn risk', 7000, 'lost', 3, 0),
    ('Upsell opportunity', 11000, 'qualified', 25, 0),
    ('Warm lead', 4000, 'lead', 9, 1),
    ('Cold lead', 2000, 'lead', 30, 2),
    ('Demo scheduled', 6000, 'qualified', 11, 1),
    ('Technical evaluation', 10000, 'proposal', 15, 4),
    ('Budget approval', 18000, 'negotiation', 19, 5),
    ('Closed lost - budget', 8000, 'lost', -2, 1),
    ('Closed won - add-on', 9000, 'won', 8, 2)
  ) AS d(title, value, stage, days_offset, pos);

-- Activities
INSERT INTO activity (contact_id, activity_type, title, content)
SELECT id, 'note', 'Initial note', 'Called the customer, left voicemail.' FROM contact LIMIT 1;
INSERT INTO activity (contact_id, activity_type, title, content)
SELECT id, 'meeting', 'Discovery call', 'Discussed requirements and timeline.' FROM contact OFFSET 1 LIMIT 1;
