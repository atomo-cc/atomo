-- Seed data for the CRM demo loop: companies, contacts, deals, Kanban positions, and timeline activity.

TRUNCATE TABLE activity, deal, contact, company RESTART IDENTITY;

-- Companies
INSERT INTO company (name, website, address, industry, size, notes) VALUES
  (
    'Acme Corp',
    'https://acme.example',
    '100 Market St, San Francisco, CA',
    'Manufacturing',
    'large',
    '[{"id":"company-note-acme","type":"paragraph","text":"Expanding the partner operations team this quarter.","order":0}]'::jsonb
  ),
  (
    'Globex Inc',
    'https://globex.example',
    '500 Innovation Way, Austin, TX',
    'Technology',
    'enterprise',
    '[{"id":"company-note-globex","type":"paragraph","text":"Security review is required before procurement approval.","order":0}]'::jsonb
  ),
  (
    'Initech',
    'https://initech.example',
    '42 Milton Ave, Dallas, TX',
    'Software',
    'medium',
    '[{"id":"company-note-initech","type":"paragraph","text":"Prefers short pilots with clear adoption metrics.","order":0}]'::jsonb
  ),
  (
    'Umbrella Health',
    'https://umbrella.example',
    '700 Research Dr, Boston, MA',
    'Healthcare',
    'large',
    '[{"id":"company-note-umbrella","type":"paragraph","text":"Needs audit-friendly activity history for every account touch.","order":0}]'::jsonb
  );

-- Contacts
INSERT INTO contact (first_name, last_name, email, phone, company_id, tags, notes)
SELECT
  v.first_name,
  v.last_name,
  v.email,
  v.phone,
  c.id,
  v.tags::jsonb,
  v.notes::jsonb
FROM (
  VALUES
    ('John', 'Doe', 'john.doe@acme.example', '123-456-7890', 'Acme Corp', '["decision-maker","manufacturing"]', '[{"id":"contact-note-john","type":"paragraph","text":"Owns the partner operations budget.","order":0}]'),
    ('Jane', 'Smith', 'jane.smith@globex.example', '222-333-4444', 'Globex Inc', '["security-review","enterprise"]', '[{"id":"contact-note-jane","type":"paragraph","text":"Asked for SSO and audit log details.","order":0}]'),
    ('Peter', 'Gibbons', 'peter@initech.example', '555-123-4567', 'Initech', '["pilot","software"]', '[{"id":"contact-note-peter","type":"paragraph","text":"Wants a two-week pilot before expanding seats.","order":0}]'),
    ('Maya', 'Patel', 'maya.patel@umbrella.example', '555-987-6543', 'Umbrella Health', '["compliance","healthcare"]', '[{"id":"contact-note-maya","type":"paragraph","text":"Evaluating workflow history for compliance reporting.","order":0}]')
) AS v(first_name, last_name, email, phone, company_name, tags, notes)
JOIN company c ON c.name = v.company_name;

-- Deals across every Kanban stage. Position is zero-based within each stage.
INSERT INTO deal (title, value, stage, position, contact_id, company_id, description, expected_close_date, actual_close_date)
SELECT
  v.title,
  v.value,
  v.stage,
  v.position,
  ct.id,
  ct.company_id,
  v.description::jsonb,
  CURRENT_DATE + (v.expected_days || ' days')::interval,
  CASE WHEN v.actual_days IS NULL THEN NULL ELSE CURRENT_DATE + (v.actual_days || ' days')::interval END
