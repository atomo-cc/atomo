# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Docs — "Writing Plugins" guide**: the missing on-ramp for custom routes/plugins —
  the `plugin.toml` format, permissions, the hook + route handler contracts, the
  transaction batch, effects, and the Javy build path, with a billing-route example.

### Fixed
- **Custom routes — deferred effects now run on the route path.** A route handler's
  `effects` (`emit`/`dbQuery`/`http`) were recorded but never fulfilled on the route
  path (only the CRUD after-hook ran them). They now run, permission-gated, **after** a
  successful `transaction` (a rolled-back batch emits nothing).
  (`WasmPluginManager::fulfill_route_effects`.) Phase-3 design doc updated; also
  documents that `fulfill_db_request` is injection-safe as written (validated
  identifier + clamped limit), so the previously-flagged `format!` needs no change.

### Tests
- **End-to-end transactional-route test.** A real **Javy-compiled** plugin fixture
  (`tests/fixtures/route-billing`, source `plugin.js` + `plugin.wasm`, built with
  Javy v8.1.1) serves `POST /ext/billing/debit`; the test drives the full HTTP path
  (router → JS plugin → `transaction`) and asserts the atomic debit **commits** when
  sufficient (10→6) and **rolls back with 402** when not. Covers the route layer the
  `run_transaction` unit test couldn't reach.

## [0.2.4] - 2026-06-06

> Headline: **transactional custom routes (phase 3)** — the synchronous atomic
> read-modify-write primitive that lets billing/idempotency logic live in a plugin
> instead of a sidecar. No breaking changes. `:latest` + `:v0.2.4` are built from this tag.

### Added
- **Custom routes — transactional DB (phase 3).** A plugin route handler can now
  return a `transaction` array of `{ sql, params, expect }` statements that the server
  runs **atomically in one DB transaction** with bound parameters. A statement's
  `expect` (`{ minRowsAffected, elseStatus?, elseBody? }`) that isn't met rolls the
  whole batch back and returns the else-response. This is the synchronous
  read-modify-write primitive — e.g. a no-overdraw `UPDATE … WHERE balance >= cost`
  paired with an idempotent ledger insert — that lets billing-style logic live in a
  plugin instead of a sidecar. Requires the plugin's `WriteDatabase` permission.
  (`atomo_server::wasm_plugins::run_route` + `atomo_server::plugin_routes`; phase 3 of
  the [Custom Routes RFC](docs/guide/advanced/custom-routes-phase3-design.md).) DB-backed
  test verifies the atomic debit commits when sufficient and rolls back (balance
  unchanged) when not.

## [0.2.3] - 2026-06-05

> Admin UI goes from demo-grade to production, plus an admin-seeding fix. No
> breaking changes. `:latest` + `:v0.2.3` are built from this tag.

### Changed
- **Admin UI — production pass.** The bundled `/admin` was demo-grade; it's now a
  real production app: the entire UI is **English** (was mixed Chinese/English —
  ~1670 strings translated); **dead Settings/Help menus** now render real pages
  (account, server build via `/version`, platform config; docs/about); unknown routes
  show a real **404** instead of silently rendering the dashboard; the Dashboard shows
  **real per-model counts**; a top-level **ErrorBoundary** replaces white-screen
  crashes; a real favicon replaces the default Vite one. The demo-data fallback,
  demo placeholders, and dead code were removed; the JS bundle is **code-split**
  (601 kB single chunk → 135 kB app + cacheable vendor chunks). Unfinished Phase-2/3
  features (observability/AI/collaboration/notifications) remain in the tree as
  documented, tree-shaken scaffolding.
