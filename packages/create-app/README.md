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
