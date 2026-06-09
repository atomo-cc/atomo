import {
  model,
  text,
  email,
  url,
  number,
  select,
  datetime,
  relation,
  allow,
  action,
} from '@atomo/schema'

export const onNewContact = action('onNewContact')
  .from('contacts')
  .input(['id', 'name', 'email'])

export const onStageChange = action('onStageChange')
  .from('contacts')
  .input(['id', 'stage'])

export const onDealStatusChange = action('onDealStatusChange')
  .from('deals')
  .input(['id', 'contactId', 'status', 'value'])

export const Company = model('companies', {
  fields: {
    id: text().id(),

    name: text()
      .required()
      .min(1),

    industry: text().optional(),

    website: url().optional(),

    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },

  access: {
    create: allow.role(['sales', 'admin']),
    read: allow.authenticated(),
    update: allow.role(['sales', 'admin']),
    delete: allow.role('admin'),
  },
})

export const Contact = model('contacts', {
  fields: {
    id: text().id(),

    name: text()
      .required()
      .min(1)
      .max(100),

    email: email()
      .required(),

    phone: text().optional(),

    stage: select(['lead', 'qualified', 'customer'])
      .default('lead'),

    companyId: relation('companies').optional(),

    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },

  access: {
    create: allow.role(['sales', 'admin']),
    read: allow.authenticated(),
    update: allow.role(['sales', 'admin']),
    delete: allow.role('admin'),
  },

  on: {
    created: [onNewContact],

    updated: [
      onStageChange.whenChanged('stage'),
    ],
  },
})

export const Deal = model('deals', {
  fields: {
    id: text().id(),

    title: text()
      .required()
      .min(1)
      .max(160),

    value: number()
      .min(0),

    status: select(['open', 'won', 'lost'])
      .default('open'),

    contactId: relation('contacts').required(),

    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },

  access: {
    create: allow.role(['sales', 'admin']),
    read: allow.authenticated(),
    update: allow.role(['sales', 'admin']),
    delete: allow.role('admin'),
  },

  on: {
    updated: [
      onDealStatusChange.whenChanged('status'),
    ],
  },
})

export const Activity = model('activities', {
  fields: {
    id: text().id(),

    contactId: relation('contacts').required(),
    dealId: relation('deals').optional(),

    type: select(['call', 'email', 'meeting'])
      .required(),

    notes: text().optional(),

    createdAt: datetime().defaultNow(),
    updatedAt: datetime().autoUpdate(),
  },

  access: {
    create: allow.role(['sales', 'admin']),
    read: allow.authenticated(),
    update: allow.role(['sales', 'admin']),
    delete: allow.role('admin'),
  },
})
