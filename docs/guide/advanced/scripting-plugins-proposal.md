# Design Proposal: Scripting Plugins (Two-Tier Authoring)

Status: proposal (Phase 0 — design only, no code). This document selects the plugin
authoring strategy and scopes the work so implementation is milestone-gated.

## Problem

Today, authoring a plugin requires compiling to `.wasm` — in practice Rust + cargo +
the `wasm32` target. That conflicts with Atomo's TypeScript-first identity (schema DSL,
SDK, and config are all TypeScript) and raises the barrier for "drop in a small extension."
The vision docs promise *"language-agnostic"* plugins and even *"TypeScript"* plugins
(`vision.md`), but "TypeScript → standalone WASM" isn't a real compile path without an
embedded JS engine. This proposal resolves that contradiction.

## Decision: two-tier authoring

- **Tier 1 — Scripting (default): JavaScript/TypeScript plugins run inside a bundled
  JS engine compiled to WASM (Javy / QuickJS).** The author writes a `.js` file (or TS
  transpiled to JS), drops it in `plugins/`, and the platform executes it inside the
  embedded interpreter. **No toolchain required.** This is the ergonomic default and
  honors the TypeScript-first identity.
- **Tier 2 — Compiled (power user): native `.wasm`** from Rust/TinyGo/Zig against the
  existing host ABI, for performance-critical or systems-level plugins.

Both tiers run in the **same** wasmtime sandbox (fuel metering + permission-checked host
functions). Tier 1 adds no new trust surface — the interpreter is just another guest.

### Why two tiers (against the stated requirements)

| Requirement | Tier 1 (JS) | Tier 2 (compiled) |
|-------------|-------------|-------------------|
| Developer convenience | ✅ no toolchain | ❌ compiler + wasm target |
| Ease of distribution | ✅ ship a script | ⚠️ per-target `.wasm` |
| Performance (excellent + stable) | ⚠️ slower but fuel-bounded/deterministic | ✅ near-native |
| User-friendliness | ✅ JS/TS | ❌ Rust learning curve |
| Secondary development | ✅ lowest barrier | ⚠️ high barrier |

Tier 1 wins 4/5; Tier 2 covers the performance escape hatch. "Stable performance" is
satisfied either way — fuel metering already bounds execution deterministically.

## JS plugin contract (Tier 1)

A script exports hook functions and receives a host-provided `atomo` global. Draft API
(to be finalized in Phase 0 step "ABI contract"):

```js
// plugins/enrich/plugin.js
export function beforeCreate(record) {
  atomo.log(`creating ${record.email}`)
  record.tags = [...(record.tags ?? []), 'new']
  return record            // return the (possibly modified) record; undefined = no change
}

export function afterCreate(record) {
  // gated by manifest permissions:
  atomo.emit({ type: 'welcome', to: record.email })   // requires WriteEvents
}
```

Host bridge maps the `atomo` global onto the existing host functions:
- `atomo.log(msg)` → `host_log`
- `atomo.emit(json)` → `host_emit_event` (requires `WriteEvents`)
- `atomo.readEvent()` → `host_read_event` (requires `ReadEvents`)
- `atomo.dbQuery({model,limit})` → `host_db_query` (requires `ReadDatabase`)
- `atomo.http({method,url,body})` → `host_http_request` (requires `HttpRequests`)

The hook lifecycle (`beforeCreate`/`afterCreate`/`before|afterUpdate`/`before|afterDelete`)
maps to the same `HookRunner` contract compiled plugins already use; the interpreter
shim translates between JSON records and the `alloc`/`(ptr,len)->i64` ABI internally so
script authors never see pointers.

## Manifest

Reuse the existing `plugin.toml` + `Permission` model (NOT the stale JSON capability model
in the old guide). Add an optional `runtime` field:

```toml
name = "enrich"
version = "0.1.0"
author = "you"
runtime = "js"            # "js" (Tier 1) | "wasm" (Tier 2, default if entry_point ends .wasm)
entry_point = "plugin.js" # .js for Tier 1, .wasm for Tier 2
permissions = ["ReadEvents", "WriteEvents"]
```

## Security

- Same sandbox: the JS engine `.wasm` is fuel-metered; script CPU/time is bounded.
- Permissions unchanged: `atomo.*` calls hit the permission-checked host functions; a
  denied call traps and aborts the script — identical to compiled plugins.
- No new filesystem/network access: the script can only reach the host functions we grant.

## Performance budget (decision gate, Phase 1)

The QuickJS/Javy spike must meet, or we reconsider:
- Bundled interpreter `.wasm` size: target < ~5 MB (acceptable one-time cost shared across all JS plugins).
- Cold start (instantiate + eval a small script): target < ~50 ms.
- Per-hook call overhead vs. a compiled plugin: documented, expected within an order of
  magnitude. Tier 2 remains available when this isn't acceptable.

## Process

**Phase 0 — Design & docs (this step, reversible):**
1. This proposal.
2. Fix `guide/plugins.md` (replace the nonexistent JSON capability model with the real
   `Permission` model; document both tiers).
3. Finalize the `atomo` JS API contract (the draft above) with examples.

**Phase 1 — Spike (decision gate):**
4. Embed Javy/QuickJS as a `.wasm` in `atomo_wasm_runtime`; eval a trivial JS string under
   wasmtime with fuel metering. Measure against the performance budget. Go/No-go here.

**Phase 2 — Implement (milestone-gated, each tested):**
5. M1: load a `.js` plugin, run it, wire `atomo.log` → `host_log`. Test: JS plugin logs.
6. M2: bridge the CRUD hook ABI — JS `beforeCreate(record)` flows through `HookRunner`.
   Test: JS hook mutates a record end-to-end.
7. M3: permission-gated `atomo.emit`/`dbQuery`/`http` onto existing host functions.
   Test: denied without permission, works with.

**Phase 3 — Validation:**
8. A real JS example plugin in-repo + integration test (mirrors `tests/host_api.rs`).
9. Perf measurement: JS vs compiled cold-start + per-call; document the gap.
10. Update `api/plugins.md` + `guide/plugins.md` quickstart ("drop in a `.js`, no toolchain");
    update the roadmap.

## Risks / decisions to confirm

- **Javy vs. raw QuickJS.** Javy (Shopify) packages QuickJS + a WASI shim and a JS→WASM
  workflow; raw QuickJS gives more control but more glue. The spike should try Javy first.
- **TS support** = transpile TS→JS before load (esbuild/swc) OR document "write JS, or
  bring your own transpile." v1: accept `.js`; TS transpile is a convenience add-on.
- **Binary size** is the main risk; the budget gate addresses it.

## Recommendation

Proceed Phase 0 now (docs only). Then run the Phase 1 spike and **stop at the budget gate**
before committing to Phase 2. Tier 2 (compiled) already works today, so there is no risk in
deferring Tier 1 if the spike doesn't meet budget.
