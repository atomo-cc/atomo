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

## Feature Change Checklist (create / update / delete)
When you add, change, or remove a feature, sweep **all** of these surfaces in the
same change so code and docs never drift. (Skip an item only if it genuinely does
not apply — don't skip silently.)

1. **Code** — implement in the right crate/package. A new crate must be added to
   the root `Cargo.toml` `[workspace] members`. Mounted into the server? Wire the
   route in `crates/atomo_server/src/handlers.rs` (or `server.rs`) and gate it
   behind a config flag if optional.
2. **Tests** — co-located unit tests (`#[cfg(test)]`) for internals **and**
   integration tests in `tests/` for the wired path. Then `cargo test`,
   `cargo clippy -- -D warnings`, and a build of any dependent crate must pass.
   Prefer tests that parse/drive the real entry point (a unit test caught a
   serde bug the integration tests had bypassed).
3. **Config & env** — a new env var goes in `ServerConfig` (field + `Default` +
   `from_env`) **and** in `.env.example` (security-relevant flags especially).
4. **Docs (`docs/`, VitePress)**:
   - The feature's own guide/proposal page — keep **status accurate**; when a
     proposal ships, reconcile it with what was actually built (don't leave it
     reading as "proposed").
   - `docs/guide/architecture.md` — component/crate list.
   - `docs/api/index.md` + the relevant `docs/api/*` page — new/changed endpoints.
   - `docs/roadmap.md` (and `docs/zh/roadmap.md` if kept in sync) — status line.
   - `docs/.vitepress/config.ts` — nav entry for any **new** page.
5. **Top-level** — `README.md` (workspace crate tree / feature list) and
   `CHANGELOG.md` (`[Unreleased]`).
6. **Commit** — Conventional Commits, one focused commit per concern
   (`feat`/`test`/`docs`), each verifiable on its own.

**Deleting a feature** is the same sweep in reverse: remove the code/tests/crate
member **and prune every reference** above — env vars, nav entries, README/crate
tree, changelog, roadmap, and cross-links — so nothing dangles.

## Keep the Platform Generic (no named consumers)
atomo is the distribution product, not any one app's backend. **Never name a
specific or private consumer project** (a sibling repo, a customer, an app you're
also building) anywhere in this repo — code, comments, docs, RFCs, README,
CHANGELOG, **or commit messages**. Motivating examples in docs/RFCs must be generic
("a credit/billing ledger", "a metered-API consumer", "a billing sidecar"), never a
product name. Reasons: it couples the platform to one customer, and it leaks that
project's existence if the repo or the deployed `docs/` site ever goes public.
When a real consumer pulls a feature, capture the *generic* gap it surfaced — not
its name. If you catch a leaked name, genericize it in the working tree; for full
removal from history a maintainer can scrub it with `git filter-repo`.

## Security & Configuration Tips
- Use `.env` for local secrets; never commit secrets. Copy from `.env.example`.
- Run DB ops via `pnpm atomo migrate -- --service <name>` and seed with `pnpm atomo seed -- --service <name>`.
- Generated code lives in `generated/`; do not hand-edit—change the source schema or templates instead.

## Agent-Specific Instructions
- Respect this guide for any code edits. For any feature add/change/remove, run the **Feature Change Checklist** above so code, tests, config, and docs land together. Avoid breaking public APIs in `crates/atomo*` without prior discussion.