FROM (
  VALUES
    ('Acme partner expansion', 50000, 'lead', 0, 'john.doe@acme.example', '[{"id":"deal-acme-expansion","type":"paragraph","text":"New partner team is comparing CRM workflow options.","order":0}]', 21, NULL),
    ('Umbrella compliance intake', 36000, 'lead', 1, 'maya.patel@umbrella.example', '[{"id":"deal-umbrella-intake","type":"paragraph","text":"Initial compliance discovery for timeline and audit needs.","order":0}]', 30, NULL),
    ('Initech pilot package', 18000, 'qualified', 0, 'peter@initech.example', '[{"id":"deal-initech-pilot","type":"paragraph","text":"Qualified pilot with success metrics tied to task completion.","order":0}]', 14, NULL),
    ('Globex security review', 72000, 'qualified', 1, 'jane.smith@globex.example', '[{"id":"deal-globex-security","type":"paragraph","text":"Security team requested architecture and admin access controls.","order":0}]', 25, NULL),
    ('Acme rollout proposal', 98000, 'proposal', 0, 'john.doe@acme.example', '[{"id":"deal-acme-rollout","type":"paragraph","text":"Proposal includes migration, onboarding, and reporting milestones.","order":0}]', 18, NULL),
    ('Initech seat expansion', 27000, 'proposal', 1, 'peter@initech.example', '[{"id":"deal-initech-expansion","type":"paragraph","text":"Expansion proposal after pilot stakeholder review.","order":0}]', 12, NULL),
    ('Umbrella audit workflow', 64000, 'proposal', 2, 'maya.patel@umbrella.example', '[{"id":"deal-umbrella-audit","type":"paragraph","text":"Proposal emphasizes activity timeline retention and task follow-up.","order":0}]', 20, NULL),
    ('Globex enterprise contract', 125000, 'negotiation', 0, 'jane.smith@globex.example', '[{"id":"deal-globex-contract","type":"paragraph","text":"Procurement is reviewing redlines and final commercial terms.","order":0}]', 10, NULL),
    ('Acme services add-on', 24000, 'negotiation', 1, 'john.doe@acme.example', '[{"id":"deal-acme-addon","type":"paragraph","text":"Services scope is agreed; waiting on budget approval.","order":0}]', 8, NULL),
    ('Initech workflow renewal', 42000, 'won', 0, 'peter@initech.example', '[{"id":"deal-initech-renewal","type":"paragraph","text":"Renewal closed after successful pilot conversion.","order":0}]', -7, -3),
    ('Acme legacy import', 16000, 'won', 1, 'john.doe@acme.example', '[{"id":"deal-acme-import","type":"paragraph","text":"Small import package closed for the admin rollout.","order":0}]', -14, -10),
    ('Globex departmental trial', 15000, 'lost', 0, 'jane.smith@globex.example', '[{"id":"deal-globex-trial","type":"paragraph","text":"Lost to internal build after department budget freeze.","order":0}]', -5, -2)
) AS v(title, value, stage, position, contact_email, description, expected_days, actual_days)
JOIN contact ct ON ct.email = v.contact_email;

-- Timeline activity for contact detail views.
INSERT INTO activity (contact_id, activity_type, title, content, metadata, created_at, updated_at)
SELECT
  ct.id,
  v.activity_type,
  v.title,
  v.content,
  v.metadata::jsonb,
  NOW() - (v.days_ago || ' days')::interval,
  NOW() - (v.days_ago || ' days')::interval
FROM (
  VALUES
    ('john.doe@acme.example', 'note', 'Budget owner confirmed', 'John confirmed that partner operations owns the rollout budget.', '{"source":"demo-script"}', 6),
    ('john.doe@acme.example', 'call', 'Pricing call', 'Reviewed services add-on scope and next approval step.', '{"durationMinutes":28,"outcome":"next-step"}', 3),
    ('jane.smith@globex.example', 'email', 'Security packet sent', 'Sent SSO, audit log, and data retention details to Jane.', '{"template":"security-follow-up"}', 5),
    ('jane.smith@globex.example', 'task', 'Follow up on redlines', 'Check whether legal has returned contract redlines.', '{"dueInDays":2,"priority":"high"}', 1),
    ('peter@initech.example', 'meeting', 'Pilot retro', 'Team agreed the pilot met adoption goals and moved renewal to closed won.', '{"attendees":["Peter Gibbons","Sales Rep"],"durationMinutes":45}', 4),
    ('maya.patel@umbrella.example', 'call', 'Compliance discovery', 'Maya outlined timeline retention and audit export requirements.', '{"durationMinutes":32,"outcome":"proposal-requested"}', 2)
) AS v(contact_email, activity_type, title, content, metadata, days_ago)
JOIN contact ct ON ct.email = v.contact_email;
