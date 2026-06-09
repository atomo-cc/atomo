import { createWorker } from '@atomo-cc/worker-sdk'
import type { Actions, TypedWorker } from '../generated/actions'
import { sendEmail } from '../lib/email'
import { domainFromWebsite, inferIndustry } from '../lib/enrichment'
import { scoreLead as scoreLeadValue } from '../lib/scoring'
import { removeContactFromSearchIndex } from '../lib/search'
import { notifySales } from '../lib/slack'

const worker = createWorker<Actions>({
  serverUrl: process.env.ATOMO_SERVER_URL ?? 'http://localhost:8080',
  token: process.env.ATOMO_WORKER_TOKEN ?? '',
}) as unknown as TypedWorker & { start(): Promise<void> }

worker.on('sendWelcomeEmail', async ({ job, crud, logger }) => {
  const input = job.payload.input

  await sendEmail({
    to: input.email,
    subject: 'Welcome to Atomo CRM',
    template: 'welcome-user',
    data: { name: input.displayName },
  })

  await crud.update('User', input.id, {
    welcomeSentAt: new Date().toISOString(),
  }, { origin: 'sendWelcomeEmail' })

  logger.info('Welcome email sent', { userId: input.id, tenantId: input.tenantId })
})

worker.on('enrichCompany', async ({ job, crud, logger }) => {
  const input = job.payload.input
  const domain = domainFromWebsite(input.website)
  const industry = await inferIndustry(input.name, domain)

  await crud.update('Company', input.id, {
    domain,
    industry,
  }, { origin: 'enrichCompany' })

  logger.info('Company enriched', { companyId: input.id, domain, industry })
})

worker.on('scoreLead', async ({ job, crud, logger }) => {
  const input = job.payload.input
  const score = scoreLeadValue({
    email: input.email,
    source: input.source,
    companyId: input.companyId,
  })

  await crud.update('Lead', input.id, { score }, { origin: 'scoreLead' })

  logger.info('Lead scored', { leadId: input.id, score })
})

worker.on('rollupLeadStats', async ({ job, crud, logger }) => {
  const input = job.payload.input
  if (!input.companyId) return

  const leads = await crud.findMany<{ id: string; score: number; status: string }>('Lead', {
    where: { companyId: input.companyId, tenantId: input.tenantId },
    limit: 1000,
  })

  await crud.update('Company', input.companyId, {
    leadCount: leads.length,
  }, { origin: 'rollupLeadStats' })

  logger.info('Lead stats rolled up', { companyId: input.companyId, leadCount: leads.length })
})

worker.on('notifyDealWon', async ({ job, logger }) => {
  const input = job.payload.input

  await notifySales({
    title: input.title,
    value: input.value,
    tenantId: input.tenantId,
  })

  logger.info('Deal won notification sent', { dealId: input.id })
})

worker.on('updateContactLastActivity', async ({ job, crud, logger }) => {
  const input = job.payload.input

  await crud.update('Contact', input.contactId, {
    lastActivityAt: input.occurredAt,
  }, { origin: 'updateContactLastActivity' })

  logger.info('Contact last activity updated', {
    contactId: input.contactId,
    activityId: input.id,
  })
})

worker.on('removeFromSearchIndex', async ({ job, logger }) => {
  const input = job.payload.input

  await removeContactFromSearchIndex({
    id: input.id,
    tenantId: input.tenantId,
  })

  logger.info('Contact removed from search index', { contactId: input.id })
})

worker.on('createLeadAndContact', async ({ job, crud, logger }) => {
  const input = job.payload.input
  const tenantId = job.payload.actor?.tenantId

  const company = await crud.create<{ id: string }>('Company', {
    name: input.companyName,
    tenantId,
  }, { origin: 'createLeadAndContact' })

  const contact = await crud.create<{ id: string }>('Contact', {
    email: input.email,
    firstName: input.firstName,
    lastName: input.lastName,
    companyId: company.id,
    tenantId,
  }, { origin: 'createLeadAndContact' })

  const lead = await crud.create('Lead', {
    email: input.email,
    source: input.source,
    companyId: company.id,
    contactId: contact.id,
    tenantId,
  }, { origin: 'createLeadAndContact' })

  logger.info('Lead/contact/company created via action', { email: input.email })
  return lead
})

await worker.start()
