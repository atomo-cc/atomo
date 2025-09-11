# Multi-tenant Setup

Approaches
- Row-level security (RLS) — recommended default
  - Add `tenant_id` to tables; enable RLS; write policies per role.
  - Enforce tenant scoping in every resolver/mutation path.
- Schema-per-tenant — strong isolation, higher ops cost
  - Separate schemas; distinct migrations per tenant; more complex deployments.

RLS checklist
- `tenant_id` is non-null and indexed.
- RLS policies cover SELECT/INSERT/UPDATE/DELETE per role.
- Service injects the current tenant claim into DB session or query.

Operational notes
- Backups: test tenant-scoped restore.
- Migrations: run gated migrations; verify RLS remains intact.
- Observability: include `tenant_id` in logs/metrics (without leaking PII).

See also
- Vision → Multi-tenant & Compliance: `/vision`
