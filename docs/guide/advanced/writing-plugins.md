---
title: "Writing Plugins"
description: How to extend atomo without forking — author a JavaScript (Javy) or compiled-WASM plugin that hooks the CRUD lifecycle and/or serves custom HTTP routes (including atomic transactional routes).
---

# Writing Plugins

Plugins are how you extend atomo **without forking the server**. A plugin can hook
into the CRUD lifecycle (before/after create/update/delete) and/or serve **custom HTTP
routes** under `/ext/<plugin>` — including [transactional routes](/guide/advanced/custom-routes-phase3-design)
that do an atomic read-modify-write (e.g. a no-overdraw debit).

There are two tiers:

- **Tier 1 — JavaScript (Javy).** Write a `.js` file, compile it to `.wasm` with
  [Javy](https://github.com/bytecodealliance/javy). Easiest; recommended for most logic.
- **Tier 2 — Compiled WASM.** Rust / TinyGo / Zig → `.wasm` against the host ABI. For
  hot paths.

Both run sandboxed (wasmtime, fuel-metered) and **permission-gated**. This page covers
Tier 1; the contract is the same for Tier 2.

## Where plugins live

Drop each plugin in its own directory under `plugins/` next to the server:

```
plugins/
└── billing/
    ├── plugin.toml
    └── plugin.wasm
```

atomo discovers and loads them at boot, logging each plugin and any mounted routes.
(In Docker, mount your `plugins/` dir into the container.)

## `plugin.toml`

```toml
name = "billing"
version = "0.1.0"
description = "Metered credit debit"
author = "you"
entry_point = "plugin.wasm"     # the Javy-compiled module (NOT the .js source)
runtime = "js"                  # "js" (Javy) | "wasm" (compiled; default if entry ends .wasm)
permissions = ["WriteDatabase", "WriteEvents"]

# Optional: custom HTTP routes. Each is served at /ext/<name><path>.
[[routes]]
method = "POST"
path = "/reserve"               # → POST /ext/billing/reserve
auth = true                     # require a valid JWT; the verified principal is injected
```

### Permissions

| Grant | Enables |
|---|---|
| `ReadDatabase` | `dbQuery` effects (a constrained read) |
| `WriteDatabase` | `transaction` batches in a route handler |
| `WriteEvents` | `emit` effects (publish a model event) |
| `HttpRequests` | `http` effects (outbound HTTP) |

A plugin that uses a capability it wasn't granted is denied.

## The handler contract

Your module reads **one JSON envelope from stdin** and writes **one JSON result to
stdout** — a single pass, no mid-run host calls.

### Lifecycle hooks

Input: `{ "hook": "beforeCreate" | "afterCreate" | …, "record": { …fields… } }`

Output: the (possibly modified) record, or `{ "record": {…}, "effects": [ … ] }`.
Empty/identical output = no change.

### HTTP routes

Input: `{ "route": { "method", "path", "query", "headers", "body", "principal" } }`
(`body` is parsed JSON when the request body is JSON; `principal` is `{ id, role }`
when `auth = true`.)

Output:

```jsonc
{
  "response":    { "status": 200, "headers": { … }, "body": { … } },
  "transaction": [ /* atomic SQL batch — see below */ ],   // optional
  "effects":     [ /* deferred emit/dbQuery/http */ ]       // optional
}
```

- `response` → the HTTP response (returned **if** the transaction commits).
- `transaction` → runs **atomically in one DB transaction** (needs `WriteDatabase`).
- `effects` → run **after** a committed transaction (a rolled-back batch emits nothing).

### Transaction batch (a route's atomic DB writes)

Each statement is `{ sql, params?, expect? }`. `params` bind as `$1, $2, …`
(injection-safe — never string-interpolated). An `expect`
(`{ minRowsAffected, elseStatus?, elseBody? }`) that isn't met rolls the **whole**
batch back and returns the else-response. Full spec + the no-overdraw debit example:
[Custom Routes Phase 3](/guide/advanced/custom-routes-phase3-design).

### Effects

- `{ "emit":    { "model", "event": "Created"|"Updated"|"Deleted"|"Custom", "data" } }`
- `{ "dbQuery": { "model", "limit" } }` — a constrained read-only `SELECT`
- `{ "http":    { "method", "url", "body"? } }`

## Build a Tier-1 (JavaScript) plugin

1. **Write `plugin.js`** — read the envelope from stdin, return the result on stdout.
   The stdin→stdout glue follows Javy's conventions (see the Javy docs for your
   version's IO API); your logic is just `envelope → result`:

   ```js
   function handle(env) {
     if (env.route) {
       const { userId, cost, idempotencyKey } = env.route.body;
       return {
         transaction: [
           { sql: `INSERT INTO credit_ledger (id,user_id,delta,idempotency_key,created_at)
                   VALUES (gen_random_uuid(),$1,$2,$3,NOW()) ON CONFLICT (idempotency_key) DO NOTHING`,
             params: [userId, -cost, idempotencyKey],
             expect: { minRowsAffected: 1, elseStatus: 409, elseBody: { result: "already" } } },
           { sql: `UPDATE credit_balance SET balance = balance - $1 WHERE user_id = $2 AND balance >= $1`,
             params: [cost, userId],
             expect: { minRowsAffected: 1, elseStatus: 402, elseBody: { result: "insufficient" } } },
         ],
         effects: [ { emit: { model: "CreditBalance", event: "Updated", data: { userId } } } ],
         response: { status: 200, body: { result: "applied" } },
       };
     }
     // hook path: env.hook + env.record
     return env.record;
   }
   ```

2. **Install Javy** — grab a release binary from
   [bytecodealliance/javy](https://github.com/bytecodealliance/javy/releases).

3. **Compile** to the module `plugin.toml` points at:

   ```bash
   javy compile plugin.js -o plugin.wasm
   ```

4. **Drop** `plugin.toml` + `plugin.wasm` into `plugins/<name>/` and restart the server.
   It logs `Loaded N JS plugin(s)` and `Mounted N custom plugin route(s)`.

## Notes & limits

- **One-shot execution.** The JS module runs start-to-finish over stdin/stdout and
  **cannot call back into the host mid-run** — so there's no synchronous "read a row,
  decide, write" loop. For request-time DB work, return a `transaction` batch (the
  guard lives in SQL, e.g. `WHERE balance >= cost`), not arbitrary reads.
- **Javy version.** The runtime loads a pre-compiled `.wasm`; if a module fails to run,
  match your Javy version to the one the bundled fixtures were built with.
- **No raw plugin SQL outside `transaction`.** `dbQuery` is a constrained, read-only
  `SELECT`; arbitrary writes go through `transaction` (bound params).
