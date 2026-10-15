# @atomo-cc/create-app

Scaffold a new [Atomo](https://atomo.cc) project — and run it with **no Rust
toolchain**.

```bash
npm create @atomo-cc/app my-crm
# or pick a template:
npm create @atomo-cc/app my-shop -- --template ecommerce

cd my-crm
docker compose up        # API on :3000, Admin UI on :3000/admin
```

It writes a starter project — `atomo/schema.ts`, a `docker-compose.yml` that pulls
the prebuilt `atomo-server` image, a README, and `.gitignore` — that runs entirely
via Docker. No Rust, no native binary download.

## Templates

`crm` (default), `blog`, `ecommerce`, `default`.

The source version also accepts `fullstack-atomo` as a compatibility alias for
`crm`. Prefer `--template crm` with already-published versions. Scaffold ownership
belongs here: the server image includes the real Admin UI at `/admin`; no separate
dashboard Dockerfile or duplicated frontend shell is required.

Legacy dashboard CLI flags do not automatically execute this CLI. Run the command
above explicitly. This does not modify an existing application's dependencies.

## Verification (contributors)

`pnpm --filter @atomo-cc/create-app test` drives the real CLI and checks npm pack
contents. The CI Admin E2E job runs one spec in dev, production preview, and on the
native server's `/admin/` route against its isolated schema/backend. For the last
mode, build with empty `VITE_API_URL` and `pnpm build:server`, start the e2e backend
with `ATOMO_ADMIN_DIR` set to `dist`, and set `E2E_BASE_URL` to its `/admin/` URL.
Use `E2E_API_URL` for a nondefault backend origin. Image packaging/deployment
remains a separate gate.

In dev/preview tests, `E2E_API_URL` also selects the proxy backend. Keep
`VITE_API_URL` empty to exercise same-origin calls on isolated frontend ports.
Relative `E2E_OUTPUT_DIR` values resolve from the admin package root so CI can
collect screenshots; absolute paths can isolate local artifacts elsewhere.

## Relationship to the `atomo` CLI

This is the pure-JS scaffolder. The Rust `atomo` CLI produces the same project
layout and additionally offers the hot-reload dev runtime (`atomo dev`) and
codegen (`atomo generate`), which require the Rust toolchain. If all you need is
to spin up and run a project, this package is enough.

## Publishing (maintainers)

Published to the `@atomo-cc` org (`publishConfig.access=public`). The bundled
`templates/` are copies of `crates/atomo_cli/templates`; re-sync with
`pnpm --filter @atomo-cc/create-app run sync:templates` after changing the
canonical templates.
