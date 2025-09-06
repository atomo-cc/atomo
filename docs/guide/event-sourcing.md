# Event Sourcing

Atomo stores every change as an immutable event, rebuilding state from the event log.

Benefits:
- Audit trails and time travel
- Reliable replication and projections
- Easy to derive read models

See also: `crates/atomo_core` and projectors in `crates/atomo_projectors`.
