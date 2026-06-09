import {
  model,
  text,
  number,
  email,
  select,
  datetime,
  relation,
  url,
  allow,
  index,
  unique,
} from '@atomo-cc/schema'

import {
  sendWelcomeEmail,
  enrichCompany,
  scoreLead,
  rollupLeadStats,
  notifyDealWon,
  updateContactLastActivity,
  removeFromSearchIndex,
} from './actions'

export const User = model('users', {
  fields: {
    id: text().id(),
    email: email().required().unique().lowercase().trim(),
    firstName: text().required().min(1).max(80),
    lastName: text().required().min(1).max(80),
    displayName: text().from(['firstName', 'lastName']).concat(' '),
    role: select(['admin', 'manager', 'sales', 'viewer']).default('sales'),
    tenantId: text().fromAuth('tenantId'),
    welcomeSentAt: datetime().optional(),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
  access: {
    read: allow.sameTenant('tenantId'),
    create: allow.authenticated(),
    update: allow.any([allow.owner('id'), allow.role('admin'), allow.role('manager')]),
    delete: allow.role('admin'),
  },
  on: {
    created: [sendWelcomeEmail],
  },
})

export const Company = model('companies', {
  fields: {
    id: text().id(),
    name: text().required().min(2).max(120),
    website: url().optional(),
    domain: text().optional(),
    industry: text().optional(),
    leadCount: number().default(0),
    openDealValue: number().default(0),
    tenantId: text().fromAuth('tenantId'),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
  constraints: [
    unique(['tenantId', 'name']),
    index(['tenantId', 'domain']),
  ],
  access: {
    read: allow.sameTenant('tenantId'),
    create: allow.any([allow.role('admin'), allow.role('manager'), allow.role('sales')]),
    update: allow.any([allow.role('admin'), allow.role('manager'), allow.role('sales')]),
    delete: allow.any([allow.role('admin'), allow.role('manager')]),
  },
  on: {
    created: [enrichCompany],
    updated: [enrichCompany.whenChanged('website', 'name')],
  },
})

export const Contact = model('contacts', {
  fields: {
    id: text().id(),
    firstName: text().required().min(1).max(80),
    lastName: text().required().min(1).max(80),
    displayName: text().from(['firstName', 'lastName']).concat(' '),
    email: email().required().lowercase().trim(),
    phone: text().optional().max(40),
    companyId: relation('companies').optional(),
    ownerId: relation('users').fromAuth('userId'),
    lastActivityAt: datetime().optional(),
    tenantId: text().fromAuth('tenantId'),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
  constraints: [
    unique(['tenantId', 'email']),
    index(['tenantId', 'companyId']),
    index(['tenantId', 'ownerId']),
  ],
  access: {
    read: allow.sameTenant('tenantId'),
    create: allow.any([allow.role('admin'), allow.role('manager'), allow.role('sales')]),
    update: allow.any([allow.owner('ownerId'), allow.role('admin'), allow.role('manager')]),
    delete: allow.any([allow.role('admin'), allow.role('manager')]),
  },
  on: {
    deleted: [removeFromSearchIndex],
  },
})

export const Lead = model('leads', {
  fields: {
    id: text().id(),
    email: email().required().lowercase().trim(),
    source: select(['website', 'referral', 'event', 'import', 'outbound']).default('website'),
    status: select(['new', 'qualified', 'disqualified', 'converted']).default('new'),
    score: number().default(0).min(0).max(100),
    companyId: relation('companies').optional(),
    contactId: relation('contacts').optional(),
    ownerId: relation('users').fromAuth('userId'),
    tenantId: text().fromAuth('tenantId'),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
  constraints: [
    index(['tenantId', 'status']),
    index(['tenantId', 'ownerId']),
  ],
  access: {
    read: allow.sameTenant('tenantId'),
    create: allow.any([allow.role('admin'), allow.role('manager'), allow.role('sales')]),
    update: allow.any([allow.owner('ownerId'), allow.role('admin'), allow.role('manager')]),
    delete: allow.any([allow.role('admin'), allow.role('manager')]),
  },
  on: {
    created: [scoreLead, rollupLeadStats],
    updated: [
      scoreLead.whenChanged('email', 'source', 'companyId'),
      rollupLeadStats.whenChanged('status', 'score', 'companyId'),
    ],
  },
})

export const Deal = model('deals', {
  fields: {
    id: text().id(),
    title: text().required().min(2).max(160),
    value: number().required().min(0),
    stage: select(['prospecting', 'proposal', 'negotiation', 'won', 'lost']).default('prospecting'),
    companyId: relation('companies').required(),
    contactId: relation('contacts').optional(),
    ownerId: relation('users').fromAuth('userId'),
    tenantId: text().fromAuth('tenantId'),
    closedAt: datetime().optional(),
    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },
  constraints: [
    index(['tenantId', 'stage']),
    index(['tenantId', 'ownerId']),
  ],
  access: {
    read: allow.sameTenant('tenantId'),
    create: allow.any([allow.role('admin'), allow.role('manager'), allow.role('sales')]),
    update: allow.any([allow.owner('ownerId'), allow.role('admin'), allow.role('manager')]),
    delete: allow.any([allow.role('admin'), allow.role('manager')]),
  },
  on: {
    updated: [notifyDealWon.when('stage', 'won')],
  },
})

export const Activity = model('activities', {
  fields: {
    id: text().id(),
    type: select(['call', 'email', 'meeting', 'note']).required(),
    note: text().optional().max(4000),
    contactId: relation('contacts').required(),
    dealId: relation('deals').optional(),
    ownerId: relation('users').fromAuth('userId'),
    tenantId: text().fromAuth('tenantId'),
    occurredAt: datetime().defaultNow(),
    createdAt: datetime().defaultNow(),
  },
  constraints: [
    index(['tenantId', 'contactId']),
    index(['tenantId', 'occurredAt']),
  ],
  access: {
    read: allow.sameTenant('tenantId'),
    create: allow.any([allow.role('admin'), allow.role('manager'), allow.role('sales')]),
    update: allow.any([allow.owner('ownerId'), allow.role('admin'), allow.role('manager')]),
    delete: allow.any([allow.role('admin'), allow.role('manager')]),
  },
  on: {
    created: [updateContactLastActivity],
  },
})

export const schema = {
  models: [User, Company, Contact, Lead, Deal, Activity],
  actions: [
    sendWelcomeEmail,
    enrichCompany,
    scoreLead,
    rollupLeadStats,
    notifyDealWon,
    updateContactLastActivity,
    removeFromSearchIndex,
  ],
}
