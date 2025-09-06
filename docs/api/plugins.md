# Plugin APIs (WASM)

Extend services with WebAssembly plugins in `services/<name>/plugins`.

## Basic Structure
```
services/<name>/
└── plugins/
    └── my-plugin/
        ├── Cargo.toml
        └── src/lib.rs
```

## Capabilities
- Handle domain events
- Transform content
- Integrate external systems

See also: `crates/atomo_wasm_runtime`.
