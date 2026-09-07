# Event Sourcing

Atomo separates model mutation history from aggregate event sourcing. Model CRUD saves full history by default and delivers post-commit events for projections/subscriptions; deployments can explicitly choose partial or disabled model history. The separate aggregate event store keeps its durable contract. Complete recovery requires complete history or a verified baseline, not merely a working notification stream.

See [Storage lifecycle](/guide/storage-lifecycle) for `full`/`off`/`retained` policy, persistent coverage gaps and independent operation auditing. Turning off model-history persistence does not disable live notifications or delete existing history. Live notifications are not a durable outbox.

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
- Clear + replay: preflight every target model for complete full history before clearing any projection. Off/retained policies or a recorded gap refuse full rebuild; changing the mode back to full does not repair missing events.
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
