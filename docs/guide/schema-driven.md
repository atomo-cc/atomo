# Schema-Driven Development

Define your domain in `schema.ts`. Atomo generates:
- Rust backend and GraphQL API
- Admin UI screens
- Client SDK types

Edit `services/<name>/schema.ts`, then run the dev runtime (`atomo dev`, or `pnpm dev:service`
in the monorepo) — or boot the standalone `atomo-server` against the schema (see
[Getting Started](/guide/getting-started)).
