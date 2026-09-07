# Projections (REST)

CQRS read projections materialize denormalized read tables from the model event stream. At server boot, one `TableProjection` is auto-registered per entity model, maintaining a `{table}_projection` read table that is updated as create/update/delete events occur.

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
