# Auth (REST)

Atomo Server provides JWT‑based auth with sessions stored in Postgres.

Environment
- `JWT_SECRET` — HMAC secret for signing tokens (required in production)
 - `PASSWORD_MIN_LENGTH` — default 8
 - `PASSWORD_REQUIRE_COMPLEXITY` — require letters and numbers (default true)

Endpoints
```http
POST /auth/login
Content-Type: application/json

{ "email": "user@example.com", "password": "secret" }
```

Response
```json
{ "token": "<jwt>", "refresh_token": "<refresh>", "user": { "id": "...", "email": "...", "role": "viewer" } }
```

```http
POST /auth/logout
Authorization: Bearer <jwt>
```

```http
POST /auth/refresh
Content-Type: application/json

{ "refreshToken": "<refresh>" }
```

Response
```json
{ "token": "<jwt>", "refresh_token": "<refresh>", "user": { "id": "...", "email": "...", "role": "viewer" } }
```

```http
GET /auth/me
Authorization: Bearer <jwt>
```

Notes
- Password hashing uses bcrypt. Configure cost with `BCRYPT_COST` (default 12).
- In production, set `JWT_SECRET` (server refuses to start without it when `ATOMO_ENV=production`).
- Include `Authorization: Bearer <jwt>` for protected routes.
 - Access tokens expire (~24h). Use `POST /auth/refresh` with a valid refresh token to rotate both.
