# Schema Metadata

Admin UI and tooling can fetch schema metadata:

```http
GET /meta/schema
```

- Returns a JSON model registry derived from `schema.ts` and platform models
- Includes table names, fields, relationships, validation rules, and UI config
- Each model's `ui` carries the schema's `ui.listView` (or `null` if not declared);
  the admin UI renders exactly those columns, in order — timestamps included
- Each model's `access` carries the schema's access-rule strings per operation
  (e.g. `"admin|manager"`, `"authenticated"`, `"system"`); the admin UI uses them
  to hide mutation controls the signed-in role can't use. Cosmetic only — the
  server enforces regardless; rules not expressible as strings serialize as `null`
- Semi‑protected route: accepts optional auth; sensitive fields are omitted by design

Raw schema in dev
- In development, the raw `schema.ts` is also served for convenience:

```http
GET /schema.ts
```

- The runtime resolves the service’s `schema.ts` with multiple fallback paths so Admin UI and tools can auto‑discover it.

Use cases
- Drive dynamic admin forms, lists, and references
- Power external tooling or validators that prefer working from raw TS schema
