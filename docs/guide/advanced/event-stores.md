# Custom Event Stores

Atomo ships with Postgres-backed event storage. You can experiment with custom stores for specialized needs.

Considerations
- Ordering guarantees: strict per-stream ordering; global ordering optional.
- Transactions: atomic append with optimistic concurrency (expected sequence).
- Snapshots: optional, for large aggregates; ensure snapshot/version consistency.
- Subscriptions: push vs pull; backpressure; delivery semantics (at-least-once).

Integration points
- Implement the event store trait in `crates/atomo_core`.
- Wire into the dev runtime via feature flags or config.

Testing
- Validate replay correctness under concurrent writers.
- Benchmark append/scan; test projector lag under load.

See also
- Guide → Event Sourcing: `/guide/event-sourcing`
