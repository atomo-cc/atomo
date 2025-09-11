# Access & Hooks

Define and enforce fine-grained business rules alongside your schema.

Access control patterns
- RBAC: role-based checks (e.g., `Admin | Manager | Sales | Support | Viewer`).
- ABAC: attribute-based conditions (owner, team, region, tenant).
- Field-level: restrict columns per role or condition.

Schema sketch
```ts
export type Role = 'Admin' | 'Manager' | 'Sales' | 'Support' | 'Viewer'

export const access = {
  Contact: {
    read: ({ role, userId }, row) => (
      role === 'Admin' || row.ownerId === userId
        ? ['id','name','email','company']
        : ['id','name']
    ),
    update: ({ role, team }, row) => (
      role === 'Admin' || (role === 'Manager' && row.teamId === team.id)
    ),
  }
}
```

Lifecycle hooks
- Before/after create/update/delete hooks with typed inputs/outputs.
- Validate invariants, normalize payloads, emit custom events.

Hook sketch
```ts
export const hooks = {
  Contact: {
    beforeCreate: async ({ input, ctx }) => {
      if (!input.email?.includes('@')) throw new Error('invalid email')
      return { ...input, createdBy: ctx.user.id }
    },
    afterCreate: async ({ entity, emit }) => {
      await emit('ContactCreated', { id: entity.id })
    }
  }
}
```

Enforcement and guarantees
- Rules compile into resolvers; checks run at mutation and selection time.
- Prefer deny-by-default; explicitly list readable fields.
- Keep hooks idempotent where possible; avoid long blocking work (use projectors/jobs).

Testing
- Unit-test hooks with fixtures and a mock `ctx`.
- Add integration tests for access matrices (roles × operations × fields).

See also
- Vision → TypeScript DSLs and Access: `/vision`
- Guide → Modeling & Access: `/guide/modeling`
