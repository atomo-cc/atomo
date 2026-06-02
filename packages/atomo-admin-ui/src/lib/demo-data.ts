import type { EntityData, SchemaMetadata } from './types'

const now = '2026-05-31T00:00:00.000Z'

export const demoSchemaMetadata: SchemaMetadata = {
  models: {
    Company: {
      tableName: 'companies',
      primaryKey: 'id',
      searchable: ['name', 'industry', 'website'],
      fields: {
        id: { name: 'id', type: 'string', optional: false, attributes: ['primary', 'readonly'] },
        name: { name: 'name', type: 'string', optional: false, attributes: ['required'], ui: { label: 'Company', placeholder: 'Company name' } },
        website: { name: 'website', type: 'url', optional: true, attributes: [], ui: { label: 'Website' } },
        industry: { name: 'industry', type: 'string', optional: true, attributes: [], ui: { label: 'Industry' } },
        size: { name: 'size', type: 'string', optional: true, attributes: [], ui: { label: 'Size' } },
        address: { name: 'address', type: 'text', optional: true, attributes: [], ui: { label: 'Address' } },
        notes: { name: 'notes', type: 'blocks', optional: false, attributes: [], ui: { label: 'Notes' } },
        createdAt: { name: 'createdAt', type: 'datetime', optional: false, attributes: ['readonly'] },
        updatedAt: { name: 'updatedAt', type: 'datetime', optional: false, attributes: ['readonly'] },
      },
      ui: {
        displayField: 'name',
        listView: ['name', 'industry', 'size', 'website'],
        editForm: ['name', 'website', 'industry', 'size', 'address', 'notes'],
        searchFields: ['name', 'industry', 'website'],
      },
    },
    Contact: {
      tableName: 'contacts',
      primaryKey: 'id',
      searchable: ['firstName', 'lastName', 'email'],
      fields: {
        id: { name: 'id', type: 'string', optional: false, attributes: ['primary', 'readonly'] },
        firstName: { name: 'firstName', type: 'string', optional: false, attributes: ['required'], ui: { label: 'First name' } },
        lastName: { name: 'lastName', type: 'string', optional: false, attributes: ['required'], ui: { label: 'Last name' } },
        email: { name: 'email', type: 'email', optional: false, attributes: ['unique', 'required'], ui: { label: 'Email' } },
        phone: { name: 'phone', type: 'string', optional: true, attributes: [], ui: { label: 'Phone' } },
        avatar: { name: 'avatar', type: 'file', optional: true, attributes: [], ui: { label: 'Avatar' } },
        companyId: { name: 'companyId', type: 'string', optional: true, attributes: [], ui: { label: 'Company' } },
        tags: { name: 'tags', type: 'array', optional: false, attributes: [], ui: { label: 'Tags' } },
        notes: { name: 'notes', type: 'blocks', optional: false, attributes: [], ui: { label: 'Notes' } },
        createdAt: { name: 'createdAt', type: 'datetime', optional: false, attributes: ['readonly'] },
        updatedAt: { name: 'updatedAt', type: 'datetime', optional: false, attributes: ['readonly'] },
      },
      ui: {
        displayField: ['firstName', 'lastName'],
        listView: ['firstName', 'lastName', 'email', 'phone', 'companyId'],
        editForm: ['firstName', 'lastName', 'email', 'phone', 'avatar', 'companyId', 'tags', 'notes'],
        searchFields: ['firstName', 'lastName', 'email'],
      },
    },
    Deal: {
      tableName: 'deals',
      primaryKey: 'id',
      searchable: ['title'],
      fields: {
        id: { name: 'id', type: 'string', optional: false, attributes: ['primary', 'readonly'] },
        title: { name: 'title', type: 'string', optional: false, attributes: ['required'], ui: { label: 'Deal' } },
        value: { name: 'value', type: 'number', optional: false, attributes: [], ui: { label: 'Value' } },
        stage: { name: 'stage', type: 'string', optional: false, attributes: [], ui: { label: 'Stage' } },
        position: { name: 'position', type: 'number', optional: false, attributes: [], ui: { label: 'Position' } },
        contactId: { name: 'contactId', type: 'string', optional: false, attributes: [], ui: { label: 'Contact' } },
        companyId: { name: 'companyId', type: 'string', optional: true, attributes: [], ui: { label: 'Company' } },
        expectedCloseDate: { name: 'expectedCloseDate', type: 'date', optional: true, attributes: [], ui: { label: 'Expected close' } },
        description: { name: 'description', type: 'blocks', optional: false, attributes: [], ui: { label: 'Description' } },
        createdAt: { name: 'createdAt', type: 'datetime', optional: false, attributes: ['readonly'] },
        updatedAt: { name: 'updatedAt', type: 'datetime', optional: false, attributes: ['readonly'] },
      },
      ui: {
        displayField: 'title',
        listView: ['title', 'value', 'stage', 'contactId', 'companyId'],
        editForm: ['title', 'value', 'stage', 'position', 'contactId', 'companyId', 'expectedCloseDate', 'description'],
        searchFields: ['title'],
      },
    },
    Activity: {
      tableName: 'activities',
      primaryKey: 'id',
      searchable: ['title', 'content'],
      fields: {
        id: { name: 'id', type: 'string', optional: false, attributes: ['primary', 'readonly'] },
        contactId: { name: 'contactId', type: 'string', optional: false, attributes: [], ui: { label: 'Contact' } },
        activityType: { name: 'activityType', type: 'string', optional: false, attributes: [], ui: { label: 'Type' } },
        title: { name: 'title', type: 'string', optional: true, attributes: [], ui: { label: 'Title' } },
        content: { name: 'content', type: 'text', optional: true, attributes: [], ui: { label: 'Content' } },
        metadata: { name: 'metadata', type: 'json', optional: true, attributes: [], ui: { label: 'Metadata' } },
        createdAt: { name: 'createdAt', type: 'datetime', optional: false, attributes: ['readonly'] },
        updatedAt: { name: 'updatedAt', type: 'datetime', optional: false, attributes: ['readonly'] },
      },
      ui: {
        displayField: 'title',
        listView: ['activityType', 'title', 'contactId', 'createdAt'],
        editForm: ['contactId', 'activityType', 'title', 'content', 'metadata'],
        searchFields: ['title', 'content'],
      },
    },
  },
  config: {
    auditLog: true,
    softDeletes: true,
    defaultPageSize: 20,
    subscriptions: false,
  },
}

