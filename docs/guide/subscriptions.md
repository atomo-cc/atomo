---
title: Subscriptions
description: Real-time model-change events over GraphQL WebSocket.
---

# Subscriptions

Atomo pushes model changes to clients over a GraphQL WebSocket at `/graphql/ws`. Every create,
update, and delete emits a `ModelEvent` on the stream.

## Subscribing

```graphql
subscription {
  modelChanges(model: "Deal") {
    eventType   # Created | Updated | Deleted | Custom
    modelName
    eventId
  }
}
```

`modelChanges` is filtered to the `model` you request — a `Deal` subscription won't receive
`Contact` events.

## Authentication & access

The WebSocket is authenticated: pass a JWT in the `connection_init` payload, e.g.

```json
{ "authorization": "Bearer <jwt>" }
```

Connections without a valid token are rejected, and the subscription is gated by the model's
`read` [access rule](/guide/advanced/access-hooks) — a role that can't read the model can't
subscribe to it.

## Multi-tenant scoping

If the `connection_init` payload includes a `tenant` field, the stream only delivers that
tenant's events (see [Multi-tenant](/guide/advanced/multi-tenant)).

## SDK

The TypeScript SDK's `SubscriptionBuilder` filters by model and event type:

```ts
client.subscribe('Deal').on_create().stream()  // only Deal Created events
```

## See also
- [GraphQL API](/api/graphql) — full operation set
- [Event Sourcing](/guide/event-sourcing) — where the events come from
