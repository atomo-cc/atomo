import { expect, it } from 'vitest'
import { listSearchField } from './list-policy'
import type { ModelMetadata } from './types'
it('selects a real title ahead of status and never invents a name field', () => {
  const model = { primaryKey: 'id', searchable: ['status', 'title'], fields: { id: {}, status: {}, title: {} } } as ModelMetadata
  expect(listSearchField(model)).toBe('title')
  expect(listSearchField({ ...model, searchable: ['status'] })).toBe('status')
  expect(listSearchField({ ...model, searchable: [] })).toBe('id')
})
