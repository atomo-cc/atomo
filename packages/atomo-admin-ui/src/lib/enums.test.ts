import { describe, it, expect } from 'vitest'
import { getEnumValues } from './enums'

describe('getEnumValues', () => {
  it('extracts values from an in: rule among other rules', () => {
    expect(
      getEnumValues({ validation: { status: 'required|in:draft,published,archived' } }, 'status'),
    ).toEqual(['draft', 'published', 'archived'])
  })

  it('trims whitespace and drops empties', () => {
    expect(getEnumValues({ validation: { s: 'in: a , b ,' } }, 's')).toEqual(['a', 'b'])
  })

  it('returns undefined without an in: rule, empty rule, or missing validation', () => {
    expect(getEnumValues({ validation: { s: 'required|min:1' } }, 's')).toBeUndefined()
    expect(getEnumValues({ validation: { s: 'in:' } }, 's')).toBeUndefined()
    expect(getEnumValues({ validation: undefined }, 's')).toBeUndefined()
  })
})
