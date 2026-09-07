import type { ModelMetadata } from './types'

/** The embedded table searches one actual scalar field; never assume every model has name. */
export function listSearchField(model: ModelMetadata): string {
  const candidates = model.searchable?.filter(name => model.fields[name] && !['password', 'token'].includes(name)) || []
  return ['title', 'name'].find(name => candidates.includes(name)) || candidates[0] || model.primaryKey
}
