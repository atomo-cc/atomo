---
title: "Design: Custom Routes Phase 3 — Synchronous Transactional DB"
description: How a plugin route handler can do an atomic read-modify-write in one transaction (e.g. a no-overdraw debit) and return the result in the same request — the one feature that lets a billing sidecar collapse into atomo.
---

# Design: Custom Routes Phase 3 — Synchronous Transactional DB

> Status: **Design (RFC)** · Builds on [Custom HTTP Routes](/guide/advanced/custom-routes-proposal)
> (phase 2 shipped) · Pull trigger: a real consumer running billing in a sidecar
> **solely** because a route handler can't do a transactional read-modify-write.
> This is the milestone that deletes that sidecar.

## The one capability

A route handler must be able to run, **atomically, in one HTTP request**:

```sql
UPDATE credit_balance SET balance = balance - :cost
 WHERE user_id = :u AND balance >= :cost          -- no-overdraw; returns whether it applied
```

paired with an idempotent ledger insert (`INSERT ... ON CONFLICT (idempotency_key) DO NOTHING`),
and branch the HTTP response on whether the debit applied (`200` vs `402`). Today
this is impossible, so the logic lives in a separate process.

## Why it's hard today (runtime ground truth)

The JS (Javy) plugin tier — the ergonomic one users write against — is structurally
incompatible with mid-handler host calls:

- **One-shot, WASI-only.** `run_module` (`crates/atomo_wasm_runtime/src/js_runtime.rs:45-76`)
  runs the Javy module's `_start` to completion: it reads its whole input from
  **stdin** once and writes its whole output to **stdout** once. The linker registers
  **only** `wasmtime_wasi` (`:56`) — there are *no* `env` host functions a JS handler
  can call. wasmtime is configured **synchronously** (`Config` never sets
  `async_support`, `:32`). So a JS handler is a pure `stdin_json -> stdout_json`
  function; it cannot suspend and call back into the host to read the DB mid-run.
- **Effects are deferred.** `dbQuery`/`http`/`emit` are *data the handler returns*;
  the host applies them **after** the handler exits (`apply_js_effects`
  `crates/atomo_server/src/wasm_plugins.rs:210-236`, fulfilled in
  `:260-320`). The DB effect is a **read-only** `SELECT` (`fulfill_db_request`
  `:358-389`) — no write, no transaction.

So "read the balance, decide, write" cannot happen inside one JS handler invocation
as the runtime is built.

### Two cross-cutting bugs to fix first (independent of the option chosen)

The investigation surfaced two real gaps in the **phase-2** route path that must be
closed before any transactional write ships:

1. **Route effects are buffered but never fulfilled.** `call_route`
   (`wasm_plugins.rs:183-203`) calls `apply_js_effects`, which only *records* effects
   into `js_effects`; the route path never calls `fulfill_js_effects` (only the CRUD
   after-hook does, `wasm_hooks.rs:73`) and **no `PgPool` is wired into the route
   dispatcher** (`plugin_routes.rs:dispatch`). So effects a route records today are
   silently dropped. The route path needs its own pool handle regardless.
2. **String-formatted SQL.** `fulfill_db_request` builds SQL with `format!`
   (`wasm_plugins.rs:375`), not bound parameters. Any write path **must** use bound
   params before shipping, or it's an injection hole.

Plus a capacity concern: the app pool is the sqlx default (**max 10 connections**,
`crates/atomo/src/client.rs:57`). A transaction held open across handler execution
ties up a connection for that whole time, so route handlers need a **dedicated,
capped** connection budget and a statement/lock timeout.

## Options

### Option A — synchronous DB host function callable from JS
A `host_db_exec(...)` import the JS handler calls mid-run; the host executes the
conditional UPDATE + ledger INSERT in an open transaction and returns the result inline.

- **Blocked as built.** Javy modules talk to the host only over WASI stdin/stdout; the
  JS linker registers no `env` imports. Exposing a host function needs a **custom Javy
  build** (registering imports into the QuickJS context) plus a guest JS SDK — a
  *toolchain* change, not just Rust code. Add the sync-over-async bridge below.
- **Effort: high.** New Javy build + guest SDK + import wiring + sync DB bridge + txn-in-Store.
- **Risk: high** — a DB txn/connection held open across arbitrary JS under `block_on`
  → pool starvation, sync-over-async deadlock.

### Option B — declarative atomic statement batch (recommended for phase 3)
The handler returns, in its normal stdout, a small **transactional plan**, e.g.:

