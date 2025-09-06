# Plugin APIs (WASM)

Extend services with WebAssembly plugins in `services/<name>/plugins`.

Basic structure
```
services/<name>/
└── plugins/
    └── my-plugin/
        ├── Cargo.toml
        └── src/lib.rs
```

Manifest & permissions
- Runtime reads a manifest to understand capabilities:
```json
{
  "name": "my-plugin",
  "version": "0.1.0",
  "description": "Enrich events with external data",
  "author": "you",
  "entry_point": "lib.wasm",
  "permissions": [
    "ReadEvents",
    "HttpRequests"
  ]
}
```

- Supported permissions:
  - `ReadEvents` — read domain events
  - `WriteEvents` — emit new/derived events
  - `ReadDatabase` — read data using restricted queries
  - `WriteDatabase` — write data with guardrails
  - `HttpRequests` — call external services

Context
- Plugins receive a context payload when invoked:
```ts
interface PluginContext {
  event_data: any
  metadata: Record<string, string>
}
```

Capabilities
- Handle domain events
- Transform content
- Integrate external systems

See also: `crates/atomo_wasm_runtime` (types: `PluginManifest`, `Permission`, `PluginContext`).
