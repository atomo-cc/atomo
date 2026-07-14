// E2E smoke schema — purpose-built to guard the consumer-reported escape class
// (admin rendering that contradicts schema truth; feedback #8/#9/#11):
//
//   Article    — admin-creatable, enum via `in:` validation, listView ends createdAt
//   AuditEvent — server-written (create: "system"): the admin must NOT offer
//                quick-create/new for it, and its dashboard card must be titled
//                "Audit Event", never a field name.

export interface Article {
  id: string
  title: string
  status: string
  /** File field seeded with a BARE media-id string (worker-style write) — the
   *  record view must render it, not crash (feedback #12A). */
  coverImage?: File
  createdAt: Date
}

export interface AuditEvent {
  id: string
  event: string
  createdAt: Date
}

export const schema = {
  models: {
    Article: {
      tableName: 'e2e_articles',
      access: { create: 'admin', read: 'authenticated', update: 'admin', delete: 'admin' },
      validation: { title: 'required|min:1|max:200', status: 'required|in:draft,published' },
      ui: { listView: ['title', 'status', 'coverImage', 'createdAt'] },
    },
    AuditEvent: {
      tableName: 'e2e_audit_events',
      access: { create: 'system', read: 'admin', update: 'never', delete: 'never' },
      ui: { listView: ['event', 'createdAt'] },
    },
  },
}

export default schema
