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
- **Create-once, keyed by email.** Restarting does not duplicate the user, and an existing admin's password is **not** updated from `ADMIN_PASSWORD`. Two consequences to know:
  - Changing `ADMIN_PASSWORD` and restarting is otherwise a silent no-op (the old password keeps working). The server now logs a `WARN` when the env password differs from the stored one. To actually rotate it on boot, set **`ADMIN_RESET_PASSWORD=true`** (a one-shot that updates the existing admin's hash from `ADMIN_PASSWORD`); unset it again afterward.
  - Changing `ADMIN_EMAIL` seeds an **additional** admin (the old one remains). Remove stale admins manually if that's not intended.

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

Self-registration (optional, default off)
- `POST /auth/register` is mounted **only** when `ATOMO_ENABLE_SELF_REGISTRATION=true`; otherwise the
  route does not exist (`404`). The platform never exposes an open sign-up surface by default.
- Provisioning is configurable so the platform imposes no convention:
  - `ATOMO_SELF_REGISTRATION_ROLE` — role granted to new users (default `viewer`).
  - `ATOMO_SELF_REGISTRATION_TENANT` — `none` (default; unscoped `tenant_id = NULL`), `per-user`/`self`
    (each user is its own tenant), or any other value used as a fixed shared tenant id.
- Duplicate emails are race-safe: registration relies on the `users.email` unique constraint, so a
  duplicate returns `409 Conflict` rather than creating a second user.

```http
POST /auth/register     # only when ATOMO_ENABLE_SELF_REGISTRATION=true
Content-Type: application/json

{ "email": "user@example.com", "password": "secret123", "firstName": "A", "lastName": "B" }
```

Response (same shape as login; role/tenant follow the configured policy)
```json
{ "token": "<jwt>", "refresh_token": "<refresh>", "user": { "id": "...", "email": "...", "role": "viewer", "tenant_id": null } }
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
