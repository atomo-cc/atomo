---
title: "Proposal: Realtime Channels & Presence"
description: A domain-agnostic Atomo core capability for ephemeral, high-frequency real-time channels, presence, and coordinator sessions — complementing durable GraphQL subscriptions.
---

# Proposal: Realtime Channels & Presence

> Status: **Phases 2 & 4 implemented** (channels + presence + fan-out + coordinator
> sessions) · Layer: **Atomo core** (`crates/atomo_realtime`, mounted into
> `atomo_server`)
>
> A **core, domain-agnostic** real-time capability for *ephemeral, high-frequency*
> traffic — presence, live fan-out, and host-authoritative coordinator sessions —
> that any service or client can use. It complements (does not replace) the
> durable real-time path. The natural consumer is host-authoritative relay
> (e.g. multiplayer game backends), not CRM dashboards.
>
> The hub, WS transport, and coordinator sessions are built and tested. What
> remains is hardening (Phase 5: standalone bin, rate limits, metrics). See
> [Status & usage](#status-usage-phase-2) below.

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

## Status & usage (Phase 2)

What exists today, in `crates/atomo_realtime` (transport-agnostic library) plus
the WS transport in `atomo_server`:

**Crate shape** — the hub is pure, in-memory, and knows nothing about HTTP or the
event store. A single owning task holds all state and is driven by mpsc commands
(no hot-path locks); each client gets a bounded outbound queue that *sheds frames*
under backpressure rather than stalling the hub. Payloads are opaque
(`Arc<RawValue>`) and never parsed by the hub.

| Module | Responsibility |
| --- | --- |
| `hub` | owning task: `channel → subscribers`, fan-out, stats |
| `presence` | per-channel membership, snapshots, join/leave deltas |
| `protocol` | `ClientMsg` / `ServerMsg` wire vocabulary |
| `client` | bounded per-client delivery with frame shedding |

**Endpoints** (mounted by `atomo_server`, gated by `enable_realtime`):

- `GET /realtime/ws` — WebSocket. Authenticate the upgrade with `?token=<jwt>`
  (reuses the server's JWT path); anonymous connections are rejected unless
  `ATOMO_REALTIME_ALLOW_ANON=true`.
- `GET /realtime/health` — liveness plus hub counters (connections, channels,
  messages, dropped frames).

**Wire protocol** — JSON text frames. Client → server (tagged by `op`):

```json
{"op":"subscribe","channel":"deal:42"}
{"op":"publish","channel":"deal:42","payload":{"typing":true}}
{"op":"presence","channel":"deal:42"}
{"op":"unsubscribe","channel":"deal:42"}
```

Server → client (tagged by `type`): `message`, `joined`, `left`, `presence`,
`error`. A publish fans out to every *other* subscriber (no self-echo).

**Coordinator sessions** — for host-authoritative relay, a session has one
elected coordinator (the first joiner). Client → server:

```json
{"op":"session_join","session":"match-1"}
{"op":"to_coordinator","session":"match-1","payload":{"input":1}}   // member → host
{"op":"to_members","session":"match-1","payload":{"snapshot":1}}    // host → members
{"op":"session_leave","session":"match-1"}
```

Server → client: `session_start` (your slot, `coordinator` flag, roster),
`member_joined` / `member_left`, `from_member` (→ coordinator), `from_coordinator`
(→ members), `coordinator_changed` (re-election), `session_closed`. Slots are
stable per session. The coordinator-leave policy (`Reelect` / `Close`) is set via
`HubConfig`. Payloads stay opaque — the hub never inspects game/app state.

**Deployment & auth (two tiers).** The hub can run two ways:

1. *Mounted in `atomo_server`* (`/realtime/ws`) — shares the server's auth, for
   in-app realtime alongside the durable API.
2. *Standalone* (`crates/atomo_realtime_server`, the `atomo-realtime-server` bin,
   default port 9100) — a lightweight, DB-free relay you run as a fleet of
   edge/region-local processes (a game's relay servers).

The standalone relay does **stateless JWT verification only** (signature + expiry
against the shared `JWT_SECRET`) — it never touches the user database. The
platform tier owns users/auth/matchmaking and hands off via a short-lived token:

```
  atomo_server (DB)  ──POST /realtime/token {session}──►  signed token { sub, sid, exp }
        │  (matchmaking decides the session)                       │
        ▼                                                          ▼
   client gets the token ──────────ws://relay/ws?token=───►  atomo-realtime-server
                                                              verifies signature only,
                                                              binds the conn to `sid`
```

So the relay is *authenticated* without *managing users*: auth at the edge =
signature check; user management + matchmaking stay on the platform tier.

**Hardening** — the standalone relay caps concurrent connections per IP
(`ATOMO_REALTIME_MAX_CONN_PER_IP`, default 64; 0 = off), throttles per-client
joins with a token bucket (`ATOMO_REALTIME_JOIN_BURST` / `ATOMO_REALTIME_JOIN_PER_SEC`,
default 20 / 10·s⁻¹ — over-limit joins get an `error` frame), and exposes
Prometheus counters at `GET /metrics`. Join throttling is also available in-library
via `HubConfig.join_rate` (off by default).

**Flags** — `ATOMO_ENABLE_REALTIME` (default on), `ATOMO_REALTIME_ALLOW_ANON`
(default off). Standalone bin also reads `JWT_SECRET`, `PORT`,
`ATOMO_REALTIME_COORDINATOR_POLICY` (`reelect`/`close`).

**Isolated dev** — the hub runs with no network: `cargo test -p atomo_realtime`
drives it directly (unit tests for presence/protocol/client/sessions + integration
tests in `tests/hub.rs` and `tests/sessions.rs`).

## First dogfood: CRM

The [CRM roadmap](/services/crm/roadmap) drives the initial requirements —
presence on a Deal/Contact, live Kanban updates, "someone is editing"
indicators — but nothing in the subsystem is CRM-specific. Any future service or
high-frequency client benefits from the same primitives.

## Phasing

1. ✅ RFC + this doc — boundary and crate placement agreed.
2. ✅ `crates/atomo_realtime` (library) + `atomo_server` WS transport:
   `/realtime/health`, `/realtime/ws`, channels + presence + fan-out.
3. ⏭️ CRM dogfood (presence/live-Kanban) — **dropped**: the right consumer of the
   ephemeral tier is host-authoritative relay (e.g. multiplayer game backends),
   not CRM dashboards. See Phase 4.
4. ✅ **Coordinator sessions** — host-authoritative relay: join → stable slot,
   one elected coordinator, directional relay (`to_coordinator` / `to_members`),
   member join/leave, and configurable coordinator-leave policy.
5. ✅ Harden: standalone **`atomo-realtime-server`** bin (**stateless JWT auth,
   no DB**) + `POST /realtime/token` mint endpoint; per-client **join rate limits**
   (token bucket via `HubConfig`), per-IP **connection caps**, and Prometheus
   **`/metrics`** on the relay. *(Later: binary framing.)*

## Open questions

Resolved:

- **Dedicated crate vs. extending the subscriptions stack** → a dedicated
  *library* crate (`crates/atomo_realtime`), mounted into `atomo_server` so it
  reuses auth/rate-limit/deploy while staying a logically isolated module.
- **Anonymous identity by default** → off by default; a service opts in with
  `ATOMO_REALTIME_ALLOW_ANON`. Authenticated connections pass `?token=<jwt>`.
- **Coordinator-session failover** → configurable via `HubConfig`: `Reelect`
  (promote the oldest remaining member, default) or `Close` (end the session and
  notify members). Games that run host-authoritative simulation typically pick
  `Close`.

Still open:

- Presence storage: per-node in-memory (today) vs. shared (Redis) for multi-node
  fan-out.
- Backpressure policy: today a full per-client queue sheds the newest frame;
  revisit if a use case needs drop-oldest or guaranteed delivery.
