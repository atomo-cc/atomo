# Audit (REST)

Inspect audit logs and activity. Auth required.

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
