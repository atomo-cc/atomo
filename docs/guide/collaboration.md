---
title: Real-time Collaboration (planned)
description: CRDT-based collaborative editing — on the roadmap, not yet implemented.
---

# Real-time Collaboration *(planned)*

CRDT-based collaborative editing (conflict-free multi-user editing, live cursors) is part of the
Atomo vision but is **not yet implemented**. See the [roadmap](/roadmap).

## What works today

Real-time **change notifications** are shipped via GraphQL subscriptions — clients are pushed
create/update/delete events as they happen. That's the foundation a collaboration layer would
build on, but conflict-free merging itself isn't built yet.

→ See [Subscriptions](/guide/subscriptions).
