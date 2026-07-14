/**
 * Cosmetic role gating for admin affordances.
 *
 * The server enforces access rules regardless — this only decides whether to
 * SHOW a mutation control, so a viewer isn't offered buttons that 403. Rules
 * come from /meta/schema as plain strings (see schema_metadata.rs): a
 * pipe-joined role list ("admin|manager"), "authenticated", "public",
 * "system" (server-only writers), or "never". Unknown/absent rules fail OPEN
 * (show the control) so a schema without access config keeps full CRUD UI.
 */

import type { ModelMetadata } from './types'

export type CrudOp = 'create' | 'read' | 'update' | 'delete'

export function canPerform(
  model: Pick<ModelMetadata, 'access'>,
  op: CrudOp,
  role: string | null | undefined,
): boolean {
  const rule = model.access?.[op]
  if (rule === undefined || rule === null || rule === '') return true // no rule → allowed

  const r = rule.toLowerCase()
  if (r === 'never' || r === 'system') return false // server-only paths — hide from every human
  if (r === 'public') return true
  if (r === 'authenticated') return role != null // any signed-in user (admin is always signed in)
  if (r === 'owner') return true // record-level — only the server can decide per row

  // Pipe-joined role list, possibly mixed with owner/authenticated alternatives.
  const parts = r.split('|').map((p) => p.trim())
  if (parts.includes('authenticated') || parts.includes('owner')) return role != null
  return role != null && parts.includes(role.toLowerCase())
}
