---
title: "Proposal: Custom HTTP Routes (plugin-served)"
description: Let plugins register HTTP endpoints served by atomo-server, so app/business logic — including transactional writes — runs inside Atomo without forking the server.
---

# Proposal: Custom HTTP Routes

> Status: **Phase 2 implemented** (registration + dispatch shipped; phase 3
> transactional DB access still RFC) · Layer: **Atomo core** (`atomo_server` + the
> plugin runtime) · Pull trigger: a real consumer that today works around the gap
> with a separate process (e.g. a billing/metering sidecar).
>
> A first-class way for a **plugin to register HTTP endpoints** that
> `atomo-server` serves and dispatches to a sandboxed handler. This turns Atomo
> from *fork-to-extend* into *extend-without-forking*, and it lets business logic
> that needs a **synchronous request/response with a transactional DB write** live
> in Atomo instead of a bolted-on sidecar.

## Where it fits

Atomo already exposes built-in routes — `/graphql`, `/auth`, `/media`,
`/realtime`, `/workflows`, `/audit` — and a plugin system
([Scripting Plugins](/guide/advanced/scripting-plugins-proposal)) whose plugins
hook into the CRUD lifecycle and emit **effects** (`emit` / `dbQuery` / `http`).

What's missing is a way to **own an endpoint**. Today, adding a custom endpoint
means editing `atomo_server` and recompiling — i.e. forking. Anything that needs
its own URL + custom logic (a billing debit, a webhook receiver, a Store-receipt
validator, an export endpoint) has to run as a *separate process* alongside
Atomo. That's the gap this closes.

## Why the existing plugin hooks aren't enough

The CRUD-hook + effect model is **fire-and-forget and deferred**: a plugin's
`dbQuery`/`http` effects are fulfilled *after* the after-hooks run (see
`wasm_hooks.rs`). There is no request the handler is responding *to*, and no way
to do a **read-modify-write inside one transaction** and return a result. So:

- You can't reject "debit if balance ≥ cost" — the hook can `Abort`, but it can't
  read the balance synchronously to decide.
- You can't return a computed HTTP response (a receipt, a signed URL, a 402).

A custom route is the missing shape: **synchronous handler, request in / response
out, with transactional DB access**.

## Capability

A plugin declares one or more routes; `atomo-server` mounts them and dispatches
matching requests to the plugin's handler:

- **Registration** — a plugin manifest (or a registration host call) declares
  `{ method, path, handler, auth }`.
- **Dispatch** — on a matching request, the server invokes the handler with the
  request (method, path params, query, headers, body) **plus the verified auth
  context** (reuse the JWT path), and returns the handler's `{ status, headers,
  body }`.
- **Transactional DB access** — the handler can run SQL in a real transaction
  (begin → read → conditional write → commit), so no-overdraw debits, idempotent
  upserts, etc. are correct and enforced *inside* Atomo.
- **Sandboxed** — same guarantees as today's plugins: WASM fuel metering,
  permission-gated DB/HTTP, no ambient filesystem.

```
  client ──► atomo-server ──► [core routes: /graphql, /auth, /media, ...]
                   │
                   └─► /ext/<plugin>/<path>  ──► plugin handler (JS/WASM)
                          • verified user injected
                          • synchronous request → response
                          • transactional dbQuery (begin/commit)
