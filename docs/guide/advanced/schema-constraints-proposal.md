---
title: "Proposal: Constraint Expressions in the Schema DSL"
description: Declare unique / check / index / default constraints in schema.ts so Atomo's generated migrations enforce data integrity — instead of hand-ALTERing the generated tables.
---

# Proposal: Constraint Expressions in the Schema DSL

> Status: **Phase 2 + most of phase 3 implemented** (`unique`/`index`/`check`
> shipped; `default` and reconciliation still RFC) · Layer: **Atomo core**
> (`atomo_schema` + migrations) · Pull trigger: a real consumer that today
> `ALTER`s Atomo's generated tables to add integrity it can't express (e.g.
> a credit/billing ledger).
>
> Extend the schema DSL so `schema.ts` can declare **unique**, **check**,
> **index**, and **default** constraints. Atomo's migration generator emits them,
> so integrity is *defined with the schema* — not bolted on by manually `ALTER`-ing
> the tables Atomo generated.

## Where it fits

`schema.ts` is Atomo's source of truth: `atomo_schema` parses it (SWC) and
generates tables, GraphQL, and the admin UI, and `atomo-server` auto-migrates on
boot (`CREATE TABLE IF NOT EXISTS` + `ALTER` diffs). The DSL already expresses
models, fields, types, relationships, and access rules.

What it **can't** express today: `UNIQUE` (single or composite), `CHECK`,
indexes, and column defaults. So anything needing real integrity has to reach
*around* the DSL and `ALTER` the generated tables by hand.

## Why it's a real gap

Hand-`ALTER`ing Atomo's generated tables has two concrete costs:

1. **It fights the migrator.** Atomo re-parses `schema.ts` and runs migration
   diffs on every boot (and on hot-reload). Constraints you added out-of-band
   live *outside* Atomo's model of the schema, so a future column diff Atomo
   emits can collide with your `CHECK`/`UNIQUE`.
2. **It's invisible and unportable.** The integrity rule isn't in `schema.ts`, so
   it isn't reviewed, diffed, regenerated, or carried to another environment with
   the schema. Every consumer re-invents the same out-of-band SQL.

A declarative `UNIQUE`/`CHECK`/index/default in the schema removes the most common
reasons to ever touch the generated tables directly.

## Capability

Express constraints alongside the model. Atomo already parses block-style
metadata from `schema.ts` (access / relationship blocks), so a `constraints`
block is the same mechanism. Illustrative (exact syntax is an open question):

```ts
export interface CreditLedger {
  id: string
  accountId: string
  idempotencyKey: string
  amount: number
  createdAt: Date
}

// @atomo constraints CreditLedger
//   unique: [accountId, idempotencyKey]   // idempotent debits
//   check:  amount <> 0
//   index:  [accountId, createdAt]
//   default: createdAt = now()
```

The migrator turns these into `UNIQUE (...)`, `CHECK (...)`, `CREATE INDEX`, and
`DEFAULT` clauses in the generated DDL, and reconciles them on schema change
(add/drop), the same way it diffs columns today.

## The boundary (most important rule)

- **Declarative integrity only** — `unique`, `check`, `index`, `default` (and
  composites). These are things a schema *should* own.
- **Out of scope: stored procedures / triggers / arbitrary PL/pgSQL.** That isn't
  declarative schema; it's logic. Logic belongs in a
  [custom route](/guide/advanced/custom-routes-proposal) (where you can run a
  transaction) or a tracked migration escape hatch — not in the model DSL. So
  a consumer's `try_debit` *function* doesn't move here; its `UNIQUE`/`CHECK` do.
- **Migrations stay generated + tracked.** Constraints flow through the same
  diff/apply path as columns, so there's one migration story, not two.

## Design principles

- **Declarative, reviewable, regenerated** — integrity lives next to the data it
  protects and travels with the schema.
- **Safe apply.** Adding a `CHECK`/`UNIQUE` to a table with violating rows must
  *fail loudly* (report the conflict), never silently skip or corrupt.
- **Additive-first.** Ship the low-risk, high-value pieces (`unique`, `index`)
  before the trickier ones (`check`, drop-reconciliation).

## Existing scaffolding

Half-parsed, not applied. The schema parser already recognises
`FieldAttribute::Unique` and `FieldAttribute::Index` (`atomo_schema`'s
`typescript_parser.rs`), but the table generator (`atomo/src/schema.rs`) only
consumes `Primary` — so `unique`/`index` are **parsed and silently dropped**,
never reaching the `CREATE TABLE` DDL. `check`/`default` expressions and
composites aren't modelled at all. Building this = emit DDL from the
already-parsed attributes (phase 2), then add the rest.

## Using it

Annotate fields and models with comments in `schema.ts` (reuses the same comment
machinery as access blocks):

```ts
interface Ledger {
  id: string;
  userId: string;        // @index
  receiptId: string;     // @unique
  balance: number;
  // @@check(balance >= 0)
  // @@unique([userId, receiptId])
  // @@index([userId, createdAt])
}
```

On boot, `generate_migrations` emits the matching DDL (idempotent — safe to re-run):
`UNIQUE` on the column, `CREATE [UNIQUE] INDEX IF NOT EXISTS idx_…/uq_…`, and a
guarded `ALTER TABLE … ADD CONSTRAINT chk_… CHECK (…)` (wrapped in a
`pg_constraint` existence check so re-applying is a no-op).

## Phasing

1. ✅ RFC + this doc.
2. ✅ **`unique` + `index`** (single & composite) → generated DDL. *Shipped:*
   `// @unique` / `// @index` (field), `// @@unique([…])` / `// @@index([…])`
   (model).
3. ⏳ **`check` + `default`** — `// @@check(expr)` **shipped** (guarded so existing
   data isn't silently invalidated mid-migration; a violation surfaces as the
   Postgres error). `default` not yet implemented.
4. **Reconciliation** — drop/alter constraints when the schema changes — plus a
   documented, *tracked* raw-SQL escape hatch (a `migrations/` file Atomo applies
   and records) for the rare thing the DSL still can't express, so nobody
   hand-`ALTER`s blind again.

## Open questions

- **Syntax**: annotation/comment block (matches today's access-block parsing) vs
  TS decorators vs a sibling `schema.constraints.ts`. The comment block reuses
  existing machinery; decorators are more discoverable.
- **`check` expression language**: a constrained mini-DSL Atomo validates, or
  pass-through SQL (more power, less portability/safety)?
- **Conflicting existing data** on first apply: fail the migration with the
  offending rows, or apply `NOT VALID` then validate?
- **Partial / conditional indexes and constraints** — v1 or later?