export const demoEntities: Record<string, EntityData[]> = {
  Company: [
    { id: 'company_acme', name: 'Acme Corp', website: 'https://acme.example', industry: 'Manufacturing', size: 'enterprise', address: '100 Market St', notes: [], createdAt: now, updatedAt: now },
    { id: 'company_nova', name: 'Nova Labs', website: 'https://nova.example', industry: 'Software', size: 'startup', address: '22 Innovation Way', notes: [], createdAt: now, updatedAt: now },
  ],
  Contact: [
    { id: 'contact_ava', firstName: 'Ava', lastName: 'Chen', email: 'ava.chen@acme.example', phone: '+14155550100', companyId: 'company_acme', tags: ['buyer', 'enterprise'], notes: [{ id: 'note_1', type: 'ParagraphBlock', text: 'Interested in a Q3 rollout.', createdAt: now }], createdAt: now, updatedAt: now },
    { id: 'contact_mateo', firstName: 'Mateo', lastName: 'Rivera', email: 'mateo@nova.example', phone: '+14155550101', companyId: 'company_nova', tags: ['technical'], notes: [], createdAt: now, updatedAt: now },
  ],
  Deal: [
    { id: 'deal_discovery', title: 'Acme discovery', value: 45000, stage: 'lead', position: 0, contactId: 'contact_ava', companyId: 'company_acme', expectedCloseDate: '2026-07-15', description: [], createdAt: now, updatedAt: now },
    { id: 'deal_platform', title: 'Nova platform rollout', value: 82000, stage: 'proposal', position: 0, contactId: 'contact_mateo', companyId: 'company_nova', expectedCloseDate: '2026-08-01', description: [], createdAt: now, updatedAt: now },
    { id: 'deal_expansion', title: 'Acme expansion', value: 120000, stage: 'negotiation', position: 0, contactId: 'contact_ava', companyId: 'company_acme', expectedCloseDate: '2026-06-30', description: [], createdAt: now, updatedAt: now },
  ],
  Activity: [
    { id: 'activity_1', contactId: 'contact_ava', activityType: 'call', title: 'Discovery call', content: 'Confirmed budget owner and success criteria.', metadata: { durationMinutes: 30 }, createdAt: now, updatedAt: now },
    { id: 'activity_2', contactId: 'contact_mateo', activityType: 'meeting', title: 'Technical review', content: 'Reviewed integration requirements.', metadata: {}, createdAt: now, updatedAt: now },
  ],
}

export function cloneDemoEntities(modelName: string): EntityData[] {
  return (demoEntities[modelName] || []).map((entity) => ({ ...entity }))
}
