# Auth (REST)

Atomo Server provides JWT‑based auth with sessions stored in Postgres.

Environment
- `JWT_SECRET` — HMAC secret for signing tokens (required in production)

Endpoints
```http
POST /auth/login
Content-Type: application/json

{ "email": "user@example.com", "password": "secret" }
```

Response
```json
{ "token": "<jwt>", "user": { "id": "...", "email": "...", "role": "viewer" } }
```

```http
POST /auth/logout
Authorization: Bearer <jwt>
```

```http
GET /auth/me
Authorization: Bearer <jwt>
```

Notes
- Development status: password hashing/verification is currently a stub in code and compares plaintext for local dev. Production uses bcrypt; ensure hashing is enabled before deploying.
- Include `Authorization: Bearer <jwt>` for protected routes.
