# Audit (REST)

Inspect audit logs and activity. Auth required.

> Audit entries are written automatically: by default, model mutations (create/update/delete via GraphQL) are recorded with its operation, entity, JSON details, and the acting user (`user_id`, from the JWT). A background listener on the model-event stream performs the writes.

> Access control: `GET /audit/logs`, `GET /audit/entity/...`, and `GET /audit/statistics` require the `Admin` or `Manager` role. `GET /audit/user/{id}/activity` is viewable by the user themselves, or by Admin/Manager for any user.

## Endpoints
- `GET /audit/logs` — filters: `entity_type`, `entity_id`, `user_id`, `operation`, `start_date`, `end_date`, `limit`, `offset`
- `GET /audit/user/{user_id}/activity` — optional `start_date`, `end_date`, paging
- `GET /audit/entity/{entity_type}/{entity_id}/audit` — entity audit trail
- `GET /audit/statistics` — aggregate stats

Example:
```http
GET /audit/logs?entity_type=Contact&limit=50
Authorization: Bearer <jwt>
```

Operations: `create | update | delete | read`.

## Payload and retention policy

`ATOMO_AUDIT_CONFIG` independently selects `full` (default), `metadata` or `off`, globally and per entity type. Metadata mode removes operation payload, IP and user agent while keeping operation/entity/actor/time identifiers. Off omits new entries; it does not delete existing rows. Explicit age/byte limits opt into bounded background deletion. Audit runs after model commit, so it is not a transactional replacement for the model event log or a guaranteed delivery ledger.

Existing read endpoints may therefore show partial, metadata-only or retained history. See [Storage lifecycle](/guide/storage-lifecycle) for configuration and [Storage diagnostics](/api/storage) for current policy and usage. Authentication and session state are independent from this switch.
