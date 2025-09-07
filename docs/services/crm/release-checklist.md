# CRM Release Checklist

Use this checklist to finalize and publish the CRM service. Track status, owners, and due dates as needed.

## Data Model & API
- [ ] Finalize schema.ts: Contact, Company, Deal, Activity (timeline), Pipeline/Stage, Tags
- [ ] Validations: required fields, enums, email/phone regex
- [ ] Pagination (cursor), sorting, filter operators parity across lists
- [ ] Soft delete + restore (deletedAt) and audit visibility
- [ ] Indexes: email, companyId, stageId, updatedAt; FKs and cascades
- [ ] Migrations: generate/commit; Seeds: admin user + sample data

## Auth, RBAC, Tenancy
- [ ] Role matrix: Admin, Manager, Sales, Support, Viewer
- [ ] Row-level access: owner/team scoping, assignedTo
- [ ] Password reset + email verification; optional 2FA for Admin
- [ ] Tenancy (if SaaS): column tenant (tenantId) with enforced filters

## Admin UI
- [ ] Lists and details for Contact, Company, Deal, Activity
- [ ] Deal Kanban board by stage (drag & drop)
- [ ] Contact timeline (merged activities; inline add)
- [ ] Search + filter chips; CSV import; bulk actions
- [ ] Audit view per record (history panel)
- [ ] Settings: Roles & permissions, Users, API tokens

## Developer Experience
- [ ] SDK types/hooks generated for all CRM entities
- [ ] Deterministic codegen outputs
- [ ] Seed flow (`atomo-cli seed` from service directory; uses .env DATABASE_URL)
- [ ] Example app using SDK for core flows

## Reliability & Security
- [ ] Rate limiting: production-ready (governor/Redis‑backed)
- [ ] Centralized permission checks; deny‑by‑default
- [ ] Backups + restore runbook validated
- [ ] CSP presets and hardened defaults

## Observability & Ops
- [ ] GraphQL resolver spans + latency metrics
- [ ] DB pool metrics/alerts; auth failure/rate-limit counters
- [ ] JSON logs: request_id, user_id, path, status, latency (PII redacted)
- [ ] SLOs and alerts for error rate, readiness

## Performance
- [ ] N+1 avoidance: batched relation loads (companies, owners)
- [ ] Index audits with EXPLAIN; slow query remediation
- [ ] Caching for metadata/lookups; CDN for Admin UI

## Compliance & Data
- [ ] PII classification; logging/metrics redaction
- [ ] Export/delete endpoints (GDPR)
- [ ] Data retention for audit and activities

## Testing
- [ ] Integration tests: CRUD + RBAC + audit + auth refresh
- [ ] E2E: Admin UI core flows
- [ ] Load tests baseline (p50/p95, pool saturation)

## Packaging & Docs
- [ ] Container image & Helm chart (or deploy script)
- [ ] CI/CD: build, migrate, seed, deploy
- [ ] Docs: data model, workflows, API snippets, Admin screenshots
- [ ] Tag v0.1.0 + changelog

## Status updates (recent)
- Deal Kanban implemented with batch persist (updateDealPositions) — pending QA
- Contact timeline implemented (notes + activities) — pending QA
- CLI seed command added and documented — verified
- Migrations: events/audit functions fixed; deal.position numeric; activity table — applied
