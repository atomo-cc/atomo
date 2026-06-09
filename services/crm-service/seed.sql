-- Seed data for the CRM service.
-- Run via: wsl bash -c "docker exec -i postgresql psql -U atomo_crm_dev -d atomo_crm_dev" < seed.sql

BEGIN;

TRUNCATE TABLE activities, deals, contacts, companies CASCADE;

-- Companies
INSERT INTO companies (id, name, industry, website) VALUES
  ('comp-acme',     'Acme Corp',       'Manufacturing', 'https://acme.example'),
  ('comp-globex',   'Globex Inc',      'Technology',    'https://globex.example'),
  ('comp-initech',  'Initech',         'Software',      'https://initech.example'),
  ('comp-umbrella', 'Umbrella Health', 'Healthcare',    'https://umbrella.example');

-- Contacts (stage is jsonb)
INSERT INTO contacts (id, name, email, phone, stage, company_id) VALUES
  ('cont-john',  'John Doe',      'john.doe@acme.example',       '123-456-7890', '"customer"',  'comp-acme'),
  ('cont-jane',  'Jane Smith',    'jane.smith@globex.example',   '222-333-4444', '"qualified"', 'comp-globex'),
  ('cont-peter', 'Peter Gibbons', 'peter@initech.example',       '555-123-4567', '"customer"',  'comp-initech'),
  ('cont-maya',  'Maya Patel',    'maya.patel@umbrella.example', '555-987-6543', '"lead"',      'comp-umbrella');

-- Deals (status is jsonb, value is bigint)
INSERT INTO deals (id, title, value, status, contact_id) VALUES
  ('deal-1', 'Acme partner expansion',     50000,  '"open"', 'cont-john'),
  ('deal-2', 'Globex security review',     72000,  '"open"', 'cont-jane'),
  ('deal-3', 'Initech pilot package',      18000,  '"open"', 'cont-peter'),
  ('deal-4', 'Umbrella compliance intake', 36000,  '"open"', 'cont-maya'),
  ('deal-5', 'Acme rollout proposal',      98000,  '"open"', 'cont-john'),
  ('deal-6', 'Globex enterprise contract', 125000, '"open"', 'cont-jane'),
  ('deal-7', 'Initech workflow renewal',   42000,  '"won"',  'cont-peter'),
  ('deal-8', 'Acme legacy import',         16000,  '"won"',  'cont-john'),
  ('deal-9', 'Globex departmental trial',  15000,  '"lost"', 'cont-jane');

-- Activities (type is jsonb)
INSERT INTO activities (id, contact_id, deal_id, type, notes) VALUES
  ('act-1', 'cont-john',  'deal-1', '"call"',    'Discussed partner expansion timeline and budget.'),
  ('act-2', 'cont-john',  'deal-5', '"meeting"', 'Reviewed rollout proposal with operations team.'),
  ('act-3', 'cont-jane',  'deal-2', '"email"',   'Sent SSO and audit log details for security review.'),
  ('act-4', 'cont-jane',  'deal-6', '"call"',    'Pricing call — procurement reviewing final terms.'),
  ('act-5', 'cont-peter', 'deal-3', '"meeting"', 'Pilot retro — team agreed adoption goals were met.'),
  ('act-6', 'cont-maya',  'deal-4', '"call"',    'Compliance discovery — outlined retention requirements.');

COMMIT;
