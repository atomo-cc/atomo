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
- **Tier 1 — Scripting (planned, default):** write a `.js`/`.ts` plugin run by a bundled
  JS engine — no toolchain to install. Status: design proposal; not yet implemented.
- **Tier 2 — Compiled (available today):** build a `.wasm` from Rust/TinyGo/Zig against the
  host ABI for performance-critical plugins.

Lifecycle
- Install: place artifact + `plugin.toml` under `plugins/` (or via the planned `atomo plugin install`).
- Run: the host invokes exported hook functions (`before_create`, `after_create`, ... ) around model operations.
- Update: replace the artifact and restart; versioned manifests planned via the marketplace.

Development (Tier 2, today)
- Build a Rust plugin to wasm: `cargo build --target wasm32-unknown-unknown --release`,
  then point `entry_point` at the resulting `.wasm`. (TinyGo/Zig also work against the same ABI.)
- The ABI (exports `memory` + `alloc`, hook fns; imports `host_*`) is documented in `/api/plugins`
  and exercised in `crates/atomo_wasm_runtime/tests/host_api.rs`.

See also
- Plugin host ABI and host functions: `/api/plugins`
- Scripting plugins direction: `/guide/advanced/scripting-plugins-proposal`
- Security and production gates: `/guide/advanced/production-readiness`
