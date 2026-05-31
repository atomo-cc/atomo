# Design Proposal: Plugin Marketplace / Registry

Status: proposal (not yet implemented). This document scopes a plugin marketplace so
implementation can start from an agreed design rather than a stub. It builds directly on
the WASM plugin runtime that already exists.

## Goal

Let developers **discover, install, and publish** Atomo WASM plugins, so a service can
pull a plugin by name/version instead of hand-placing files in `plugins/`. The runtime,
manifest, permissions, sandboxing, hooks, and host-request fulfillment already exist — the
marketplace is the *distribution and lifecycle* layer around them.

## What already exists (build on, don't rebuild)

- WASM runtime: `WasmRuntime`/`WasmPlugin` with fuel metering, permission-checked host
  functions (`host_log`/`host_read_event`/`host_emit_event`/`host_db_query`/`host_http_request`),
  the `alloc`/`call_hook` ABI, and host request fulfillment (constrained DB read + HTTP).
- `PluginManifest { name, version, description, author, entry_point, permissions }`.
- `WasmPluginManager`: discovers `plugins/<dir>/plugin.toml` + `.wasm` at boot, loads,
  executes, bridges hooks into the CRUD lifecycle.

The marketplace adds: a registry of published plugins, an install flow that materializes
a plugin into a service's `plugins/` dir, and a publish flow that uploads a built artifact.

## Scope (v1)

In scope:
- A **registry index**: name → versions → metadata (`plugin.toml` + checksum + artifact URL).
- `atomo plugin search/install/publish` CLI commands.
- Install: download the `.wasm` + manifest for a name@version, verify checksum, place under
  `plugins/<name>/`, and record it in a service lockfile (`plugins.lock`).
- Publish: package `plugin.toml` + `.wasm`, compute checksum, upload to the registry.
- A read-only marketplace browse view in the admin UI (list/search registry entries).

Explicitly out of scope for v1:
- Paid plugins / billing.
- Server-side execution of untrusted uploads (publish stores artifacts; it does not run them).
- Automatic updates / semver range resolution (v1 pins exact versions).
- A hosted public registry (v1 targets a self-hostable registry; a public one is later).

## Architecture

Two pieces — a **registry service** and **client tooling**.

### Registry (storage + API)
- Reuse Postgres + object storage. Tables:
  - `plugins(name PK, description, author, latest_version, created_at)`
  - `plugin_versions(name, version, checksum, manifest JSONB, artifact_url, published_at,
     PRIMARY KEY(name, version))`
- Artifacts (`.wasm`) stored in object storage (S3-compatible) or, for self-hosting, a
  local blob dir; `artifact_url` points at it.
- REST API (new routes, or a separate `atomo-registry` binary):
  - `GET /registry/plugins?q=` — search
  - `GET /registry/plugins/{name}` — versions + metadata
  - `GET /registry/plugins/{name}/{version}/download` — artifact bytes
  - `POST /registry/plugins/{name}/{version}` — publish (auth required; multipart: manifest + wasm)
- AuthN/Z: publishing requires an API token tied to an owner; reuse the existing JWT/role
  system. Read/search can be public (self-host policy configurable).

### Client tooling (CLI)
- `atomo plugin search <query>` → hits `GET /registry/plugins?q=`.
- `atomo plugin install <name>@<version>` → downloads, **verifies checksum**, writes
  `plugins/<name>/{plugin.toml, <entry>.wasm}`, updates `plugins.lock`.
- `atomo plugin publish [--registry URL]` → from a plugin source dir: build to wasm
  (`cargo build --target wasm32-unknown-unknown --release` or `--target wasm32-wasi`),
  read `plugin.toml`, checksum, POST to the registry.
- Registry URL from `ATOMO_REGISTRY_URL` env or a config field; defaults to a self-host URL.

## Trust & security (the hard part)

- **Checksum verification** on install is mandatory (sha256 in `plugin.lock` and the
  registry); a mismatch aborts the install.
- **Permissions are surfaced at install time**: the CLI prints the manifest's requested
  `permissions` and requires confirmation (or `--yes`), so installing a plugin that wants
  `WriteDatabase`/`HttpRequests` is a conscious choice. The runtime already enforces them.
- **Publishing does not execute** uploaded code; the registry only stores artifacts.
- Optional v2: signature verification (publisher signs the artifact; install verifies
  against a trusted key) — design the `plugin_versions` row to carry an optional `signature`
  column now so it's forward-compatible.

## Data contract

`plugins.lock` (per service, committed):
```toml
[[plugin]]
name = "enrich-contacts"
version = "0.2.1"
checksum = "sha256:..."
```

Registry publish payload: `multipart/form-data` with `manifest` (the `plugin.toml` text)
and `artifact` (the `.wasm` bytes); server computes/stores the checksum.

## Milestones

1. ✅ Registry data model + read API (`search`, `get`, `download`) against Postgres + a local
   blob dir. No publish yet. (`registry.rs`, `registry_routes.rs`; tested in `tests/registry.rs`.)
2. `atomo plugin install` with checksum verification + `plugins.lock`. Integration test:
   install from a local registry fixture, assert files materialized + lockfile written.
3. `atomo plugin publish` (auth-gated) + the upload path. Round-trip test: publish then
   install the same artifact, checksums match.
4. Permission-confirmation UX in the CLI; admin-UI browse view.
5. (v2) signatures, semver ranges, hosted registry.

## Risks / decisions to confirm

- **Self-hosted vs hosted registry.** v1 should be self-hostable (a route set or small
  binary) so it works without Atomo Cloud. A public hosted registry is a Cloud feature.
- **Build toolchain for publish.** Compiling plugins to wasm requires the `wasm32-*` target;
  the CLI should detect/install it or fail with a clear message rather than assume it.
- **Artifact storage abstraction.** Start with a local blob dir behind a trait so S3 can be
  added later without touching the API.

## Recommendation

Implement **milestones 1–2 first** (registry read API + `install` with checksum verify) —
that delivers the core "pull a plugin by name" value on top of the existing runtime, is
self-hostable, and is fully testable without any hosted infrastructure. Defer publish-auth,
the admin UI, and signatures until the install path is proven.
