---
title: Local-First (planned)
description: Offline-first client sync — on the roadmap, not yet implemented.
---

# Local-First Architecture *(planned)*

Offline-first operation with on-device source-of-truth and sync-on-reconnect is part of the Atomo
vision but is **not yet implemented end-to-end**. See the [roadmap](/roadmap).

## What exists today

The TypeScript SDK includes an **experimental** offline queue with sync-on-reconnect scaffolding,
but it isn't integration-tested and shouldn't be relied on yet. The server side (event sourcing)
provides the change history a sync engine would consume.

→ See [Event Sourcing](/guide/event-sourcing) and [Subscriptions](/guide/subscriptions).
