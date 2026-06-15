---
title: 'Metered Command Primitives'
description: Generic platform building blocks for metered, single-use, transactionally-composed commands — an expiring single-use token store, an integer-unit budget ledger, and transactional job enqueue.
---

# Metered Command Primitives

> Status: **implemented** (library primitives in `atomo_server`) · Layer: **Atomo core**
> (`atomo_server::metered` + `atomo_server::jobs`) · Pull trigger: a consumer exposing a *metered*
> command (a credit/billing debit, a rate-limited public command, a metered-API gateway) that needs
> the whole command to commit or roll back as one unit.

A *metered command* is one that must, atomically: reserve some budget, spend a one-time grant
(a single-use token), and start durable work (a job). If any step fails, none must take effect — a
half-applied command that consumes a token without enqueuing work, or charges a budget for a job
that never ran, is a correctness bug.

Atomo provides three composable primitives for this. They own **no business policy**: scopes,
purposes, limits, windows, and units are all caller-supplied opaque values. Nothing here knows what
is being metered, who the caller is, or what the work does.

## The primitives

### 1. Transactional job enqueue — `JobStore::enqueue_tx`

[`JobStore::enqueue`](/api/jobs) runs on its own pooled connection, so it cannot be part of a
larger atomic operation. `enqueue_tx` takes a caller-supplied `&mut PgConnection` (pass `&mut *tx`)
so the enqueue commits or rolls back **with** the caller's other writes:

```rust
let (job_id, created) = job_store
    .enqueue_tx(&mut tx, queue, kind, payload, Some(idem_key), max_attempts, priority, tenant_id)
    .await?;
```

It is idempotent on `(queue, idempotency_key)` exactly like `enqueue`: a second enqueue for the same
key returns the existing id with `created = false`. It **emits no event** — a rolled-back
transaction must not leak a phantom `Job` Created event. After the transaction commits, call
`job_store.emit_enqueued(&job_id, queue, kind)` when `created` is true.

### 2. Expiring single-use token store — `metered::ExpiringTokenStore`

A one-time grant: mint an opaque token now, redeem it exactly once before it expires. Only the
SHA-256 of the token is stored; the plaintext is returned once at mint and is not recoverable from
the database (same discipline as worker tokens).

```rust
// At grant time (e.g. after an upload is accepted):
let token = tokens.mint("upload", &subject, json!({ "assetId": id }), 3600).await?; // plaintext, shown once

// Inside the command transaction — succeeds at most once:
if let Some(grant) = tokens.consume(&mut tx, "upload", &subject, &presented).await? {
    let asset_id = grant.payload; // the opaque payload attached at mint
}
```

- `purpose` namespaces token kinds; `subject` is an opaque caller-chosen scope (e.g. a session
  identifier). Consumption requires both to match.
- Single use is enforced by a single `UPDATE … WHERE consumed_at IS NULL … RETURNING` — concurrent
  redeemers race for one row lock, so exactly one wins.
- `consume` takes `&mut PgConnection` and must run inside the caller's transaction, so a rolled-back
  command leaves the token unspent.
- `purge_expired()` reclaims expired rows; wire it to a scheduler under your retention policy.

### 3. Integer-unit budget ledger — `metered::BudgetLedger`

An append-only ledger of signed integer `amount`s (the unit is the caller's — micro-USD, credits,
tokens) charged against an opaque `scope`. Capacity is the windowed sum over a recent interval.

```rust
// Inside the command transaction — reserve only if it keeps the window within budget:
if ledger.try_reserve(&mut tx, "global:hourly", cost_micros, hourly_limit, 3600).await? {
    // within budget — proceed
}
let spent = ledger.windowed_sum("global:hourly", 3600).await?; // read-only, for reporting
```

`try_reserve` takes a transaction-scoped advisory lock keyed by `scope`, recomputes the windowed
sum, and inserts the reservation only if `windowed_sum + amount <= limit`. The advisory lock
serializes concurrent reservations on the same scope, so two callers can **never both pass a check
that only one budget allows** (verified by a concurrency test). The lock releases on commit/rollback.

## Composing a metered command

The three primitives combine in one transaction so the command is all-or-nothing:

```rust
let mut tx = pool.begin().await?;
if !ledger.try_reserve(&mut tx, &scope, cost, limit, window).await? {
    return /* over budget */;
}
let Some(grant) = tokens.consume(&mut tx, "upload", &subject, &presented).await? else {
    return /* invalid/used token */;
};
let (job_id, created) = job_store
    .enqueue_tx(&mut tx, &queue, &kind, payload(grant.payload), Some(&idem), attempts, 0, tenant)
    .await?;
tx.commit().await?;           // budget, token, and job commit together — or none do
if created { job_store.emit_enqueued(&job_id, &queue, &kind); }
```

The result/cost of the work is reconciled separately when the job completes — workers report
completion through the job lifecycle (`/jobs` complete/fail) and the resulting `Job` event, so a
read of command status causes **no** state change. (Reconciling a metered command's actual cost back
into the ledger via the completion event is a consumer-owned projection.)

## Boundary (what this is not)

- **No business policy.** These primitives do not define what a token grants, what a budget protects,
  or who may call. A consumer composes them with its own schema, routes, and rules.
- **No HTTP surface.** They are library APIs, intentionally not network-exposed: the atomic
  guarantees require composing them inside one transaction with consumer-owned writes, which only a
  same-process or same-database consumer can do. Exposing each primitive as a standalone endpoint
  would break atomicity across the network boundary.
- **Not event-sourced (v1).** The token and ledger tables are operational state, not part of the
  event log. The job remains event-sourced.

## Storage

Two idempotently-created tables (self-initialized at boot like the other operational stores):

- `expiring_tokens (token_sha256 PK, purpose, subject, payload, expires_at, consumed_at, created_at)`
- `budget_ledger (id PK, scope, amount, created_at)` with a `(scope, created_at)` window index

## Configuration

`ATOMO_ENABLE_METERED_COMMANDS` (`ServerConfig::enable_metered_commands`, **default `true`**)
controls whether the two tables are created at boot. Leave it on to make the primitives available;
set it to `false` to skip them on deployments that don't use the feature. There are no other env
vars — scopes, purposes, limits, windows, and units are all passed by the caller at the call site.

## Tests

- Unit (`atomo_server::metered`): token hashing is stable/opaque; advisory-key derivation is
  deterministic and scope-specific.
- Integration (`crates/atomo_server/tests/metered_store.rs`, Postgres-gated): single-use
  consumption; scope/expiry rejection; windowed budget enforcement; **concurrent reservations never
  over-commit**; and **transactional atomicity** — a rolled-back command consumes no token, reserves
  no budget, and enqueues no job, while a committed one does all three.

```bash
DATABASE_URL=postgres:///atomo_test \
  cargo test -p atomo_server --test metered_store -- --ignored --test-threads=1
```
