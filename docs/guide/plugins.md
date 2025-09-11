# WASM Plugins

Extend your service with safe, fast WebAssembly plugins.

When to use
- Event handlers (react to domain events)
- Content processors (enrichment, transformation)
- Integrations (webhooks, 3rd‑party APIs) with capabilities sandboxed

Project layout
- Location: `services/<name>/plugins`
- Runtime: `crates/atomo_wasm_runtime` (executes plugins with wasmtime)

Manifest & capabilities
```json
{
  "name": "content-auto-tag",
  "version": "0.1.0",
  "permissions": [
    { "cap": "net.fetch", "domains": ["https://api.example.com"] },
    { "cap": "clock.now" },
    { "cap": "env.read", "vars": ["OPENAI_API_KEY"] }
  ],
  "events": ["ContentCreated", "ContentUpdated"]
}
```

Security model
- Capability‑based: only granted caps are available (e.g., `net.fetch`, `env.read`).
- Resource budgets: per‑plugin CPU/memory caps and execution timeouts.
- Isolation: no filesystem or network by default; opt‑in per manifest.
- Review/signing: recommend signed plugin artifacts and provenance checks.

Lifecycle
- Install: place artifact + manifest under `plugins/` (or via CLI, planned).
- Grant: review and approve capabilities (policy gates).
- Run: host attaches context (tenant, session, logger) and invokes handlers.
- Suspend/Update: hot‑swap with versioned manifests; preserve capability review.

Development
- Tooling: build TinyGo/Rust/AssemblyScript to WASM (examples forthcoming).
- Interfaces: stable ABI for events and common helpers (logging, fetch, time).
- Testing: run plugins in‑process with the same host runtime used in dev.

See also
- Vision → Extensibility and Plugins: `/vision`
- Security and production gates: `/guide/advanced/production-readiness`
