# WASM Plugins

Extend your service with safe, fast WebAssembly plugins.

When to use
- Event handlers (react to domain events)
- Content processors (enrichment, transformation)
- Integrations (webhooks, 3rd‑party APIs) with capabilities sandboxed

Project layout
- Location: `services/<name>/plugins`
- Runtime: `crates/atomo_wasm_runtime` (executes plugins with wasmtime)

Manifest & permissions
- Each plugin directory has a `plugin.toml` manifest (TOML, not JSON). Plugins are
  auto-discovered from `plugins/` at server boot.
```toml
name = "content-auto-tag"
version = "0.1.0"
author = "you"
entry_point = "plugin.wasm"
permissions = ["ReadEvents", "WriteEvents", "HttpRequests"]
```
- Permissions are a fixed set (enforced by the runtime): `ReadEvents`, `WriteEvents`,
  `ReadDatabase`, `WriteDatabase`, `HttpRequests`. A host call without the matching
  permission traps and aborts the plugin. See `/api/plugins` for the host-function contract.

Security model
- Sandbox: fuel metering (default budget 1,000,000) bounds CPU/execution per call.
- Capability gating: host functions check the plugin's granted permissions before running.
- Isolation: no filesystem or ambient network — a plugin can only reach the host functions it is granted.

Authoring tiers (see the Scripting Plugins proposal: `/guide/advanced/scripting-plugins-proposal`)
- **Tier 1 — Scripting (available, default):** write a `.js` plugin run by a bundled JS
  engine (Javy/QuickJS) — no toolchain to install. Set `runtime = "js"` in `plugin.toml`.
  See the quickstart below.
- **Tier 2 — Compiled (available):** build a `.wasm` from Rust/TinyGo/Zig against the
  host ABI for performance-critical plugins.

Lifecycle
- Install: place artifact + `plugin.toml` under `plugins/` (or via the planned `atomo plugin install`).
- Run: the host invokes exported hook functions (`before_create`, `after_create`, ... ) around model operations.
- Update: replace the artifact and restart; versioned manifests planned via the marketplace.

Scripting plugins (JavaScript, no toolchain)
- The fastest way to extend a service: drop in a `.js` file. No Rust, Node, or npm needed at
  runtime — the server runs it in a bundled, fuel-metered JS engine (Javy/QuickJS).
- ABI: the runtime passes `{ "hook": "<name>", "record": {...} }` on **stdin**; your plugin
  writes `{ "record": {...}, "effects": [...] }` to **stdout**. Effects are permission-gated:
  - `{ emit: { model, event: Created|Updated|Deleted|Custom, data } }` — needs `WriteEvents`;
    published onto the model-event stream (projectors/audit/subscriptions consume it).
  - `{ dbQuery: { model, limit } }` — needs `ReadDatabase`; a bounded read-only select.
  - `{ http: { method, url, body? } }` — needs `HttpRequests`.
  A requested effect without its permission **aborts** the hook.
- `plugin.toml` sets `runtime = "js"` and `entry_point = "plugin.wasm"`.
- Build once with [Javy](https://github.com/bytecodealliance/javy): `javy build index.js -o plugin.wasm`.
- Working example: `services/crm-service/plugins/normalize-contact/` (normalizes Contact
  email/name, emits a typed `Notification.Created`). Exercised by
  `crates/atomo_server/tests/example_plugin.rs`.

```js
// index.js — normalize on write, notify after create
function readStdin(){const p=[];const b=new Uint8Array(4096);let n;
  while((n=Javy.IO.readSync(0,b))>0)p.push(b.slice(0,n));
  const t=p.reduce((a,x)=>a+x.length,0),all=new Uint8Array(t);let o=0;
  for(const x of p){all.set(x,o);o+=x.length;}return new TextDecoder().decode(all);}
const { hook, record } = JSON.parse(readStdin());
const out = { record: record || {}, effects: [] };
if (hook.startsWith("before_")) {
  if (typeof out.record.email === "string") out.record.email = out.record.email.trim().toLowerCase();
}
if (hook === "after_create") {
  out.effects.push({ emit: { model: "Notification", event: "Created", data: { email: out.record.email } } });
}
Javy.IO.writeSync(1, new TextEncoder().encode(JSON.stringify(out)));
```

Perf note: JS per-call latency is ~1–2 ms (fine for CRUD hooks); the engine module is ~1.2 MB
and compiled once at load. For hot, latency-critical paths or huge fan-out, use the compiled
Tier 2 instead. See the proposal's perf table for measured numbers.

Development (Tier 2, today)
- Build a Rust plugin to wasm: `cargo build --target wasm32-unknown-unknown --release`,
  then point `entry_point` at the resulting `.wasm`. (TinyGo/Zig also work against the same ABI.)
- The ABI (exports `memory` + `alloc`, hook fns; imports `host_*`) is documented in `/api/plugins`
  and exercised in `crates/atomo_wasm_runtime/tests/host_api.rs`.

See also
- Plugin host ABI and host functions: `/api/plugins`
- Scripting plugins direction: `/guide/advanced/scripting-plugins-proposal`
- Security and production gates: `/guide/advanced/production-readiness`
