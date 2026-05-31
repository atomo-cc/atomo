# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