- **Admin seeding — fail loud on ignored credential changes.** Seeding from
  `ADMIN_EMAIL`/`ADMIN_PASSWORD` is create-once keyed by email, so changing
  `ADMIN_PASSWORD` and restarting was a **silent no-op** (the old password kept
  working, the new one was rejected). The server now logs a `WARN` when the env
  password differs from the seeded admin's, and honors **`ADMIN_RESET_PASSWORD=true`**
  to rotate the existing admin's password on boot. Documented in `.env.example`,
  `api/auth.md`, and `configuration.md`. (consumer feedback #7)

## [0.2.2] - 2026-06-05

> Consumer-feedback round (the "silent failures" theme). Fixes a real auth bug
> (`/auth/me` 401'd on valid login tokens), stops models silently half-registering
> or losing timestamps, adds `/version` + partial-unique constraints, validates the
> admin session on load, and lands the phase-3 transactional-routes design. No
> breaking changes. `:latest` + `:v0.2.2` are built from this tag.

### Added
- **Schema — partial unique/index**: `// @@unique([col]) WHERE <predicate>` and
  `// @@index([col]) WHERE <predicate>` now emit partial indexes
  (`CREATE [UNIQUE] INDEX ... WHERE <predicate>`). Lets a nullable anti-abuse anchor
  like `UNIQUE(store_account_id) WHERE store_account_id IS NOT NULL` live in the
  schema instead of hand-written SQL. (consumer feedback #6)
- **Server — `GET /version`**: returns `{ name, version, commit, buildTime }`, baked
  into the image at build time (`ATOMO_VERSION`/`ATOMO_GIT_SHA`/`ATOMO_BUILD_TIME` via
  Docker build args), so a consumer can verify *which* build is running without
  inferring from timestamps. (consumer feedback #4)

### Fixed
- **Auth — `/auth/me` (and `/auth/logout`) 401'd on a valid login token.** Those
  routes read an `AuthUser` from request extensions but were nested without the auth
  middleware that injects it, so they returned 401 unconditionally. Now guarded with
  `auth_middleware`; a token from `/auth/login` is accepted. (consumer feedback #3)
- **Schema — models silently lacked `created_at`/`updated_at`.** `generate_migrations`
  auto-added `deleted_at`/`tenant_id` but not the timestamps, so a model that declared
  only `updatedAt` got no `created_at` column and the admin list view (orders by
  `created_at`) 500'd at query time. Now every model auto-provisions both timestamps
  (`TIMESTAMPTZ NOT NULL DEFAULT NOW()`), **and** the list sort is tolerant — an
  `orderBy` on a column a model lacks is dropped instead of erroring. (consumer feedback #2)

### Changed
- **Admin UI — validates the session on load.** It previously treated "a token
  exists in localStorage" as signed-in and never re-checked, so an expired/revoked
  token showed a half-rendered admin that 401'd on the first data fetch, and the
  sidebar showed a hardcoded user (`管理员 / admin@atomo.dev`). Now it calls
  `/auth/me` on mount (refresh + reopen): valid → renders with the real signed-in
  user (name + role) and a sign-out button; invalid → clears the token and shows the
  login form cleanly. Builds on the `/auth/me` fix above.
- **Server — fail loud on silent half-registration.** A model with no `id` field gets
  its table created but is **not** registered (invisible to `/meta/schema`, the admin
  UI, GraphQL by-id lookups, the projector) — previously with zero warning. atomo now
  emits a boot `WARN` naming the model and explaining that `id` is the primary key (a
  declared `primaryKey` other than `id` is not honored). Enum/Block pseudo-models are
  excluded. (consumer feedback #1)
- **Docs**: added the [Custom Routes Phase 3 design](docs/guide/advanced/custom-routes-phase3-design.md)
  — synchronous transactional DB in route handlers (the primitive that lets a billing
  sidecar collapse into atomo); recommends a declarative atomic-statement batch run in
  one host-owned transaction. (consumer feedback #5)

## [0.2.1] - 2026-06-05

> Build/docs patch — no behavior change. Headline: the `atomo-server` image is
> **~56% smaller** (124 MB → ~55 MB). `:latest` and `:v0.2.1` are the slim image;
> `:v0.2.0` is left untouched.

### Changed
- **Image size**: `atomo-server` runtime base switched from `debian:bookworm-slim`
  to `gcr.io/distroless/cc-debian12` (~75 MB → ~25 MB; CA certs + a `nonroot` user
  ship in the base, so the `apt-get ca-certificates` layer is gone), plus a
  `[profile.release]` that strips symbols and uses thin LTO (binary ~40 MB →
  ~28 MB). Verified: the distroless image boots, serves `/health` 200, and still
  emits + enforces schema constraints. Note: distroless has **no shell** — use the
  `:debug` base or a sidecar to inspect a running container.
- **Docs**: genericized the two extensibility RFC examples (removed a named
  consumer) and added an AGENTS rule to keep the platform vendor-neutral.
- **Docs**: README is now **multilingual** — English is the default `README.md`
  (was Chinese), with `README.zh-CN.md` / `.es` / `.ja` / `.fr` / `.de` and a
  language switcher in each.

## [0.2.0] - 2026-06-05

> First tagged release since `0.1.0`. Headline: two **extensibility** features —
> declarable schema constraints and plugin-served custom HTTP routes — plus the
> realtime tier, the no-Rust distribution (Docker image + npm scaffolder/SDK), and
> the bundled generic Admin UI. The `ghcr.io/atomo-cc/atomo-server:0.2.0` /
> `:latest` image is built from this tag.

### Added
- **Extensibility — Custom HTTP routes**: plugins can now declare `[[routes]]`
  (`method`/`path`/`auth`) in `plugin.toml`; `atomo-server` mounts each at
  `/ext/<plugin><path>` and dispatches the request to the plugin's JS (Javy)
  handler — a synchronous request envelope (`{method, path, query, headers, body,
  principal}`) in, a `{status, headers, body}` response out (effects applied after,
  same model as CRUD hooks). `auth = true` requires a valid JWT and injects the
  verified principal via the existing auth path. This is the *extend-without-forking*
  seam: app/business endpoints (webhooks, receipt validators, exports) live in a
  plugin instead of a fork. (`atomo_wasm_runtime::RouteDef`,
  `WasmPluginManager::{plugin_routes,call_route}`, `atomo_server::plugin_routes`.)
  Phase 2 of the [Custom Routes RFC](docs/guide/advanced/custom-routes-proposal.md);
  synchronous transactional DB access in handlers remains phase 3. 7 new tests.
- **Extensibility — Schema constraints**: `schema.ts` now supports declarable
  database constraints via annotations — field-level `// @unique` / `// @index`,
  and model-level `// @@unique([a,b])` / `// @@index([a,b])` / `// @@check(expr)`.
  `generate_migrations` emits the matching `UNIQUE` columns, `CREATE [UNIQUE] INDEX
  IF NOT EXISTS`, and guarded `ADD CONSTRAINT ... CHECK` DDL on boot, so data
  integrity (uniqueness, lookups, value rules) is enforced in Postgres without
  forking. (`atomo_schema::ModelConstraint`, `atomo::schema::generate_migrations`.)
  Phase 2 of the [Schema Constraints RFC](docs/guide/advanced/schema-constraints-proposal.md).
  New parser + migration tests.
- **DX**: schema hot-reload for the no-Rust path — with `ATOMO_SCHEMA_WATCH=true` the server polls the mounted `schema.ts` (mtime, robust across Docker bind mounts) and exits on change; the compose `restart: unless-stopped` policy relaunches it, re-parsing + migrating on boot. Editing the schema is now edit-and-live (~2s) with no rebuild, restart command, or Rust. Wired into the root compose and the `atomo init` / `create-app` scaffolds.
- **Realtime**: hardening — per-client **join rate limiting** (token bucket in `atomo_realtime`, opt-in via `HubConfig.join_rate`; over-limit subscribe/session-join gets an `error` frame), and on the standalone relay: per-IP **connection caps** (`ATOMO_REALTIME_MAX_CONN_PER_IP`) and a Prometheus **`/metrics`** endpoint (hub gauges + counters). Join throttle on by default for the relay. 8 new tests (rate limiter, conn cap, metrics format, end-to-end throttle).
- **Realtime**: standalone `atomo-realtime-server` bin (`crates/atomo_realtime_server`) — runs the hub as a lightweight, **DB-free** relay (default :9100) with **stateless JWT verification** (signature + expiry against the shared `JWT_SECRET`), so it deploys as an edge/region fleet separate from the durable server. A token naming a session (`sid`) auto-binds the connection to it. Paired with `POST /realtime/token` on `atomo_server` (authenticated) that mints these short-lived tokens — the platform→relay handoff (user mgmt + matchmaking stay on the platform tier). Relay JWT verification covered by 4 tests.
- **Realtime**: coordinator sessions (RFC Phase 4) in `atomo_realtime` — host-authoritative relay over `/realtime/ws`: `session_join` assigns a stable slot and elects the first joiner as coordinator; directional relay (`to_coordinator` → host, `to_members` → host broadcast); `member_joined`/`member_left`/`coordinator_changed`/`session_closed` frames; coordinator-leave policy (`Reelect`/`Close`) via `HubConfig`. Domain-agnostic (opaque payloads) — fits multiplayer game/relay backends. 11 new tests; the WS transport forwards the new frames unchanged.
- **SDK**: `RealtimeClient` (`@atomo-cc/client-sdk` + `/realtime` subpath) — a framework-agnostic client for the ephemeral tier that mirrors the server's `/realtime/ws` protocol: subscribe/publish/presence, per-channel handlers, presence tracking, send-queue-before-open, and auto-reconnect with re-subscribe. Foundation for the CRM realtime dogfood (RFC Phase 3). 10 unit tests via a mock WebSocket.
- **Tooling**: `@atomo-cc/create-app` — a pure-JS project scaffolder (`npm create @atomo-cc/app <name>`) that writes `schema.ts` + a `docker-compose.yml` (pulling the published server image) + README, so a new project starts with **no Rust toolchain**. Templates: crm/blog/ecommerce/default (bundled copies of the canonical CLI templates).
- **Admin UI**: the `atomo-server` image bundles a **generic Admin UI** served at `/admin` — it introspects `/meta/schema` and gives data browse/edit for *any* model, with no build-time dependency on any service (the admin was decoupled from CRM: the one hardcoded `services/crm-service` import was removed, so service-specific views now load as runtime plugins via `componentPluginManager`/`pluginUrl`). Served as a static-file route gated on `ATOMO_ADMIN_DIR` (set by default in the image; unset to disable). Production API base is root-relative so same-origin calls (`/graphql`, `/media`) work.
- **Distribution**: Docker image for `atomo-server` (multi-stage `Dockerfile`) + root `docker-compose.yml` (Postgres + server) so the backend runs with **no Rust toolchain on the host** — `docker compose up --build`. Docs updated (install/getting-started/deployment) with a "Run without Rust" path.
- **Distribution**: `atomo init` now scaffolds a `docker-compose.yml` (pulls the published `ghcr.io/atomo-cc/atomo-server` image, mounts the project's `atomo/schema.ts`) and a "Run it (no Rust required)" README — so a generated project runs with `docker compose up`, no Rust or CLI build.
- **CI**: `docker.yml` workflow (manual dispatch) builds and pushes the `atomo-server` image to GHCR.
- **CI**: `cargo-chef` in the server Dockerfile caches the dependency compile as its own layer (persisted via the GHA build cache), so image rebuilds where only app code changed drop from ~12 min to ~1-3 min.

### Changed
- **Publishing**: prepared `@atomo-cc/client-sdk` for npm — `publishConfig.access=public` (scoped packages default to restricted) and `prepublishOnly: tsc` so `dist/` is always fresh in the tarball. Marked the apps `@atomo-cc/admin-ui` and `@atomo-cc/docs` `private: true` so they can't be published by accident. Documented the release flow (publish under the `next` dist-tag pre-1.0) in the SDK README.
- **npm scope**: renamed all packages from `@atomo/*` to `@atomo-cc/*` to match the owned npm org (`atomo-cc`). Affects package names, internal workspace imports/deps, the `atomo init` template, and docs. No published consumers were affected (packages were unpublished).

### Added
- 初始化 monorepo 结构
- 基础 Rust workspace 配置
- 核心域模型和事件定义
- CLI 工具与项目模板系统
- 事件溯源基础架构
- GraphQL 标量类型集成
- GitHub Actions CI/CD 流程

### Added (Phase 1-3 Implementation)
- **Auth**: Argon2id password hashing with bcrypt fallback for existing users
- **Auth**: OAuth2/OIDC SSO support (Google, GitHub, Microsoft, Okta)
- **Auth**: RBAC enforcement in all GraphQL resolvers from schema access rules
- **API**: GraphQL WebSocket subscriptions with model-name filtering (`/graphql/ws`)
- **API**: Ephemeral realtime channels, presence & server fan-out — authenticated WebSocket at `/realtime/ws` (+ `/realtime/health`), backed by the domain-agnostic in-memory `atomo_realtime` hub; gated by `ATOMO_ENABLE_REALTIME`, anonymous access opt-in via `ATOMO_REALTIME_ALLOW_ANON`
- **API**: Where/orderBy JSON parsing (Hasura-style filter syntax)
- **API**: Pagination metadata (totalCount, hasNextPage, hasPreviousPage)
- **API**: Relationship resolution (belongsTo/hasMany joins via include)
- **API**: Structured error responses with error codes (NOT_FOUND, UNAUTHORIZED, FORBIDDEN, VALIDATION_ERROR)
- **Data**: Dynamic SQL builder generating parameterized SELECT/INSERT/UPDATE/DELETE
- **Data**: Full CRUD operations with schema-driven query execution
- **Data**: Soft deletes (deleted_at column, automatic query filtering)
- **Data**: Event store with event_log table, replay, and entity history
- **Data**: CQRS read projections (TableProjection with event-driven updates)
- **Data**: Read cache with TTL and event-driven invalidation per model
- **Data**: Auto-run migrations on dev startup (CREATE TABLE IF NOT EXISTS)
- **Data**: Migration diff generation (ALTER TABLE for type/nullable/drop changes)
- **Data**: Multi-tenant row isolation via TenantCtx (x-tenant-id header)
- **Plugins**: WASM sandboxing with fuel metering and permission-checked host functions
- **Plugins**: Plugin lifecycle (discovery from plugin.toml, loading, execution)
- **Plugins**: WASM hooks in CRUD lifecycle (before/after create/update/delete)
- **AI**: pgvector EmbeddingStore with cosine similarity search
- **Workflow**: Workflow engine with event/manual/schedule triggers and retry policies
- **SDK**: Local-first OfflineQueue with localStorage persistence and sync-on-reconnect
- **CLI**: `atomo test` command for running service tests
- **CLI**: `atomo deploy` with build validation and manifest generation
- **CLI**: Blog and ecommerce project templates (`--template blog|ecommerce`)
- **Server**: Rate limiting middleware (per-IP token bucket, env-configurable)
- **Server**: Structured tracing middleware with x-request-id in spans
- **Server**: Input validation enforcement (required, email, min, max, numeric)
- **Projectors**: ProjectorManager with event stream listener

### Added (Integration & Wiring)
- **Schema**: Validation rules parsed from `schema.ts` (brace-balanced, handles nested access/relationships blocks) and enforced in create mutation
- **Plugins**: `WasmHookRunner` bridges loaded WASM plugins into the CRUD before/after hook lifecycle
- **Server**: WASM plugins auto-discovered from `plugins/` at boot
- **Server**: CQRS projector and workflow event listeners started at server boot
- **Auth**: OAuth callback now find-or-creates a user and issues a JWT session
- **Client**: `event_receiver()` accessor exposing the model-event broadcast stream

### Added (Bootstrap & Ops)
- **Auth**: Admin bootstrap from `ADMIN_EMAIL`/`ADMIN_PASSWORD` env vars on server start (ULID id, argon2id hash, idempotent)
- **Server**: `ensure_platform_tables()` creates `users`/`sessions`/`audit_log` at boot (idempotent)
- **Workflow**: cron scheduler (`start_scheduler`) fires `Schedule { cron }` workflows on a 30s tick
- **Projectors**: REST routes `GET /projections` and `POST /projections/rebuild`; per-projection failures are non-fatal
- **Projectors**: auto-register a `TableProjection` per real entity model at boot (creates `{table}_projection` read tables)

### Added (Data Lifecycle & Audit)
- **Data**: soft-delete lifecycle — `delete` soft-deletes, `restore` undoes, `hardDelete` purges; `deletedRecords` lists the trash (with pagination metadata)
- **API**: `paginatedRecords` accepts `where`/`orderBy` (filtering/sorting/search with accurate `totalCount`)
- **Audit**: model mutations are auto-recorded with the acting user (`ModelEvent.actor` → audit `user_id`) via a boot-time listener
- **Admin UI**: Trash view (list/restore/purge soft-deleted records per model)

### Added (Workflow Designer)
- **SDK/UI**: `workflow-serde` layer (lossless `Workflow` JSON ↔ editor graph) with vitest round-trip tests
- **Admin UI**: `WorkflowDesigner` — list-based editor (name, trigger, ordered steps, add/remove/reorder); typed `ActionEditor` for all 5 action variants; read-only `WorkflowGraphView` preview
- **Tooling**: vitest added to `atomo-admin-ui` (`pnpm test` = type-check + unit tests)
- **API**: `GET /workflows/{name}` returns a full definition; admin UI gains workflow edit + delete

### Added (Testing)
- HTTP-layer E2E tests (`atomo_server/tests/http_e2e.rs`): in-process router via `tower::oneshot` — health, auth-gating, login→create→list
- Middleware tests (`atomo_server/tests/middleware.rs`): per-IP rate limiting + CORS headers (no DB)
- Real WASM guest test (`atomo_wasm_runtime/tests/host_api.rs`): fuel metering, `host_log`, permission-gated `host_emit_event`, `call_hook` ABI
- Workflow unit tests + pure `cron_should_fire()` helper; expanded data-layer tests (update/delete events, count, find_unique)

### Changed
- 无

### Deprecated
- 无

### Removed
- 无

### Fixed
- 修复 GraphQL 标量特征实现
- 修复 handlebars 模板类型兼容性
- 修复编译错误和依赖冲突
- Migration table names now match the SQL builder's pluralized convention (was `test_user` vs `test_users`)
- Generated `id`/primary-key columns get `PRIMARY KEY DEFAULT gen_random_uuid()`; `created_at`/`updated_at` default to `NOW()`
- UUID-shaped strings bound as native `uuid` to fix `operator does not exist: uuid = text`
- `event_log.timestamp` stored as `TEXT` to match `ModelEvent` RFC3339 string (events were being silently dropped)
- Validation rule extraction regex replaced with brace-balanced parser (rules in nested model blocks were never matched)
- JSON array/object params bound as native `jsonb` (previously stringified → `column is of type jsonb but expression is of type text` on every create with array fields)
- New user IDs use ULID (`EntityId::new()`) instead of UUID, fixing a login 500 (`EntityId` parses ULIDs, not UUIDs)
- Projection table DDL quotes column identifiers (reserved word `order` in block types broke table creation)
- Projection auto-registration skips block sub-types (no `id`) and avoids the doubled `_projection` table suffix
- **CRITICAL**: `update`/`delete` GraphQL mutations now honor their `where` filter (it was discarded — `delete` removed ALL rows of a model, `update` modified all rows)
- `paginatedRecords` now accepts `where`/`orderBy` (admin list-view filtering/search/sort was rejected as unknown arguments)
- id columns are consistently `TEXT` (EntityId is a ULID, not a UUID); equality on string values casts the column to text so it works for both TEXT and UUID columns (fixed `operator does not exist: text = uuid` on restore)
- `audit_log.operation_details` bound as `jsonb` (was failing as text → audit writes silently dropped)

### Security
- 无

## [0.1.0] - 2024-01-XX

### Added
- 项目初始化
- 基础架构设计
- 核心功能框架

---

**Legend:**
- `Added` for new features.
- `Changed` for changes in existing functionality.
- `Deprecated` for soon-to-be removed features.
- `Removed` for now removed features.
- `Fixed` for any bug fixes.
- `Security` in case of vulnerabilities.
