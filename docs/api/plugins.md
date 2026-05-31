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
- Each plugin directory contains a `plugin.toml` manifest. Plugins are auto-discovered from the `plugins/` directory at server boot.
```toml
name = "my-plugin"
version = "0.1.0"
description = "Enrich events with external data"
author = "you"
entry_point = "my_plugin.wasm"
permissions = ["ReadEvents", "HttpRequests"]
```

- Supported permissions:
  - `ReadEvents` — read domain events
  - `WriteEvents` — emit new/derived events
  - `ReadDatabase` — read data using restricted queries
  - `WriteDatabase` — write data with guardrails
  - `HttpRequests` — call external services

Sandboxing
- The runtime enables fuel metering (default budget 1,000,000) so plugins cannot run unbounded.
- Host functions check the plugin's granted permissions before executing; a denied call traps and aborts the guest.

Host functions (module `env`)
- `host_log(ptr: i32, len: i32)` — append a UTF-8 string from guest memory to the plugin log buffer (always allowed).
- `host_emit_event(ptr: i32, len: i32)` — requires `WriteEvents`. Reads a JSON payload from guest memory and records it; the host can read these back via the runtime's `emitted_events()` accessor.
- `host_read_event(ptr: i32, cap: i32) -> i32` — requires `ReadEvents`. Writes the next host-provided event (seeded via `set_readable_events`) into guest memory at `ptr` (up to `cap` bytes) and returns the number of bytes written (`0` when no more events).

CRUD lifecycle hooks
- Plugins may export hook functions invoked around model operations: `before_create`, `after_create`, `before_update`, `after_update`, `before_delete`, `after_delete`.
- Hook ABI: the host calls the guest's `alloc(len: i32) -> i32` to allocate memory, writes the record JSON, then calls `{hook}(ptr: i32, len: i32) -> i64`.
  - Return `0` for "no change".
  - Otherwise return a packed `i64` (`ptr << 32 | len`) pointing at the modified-record JSON in guest memory.
- A trap/error in a `before_*` hook aborts the operation; `after_*` hooks are fire-and-forget.

Capabilities
- Handle domain events
- Transform content (read and mutate the record via before-hooks)
- Integrate external systems

See also: `crates/atomo_wasm_runtime` (types: `PluginManifest`, `Permission`, `PluginContext`).
