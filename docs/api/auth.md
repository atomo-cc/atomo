# Auth (REST)

Atomo Server provides JWT‑based auth with sessions stored in Postgres.

Environment
- `JWT_SECRET` — HMAC secret for signing tokens (required in production)
 - `PASSWORD_MIN_LENGTH` — default 8
 - `PASSWORD_REQUIRE_COMPLEXITY` — require letters and numbers (default true)
 - `ADMIN_EMAIL` / `ADMIN_PASSWORD` — bootstrap an admin user on server start

Admin bootstrap
- On startup the server ensures platform tables exist and, if `ADMIN_EMAIL` and `ADMIN_PASSWORD` are both set, creates an admin user when that email does not already exist.
- The user is created with a ULID id, the `admin` role, and an argon2id password hash.
- Idempotent: restarting does not duplicate the user, and an existing user's password is left unchanged.

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

OAuth2/OIDC SSO
- Providers are configured via env vars and auto-discovered at startup: `google`, `github`, `microsoft`, `okta`.
- Per provider, set `OAUTH_<PROVIDER>_CLIENT_ID`, `_CLIENT_SECRET`, `_AUTH_URL`, `_TOKEN_URL`, `_USERINFO_URL`, and optionally `_REDIRECT_URI`.

```http
GET /auth/oauth/providers          # list configured providers
GET /auth/oauth/authorize?provider=google   # redirect to provider
GET /auth/oauth/callback/{provider}?code=...&state=...   # find-or-create user, returns JWT
```

Callback response
```json
{ "access_token": "<jwt>", "user": { "id": "...", "email": "...", "role": "viewer" } }
```

Notes
- Password hashing uses argon2id; existing bcrypt hashes are still verified for seamless migration.
- In production, set `JWT_SECRET` (server refuses to start without it when `ATOMO_ENV=production`).
- Include `Authorization: Bearer <jwt>` for protected routes.
 - Access tokens expire (~24h). Use `POST /auth/refresh` with a valid refresh token to rotate both.
- New OAuth users are created with the `viewer` role by default.
