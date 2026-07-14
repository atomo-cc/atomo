import { describe, it, expect } from 'vitest'
import { buildWhere, conditionsToWhere } from './where'
import type { FilterCondition } from '../components/filters/AdvancedFilterPanel'

const cond = (field: string, operator: FilterCondition['operator'], value: any): FilterCondition => ({
  id: `${field}-${operator}`,
  field,
  operator,
  value,
})

describe('conditionsToWhere', () => {
  it('maps every supported operator to the server where vocabulary', () => {
    expect(conditionsToWhere([cond('status', 'equals', 'won')])).toEqual({ status: { equals: 'won' } })
    expect(conditionsToWhere([cond('status', 'not_equals', 'lost')])).toEqual({ status: { not: 'lost' } })
    expect(conditionsToWhere([cond('title', 'contains', 'acme')])).toEqual({ title: { contains: 'acme' } })
    expect(conditionsToWhere([cond('slug', 'starts_with', 'a')])).toEqual({ slug: { startsWith: 'a' } })
    expect(conditionsToWhere([cond('slug', 'ends_with', 'z')])).toEqual({ slug: { endsWith: 'z' } })
    expect(conditionsToWhere([cond('value', 'greater_than', 5)])).toEqual({ value: { gt: 5 } })
    expect(conditionsToWhere([cond('value', 'less_than_or_equal', 9)])).toEqual({ value: { lte: 9 } })
  })

  it('expands between into gte + lte on one field', () => {
    expect(conditionsToWhere([cond('value', 'between', [10, 20])])).toEqual({
      value: { gte: 10, lte: 20 },
    })
  })

  it('maps null checks onto isNull true/false', () => {
    expect(conditionsToWhere([cond('closedAt', 'is_null', undefined)])).toEqual({
      closedAt: { isNull: true },
    })
    expect(conditionsToWhere([cond('ownerId', 'is_not_null', undefined)])).toEqual({
      ownerId: { isNull: false },
    })
  })

  it('coerces comma lists for in/not_in', () => {
    expect(conditionsToWhere([cond('stage', 'in', 'won, lost')])).toEqual({
      stage: { in: ['won', 'lost'] },
    })
  })

  it('merges multiple conditions on the same field', () => {
    expect(
      conditionsToWhere([cond('value', 'greater_than_or_equal', 1), cond('value', 'less_than', 9)]),
    ).toEqual({ value: { gte: 1, lt: 9 } })
  })

  it('drops empty values and empty condition sets', () => {
    expect(conditionsToWhere([cond('title', 'contains', '')])).toEqual({})
  })
})

describe('buildWhere', () => {
  it('merges search into the search field as contains', () => {
    expect(buildWhere({ search: 'ada', searchField: 'email' })).toEqual({
      email: { contains: 'ada' },
    })
  })

  it('search without a searchField adds nothing (no dead search)', () => {
    expect(buildWhere({ search: 'ada' })).toBeUndefined()
  })

  it('combines conditions, simple filters, and search', () => {
    expect(
      buildWhere({
        search: 'ada',
        searchField: 'email',
        conditions: [cond('stage', 'equals', 'won')],
        filters: { ownerId: 'u1' },
      }),
    ).toEqual({
      email: { contains: 'ada' },
      stage: { equals: 'won' },
      ownerId: { contains: 'u1' },
    })
  })

  it('returns undefined when nothing filters', () => {
    expect(buildWhere({})).toBeUndefined()
  })
})
