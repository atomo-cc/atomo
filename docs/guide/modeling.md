# Modeling & Access

> Define models in `services/<name>/schema.ts`. Atomo parses this to generate backend, GraphQL, and UI.

## Model Example (CRM)
```ts
export interface Contact {
  id: string
  firstName: string
  lastName: string
  email?: string
  phone?: string
  companyId?: string
  tags?: string[]
  createdAt: Date
  updatedAt: Date
}
```

## Metadata
```ts
export const schema = {
  models: {
    Contact: {
      tableName: 'contact',
      primaryKey: 'id',
      searchable: ['firstName','lastName','email'],
      access: { create: 'sales|manager|admin', read: 'authenticated' },
      relationships: { company: { type: 'belongsTo', model: 'Company', foreignKey: 'companyId' } },
      validation: { email: 'email', firstName: 'required|min:1|max:100' },
      ui: { listView: ['firstName','lastName','email','company'] }
    }
  }
}
```

- access: role strings for create/read/update/delete. Also served to the admin UI
  via `/meta/schema`, which hides mutation buttons the signed-in role can't use
  (cosmetic — the server enforces regardless).
- relationships: belongsTo/hasMany with foreign keys.
- validation: simple rules (email, required, min/max, `in:a,b,c`). Builder-DSL
  `select(['a','b'])` fields emit an `in:` rule automatically — the runtime
  validator rejects out-of-set values on every write path, and the admin form
  renders a dropdown of the allowed values instead of free text.
- ui: config for generated admin screens. `listView` declares exactly which columns
  the admin list grid shows, in order — including `createdAt`/`updatedAt` if you list
  them (useful on append-only models like event logs). Served to the admin UI via
  `/meta/schema`. Without a `listView`, the grid defaults to the first six fields.

## Extending built-in tables

Platform tables (currently `users`) can be extended from the schema with extra
columns and constraints — e.g. linking users to an external store account with a
nullable anti-abuse anchor — instead of hand-written SQL migrations:

```ts
export const schema = {
  models: { /* ... */ },
  builtins: {
    users: {
      columns: { storeAccountId: 'TEXT' },
      constraints: ['@@unique([storeAccountId]) WHERE store_account_id IS NOT NULL'],
    },
  },
}
```

This emits idempotent DDL (`ALTER TABLE users ADD COLUMN IF NOT EXISTS ...`,
`CREATE UNIQUE INDEX IF NOT EXISTS ... WHERE ...`) applied at boot right after the
platform tables are ensured. Rules, enforced fail-loud at startup:

- **Whitelist**: only `users` is extendable today — anything else stops boot.
- **Append-only**: you can add columns and constraints, never modify or drop.
- **Nullable columns only**: a `NOT NULL` type is rejected (it would break existing rows).
- Constraint strings use the same `@@unique([..]) WHERE ..` / `@@index([..])` /
  `@@check(..)` annotations as models; predicates are raw SQL over snake_case columns.
