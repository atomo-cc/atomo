# Security & Auth

This page summarizes current auth/security practices and the target production posture.

Auth and sessions
- JWT issuance/verification; sessions persisted in Postgres.
- REST endpoints: `/auth/login`, `/auth/logout`, `/auth/me`.
- Roles: `Admin | Manager | Sales | Support | Viewer` (baseline RBAC patterns).

Passwords
- Hashing: argon2id is used in all environments; legacy bcrypt hashes are still verified so existing users migrate automatically on next login.
- Production: enforce password policies (length, complexity) via `PASSWORD_MIN_LENGTH` / `PASSWORD_REQUIRE_COMPLEXITY`.
- Secrets: use `.env` locally; secret manager in production; never commit secrets.

Multi‑tenancy
- Recommended: Postgres Row‑Level Security (RLS) with `tenant_id` column.
- Alternative: schema‑per‑tenant for small/high‑isolation sets (higher ops cost).
- Verify tenancy on every resolver/mutation path; avoid leaking cross‑tenant data in logs/metrics.

PII & compliance
- Classify sensitive fields; restrict read paths; redact in logs.
- DSAR flows: implement export/delete via eventful workflows with audit.
- Audit trail: ensure user/actor metadata on sensitive mutations.

Defensive defaults
- Rate limiting (planned): per IP/route limits at the gateway/server layer.
- Input validation: schema‑driven constraints; avoid panics; return typed errors.
- Transport security: TLS everywhere; HSTS; secure cookies for session flows.

See also
- Vision → Security, Auth, and Permissions: `/vision`
- Roadmap → Docs vs Code (ground truth): `/roadmap`
