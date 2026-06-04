# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **DX**: schema hot-reload for the no-Rust path — with `ATOMO_SCHEMA_WATCH=true` the server polls the mounted `schema.ts` (mtime, robust across Docker bind mounts) and exits on change; the compose `restart: unless-stopped` policy relaunches it, re-parsing + migrating on boot. Editing the schema is now edit-and-live (~2s) with no rebuild, restart command, or Rust. Wired into the root compose and the `atomo init` / `create-app` scaffolds.
- **Realtime**: hardening — per-client **join rate limiting** (token bucket in `atomo_realtime`, opt-in via `HubConfig.join_rate`; over-limit subscribe/session-join gets an `error` frame), and on the standalone relay: per-IP **connection caps** (`ATOMO_REALTIME_MAX_CONN_PER_IP`) and a Prometheus **`/metrics`** endpoint (hub gauges + counters). Join throttle on by default for the relay. 8 new tests (rate limiter, conn cap, metrics format, end-to-end throttle).
- **Realtime**: standalone `atomo-realtime-server` bin (`crates/atomo_realtime_server`) — runs the hub as a lightweight, **DB-free** relay (default :9100) with **stateless JWT verification** (signature + expiry against the shared `JWT_SECRET`), so it deploys as an edge/region fleet separate from the durable server. A token naming a session (`sid`) auto-binds the connection to it. Paired with `POST /realtime/token` on `atomo_server` (authenticated) that mints these short-lived tokens — the platform→relay handoff (user mgmt + matchmaking stay on the platform tier). Relay JWT verification covered by 4 tests.
- **Realtime**: coordinator sessions (RFC Phase 4) in `atomo_realtime` — host-authoritative relay over `/realtime/ws`: `session_join` assigns a stable slot and elects the first joiner as coordinator; directional relay (`to_coordinator` → host, `to_members` → host broadcast); `member_joined`/`member_left`/`coordinator_changed`/`session_closed` frames; coordinator-leave policy (`Reelect`/`Close`) via `HubConfig`. Domain-agnostic (opaque payloads) — fits multiplayer game/relay backends. 11 new tests; the WS transport forwards the new frames unchanged.
- **SDK**: `RealtimeClient` (`@atomo-cc/client-sdk` + `/realtime` subpath) — a framework-agnostic client for the ephemeral tier that mirrors the server's `/realtime/ws` protocol: subscribe/publish/presence, per-channel handlers, presence tracking, send-queue-before-open, and auto-reconnect with re-subscribe. Foundation for the CRM realtime dogfood (RFC Phase 3). 10 unit tests via a mock WebSocket.
- **Tooling**: `@atomo-cc/create-app` — a pure-JS project scaffolder (`npm create @atomo-cc/app <name>`) that writes `schema.ts` + a `docker-compose.yml` (pulling the published server image) + README, so a new project starts with **no Rust toolchain**. Templates: crm/blog/ecommerce/default (bundled copies of the canonical CLI templates).
- **Admin UI**: the server can serve a SPA at `/admin` (static-file route gated on `ATOMO_ADMIN_DIR`) when an admin bundle is provided — the `atomo-server` image itself stays **generic and bundles no admin UI** (it was CRM-coupled and doesn't belong in the core image; a game/relay backend ships none, an admin-needing app supplies its own). The admin UI's production API base is fixed to root-relative so same-origin calls (`/graphql`, `/media`) work when it *is* served at `/admin`. A `build:server` script (`vite build --base=/admin/`) builds such a bundle.
- **Distribution**: Docker image for `atomo-server` (multi-stage `Dockerfile`) + root `docker-compose.yml` (Postgres + server) so the backend runs with **no Rust toolchain on the host** — `docker compose up --build`. Docs updated (install/getting-started/deployment) with a "Run without Rust" path.
- **Distribution**: `atomo init` now scaffolds a `docker-compose.yml` (pulls the published `ghcr.io/atomo-cc/atomo-server` image, mounts the project's `atomo/schema.ts`) and a "Run it (no Rust required)" README — so a generated project runs with `docker compose up`, no Rust or CLI build.
- **CI**: `docker.yml` workflow (manual dispatch) builds and pushes the `atomo-server` image to GHCR.

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
