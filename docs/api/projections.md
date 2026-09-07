# Projections (REST)

At server boot, one `CurrentStateProjection` is auto-registered per entity model. Its read table is synchronized from the current base table, then model notifications refresh the affected entity from current base state.

Projection columns retain the existing TEXT representation: base SQL NULL becomes an empty string, while numbers and booleans become text. This matches `TableProjection` event handling. Query the base table when distinguishing SQL NULL from an empty value matters; the projection representation does not change base-column nullability or historical completeness.

Startup migration adds missing nullable TEXT projection columns in a transaction. Existing columns are preserved; incompatible types fail explicitly with `CURRENT_PROJECTION_SCHEMA_CONFLICT`, without silently dropping or retyping them. Startup synchronization upserts current active rows and removes projection rows whose base records are absent or soft-deleted. It repairs missed notifications and works with history disabled. Existing extra projection columns are preserved for retained rows.

This is **current-state synchronization**, not historical replay: it never reads `event_log`, repairs missing history or clears coverage gaps. Source writes briefly wait while the initial snapshot is synchronized. Unchanged startup rows are not rewritten. Mutations, initialization and refresh acquire the shared lifecycle lock before base/projection locks; historical rebuild and retention take its exclusive side. Per-entity live refreshes are serialized and re-read current base state, so an old queued notification cannot restore an outdated payload. Notification delivery remains best-effort; a later restart can reconcile missed changes.

Both endpoints require an authenticated administrator. Missing or invalid authentication returns HTTP 401; authenticated non-administrators receive HTTP 403.

## Endpoints

```http
GET /projections             # list registered projections (name + source model)
POST /projections/rebuild     # truncate and rebuild all projections
```

## Responses

```json
// GET /projections
{ "projections": [ { "name": "contacts_projection", "source_model": "Contact" } ] }
```

```json
// POST /projections/rebuild
{ "status": "rebuilt", "count": 4 }
```

Notes
- Rebuild first verifies every source model has full, complete history before any projection is cleared. Off/retained policies or persistent coverage gaps refuse rebuild with `HISTORY_REPLAY_UNAVAILABLE`; changing policy back to full does not recreate missing events.
- Built-in table projections rebuild in one shared transaction: a later failure rolls back earlier target changes. Custom projections must explicitly implement connection-bound transactional rebuilding; unsupported implementations fail before mutation.
- Only real entity models (those with an `id` field) get projections; enum and embedded block sub-types are skipped.

History coverage and current policy are available through the administrator-only [Storage diagnostics API](/api/storage). [Storage lifecycle](/guide/storage-lifecycle) explains retention, legacy metadata behavior and the absence of a capability-reset shortcut.

## Rebuild errors

- HTTP 409, `{"code":"HISTORY_REPLAY_UNAVAILABLE","message":"Complete model history is unavailable. Projections were not rebuilt."}`: a policy or coverage gap prevents full replay.
- HTTP 409, code `PROJECTION_REBUILD_UNSUPPORTED`: a custom projection does not implement the transactional rebuild contract.
- HTTP 503, code `PROJECTION_REBUILD_FAILED`: an unexpected rebuild failure. Responses do not expose SQL details.

Custom Rust projections opt in with `supports_transactional_rebuild()` and `rebuild_in(&mut PgConnection)`. Perform all rebuild writes through the provided connection so the manager can roll back every target on error. Do not create a separate transaction or acquire another pool connection inside that method.
