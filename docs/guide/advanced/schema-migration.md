---
title: 'Schema Migration'
description: How Atomo forward-migrates an existing database to match an evolved schema — what changes apply automatically, and when an explicit backfill is required.
---

# Schema Migration

Atomo generates migrations from your `schema.ts` and applies them idempotently at startup. The
generator emits `CREATE TABLE IF NOT EXISTS` for new models and a set of guarded `ALTER`/`CREATE
INDEX` statements that bring a database created from an **older** schema up to the current one. This
page describes exactly what forward migration does and does not do automatically.

## What migrates automatically

Re-running migrations against an existing database is safe and idempotent. For each model:

- **New optional field** → added as a nullable column. Existing rows get `NULL`.
- **New field with a default** → added with its `DEFAULT`, which **backfills existing rows**. This
  includes a schema-declared `.default(..)`, timestamps (`created_at`/`updated_at` → `NOW()`), and
  JSON array/block fields (→ `'[]'`).
- **New required field that has a default** → safe on a populated table, because the default
  backfills existing rows. Added with `DEFAULT … NOT NULL`.
- **Defaults, uniqueness, and indexes are reconciled** on existing tables:
  - A declared `.default(..)` is applied to the column (same clause for new tables and forward
    adds).
  - A `@unique` field becomes a `CREATE UNIQUE INDEX IF NOT EXISTS uq_<table>_<col>` — emitted as an
    index (not an inline column constraint) so it applies to tables created before the column was
    unique.
  - An `@index` field becomes a `CREATE INDEX IF NOT EXISTS idx_<table>_<col>`.
  - Model-level `@@unique`, `@@index`, and `@@check` constraints, and `belongsTo` foreign keys, are
    emitted as guarded, idempotent statements.

## What requires an explicit backfill

Adding a **required field with no default** to a **populated** table cannot be done automatically —
there is no value to give existing rows. Rather than emit a bare `ADD COLUMN … NOT NULL` that fails
at startup with an opaque constraint error, Atomo emits a guarded statement that:

- adds the column when the table is **empty**, and
- otherwise **fails with an actionable message**:

```text
atomo: cannot add required column <table>.<col> with no default to a populated table;
declare a .default(...) on the field, or write an explicit backfill migration
(add it nullable, backfill, then SET NOT NULL)
```

You have two ways forward:

1. **Give the field a default** in the schema (`.default(..)`) — the migration then backfills
   existing rows and the column is added safely.
2. **Write an explicit backfill migration** when no single default is correct:

   ```sql
   ALTER TABLE <table> ADD COLUMN <col> <type>;          -- nullable first
   UPDATE <table> SET <col> = ... WHERE <col> IS NULL;   -- backfill with real values
   ALTER TABLE <table> ALTER COLUMN <col> SET NOT NULL;  -- then enforce
   ```

## Limits (v1)

Forward migration is **additive**. It does not drop or rename columns, narrow types, or remove
constraints — those need an explicit migration so data loss is always a deliberate choice.

Current v1 limitations:

- **Enum types** are not yet forward-migrated. Adding a variant to an existing Postgres `ENUM` type
  requires a manual `ALTER TYPE ... ADD VALUE` migration.
- The builder DSL currently captures field-level `.default('...')` (string values) and not
  field-level `.unique()`/`.index()`; declare uniqueness/indexes via model-level `@@unique`/`@@index`
  (or the typescript-interface schema) until field-level parsing lands.

## Testing

Real-PostgreSQL evolution tests live in `crates/atomo/tests/schema_evolution.rs` (DB-gated):

```bash
DATABASE_URL=postgres:///atomo_test \
  cargo test -p atomo --test schema_evolution -- --ignored --test-threads=1
```

They prove optional/defaulted adds (with backfill), uniqueness enforcement via the reconciled index,
idempotent re-application, and the actionable failure for a required-no-default add on a populated
table.