```

## The boundary (most important rule)

- **Core data routes stay built-in.** `/graphql` CRUD, auth, media, realtime are
  Atomo's; custom routes are for *app/business* logic on top.
- **Namespaced mount** (e.g. `/ext/<plugin>/...`) so plugin routes can never
  shadow or collide with core routes.
- **Durable outcomes still belong to the data layer.** A route that mutates
  domain data should write through the normal model/event path where it can; raw
  transactional SQL is the escape hatch for things the schema can't express
  (and pairs with [Schema Constraints](/guide/advanced/schema-constraints-proposal)).

## Design principles

- **Sync request/response**, distinct from the deferred-effect model — this is the
  whole point.
- **Reuse Atomo identity.** The handler receives the verified principal; routes
  declare their auth requirement (public / authenticated / role).
- **Sandboxed + permissioned**, exactly like existing plugins (fuel, capability
  grants for DB/HTTP).
- **Ergonomic first.** JS handlers (via the embedded Javy/QuickJS runtime) for
  reach; WASM for hot paths.

## What this obsoletes

The narrower "transactional command hook / conditional-append on `create()`" idea
(raised by the metered-billing fit-gap) becomes **unnecessary**: with a custom
route you write the transaction in your own handler. One general primitive
replaces a family of special-purpose ones.

## Implementation

Phase 2 lives in:

- `atomo_wasm_runtime::plugin` — `RouteDef { method, path, auth }` + a `routes`
  field on `PluginManifest` (parsed from `[[routes]]` in `plugin.toml`).
- `atomo_server::wasm_plugins` — `WasmPluginManager` stores per-plugin routes,
  exposes `plugin_routes()`, and `call_route(plugin, request_json)` runs the JS
  module with a `{ "route": <request> }` envelope and applies returned effects.
- `atomo_server::plugin_routes` — `plugin_routes_router(manager, auth, routes)`
  mounts each route at `/ext/<plugin><path>`, builds the request envelope
  (method/path/query/headers/body/principal), enforces `auth` via the existing JWT
  path, and maps the handler's `{status, headers, body}` to the HTTP response.
- `atomo_server::server` merges that router alongside realtime/workflows.

The older native `Plugin::register_routes(&self, router) -> Router` hook (default
no-op, in-process Rust only) is **superseded** by this WASM/JS-facing path — that
was the seam users actually write against.

## Phasing

1. ✅ RFC + this doc.
2. ✅ **Route registration + dispatch** (JS/Javy handlers first): manifest-declared
   `{method, path, auth}` → handler returning `{status, headers, body}`, mounted
   under `/ext/<plugin>`; verified principal injected. *Shipped — see "Using it"
   below.*
3. ✅ **Transactional DB access** in the handler — the piece that enables
   money/idempotency logic. *Shipped:* a handler returns a `transaction` array of
   `{ sql, params, expect }` statements that the server runs **atomically in one DB
   transaction** with bound params; an `expect` (`minRowsAffected`) that fails rolls
   the batch back and returns an else-response — so a no-overdraw debit + idempotent
   ledger insert lives in the plugin, not a sidecar. Requires `WriteDatabase`. Full
   spec + example in
   **[Custom Routes Phase 3 — Synchronous Transactional DB](/guide/advanced/custom-routes-phase3-design)**.
4. Harden: per-route auth + rate limits, request-size caps, timeouts, structured
   logs without PII; WASM handler support.

## Using it (phase 2)

Declare routes in `plugin.toml` and handle them in the plugin's JS entry point.

```toml
# plugin.toml
name = "billing"
runtime = "js"
entry_point = "plugin.js"

[[routes]]
method = "POST"
path = "/debit"
auth = true        # require a valid JWT; verified principal is injected
```

The server mounts this at `POST /ext/billing/debit`. On a request it calls the
plugin's JS module with a `{ "route": <request> }` envelope and expects a
`{ response, effects }` object back:

```js
// plugin.js — the module reads the envelope from stdin and writes JSON to stdout
function handle(env) {
  const req = env.route;                 // { method, path, query, headers, body, principal }
  if (req && req.route !== undefined) { /* ... */ }
  const userId = req.principal && req.principal.id;
  return {
    response: { status: 200, headers: { "content-type": "application/json" },
                body: { ok: true, user: userId, echo: req.body } },
    // effects: [ { dbQuery: ... }, { emit: ... } ]   // deferred (see phase 3)
  };
}
```

- `request.body` arrives as parsed JSON when the body is JSON, else as a raw
  string. `headers` is a `{ name: value }` map; `query` is the raw query string.
- The handler's `response.status` / `response.headers` / `response.body` become the
  HTTP response (`body` is serialized to JSON unless it's already a string).
- Effects are applied (permission-gated) after the handler returns — same model as
  the CRUD hooks. **Synchronous reads mid-handler are not available yet** (phase 3).

## Open questions

- Registration: manifest (`plugin.toml`) vs a runtime host call vs both.
- Mount shape: fixed `/ext/<plugin>/*` vs a configurable base path per plugin.
- Transaction API exposed to handlers: explicit `begin/commit`, or a
  "run these statements atomically" batch (safer, less rope).
- JS-only to start, or WASM handlers from day one?
- Streaming/SSE responses — in scope, or v2?
