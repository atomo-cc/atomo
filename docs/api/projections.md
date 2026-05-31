# Projections (REST)

CQRS read projections materialize denormalized read tables from the model event stream. At server boot, one `TableProjection` is auto-registered per entity model, maintaining a `{table}_projection` read table that is updated as create/update/delete events occur.

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
- Rebuild is resilient: a failure on one projection is logged and skipped rather than aborting the whole request.
- Only real entity models (those with an `id` field) get projections; enum and embedded block sub-types are skipped.
