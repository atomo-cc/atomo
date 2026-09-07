# Storage lifecycle implementation plan

Status: local implementation and verification complete. Feature evidence: [TODO](TODO.md). Authorized v0.7.0 publication is tracked separately in [RELEASE_TODO.md](RELEASE_TODO.md).

Provide optional, bounded model history, independent audit retention, and bounded read caching without changing the default durable model-history contract. Keep the platform generic. The original implementation scope excluded publication; the subsequent release authorization is governed by the separate release checklist. No production database reset is authorized.

## Requirements

1. Cache: explicit enable switch, global and per-model policy; TTL, optional TTI, maximum entries, estimated-byte budget, maximum entry size, expiration reclamation and metrics. Preserve tenant/query/include isolation. Reject stale read fills after writes and propagate invalidation across instances or explicitly bypass caching when consistency cannot be maintained. Optional demand-driven early refresh must coalesce, respect absolute expiration and not extend stale responses indefinitely.
2. History: full/off/retained policies with backward-compatible full default, per-model overrides, complete CRUD and bulk-path coverage, post-commit event delivery independent of history. Persist coverage gaps and retention boundaries across restarts. Keep the separate domain aggregate event store durable.
3. Audit: independent full/metadata/off policy, bounded retention, generic per-model overrides. Security/session mechanisms must not be inadvertently disabled.
4. Retention: opt-in scheduled bounded batches; age and byte budgets; no silent claim of complete replay after truncation. Projection rebuild must preflight all target histories before destructive operations. Configuration changes never imply deletion of existing history without explicit configured retention.
5. Wire settings through core builder and ServerConfig/default/from_env, document env configuration. Expose capability/usage diagnostics via authenticated administrative routes; no secrets or tenant payloads in metrics.
6. Tests: real core/HTTP/PG paths, restart policy changes, bulk operations, history-off event fan-out, audit modes, rebuild refusal before mutation, concurrent cache fills, TTL/TTI/capacity/tenant/include separation and maintenance shutdown. Use an isolated test database, serial PG-gated execution. Check workspace, test, fmt/clippy, dependent build, applicable frontend/admin smoke.
7. Docs sweep: README, Unreleased changelog, configuration, architecture, API, history/cache guides, roadmap(s), VitePress navigation. Record non-applicable surfaces explicitly. Focused conventional commits after verification; no push/release implicit.

## Collaboration boundaries

- Cache worker: cache implementation and cache-only sections of core client, cache tests.
- History worker: history/audit/retention implementation, projector guard, history-only client sections and server wiring, tests.
- Coordinator: configuration integration, diagnostics, docs, cross-feature verification and review. Coordinate shared edits before touching them.

## Validation resources

Nullable mutation regression: explicit null must become a context-typed SQL NULL for single inserts, homogeneous batch inserts and updates. Keep non-null scalar/date/JSON binding semantics and WHERE parameter numbering intact; validate with a real database before shipping. Do not alter existing live data or weaken constraints to accommodate a binding error.

This correction adds no configuration, endpoint, SDK signature or admin behavior; those feature-checklist surfaces remain unchanged. The mutation API guide and Unreleased changelog document the corrected existing contract; core tests, PostgreSQL regressions, workspace lint and dependent server build provide validation.

Prefer a single low-debug, non-incremental Cargo target directory with bounded build concurrency. Monitor free disk; avoid rebuilding duplicate debug/release profiles unnecessarily. Test services use isolated database names and are stopped after verification.

## Feature checklist applicability

- Code/config/tests/docs: cache, history, audit, replay preflight and authenticated diagnostics are updated together; validation evidence belongs in TODO.md.
- Admin UI: no new settings screen is introduced. Existing Settings and Observability audit claims now use effective runtime policy rather than schema defaults, including off/metadata/mixed/unavailable states. Extend the existing `packages/atomo-admin-ui/e2e/tests/admin-smoke.spec.ts`; run it against the real server, including an off or mixed policy configuration.
- TypeScript SDK: no new client convenience method or response schema is exported; the diagnostics endpoint is documented as REST. Existing SDK build/tests remain regression checks.
- CLI: no new command or CLI flag is introduced; standalone/core settings use environment/JSON/Rust builders and are documented in .env.example and configuration guide.
- Crate manifests: no new crate or dependency is required. Existing architecture component descriptions are updated.
- Release/CI dispatch: no release or merge is requested. Local changes remain Unreleased; release-specific publish/tag/remote CI-dispatch steps do not apply before a substantive merge.
