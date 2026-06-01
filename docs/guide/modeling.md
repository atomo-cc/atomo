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

- access: role strings for create/read/update/delete.
- relationships: belongsTo/hasMany with foreign keys.
- validation: simple rules (email, required, min/max).
- ui: hints for generated admin screens.
