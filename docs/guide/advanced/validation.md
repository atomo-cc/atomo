---
title: Validation Rules
description: Declarative field validation enforced in the data layer on create and update.
---

# Validation Rules

Declare per-field validation on a model and Atomo enforces it in the **data layer** — on every
create and update path, not just through GraphQL.

## Declaring rules

Add a `validation` map to the model (field → pipe-separated rules):

```ts
export const schema = {
  models: {
    Contact: {
      tableName: 'contact',
      validation: {
        email:     'email',
        firstName: 'required|min:1|max:100',
        lastName:  'max:100',
      },
      // ...
    },
    Deal: {
      validation: {
        title:     'required|min:1|max:255',
        value:     'numeric|min:0',
        contactId: 'required|exists:contact,id',
      },
    },
  },
}
```

## Supported rules

| Rule | Checks |
|---|---|
| `required` | value present and non-empty |
| `email` | looks like an email (contains `@` and `.`) |
| `min:N` | string length ≥ N, or number ≥ N |
| `max:N` | string length ≤ N, or number ≤ N |
| `numeric` | value is a number (or numeric string) |
| `url` | starts with `http://` or `https://` |
| `in:a,b,c` | value is one of the listed options |
| `exists:<table>,<col>` | **no-op in the validator** — see note below |

A field with no rule is unconstrained. Rules combine with `|` (all must pass).

## Create vs. update (update-aware)

- On **create**, all declared rules are checked.
- On **update**, only rules for fields **present in the patch** are checked — so a partial update
  (e.g. just `{ stage }`) does not fail on a `required` field it isn't touching, but a field it
  *does* set must still satisfy its rules (e.g. setting `title: ""` is rejected).

A failing rule aborts the operation with a `VALIDATION_ERROR`.

## Referential integrity (`exists:` and foreign keys)

`exists:<table>,<col>` is intentionally a **no-op** in the synchronous validator (it can't run a
DB query). Referential integrity is enforced by the **database instead**: Atomo generates
`FOREIGN KEY` constraints for each `belongsTo` relationship, so creating a `Deal` with a
`contactId` that doesn't reference a real `contact` row is rejected by Postgres. Declare the
relationship in the model's `relationships` block and the FK is emitted automatically.

## See also
- [Modeling & Access](/guide/modeling)
- [Access Control (RBAC)](/guide/advanced/access-hooks)
