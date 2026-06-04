//! Pure, no-network exercises of the hub: fan-out, presence, and disconnect.
//! These drive the library directly the way a transport would — proving the
//! "isolated dev without standing up the whole server" path from the RFC.

use std::time::Duration;

use atomo_realtime::hub::payload_from_str;
use atomo_realtime::{ClientMsg, Connection, Hub, Principal, ServerMsg};

/// Receive the next frame for a connection, failing if none arrives promptly.
async fn next(conn: &mut Connection) -> ServerMsg {
    tokio::time::timeout(Duration::from_secs(1), conn.outbound.recv())
        .await
        .expect("timed out waiting for a frame")
        .expect("hub closed the connection")
}

/// Assert that no frame is delivered within a short window.
async fn expect_idle(conn: &mut Connection) {
    let got = tokio::time::timeout(Duration::from_millis(100), conn.outbound.recv()).await;
    assert!(got.is_err(), "expected no frame, got {got:?}");
}

#[tokio::test]
async fn publish_fans_out_to_other_subscribers_not_self() {
    let hub = Hub::new();
    let mut alice = hub.connect(Principal::new("alice", None)).await;
    let mut bob = hub.connect(Principal::new("bob", None)).await;

    // Both join "room". Each gets its own presence snapshot back first.
    alice.handle.dispatch(ClientMsg::Subscribe { channel: "room".into() }).await;
    assert!(matches!(next(&mut alice).await, ServerMsg::Presence { .. }));

    bob.handle.dispatch(ClientMsg::Subscribe { channel: "room".into() }).await;
    assert!(matches!(next(&mut bob).await, ServerMsg::Presence { .. }));
    // Alice learns Bob joined.
    match next(&mut alice).await {
        ServerMsg::Joined { who, channel } => {
            assert_eq!(who, "bob");
            assert_eq!(channel, "room");
        }
        other => panic!("expected Joined, got {other:?}"),
    }

    // Alice publishes; Bob receives, Alice does not echo to herself.
    // Parse the frame from wire JSON exactly as the server's WS pump does — this
    // is the path that an internally-tagged enum would have broken.
    let frame: ClientMsg =
        serde_json::from_str(r#"{"op":"publish","channel":"room","payload":{"typing":true}}"#)
            .unwrap();
    alice.handle.dispatch(frame).await;

    match next(&mut bob).await {
        ServerMsg::Message { channel, from, payload } => {
            assert_eq!(channel, "room");
            assert_eq!(from.as_deref(), Some("alice"));
            assert_eq!(payload.get(), r#"{"typing":true}"#);
        }
        other => panic!("expected Message, got {other:?}"),
    }
    expect_idle(&mut alice).await; // no self-echo
}

#[tokio::test]
async fn presence_snapshot_lists_members_and_disconnect_emits_left() {
    let hub = Hub::new();
    let mut alice = hub.connect(Principal::new("alice", None)).await;
    let mut bob = hub.connect(Principal::new("bob", None)).await;

    alice.handle.dispatch(ClientMsg::Subscribe { channel: "deal:1".into() }).await;
    let _ = next(&mut alice).await; // own presence snapshot
    bob.handle.dispatch(ClientMsg::Subscribe { channel: "deal:1".into() }).await;
    let _ = next(&mut bob).await; // own presence snapshot
    let _ = next(&mut alice).await; // Joined bob

    // Explicit presence query returns both members, sorted.
    alice.handle.dispatch(ClientMsg::Presence { channel: "deal:1".into() }).await;
    match next(&mut alice).await {
        ServerMsg::Presence { channel, members } => {
            assert_eq!(channel, "deal:1");
            assert_eq!(members, vec!["alice".to_string(), "bob".to_string()]);
        }
        other => panic!("expected Presence, got {other:?}"),
    }

    // Bob's connection dropping makes the hub emit a Left to Alice.
    bob.handle.disconnect().await;
    match next(&mut alice).await {
        ServerMsg::Left { channel, who } => {
            assert_eq!(channel, "deal:1");
            assert_eq!(who, "bob");
        }
        other => panic!("expected Left, got {other:?}"),
    }
}

#[tokio::test]
async fn payload_helper_round_trips_opaque_json() {
    let hub = Hub::new();
    let mut a = hub.connect(Principal::anonymous("anon-a")).await;
    let mut b = hub.connect(Principal::anonymous("anon-b")).await;
    for c in [&a, &b] {
        c.handle.dispatch(ClientMsg::Subscribe { channel: "c".into() }).await;
    }
    let _ = next(&mut a).await;
    let _ = next(&mut b).await;
    let _ = next(&mut a).await; // joined b

    let payload = payload_from_str(r#"[1,2,3]"#).unwrap();
    a.handle.dispatch(ClientMsg::Publish { channel: "c".into(), payload: serde_json::from_str(r#"[1,2,3]"#).unwrap() }).await;
    match next(&mut b).await {
        ServerMsg::Message { payload, .. } => assert_eq!(payload.get(), "[1,2,3]"),
        other => panic!("expected Message, got {other:?}"),
    }
    // The helper builds the same opaque shape used on the wire.
    assert_eq!(payload.get(), "[1,2,3]");
}
