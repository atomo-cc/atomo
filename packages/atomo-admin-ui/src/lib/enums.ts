/**
 * Enum values for a field, derived from its `in:a,b,c` validation rule (the
 * canonical source — emitted by `select()` in the builder DSL or written
 * directly in metadata-style schemas). Shared by the record form (dropdown
 * input) and the filter panel (enum operators + value picker) so the two
 * surfaces can never disagree about what an enum field is.
 */

import type { ModelMetadata } from './types'

export function getEnumValues(
  modelMetadata: Pick<ModelMetadata, 'validation'>,
  fieldName: string,
): string[] | undefined {
  const inRule = modelMetadata.validation?.[fieldName]
    ?.split('|')
    .find((r) => r.startsWith('in:'))
  if (!inRule) return undefined
  const values = inRule
    .slice(3)
    .split(',')
    .map((v) => v.trim())
    .filter(Boolean)
  return values.length > 0 ? values : undefined
}
