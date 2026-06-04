//! # atomo_realtime
//!
//! A **transport-agnostic**, in-memory hub for Atomo's *ephemeral, high-frequency*
//! real-time tier: channels, presence, and server fan-out. It is the core
//! capability described in the [Realtime Channels & Presence RFC].
//!
//! ## What this crate is — and is not
//!
//! - **No HTTP / no WebSocket.** This crate knows nothing about Axum, sockets,
//!   or framing. A transport layer (e.g. `atomo_server`) authenticates a
//!   connection, calls [`Hub::connect`], then pumps bytes between the socket and
//!   the returned [`Connection`]. That is the "one backend" seam: the server
//!   owns the route and reuses its existing auth.
//! - **No event store / no durability.** Channels are ephemeral and live only in
//!   memory; presence and per-update traffic are *never* event-sourced. Durable
//!   outcomes (later phases) leave through the normal command/mutation path —
//!   not through here. This is what keeps the high-frequency tier off the
//!   durable write path while staying in the same process.
//! - **Domain-agnostic.** Payloads are opaque ([`Payload`], a raw JSON value);
//!   the hub never inspects them, so every service and client reuses it
//!   unchanged.
//!
//! ## Shape
//!
//! A single owning task ([`hub`]) holds all state and is driven by an mpsc
//! command channel — no locks on the hot path. Each connected client gets a
//! bounded outbound queue that *sheds frames* under backpressure rather than
//! blocking the hub ([`client`]). Membership and join/leave deltas live in
//! [`presence`]; the wire vocabulary is in [`protocol`].
//!
//! [Realtime Channels & Presence RFC]: https://example.invalid/guide/advanced/realtime-channels-proposal

pub mod client;
pub mod hub;
pub mod presence;
pub mod protocol;
pub mod rate_limit;
pub mod sessions;

pub use hub::{ClientHandle, ClientId, Connection, Hub, HubConfig, StatsSnapshot};
pub use presence::Member;
pub use protocol::{ChannelName, ClientMsg, MemberInfo, Payload, Principal, ServerMsg};
pub use rate_limit::RateLimit;
pub use sessions::{CoordinatorLeavePolicy, SessionMember};
