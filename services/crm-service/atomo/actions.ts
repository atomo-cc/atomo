import { action, text } from '@atomo-cc/schema'

// Lifecycle actions: attached under model.on.* and executed asynchronously by workers.
export const sendWelcomeEmail = action('sendWelcomeEmail')
  .from('users')
  .input(['id', 'email', 'displayName', 'tenantId'])

export const enrichCompany = action('enrichCompany')
  .from('companies')
  .input(['id', 'name', 'website', 'tenantId'])

export const scoreLead = action('scoreLead')
  .from('leads')
  .input(['id', 'email', 'source', 'companyId', 'tenantId'])

export const rollupLeadStats = action('rollupLeadStats')
  .from('leads')
  .input(['id', 'companyId', 'status', 'score', 'tenantId'])

export const notifyDealWon = action('notifyDealWon')
  .from('deals')
  .input(['id', 'title', 'value', 'companyId', 'ownerId', 'tenantId'])

export const updateContactLastActivity = action('updateContactLastActivity')
  .from('activities')
  .input(['id', 'contactId', 'type', 'occurredAt', 'tenantId'])

export const removeFromSearchIndex = action('removeFromSearchIndex')
  .from('contacts')
  .input(['id', 'tenantId'])

// User-callable action: exposed as atomo.actions.createLeadAndContact(...).
// It is not attached to lifecycle events because it returns a record.
export const createLeadAndContact = action('createLeadAndContact')
  .input({
    email: text().email().required(),
    firstName: text().required().min(1).max(80),
    lastName: text().required().min(1).max(80),
    companyName: text().required().min(2).max(120),
    source: text().required(),
  })
  .returns('leads')
