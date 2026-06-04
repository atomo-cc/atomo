---
title: "Proposal: Realtime Channels & Presence"
description: A domain-agnostic Atomo core capability for ephemeral, high-frequency real-time channels, presence, and coordinator sessions — complementing durable GraphQL subscriptions.
---

# Proposal: Realtime Channels & Presence

> Status: **Proposed (RFC)** · Layer: **Atomo core** (`crates/atomo_realtime`) ·
> First dogfood: the [CRM service](/services/crm/roadmap)
>
> A **core, domain-agnostic** real-time capability for *ephemeral, high-frequency*
> traffic — presence, live fan-out, and optional coordinator sessions — that any
> service or client can use. It complements (does not replace) the durable
> real-time path.

## Where it fits

Atomo already has a **durable** real-time path:

- **[GraphQL Subscriptions](/guide/subscriptions)** push create/update/delete
  events as they happen — backed by the event store.
- **[Collaboration](/guide/collaboration)** (CRDT editing) is planned on top of
  that durable foundation.

What's missing is the **ephemeral, high-frequency tier**: transient signals that
must travel fast and should **never** become domain events — presence,
"who's-typing", live UI nudges, and low-latency session fan-out. This proposal
adds that tier as a first-class core feature.

| Tier | Mechanism | Persisted? | Use |
| --- | --- | --- | --- |
| Durable | GraphQL Subscriptions / event store | ✅ events | data changes, history |
| **Ephemeral (this)** | **Realtime Channels** | ❌ in-memory | presence, live fan-out, sessions |

## Capability

A domain-agnostic subsystem (`crates/atomo_realtime`) providing:

- **Channels / rooms** — subscribe, publish, server fan-out to subscribers.
- **Presence** — join/leave, membership snapshots, last-seen.
- **Ephemeral messaging** — small, frequent, low-latency, opaque payloads.
- **Coordinator sessions** *(optional/advanced)* — a session where one
  participant is designated authoritative and others exchange messages relayed
  through it, for workloads needing a single in-session source of truth.

## The boundary (most important rule)

```
                ┌──────────────── atomo_server (one process) ───────────────┐
  clients ──WS──┼─►  /realtime/ws  ──►  atomo_realtime (lib)                 │
                │     (auth + rate-limit reuse)   • channels / presence       │
                │                                 • fan-out / coordinator      │
                │                                       │ durable OUTCOMES     │
                │     /graphql, /graphql/ws  ───────────┼── only (command) ──► │
                │                                       ▼                      │
                │                            atomo_core (event store + GraphQL)│
                └────────────────────────────────────────────────────────────┘
```

- The realtime hub is a **library crate mounted into `atomo_server`** (same
  process), sharing its auth, rate-limit, metrics, and deploy — *not* a separate
  server. It is a **logically isolated module**: its state is ephemeral and
  in-memory and it never imports `event_store`.
- Channels/presence/per-update traffic is **never event-sourced**.
- Only **durable outcomes** (the *result* of an interaction) are committed to
  `atomo_core` via the normal command API — keeping the event log clean and the
  high-frequency path off the durable write path.

## Design principles

- **Domain-agnostic transport.** Channels, presence and sessions carry opaque
  payloads; the subsystem knows nothing about any service's domain types. This is
  what lets *every* service — and external clients — reuse it unchanged.
- **High-frequency, low-latency first.** Sized for small, frequent messages with
  backpressure (drop stale frames for slow subscribers rather than block the
  hub). This headroom is what makes the feature broadly reusable.
- **Auth via Atomo identity** on the WS handshake (anonymous allowed where a
  service opts in); per-IP connection caps and join rate limits.
- **Observability** — channel/presence counts, msgs/s, fan-out latency, dropped
  frames; structured logs without payload PII.

## First dogfood: CRM

The [CRM roadmap](/services/crm/roadmap) drives the initial requirements —
presence on a Deal/Contact, live Kanban updates, "someone is editing"
indicators — but nothing in the subsystem is CRM-specific. Any future service or
high-frequency client benefits from the same primitives.

## Phasing

1. RFC + this doc — agree on the boundary and crate placement.
2. `crates/atomo_realtime`: `/health`, `/ws`, channels + presence + fan-out.
3. CRM dogfood: presence + live Kanban; durable outcome → `atomo_core`.
4. Optional coordinator-session mode.
5. Harden: auth, rate limits, metrics; (later) binary framing.

## Open questions

- New crate `crates/atomo_realtime` vs. extending the existing subscriptions/WS
  stack? (Leaning: dedicated crate, to isolate the high-frequency tier.)
- Presence storage: per-node in-memory vs. shared (Redis) for multi-node fan-out.
- Coordinator-session failover: re-elect within the session, or end the session?
- Anonymous identity by default, or require Atomo auth per channel policy?