```json
{
  "transaction": [
    { "update": "credit_balance",
      "set": "balance = balance - :cost",
      "where": "user_id = :u AND balance >= :cost",
      "params": { "cost": 5, "u": "user_123" },
      "expect": "rows_affected",          // surfaced back to the handler's response mapping
      "elseStatus": 402 },                 // if 0 rows, respond 402 and roll back
    { "insert": "credit_ledger",
      "values": { "id": ":jobId", "user_id": ":u", "delta": -5, "idempotency_key": ":idem" },
      "onConflict": "idempotency_key", "do": "nothing" }
  ],
  "response": { "status": 200, "body": { "ok": true } }
}
```

The host runs the whole plan in **one `pool.begin()` transaction** after the handler
returns, commits on success, and maps the result (rows-affected / conflict) into the
HTTP response.

- **Fits the runtime exactly.** No host callbacks, no Javy change, no sync-over-async —
  it's the existing deferred-effect model upgraded from "fire-and-forget" to
  "transactional, result surfaced." Requires the two cross-cutting fixes above (wire a
  pool into the route path; bound params).
- **Expresses the canonical debit.** The no-overdraw guard lives in the SQL predicate
  (`WHERE balance >= :cost`); rows-affected says whether it applied; the idempotent
  ledger insert is `ON CONFLICT DO NOTHING`. **No arbitrary mid-handler read is needed.**
  The only thing B can't do is a debit whose decision depends on a value the handler
  reads, computes on in JS, then writes — but the canonical case deliberately pushes
  that decision into the predicate.
- **Effort: medium.** Define the plan DSL + a safe, parameterized executor + run-in-txn
  + result mapping. **No toolchain change.**
- **Risk: low–medium.** Short, host-owned transaction (far less pool-starvation risk
  than A/C). Main work is keeping the DSL injection-proof and bounded.

### Option C — compiled WASM handlers with a synchronous host ABI
Route handlers compiled to WASM that import a synchronous `env` DB host function,
mirroring the existing `host_db_query` pattern (`runtime.rs:60-156`) but executing for real.

- **Runtime already supports the import shape** (the compiled tier does
  `linker.func_wrap("env", ...)` and guests call it mid-run). Missing: make that host
  fn actually run SQL via the sync-over-async bridge, hold/commit a txn in `PluginState`,
  and route compiled handlers through `call_route` (today it bails for non-JS,
  `wasm_plugins.rs:202`). Phase 4 of the proposal already lists "WASM handler support."
- **Effort: medium-high.** Sync DB bridge + txn-in-Store + dispatch extension + guest SDK.
- **Risk: medium** — same txn-held-open concern as A, minus the Javy toolchain blocker;
  worse ergonomics (Rust/AssemblyScript handlers), which cuts against "ergonomic first."

## Recommendation

- **Ship Option B as phase 3.** It is the only option that works against the runtime as
  it exists today and fully expresses the atomic debit + idempotent ledger insert,
  with the lowest risk and no toolchain change.
- **Keep Option C as the phase-4 path** for handlers that genuinely need arbitrary
  read-compute-write loops (the import machinery already exists).
- **Treat Option A as a long-term ergonomic end-state**, gated on a custom Javy build —
  a separate, larger investment, not near-term.

## Phasing

1. ✅ Design (this doc).
2. **Plumbing**: wire a (dedicated, capped) `PgPool` into the route dispatch path; fix
   `fulfill_db_request` to use bound params; add a statement/lock timeout for route txns.
3. **Option B**: the transactional-plan DSL + parameterized executor + run-in-one-txn +
   result→response mapping. Permission-gate it behind `WriteDatabase`.
4. **Harden**: row/size caps, idempotency conventions, structured logs without PII;
   then revisit Option C for arbitrary handlers.

## Reference spec (from the pulling consumer)

The canonical primitive a handler must express atomically:

```sql
-- no-overdraw debit: applies iff balance suffices; rows-affected = did-it-apply
UPDATE credit_balance SET balance = balance - :cost
 WHERE user_id = :u AND balance >= :cost;
-- idempotent ledger entry: a retried request neither double-charges nor loses the debit
INSERT INTO credit_ledger (id, user_id, delta, idempotency_key, ...)
VALUES (:jobId, :u, -:cost, :idem, ...)
ON CONFLICT (idempotency_key) DO NOTHING;
```

When phase 3 lands, this moves from a sidecar worker into `/ext/<plugin>/debit`, and
the worker collapses into atomo.
