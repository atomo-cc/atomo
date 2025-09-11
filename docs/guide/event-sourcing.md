# Event Sourcing

Atomo models every change as an immutable event and derives read models via projections. This enables auditability, deterministic recovery (replay), and scalable query patterns (CQRS).

Core concepts
- Event streams: per aggregate or contextual streams with strict ordering.
- Event envelopes: type, payload, metadata (actor, time, version), stream id, sequence.
- Projections: idempotent handlers that build read models in Postgres/search/KV.
- Checkpoints: per‑projector positions (offset/time) stored transactionally.

Evolution and compatibility
- Versioning: event types carry a semantic version; breaking changes increment the major.
- Upcasting: old events are adapted in memory to the latest schema before handling.
- Backfills: use replay with upcasters to re‑materialize read models safely.

Replay and recovery
- Clear + replay: for corrupted read models, truncate the target and rebuild from the log.
- Targeted replay: start from a checkpoint (time/offset) for faster recovery.
- Idempotency: projector logic must be idempotent; use UPSERTs/merge semantics.

Consistency model
- Commands append events atomically.
- Projections update read models asynchronously (eventual consistency).
- For UI, prefer optimistic updates + subscription refresh.

Tips
- Keep event payloads minimal and domain‑oriented.
- Avoid encoding read‑model shape in events; let projections own tables/indexes.
- Partition large projections by tenant or domain to shorten replay time.

See also
- `crates/atomo_core` (event types, append) and projector examples
- Guide → Database & Projections: `/guide/database`
