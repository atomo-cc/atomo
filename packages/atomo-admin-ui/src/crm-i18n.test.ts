import { describe, it, expect } from 'vitest'
import i18n, { en } from '../../../services/crm-service/admin-ui/i18n'

describe('crm i18n', () => {
  it('defaults to English', () => {
    expect(i18n.language).toBe('en')
  })

  it('resolves keys to English strings', () => {
    expect(i18n.t('deals.title')).toBe(en.translation.deals.title)
    expect(i18n.t('timeline.addNote')).toBe('Add Note')
  })
})
