/**
 * Translate UI filter conditions + search into the server's GraphQL `where` JSON.
 *
 * The server (atomo::graphql::parse_where) supports, per field:
 *   equals, not, contains, startsWith, endsWith, gt, gte, lt, lte, in, notIn,
 *   isNull (true → IS NULL, false → IS NOT NULL)
 * Clauses are AND-combined; there is no OR. Operators the server can't express
 * must not be offered in the UI — a dead filter is worse than a missing one.
 */

import type { FilterCondition, FilterOperator } from '../components/filters/AdvancedFilterPanel'

/** UI operator → server where-op. `between` expands to gte+lte. */
const OPERATOR_MAP: Partial<Record<FilterOperator, string>> = {
  equals: 'equals',
  not_equals: 'not',
  contains: 'contains',
  starts_with: 'startsWith',
  ends_with: 'endsWith',
  greater_than: 'gt',
  greater_than_or_equal: 'gte',
  less_than: 'lt',
  less_than_or_equal: 'lte',
  in: 'in',
  not_in: 'notIn',
}

/** Operators the server can express — the filter panel only offers these. */
export const SUPPORTED_OPERATORS: FilterOperator[] = [
  ...(Object.keys(OPERATOR_MAP) as FilterOperator[]),
  'between',
  'is_null',
  'is_not_null',
]

function coerceListValue(value: any): any[] {
  if (Array.isArray(value)) return value
  return String(value ?? '')
    .split(',')
    .map((v) => v.trim())
    .filter(Boolean)
}

/**
 * Build the `where` JSON from advanced-filter conditions. Multiple conditions on
 * the same field merge into one op object ({gte: a, lte: b}); a later duplicate
 * of the same op on the same field wins (the server ANDs everything).
 */
export function conditionsToWhere(conditions: FilterCondition[]): Record<string, any> {
  const where: Record<string, any> = {}
  const opsFor = (field: string) => (where[field] = where[field] ?? {})

  for (const c of conditions) {
    if (!c.field) continue
    switch (c.operator) {
      case 'between': {
        const [from, to] = Array.isArray(c.value) ? c.value : [c.value?.from, c.value?.to]
        if (from !== undefined && from !== '') opsFor(c.field).gte = from
        if (to !== undefined && to !== '') opsFor(c.field).lte = to
        break
      }
      case 'is_null':
        opsFor(c.field).isNull = true
        break
      case 'is_not_null':
        opsFor(c.field).isNull = false
        break
      default: {
        const op = OPERATOR_MAP[c.operator]
        if (!op) continue // unsupported — never silently ignore user intent in the UI
        const value = op === 'in' || op === 'notIn' ? coerceListValue(c.value) : c.value
        if (value === undefined || value === '') continue
        opsFor(c.field)[op] = value
      }
    }
  }

  // Drop fields whose conditions all collapsed to nothing.
  for (const key of Object.keys(where)) {
    if (Object.keys(where[key]).length === 0) delete where[key]
  }
  return where
}

/**
 * Merge search + advanced filters + legacy simple filters into one `where` JSON.
 * `searchField` is the single field the search box targets (the server has no OR,
 * so multi-field search is not expressible — the input's placeholder names the field).
 */
export function buildWhere(opts: {
  search?: string
  searchField?: string
  conditions?: FilterCondition[]
  filters?: Record<string, any>
}): Record<string, any> | undefined {
  const where: Record<string, any> = conditionsToWhere(opts.conditions ?? [])

  for (const [key, value] of Object.entries(opts.filters ?? {})) {
    if (value !== undefined && value !== '') {
      where[key] = { ...(where[key] ?? {}), ...(typeof value === 'string' ? { contains: value } : { equals: value }) }
    }
  }

  if (opts.search && opts.searchField) {
    where[opts.searchField] = { ...(where[opts.searchField] ?? {}), contains: opts.search }
  }

  return Object.keys(where).length ? where : undefined
}
