#!/usr/bin/env node
/**
 * CRM seed data — realistic, fully related records.
 * Usage: node seed.mjs [--base-url http://localhost:3000]
 */

const BASE = process.argv.includes('--base-url')
  ? process.argv[process.argv.indexOf('--base-url') + 1]
  : 'http://localhost:3000'

// ── Auth ────────────────────────────────────────────────────────────────────

async function login() {
  const res = await fetch(`${BASE}/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email: 'admin@crm.local.dev', password: 'admin123' }),
  })
  if (!res.ok) throw new Error(`Login failed: ${res.status} ${await res.text()}`)
  const { token } = await res.json()
  return token
}

// ── GraphQL helper ──────────────────────────────────────────────────────────

async function gql(token, query, variables) {
  const res = await fetch(`${BASE}/graphql`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ query, variables }),
  })
  const text = await res.text()
  let json
  try { json = JSON.parse(text) } catch { throw new Error(`Non-JSON response (${res.status}): ${text.slice(0, 200)}`) }
  if (json.errors) throw new Error(JSON.stringify(json.errors, null, 2))
  return json.data
}

const sleep = ms => new Promise(r => setTimeout(r, ms))

async function create(token, model, data) {
  await sleep(150)
  const r = await gql(token,
    `mutation($d: JSONObject!) { create(model: "${model}", data: $d) }`,
    { d: data },
  )
  return r.create
}

// ── Seed data ───────────────────────────────────────────────────────────────

async function seed() {
  console.log(`Seeding CRM at ${BASE} ...`)
  const token = await login()
  console.log('  Authenticated.')

  // -- Users (sales team) --
  const usersData = [
    { email: 'sarah.chen@example.com', firstName: 'Sarah', lastName: 'Chen', role: 'MANAGER' },
    { email: 'james.wilson@example.com', firstName: 'James', lastName: 'Wilson', role: 'SALES' },
    { email: 'maria.garcia@example.com', firstName: 'Maria', lastName: 'Garcia', role: 'SALES' },
    { email: 'david.kim@example.com', firstName: 'David', lastName: 'Kim', role: 'SALES' },
    { email: 'emma.taylor@example.com', firstName: 'Emma', lastName: 'Taylor', role: 'VIEWER' },
  ]
  const users = []
  for (const u of usersData) {
    let user
    try {
      const r = await gql(token,
        `mutation($e: String!, $fn: String, $ln: String, $r: UserRole!) {
          createUser(email: $e, firstName: $fn, lastName: $ln, role: $r) { id email firstName lastName role }
        }`,
        { e: u.email, fn: u.firstName, ln: u.lastName, r: u.role },
      )
      user = r.createUser
    } catch {
      const r = await gql(token,
        `{ paginatedRecords(model: "User", limit: 1, where: "email = '${u.email}'") { data } }`,
      )
      const rows = r.paginatedRecords.data
      if (rows.length) user = { id: rows[0].id, firstName: u.firstName, lastName: u.lastName, role: u.role }
      else throw new Error(`Cannot find or create user ${u.email}`)
    }
    users.push(user)
    console.log(`  User: ${user.firstName} ${user.lastName} (${user.role})`)
  }
  const [sarah, james, maria, david] = users

  // -- Companies --
  const companiesData = [
    { name: 'Meridian Technologies', website: 'https://meridiantech.io', domain: 'meridiantech.io', industry: 'SaaS', leadCount: 3, openDealValue: 95000 },
    { name: 'Atlas Manufacturing', website: 'https://atlasmfg.com', domain: 'atlasmfg.com', industry: 'Manufacturing', leadCount: 2, openDealValue: 240000 },
    { name: 'Bluebird Healthcare', website: 'https://bluebirdhealth.org', domain: 'bluebirdhealth.org', industry: 'Healthcare', leadCount: 2, openDealValue: 180000 },
    { name: 'Summit Financial Group', website: 'https://summitfg.com', domain: 'summitfg.com', industry: 'Finance', leadCount: 1, openDealValue: 320000 },
    { name: 'Greenline Logistics', website: 'https://greenlinelogistics.com', domain: 'greenlinelogistics.com', industry: 'Logistics', leadCount: 2, openDealValue: 0 },
    { name: 'Prism Design Studio', website: 'https://prismdesign.co', domain: 'prismdesign.co', industry: 'Design', leadCount: 1, openDealValue: 45000 },
    { name: 'NovaEdge AI', website: 'https://novaedge.ai', domain: 'novaedge.ai', industry: 'AI/ML', leadCount: 1, openDealValue: 150000 },
    { name: 'Coastline Hospitality', website: 'https://coastlinehospitality.com', domain: 'coastlinehospitality.com', industry: 'Hospitality', leadCount: 1, openDealValue: 0 },
  ]
  const companies = []
  for (const c of companiesData) {
    const r = await create(token, 'Company', c)
    companies.push(r)
    console.log(`  Company: ${r.name}`)
  }
  const [meridian, atlas, bluebird, summit, greenline, prism, novaedge, coastline] = companies

  // -- Contacts (2-3 per company) --
  const contactsData = [
    // Meridian Technologies
    { firstName: 'Lisa', lastName: 'Nguyen', email: 'lisa.nguyen@meridiantech.io', phone: '+1-415-555-0101', companyId: meridian.id, ownerId: james.id },
    { firstName: 'Ryan', lastName: 'Patel', email: 'ryan.patel@meridiantech.io', phone: '+1-415-555-0102', companyId: meridian.id, ownerId: james.id },
    { firstName: 'Karen', lastName: 'Zhou', email: 'karen.zhou@meridiantech.io', companyId: meridian.id, ownerId: james.id },
    // Atlas Manufacturing
    { firstName: 'Michael', lastName: 'Brown', email: 'mbrown@atlasmfg.com', phone: '+1-312-555-0201', companyId: atlas.id, ownerId: maria.id },
    { firstName: 'Jennifer', lastName: 'Davis', email: 'jdavis@atlasmfg.com', phone: '+1-312-555-0202', companyId: atlas.id, ownerId: maria.id },
    // Bluebird Healthcare
    { firstName: 'Dr. Amanda', lastName: 'Foster', email: 'afoster@bluebirdhealth.org', phone: '+1-617-555-0301', companyId: bluebird.id, ownerId: sarah.id },
    { firstName: 'Thomas', lastName: 'Wright', email: 'twright@bluebirdhealth.org', companyId: bluebird.id, ownerId: sarah.id },
    // Summit Financial
    { firstName: 'Alexandra', lastName: 'Moore', email: 'amoore@summitfg.com', phone: '+1-212-555-0401', companyId: summit.id, ownerId: david.id },
    { firstName: 'Robert', lastName: 'Clark', email: 'rclark@summitfg.com', phone: '+1-212-555-0402', companyId: summit.id, ownerId: david.id },
    // Greenline Logistics
    { firstName: 'Patricia', lastName: 'Lee', email: 'plee@greenlinelogistics.com', phone: '+1-503-555-0501', companyId: greenline.id, ownerId: james.id },
    { firstName: 'Kevin', lastName: 'Martinez', email: 'kmartinez@greenlinelogistics.com', companyId: greenline.id, ownerId: james.id },
    // Prism Design Studio
    { firstName: 'Olivia', lastName: 'Anderson', email: 'olivia@prismdesign.co', phone: '+1-323-555-0601', companyId: prism.id, ownerId: maria.id },
    // NovaEdge AI
    { firstName: 'Daniel', lastName: 'Russo', email: 'drusso@novaedge.ai', phone: '+1-650-555-0701', companyId: novaedge.id, ownerId: david.id },
    { firstName: 'Priya', lastName: 'Sharma', email: 'psharma@novaedge.ai', companyId: novaedge.id, ownerId: david.id },
    // Coastline Hospitality
    { firstName: 'Marcus', lastName: 'Johnson', email: 'mjohnson@coastlinehospitality.com', phone: '+1-858-555-0801', companyId: coastline.id, ownerId: maria.id },
  ]
  const contacts = []
  for (const c of contactsData) {
    const r = await create(token, 'Contact', c)
    contacts.push(r)
    console.log(`  Contact: ${r.first_name || r.firstName} ${r.last_name || r.lastName}`)
  }

  // -- Leads (various stages) --
  const leadsData = [
    // Meridian — 3 leads
    { email: 'lisa.nguyen@meridiantech.io', source: 'website', status: 'qualified', score: 72, companyId: meridian.id, contactId: contacts[0].id, ownerId: james.id },
    { email: 'ryan.patel@meridiantech.io', source: 'referral', status: 'new', score: 45, companyId: meridian.id, contactId: contacts[1].id, ownerId: james.id },
    { email: 'info@meridiantech.io', source: 'event', status: 'new', score: 30, companyId: meridian.id, ownerId: james.id },
    // Atlas — 2 leads
    { email: 'mbrown@atlasmfg.com', source: 'outbound', status: 'qualified', score: 85, companyId: atlas.id, contactId: contacts[3].id, ownerId: maria.id },
    { email: 'jdavis@atlasmfg.com', source: 'event', status: 'converted', score: 91, companyId: atlas.id, contactId: contacts[4].id, ownerId: maria.id },
    // Bluebird — 2 leads
    { email: 'afoster@bluebirdhealth.org', source: 'referral', status: 'qualified', score: 78, companyId: bluebird.id, contactId: contacts[5].id, ownerId: sarah.id },
    { email: 'procurement@bluebirdhealth.org', source: 'website', status: 'new', score: 35, companyId: bluebird.id, ownerId: sarah.id },
    // Summit — 1 lead (high value)
    { email: 'amoore@summitfg.com', source: 'referral', status: 'qualified', score: 92, companyId: summit.id, contactId: contacts[7].id, ownerId: david.id },
    // Greenline — 2 leads
    { email: 'plee@greenlinelogistics.com', source: 'website', status: 'disqualified', score: 15, companyId: greenline.id, contactId: contacts[9].id, ownerId: james.id },
    { email: 'kmartinez@greenlinelogistics.com', source: 'import', status: 'new', score: 40, companyId: greenline.id, contactId: contacts[10].id, ownerId: james.id },
    // Prism — 1 lead
    { email: 'olivia@prismdesign.co', source: 'event', status: 'qualified', score: 68, companyId: prism.id, contactId: contacts[11].id, ownerId: maria.id },
    // NovaEdge — 1 lead
    { email: 'drusso@novaedge.ai', source: 'outbound', status: 'qualified', score: 88, companyId: novaedge.id, contactId: contacts[12].id, ownerId: david.id },
    // Coastline — 1 lead (disqualified)
    { email: 'mjohnson@coastlinehospitality.com', source: 'website', status: 'disqualified', score: 10, companyId: coastline.id, contactId: contacts[14].id, ownerId: maria.id },
  ]
  const leads = []
  for (const l of leadsData) {
    const r = await create(token, 'Lead', l)
    leads.push(r)
    console.log(`  Lead: ${r.email} (${r.status}, score ${r.score})`)
  }

  // -- Deals (across pipeline stages) --
  const dealsData = [
    // Meridian
    { title: 'Meridian Platform License', value: 45000, stage: 'proposal', companyId: meridian.id, contactId: contacts[0].id, ownerId: james.id },
    { title: 'Meridian Professional Services', value: 50000, stage: 'prospecting', companyId: meridian.id, contactId: contacts[1].id, ownerId: james.id },
    // Atlas
    { title: 'Atlas ERP Integration', value: 240000, stage: 'negotiation', companyId: atlas.id, contactId: contacts[3].id, ownerId: maria.id },
    // Bluebird
    { title: 'Bluebird Patient Portal', value: 180000, stage: 'proposal', companyId: bluebird.id, contactId: contacts[5].id, ownerId: sarah.id },
    // Summit (big deal)
    { title: 'Summit Trading Platform', value: 320000, stage: 'negotiation', companyId: summit.id, contactId: contacts[7].id, ownerId: david.id },
    // Prism
    { title: 'Prism Design System Subscription', value: 45000, stage: 'prospecting', companyId: prism.id, contactId: contacts[11].id, ownerId: maria.id },
    // NovaEdge
    { title: 'NovaEdge ML Pipeline', value: 150000, stage: 'proposal', companyId: novaedge.id, contactId: contacts[12].id, ownerId: david.id },
    // Won deals (historical)
    { title: 'Atlas Pilot Phase', value: 35000, stage: 'won', companyId: atlas.id, contactId: contacts[4].id, ownerId: maria.id },
    { title: 'Bluebird Analytics Add-on', value: 28000, stage: 'won', companyId: bluebird.id, contactId: contacts[6].id, ownerId: sarah.id },
    // Lost deal
    { title: 'Greenline Fleet Tracker', value: 75000, stage: 'lost', companyId: greenline.id, contactId: contacts[9].id, ownerId: james.id },
  ]
  const deals = []
  for (const d of dealsData) {
    const r = await create(token, 'Deal', d)
    deals.push(r)
    console.log(`  Deal: ${r.title} ($${Number(r.value).toLocaleString()}, ${r.stage})`)
  }

  // -- Activities (recent history) --
  const now = new Date()
  const d = (daysAgo, h = 10, m = 0) => new Date(now - daysAgo * 86400000 + h * 3600000 + m * 60000).toISOString()

  const activitiesData = [
    // Meridian — active engagement
    { type: 'email', note: 'Sent product overview deck and pricing sheet', contactId: contacts[0].id, dealId: deals[0].id, ownerId: james.id, occurredAt: d(7, 10, 30) },
    { type: 'call', note: 'Discovery call — Lisa confirmed budget approval for Q3', contactId: contacts[0].id, dealId: deals[0].id, ownerId: james.id, occurredAt: d(5, 14) },
    { type: 'meeting', note: 'Demo session with Lisa and Ryan. Strong interest in API-first approach', contactId: contacts[0].id, dealId: deals[0].id, ownerId: james.id, occurredAt: d(3, 15) },
    { type: 'email', note: 'Initial outreach to Ryan about integration needs', contactId: contacts[1].id, dealId: deals[1].id, ownerId: james.id, occurredAt: d(4, 9, 15) },
    // Atlas — deep negotiation
    { type: 'meeting', note: 'Technical architecture review with Michael and engineering team', contactId: contacts[3].id, dealId: deals[2].id, ownerId: maria.id, occurredAt: d(12, 10) },
    { type: 'call', note: 'Discussed contract terms. Michael wants 3-year commitment for volume discount', contactId: contacts[3].id, dealId: deals[2].id, ownerId: maria.id, occurredAt: d(6, 16, 30) },
    { type: 'email', note: 'Sent revised proposal with 3-year pricing tiers', contactId: contacts[3].id, dealId: deals[2].id, ownerId: maria.id, occurredAt: d(2, 11) },
    { type: 'note', note: 'Jennifer confirmed pilot results exceeded expectations. Converting to full contract', contactId: contacts[4].id, dealId: deals[7].id, ownerId: maria.id, occurredAt: d(60, 14) },
    // Bluebird — proposal stage
    { type: 'call', note: 'Initial qualification call with Dr. Foster. HIPAA compliance is critical requirement', contactId: contacts[5].id, dealId: deals[3].id, ownerId: sarah.id, occurredAt: d(25, 11) },
    { type: 'meeting', note: 'Security review with IT team. Passed preliminary assessment', contactId: contacts[5].id, dealId: deals[3].id, ownerId: sarah.id, occurredAt: d(18, 13) },
    { type: 'email', note: 'Sent HIPAA compliance documentation and SOC2 report', contactId: contacts[5].id, dealId: deals[3].id, ownerId: sarah.id, occurredAt: d(8, 9) },
    // Summit — high-value negotiation
    { type: 'meeting', note: 'Executive briefing with Alexandra and CFO. Budget pre-approved', contactId: contacts[7].id, dealId: deals[4].id, ownerId: david.id, occurredAt: d(20, 10) },
    { type: 'call', note: 'Technical deep-dive on real-time data requirements', contactId: contacts[7].id, dealId: deals[4].id, ownerId: david.id, occurredAt: d(7, 14, 30) },
    { type: 'email', note: 'Shared architecture diagram and SLA proposal', contactId: contacts[7].id, dealId: deals[4].id, ownerId: david.id, occurredAt: d(1, 8, 45) },
    // NovaEdge
    { type: 'call', note: 'Cold outreach to Daniel — showed immediate interest in ML pipeline tooling', contactId: contacts[12].id, dealId: deals[6].id, ownerId: david.id, occurredAt: d(15, 10) },
    { type: 'meeting', note: 'Technical demo with Daniel and Priya. Need GPU cluster integration', contactId: contacts[12].id, dealId: deals[6].id, ownerId: david.id, occurredAt: d(6, 15) },
    // Prism
    { type: 'email', note: 'Follow-up after design conference booth visit', contactId: contacts[11].id, dealId: deals[5].id, ownerId: maria.id, occurredAt: d(8, 10) },
    { type: 'call', note: 'Olivia interested in design system subscription, scheduling demo', contactId: contacts[11].id, dealId: deals[5].id, ownerId: maria.id, occurredAt: d(4, 11, 30) },
    // Greenline (lost)
    { type: 'note', note: 'Deal lost — went with competitor offering local-only deployment', contactId: contacts[9].id, dealId: deals[9].id, ownerId: james.id, occurredAt: d(20, 16) },
    // Coastline
    { type: 'call', note: 'Initial call with Marcus — not a fit for current needs, revisit in Q4', contactId: contacts[14].id, ownerId: maria.id, occurredAt: d(22, 9) },
  ]
  const activities = []
  for (const a of activitiesData) {
    const r = await create(token, 'Activity', a)
    activities.push(r)
    console.log(`  Activity: ${r.type} — ${(a.note || '').slice(0, 50)}...`)
  }

  console.log('\n  Summary:')
  console.log(`    ${users.length} users`)
  console.log(`    ${companies.length} companies`)
  console.log(`    ${contacts.length} contacts`)
  console.log(`    ${leads.length} leads`)
  console.log(`    ${deals.length} deals`)
  console.log(`    ${activities.length} activities`)
  console.log('\n  Done!')
}

seed().catch(e => { console.error('Seed failed:', e.message); process.exit(1) })
