//! Wire vocabulary for the realtime tier.
//!
//! These types are pure data — `serde` only. The transport layer parses an
//! inbound frame into a [`ClientMsg`], hands it to the hub, and serializes
//! outbound [`ServerMsg`]s back onto the socket. Payloads stay opaque end to
//! end (see [`Payload`]).

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::sync::Arc;

use serde::de::{Deserializer, Error as DeError};

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
///
/// Deserialization is hand-written (not `#[serde(tag = "op")]`): an
/// internally-tagged enum buffers the input into an intermediate value, and a
/// [`RawValue`] payload cannot be read back out of that buffer. Parsing through
/// a flat helper struct reads the payload straight from the input bytes, keeping
/// it opaque.
#[derive(Debug)]
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

impl<'de> Deserialize<'de> for ClientMsg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Flat, non-enum shape: deserialized directly from the input, so the
        // `RawValue` payload is preserved byte-for-byte without buffering.
        #[derive(Deserialize)]
        struct Wire {
            op: String,
            #[serde(default)]
            channel: ChannelName,
            #[serde(default)]
            payload: Option<Box<RawValue>>,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(match wire.op.as_str() {
            "subscribe" => ClientMsg::Subscribe { channel: wire.channel },
            "unsubscribe" => ClientMsg::Unsubscribe { channel: wire.channel },
            "presence" => ClientMsg::Presence { channel: wire.channel },
            "publish" => ClientMsg::Publish {
                channel: wire.channel,
                payload: wire
                    .payload
                    .ok_or_else(|| D::Error::custom("publish frame requires a `payload`"))?,
            },
            other => return Err(D::Error::custom(format!("unknown op: {other}"))),
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_msg_parses_each_op_tag() {
        assert!(matches!(
            serde_json::from_str::<ClientMsg>(r#"{"op":"subscribe","channel":"c"}"#).unwrap(),
            ClientMsg::Subscribe { channel } if channel == "c"
        ));
        assert!(matches!(
            serde_json::from_str::<ClientMsg>(r#"{"op":"unsubscribe","channel":"c"}"#).unwrap(),
            ClientMsg::Unsubscribe { channel } if channel == "c"
        ));
        assert!(matches!(
            serde_json::from_str::<ClientMsg>(r#"{"op":"presence","channel":"c"}"#).unwrap(),
            ClientMsg::Presence { channel } if channel == "c"
        ));
    }

    #[test]
    fn publish_payload_stays_byte_for_byte_opaque() {
        let msg: ClientMsg =
            serde_json::from_str(r#"{"op":"publish","channel":"c","payload":{"x":[1,2],"s":"hi"}}"#)
                .unwrap();
        match msg {
            ClientMsg::Publish { channel, payload } => {
                assert_eq!(channel, "c");
                // The hub never parses payloads; the original JSON is preserved verbatim.
                assert_eq!(payload.get(), r#"{"x":[1,2],"s":"hi"}"#);
            }
            other => panic!("expected Publish, got {other:?}"),
        }
    }

    #[test]
    fn unknown_op_is_rejected() {
        assert!(serde_json::from_str::<ClientMsg>(r#"{"op":"explode","channel":"c"}"#).is_err());
    }

    #[test]
    fn publish_without_payload_is_rejected() {
        let err = serde_json::from_str::<ClientMsg>(r#"{"op":"publish","channel":"c"}"#).unwrap_err();
        assert!(err.to_string().contains("payload"), "got: {err}");
    }

    #[test]
    fn message_serializes_with_type_tag_and_opaque_payload() {
        let payload: Payload = serde_json::from_str::<Box<RawValue>>(r#"{"k":1}"#).unwrap().into();
        let msg = ServerMsg::Message {
            channel: "room".into(),
            from: Some("alice".into()),
            payload,
        };
        let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&msg).unwrap()).unwrap();
        assert_eq!(v["type"], "message");
        assert_eq!(v["channel"], "room");
        assert_eq!(v["from"], "alice");
        assert_eq!(v["payload"]["k"], 1);
    }

    #[test]
    fn message_omits_from_when_absent() {
        let payload: Payload = serde_json::from_str::<Box<RawValue>>("true").unwrap().into();
        let msg = ServerMsg::Message {
            channel: "room".into(),
            from: None,
            payload,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("\"from\""), "from should be skipped: {json}");
    }

    #[test]
    fn presence_serializes_member_list() {
        let msg = ServerMsg::Presence {
            channel: "room".into(),
            members: vec!["a".into(), "b".into()],
        };
        let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&msg).unwrap()).unwrap();
        assert_eq!(v["type"], "presence");
        assert_eq!(v["members"], serde_json::json!(["a", "b"]));
    }

    #[test]
    fn principal_constructors_and_serde_defaults() {
        let auth = Principal::new("u1", Some("Admin".into()));
        assert!(!auth.anonymous);
        let anon = Principal::anonymous("anon:1");
        assert!(anon.anonymous && anon.role.is_none());

        // role is skipped when None; anonymous defaults to false on the wire.
        let json = serde_json::to_string(&Principal::new("u1", None)).unwrap();
        assert!(!json.contains("role"), "role should be skipped: {json}");
        let back: Principal = serde_json::from_str(r#"{"id":"u1"}"#).unwrap();
        assert_eq!(back.id, "u1");
        assert!(!back.anonymous);
        assert!(back.role.is_none());
    }
}
