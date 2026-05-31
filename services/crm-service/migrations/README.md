# CRM service migrations

These `.sql` files are the **applied migration history** for the CRM service database. The
`atomo migrate` command applies them in order and records each in the `_atomo_migrations`
tracking table (by filename + checksum). They are **not** regenerated from `schema.ts`.

## Status: stale relative to current codegen (do not hand-edit, do not delete)

The earliest files (notably `20250830_082603_initial_crm_schema.sql`) were produced by an
**older codegen** and have since drifted from what the platform now generates from
`schema.ts`. Known differences:

- `version INTEGER` columns + per-table indexes the current generator does not emit
- **no `deleted_at`** column (current codegen adds it for soft deletes)
- **no `tenant_id`** column (added by current codegen for multi-tenant scoping)
- enum-as-table junk (e.g. `companysize` with `_enum_value_0..5`)
- `UUID` ids on block tables vs `TEXT` on core models

## Why they are kept (not deleted)

They are tracked, applied history. Deleting them would orphan the `_atomo_migrations` records
and break reproducibility for any database that already applied them. History is append-only.

## Forward path

Generate **new** migrations rather than editing these: `atomo generate-migration <name>` diffs
the current `schema.ts` against the live database and writes a forward migration (this is how
`deleted_at` / `tenant_id` / corrected columns get added to an existing CRM DB). For a brand-new
database, the platform's boot-time `enable_migrations` path creates tables directly from
`schema.ts` via the corrected generator — so fresh installs are already correct.

See `docs/guide/advanced/crm-conformance-plan.md` (A1) for the full drift assessment.
