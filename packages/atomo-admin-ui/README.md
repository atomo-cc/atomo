# Atomo Admin UI

A schema-driven admin interface for an Atomo backend. It introspects the server's
models at runtime (via `/meta/schema`) and generates the browse, create, and edit
views — no per-app configuration or code generation.

## Overview

The admin is **service-agnostic**: it has no build-time dependency on any service's
source. It renders whatever models the connected server exposes, and service-specific
views (e.g. a CRM kanban) attach at runtime as component plugins.

### Core ideas

- **Schema-driven** — the UI is generated from server metadata (`/meta/schema`), so it stays in sync with backend models automatically.
- **Powered by Dashin** — leverages [Dashin](https://github.com/dashindev/dashin) (`@dashin-dev/dashin` and `@dashin-dev/source-atomo`) for batteries-included CRUD (`CrudTable`), stacked relationship drill-in (`RelatedPreview`), and token-driven responsive design.
- **Embedded Single-Image Delivery** — compiled directly into `/app/admin` during Docker multi-stage builds and served natively by Atomo's Axum server via `ServeDir` on the single backend port.
- **Real CRUD & Observability** — talks to Atomo's GraphQL & REST endpoints, with built-in CQRS projector lag tracking and visual workflow pipeline management.

## Project structure

```
src/
├── main.tsx                     # Entry: router + react-query + ErrorBoundary
├── App.tsx                      # Auth gate (validates the session via /auth/me)
├── components/
│   ├── DynamicRenderer.tsx      # Route → view dispatch; loading/error/404 states
│   ├── Navigation.tsx           # Sidebar built from the schema's models
│   ├── ErrorBoundary.tsx        # Top-level crash guard
│   ├── Login.tsx                # Sign-in form
│   ├── views/                   # Dashboard, EntityList/Detail, Workflows, Trash,
│   │                            #   Settings, Help
│   ├── forms/                   # DynamicForm, FormField, BlocksEditor, …
│   ├── tables/                  # EntityTable, TableSettings
│   ├── filters/                 # AdvancedFilterPanel
│   ├── upload/                  # MediaUploader
│   └── ui/                      # Radix + Tailwind primitives
└── lib/
    ├── api.ts                   # API client (GraphQL CRUD, auth, /version)
    ├── schema-parser.ts         # Loads model metadata from the server
    ├── component-plugins.ts     # Runtime component-plugin registry (extension seam)
    ├── service-plugin-loader.ts # Loads service-served Admin UI plugins
    ├── validation.ts, types.ts, utils.ts, export.ts
```

## Menus

- **Dashboard** — per-model record counts + quick actions, from the live schema.
- **Entities** (one per model) — searchable, filterable, paginated list with bulk
  delete and export; create/detail/edit with real mutations.
- **Workflows** — list, register, run, and delete workflows (REST).
- **Trash** — list soft-deleted records; restore or permanently purge.
- **Settings** — account, connected server build (`GET /version`), platform config.
- **Help** — documentation links and about.

Unknown routes render a 404; a render-time crash is caught by the error boundary.

## Tech stack

- React 18 + TypeScript, Vite
- Tailwind CSS, Radix UI, Lucide icons
- React Query (data), React Hook Form + Zod (forms/validation)
- React Router v6, TanStack Virtual

## Develop

```bash
pnpm install
pnpm dev          # dev server (root path)
pnpm build        # production build
pnpm type-check   # tsc --noEmit
pnpm format       # prettier
```

`pnpm type-check` should stay green. From the repo root,
`pnpm --filter "./packages/*" test` verifies the Admin UI + TypeScript SDK baseline.
In production the SPA is served by `atomo-server` at `/admin` (built with
`vite build --base=/admin/`).

## Customize

- **Design tokens** — edit the repo-root `design-tokens.ts` (colors, spacing, etc.).
- **Custom field types** — add a `case` in `components/forms/FormField.tsx`.
- **Service-specific views** — ship a runtime component plugin and register it via
  `service-plugin-loader.ts` (the admin core stays service-agnostic).

## Roadmap (scaffolded, not yet wired)

These components exist in the tree but are **not yet routed into the UI** — they're
intended Phase-2/3 features awaiting integration (and currently use placeholder data).
They're tree-shaken out of the production bundle until wired:

- `components/observability/` — event stream, performance metrics, workflow monitor,
  error tracker
- `components/ai/` — global search, "magic wand" assist
- `components/collaboration/` — presence indicator
- `components/notifications/` — in-app notifications

## License

MIT — see [LICENSE](../../LICENSE).
