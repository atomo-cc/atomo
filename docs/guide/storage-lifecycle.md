# Storage lifecycle

Available in v0.7.0, storage lifecycle separates current model state, model mutation history, operation audit and read caching. Defaults preserve full model history and full audit payloads; cache defaults remain enabled with a 60-second TTL. Implementation evidence is tracked in the repository's `docs/implementation/storage-lifecycle/TODO.md`; publication verification is tracked separately in `RELEASE_TODO.md` in the same directory.

## Independent responsibilities

| Layer | Owner | Policy |
| --- | --- | --- |
| Current model rows | Core CRUD transactions | Saved regardless of model-history mode |
| Model change history (`event_log`) | `atomo::event_store::EventStore` | `full`, `off`, or `retained` |
| Operation audit (`audit_log`) | Server audit service | `full`, `metadata`, or `off`, with independent retention |
| Read cache | `atomo::cache::ReadCache` | [TTL, TTI and capacity](/guide/caching) |
| Aggregate event sourcing | `atomo_core` aggregate store | Separate durable contract; unaffected by these switches |

The model row and configured model-history write are part of the same CRUD transaction, including bulk paths. Post-commit notifications remain enabled when model-history saving is off. These in-memory notifications are not a durable outbox: disabling history does not introduce guaranteed delivery to disconnected consumers. Account/session persistence is not disabled by an audit or model-history setting.

## History configuration

`ATOMO_HISTORY_CONFIG` is a JSON object; omitted fields use defaults:

```bash
ATOMO_HISTORY_CONFIG='{"default":{"mode":"full"},"models":{"Counter":{"mode":"off"},"Activity":{"mode":"retained","max_age_secs":86400,"max_bytes":268435456}},"maintenance_interval_secs":60,"batch_size":1000}'
```

Use actual schema model names. Each model policy replaces the default history policy; it is not a field-by-field merge. `full` saves complete mutation payloads without automatic deletion. `off` omits new history payloads but records a persistent coverage gap when a mutation occurs. It does not delete existing rows. `retained` saves new history and explicitly authorizes background deletion by age and/or estimated-byte budget. It requires at least one positive limit; limits on `full`/`off` history are rejected.

No mode change resets a world, current model table, database or event sequence. Returning to `full` only saves future history and cannot fill a previous gap.

## Audit configuration

```bash
ATOMO_AUDIT_CONFIG='{"default":{"mode":"full"},"models":{"Counter":{"mode":"metadata","max_age_secs":86400,"max_bytes":67108864}},"maintenance_interval_secs":60,"batch_size":1000}'
```

`full` retains the normal audit details. `metadata` keeps operation/entity/actor/time identifiers, replaces operation details with `{}`, and omits IP address and user agent. `off` omits new audit entries. Audit history is independent from model history: turning one off does not turn the other off. Per-model policies replace the default audit policy. Age/byte limits explicitly enable deletion of old audit entries, including when new audit saving is off. Without those limits existing audit rows remain.

Audit entries are written by the server's post-commit event listener, not in the core model transaction. Do not treat this optional listener as a transactionally complete security ledger. Authentication, authorization and durable jobs keep their own contracts.

## Bounded maintenance

Server-owned maintenance tasks run at `maintenance_interval_secs` (1?86400 seconds), deleting at most `batch_size` rows per pass (1?10000). They stop with the server scope. A library-only caller can invoke `EventStore::maintain()` explicitly; merely constructing a client does not start the server's history/audit maintenance loops.

Age and byte limits apply **per model/entity type**, including a default policy reused by several models. They are not a single database-wide quota. When both limits exist, rows exceeding either qualify for removal; newest rows are preferred. Batching and concurrent writes mean a table may temporarily exceed its target. Limits do not reject otherwise valid business writes to enforce a strict disk ceiling.

Byte accounting uses PostgreSQL row-size estimates. Indexes, WAL, table free space, TOAST/allocator overhead and unrelated storage require separate monitoring. SQL deletion makes space available for reuse and does not guarantee an immediate reduction in database files or a host virtual disk. These policies are not a backup strategy.

## Replay capability and historical gaps

`model_history_coverage` persists mode, completeness, first gap, retention boundary and removed-event count across restarts. Disabling saves and later enabling them cannot silently restore `complete=true`. Retention and its coverage update commit together.

Projection rebuild checks every target model before clearing any target. Models in `off`/`retained` mode or with incomplete history refuse full replay with `HISTORY_REPLAY_UNAVAILABLE`. Built-in projections rebuild together in a single transaction; unsupported custom rebuild implementations fail before any target is cleared. Replay reads and rebuild coordinate with history-policy/retention changes through database lifecycle locks. All instances sharing a database must use a consistent history policy; rolling deployments with conflicting policies are not a supported way to establish replay completeness. Existing legacy databases without coverage metadata retain their legacy behavior; metadata is not independent proof that their earlier logs were complete.

There is no API to erase the gap marker or fabricate a complete baseline. A future recovery procedure must establish and verify a real baseline before changing capability. Do not manually mark incomplete history complete to bypass rebuild protection. See [Projections API](/api/projections).

Automatic table projections use current base rows to repair missing rows and add missing projection columns at startup. This current-state synchronization works with history off and does not read event history, reset coverage gaps, or claim historical replay is possible. Existing extra columns remain; incompatible column types require an explicit migration. Runtime refreshes re-read current state so delayed notifications cannot overwrite it with an old payload.

## Operator workflow

1. Read [`GET /storage/diagnostics`](/api/storage) as an administrator to establish current usage and coverage.
2. Choose model-specific history/audit policies based on recovery and auditing requirements. Keep full history for records that require complete replay.
3. Configure bounded cache separately; use multi-instance bypass when local invalidation is insufficient.
4. Restart with explicit configuration, observe maintenance and workload behavior, and monitor total database/host space independently.

Rust configuration uses `HistoryConfig` and `AuditConfig` in `atomo::history`, `Atomo::builder().history_config(config)` and `ServerConfig.history`/`ServerConfig.audit`. Environment parsing rejects unknown fields and invalid limits. Policies are deployment configuration, not mutable through the diagnostics API.

## Existing admin audit displays

Settings shows effective audit saving as Full details, Metadata only, Off, or Per-model policy. Observability describes the same policy without promising complete delivery. If diagnostics fail or the user lacks administrator access, the interface reports an unknown policy rather than inferring enabled saving from schema metadata. These displays do not change policy or add an administrative retention trigger.
