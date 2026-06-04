# CRM Refinement & Dogfooding Roadmap

> Status: **Living document** · Flagship service: `services/crm-service`
>
> The CRM is Atomo's flagship dogfood: we refine it as a real product, and use
> the friction we hit to drive **domain-agnostic platform capabilities** in
> `crates/atomo_*`. This roadmap tracks both — CRM product polish, and the
> platform features that polish surfaces.
>
> Guiding rule: anything promoted into the platform core must stay **domain-
> agnostic and reusable** by any service or external client — never CRM- (or any
> single app-) specific.

## A. CRM product refinement

At a glance (detail lives in the [Release Checklist](/services/crm/release-checklist)):

- **Data model completeness** — Contact, Company, Deal, Activity, Pipeline/Stage,
  Tags; validations, soft-delete/restore, indexes & migrations.
- **RBAC & tenancy** — role matrix, row-level (owner/team) scoping, optional
  multi-tenant filters.
- **Admin UI** — lists/details, Deal Kanban, Contact timeline, search/filters,
  CSV import, per-record audit panel.
- **DX** — generated SDK types/hooks, deterministic codegen, seed flows, example
  app exercising core flows.
- **Reliability / Observability / Performance** — rate limiting, N+1 avoidance,
  resolver spans, structured logs, SLOs.

These are product goals; the items below are the **platform capabilities** the
CRM dogfood is driving.

## B. Platform capabilities driven by CRM dogfooding

### B1. Realtime Channels & Presence — *core feature, CRM-driven*

This has been promoted into the **Atomo core** as a domain-agnostic capability;
the canonical spec lives at
**[Proposal: Realtime Channels & Presence](/guide/advanced/realtime-channels-proposal)**.
The CRM is its **first dogfood / requirements driver** — it is *not* a CRM
feature, but a common platform capability the CRM happens to surface first.

> **Status:** the core (Phase 2) is built — `crates/atomo_realtime` (the
> transport-agnostic hub) plus the `/realtime/ws` transport in `atomo_server`,
> with channels, presence, and fan-out. The CRM dogfood below is **Phase 3**: the
> next step is wiring these CRM surfaces onto the hub.

**What the CRM needs from it** (the dogfood requirements):

- **Presence** — who is currently viewing/editing a Deal or Contact.
- **Collaborative editing** — live "someone is editing" indicators; optional
  soft field locks.
- **Live views** — Kanban cards, dashboards and activity feeds updating across
  all open clients instantly.
- **Transient notifications** — assignment / @mention toasts.

All of the above are *transient* signals that must **not** become domain events —
exactly the ephemeral, high-frequency tier the core proposal defines (with only
durable *outcomes* committed to `atomo_core`). See the proposal for the design,
boundary, and phasing.

### B2. (Future) Other capabilities

As the CRM dogfood surfaces them — e.g. richer workflow triggers, audit/history
UX, import/export tooling — capture them here before promotion into the core,
held to the same domain-agnostic bar.
