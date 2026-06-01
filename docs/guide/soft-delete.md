---
title: Soft Delete & Trash
description: Delete soft-deletes, restore brings back, hardDelete purges.
---

# Soft Delete & Trash

Deletes in Atomo are **soft** by default: a deleted row is hidden, not removed, so it can be
restored. Every generated table has a `deleted_at` column.

## Lifecycle

```graphql
# Soft-delete (sets deleted_at) — hidden from normal reads
mutation { delete(model: "Deal", where: { id: { equals: "<id>" } }) }

# The trash view: only soft-deleted rows
query { deletedRecords(model: "Deal", limit: 20, offset: 0) { data pageInfo { totalCount } } }

# Restore a soft-deleted row
mutation { restore(model: "Deal", where: { id: { equals: "<id>" } }) }

# Permanently purge (irreversible)
mutation { hardDelete(model: "Deal", where: { id: { equals: "<id>" } }) }
```

## Behavior

- `records` / `paginatedRecords` **exclude** soft-deleted rows; `deletedRecords` shows **only**
  them.
- `delete` and `restore` emit `Deleted` events carrying the row id, so projections and event
  history stay consistent.
- `hardDelete` removes the row from the table entirely (no event-sourced recovery).

## See also
- [GraphQL API](/api/graphql)
- [Event Sourcing](/guide/event-sourcing)
