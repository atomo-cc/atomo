//! Wire vocabulary for the realtime tier.
//!
//! These types are pure data — `serde` only. The transport layer parses an
//! inbound frame into a [`ClientMsg`], hands it to the hub, and serializes
//! outbound [`ServerMsg`]s back onto the socket. Payloads stay opaque end to
//! end (see [`Payload`]).

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::sync::Arc;

/// A channel (a.k.a. room) name. Opaque to the hub.
pub type ChannelName = String;

/// An opaque, application-defined message body.
///
/// Stored as a reference-counted [`RawValue`] so the hub can fan one publish out
/// to many subscribers without re-parsing or re-serializing it — and so it never
/// has to understand any service's domain types.
pub type Payload = Arc<RawValue>;

/// Who a connection belongs to, as seen by the hub.
///
/// The transport maps its own auth principal (e.g. a verified JWT user) onto
/// this. Anonymous connections are allowed where a service opts in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Principal {
    /// Stable identifier surfaced in presence and as the `from` of messages.
    pub id: String,
    /// Optional role label, carried through for transport-side policy. The hub
    /// does not interpret it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// True when the connection was not authenticated.
    #[serde(default)]
    pub anonymous: bool,
}

impl Principal {
    /// An authenticated principal with a stable id and optional role.
    pub fn new(id: impl Into<String>, role: Option<String>) -> Self {
        Self {
            id: id.into(),
            role,
            anonymous: false,
        }
    }

    /// An anonymous principal (no verified identity).
    pub fn anonymous(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: None,
            anonymous: true,
        }
    }
}

/// A frame sent by a client to the hub. Tagged by `op`.
///
/// ```json
/// {"op":"subscribe","channel":"deal:42"}
/// {"op":"publish","channel":"deal:42","payload":{"typing":true}}
/// ```
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Join a channel: receive its messages and presence, and appear in it.
    Subscribe { channel: ChannelName },
    /// Leave a channel.
    Unsubscribe { channel: ChannelName },
    /// Publish an opaque payload to every other subscriber of a channel.
    Publish {
        channel: ChannelName,
        payload: Box<RawValue>,
    },
    /// Request a one-shot membership snapshot for a channel.
    Presence { channel: ChannelName },
}

/// A frame sent by the hub to a client. Tagged by `type`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// A payload published to a subscribed channel.
    Message {
        channel: ChannelName,
        /// Principal id of the publisher, when known.
        #[serde(skip_serializing_if = "Option::is_none")]
        from: Option<String>,
        payload: Payload,
    },
    /// Another principal joined a channel this client is subscribed to.
    Joined { channel: ChannelName, who: String },
    /// Another principal left a channel this client is subscribed to.
    Left { channel: ChannelName, who: String },
    /// Membership snapshot — principal ids currently in the channel.
    Presence {
        channel: ChannelName,
        members: Vec<String>,
    },
    /// A non-fatal error (e.g. an unparseable client frame).
    Error { message: String },
}
