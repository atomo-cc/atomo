# Repository Guidelines

## Project Structure & Module Organization
- Rust workspace in `crates/`: core library (`atomo_core`), server (`atomo_server`), CLI (`atomo_cli`), schema tools, WASM runtime, etc.
- Frontend/SDK in `packages/`: admin UI (`atomo-admin-ui`), TypeScript SDK (`atomo-client-sdk`).
- Services in `services/` (e.g., `services/crm-service`) with `schema.ts`, plugins, workflows, and generated code.
- Docs in `docs/` (VitePress). Shared tests in `tests//` and migrations in `migrations/`.

## Build, Test, and Development Commands
- Build (Rust): `cargo build --workspace` or `pnpm build:core`; full build: `pnpm build:all`.
- Dev (service): `pnpm dev:service` or `pnpm atomo dev -- --service crm` from repo root.
- Dev (admin/docs): `pnpm dev:admin`, `pnpm dev:docs`.
- Test: `cargo test --workspace` and `pnpm test` (runs package and service tests).
- Lint/format: `pnpm lint`, `pnpm format` (uses `clippy`, `rustfmt`, ESLint, Prettier).
- API docs: `pnpm docs:api`; serve docs: `pnpm docs:serve`.

## Coding Style & Naming Conventions
- Rust: rustfmt defaults (4‑space indent). Use `cargo fmt --all` and `cargo clippy -- -D warnings`.
- TypeScript: Prettier (2‑space indent), ESLint strict. React components `PascalCase`, variables/functions `camelCase`, files `kebab-case.ts(x)`.
- Rust naming: crates/modules `snake_case`, types/traits `PascalCase`, functions `snake_case`.

## Testing Guidelines
- Rust unit tests co-located; integration tests in `tests/`. Prefer meaningful property/edge cases.
- TypeScript tests as `*.test.ts(x)` within package `src/`. Service-level flows via `pnpm atomo test -- --service <name>`.
- No hard coverage gate yet; add tests for new behavior and regressions.

## Commit & Pull Request Guidelines
- Follow Conventional Commits (e.g., `feat(server): add GraphQL auth`).
- PRs must include: clear description, linked issues, test coverage, and docs updates (README/docs/CLI help). Add screenshots for UI changes.
- Keep patches focused and minimal; avoid unrelated refactors.

## Security & Configuration Tips
- Use `.env` for local secrets; never commit secrets. Copy from `.env.example`.
- Run DB ops via `pnpm atomo migrate -- --service <name>` and seed with `pnpm atomo seed -- --service <name>`.
- Generated code lives in `generated/`; do not hand-edit—change the source schema or templates instead.

## Agent-Specific Instructions
- Respect this guide for any code edits. Update tests and docs with behavior changes. Avoid breaking public APIs in `crates/atomo*` without prior discussion.
