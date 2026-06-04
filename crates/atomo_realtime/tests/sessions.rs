//! End-to-end coordinator-session exercises driving the hub directly (no network),
//! the way the WS transport would: join → directional relay → leave/re-elect/close.

use std::time::Duration;

use atomo_realtime::hub::HubConfig;
use atomo_realtime::{ClientMsg, Connection, CoordinatorLeavePolicy, Hub, Principal, ServerMsg};

async fn next(conn: &mut Connection) -> ServerMsg {
    tokio::time::timeout(Duration::from_secs(1), conn.outbound.recv())
        .await
        .expect("timed out waiting for a frame")
        .expect("hub closed the connection")
}

async fn expect_idle(conn: &mut Connection) {
    let got = tokio::time::timeout(Duration::from_millis(100), conn.outbound.recv()).await;
    assert!(got.is_err(), "expected no frame, got {got:?}");
}

fn payload(json: &str) -> Box<serde_json::value::RawValue> {
    serde_json::from_str(json).unwrap()
}

async fn join(conn: &Connection, session: &str) {
    conn.handle
        .dispatch(ClientMsg::SessionJoin { session: session.into() })
        .await;
}

#[tokio::test]
async fn join_assigns_slots_and_announces_members() {
    let hub = Hub::new();
    let mut alice = hub.connect(Principal::new("alice", None)).await;
    let mut bob = hub.connect(Principal::new("bob", None)).await;

    join(&alice, "match").await;
    match next(&mut alice).await {
        ServerMsg::SessionStart { slot, coordinator, members, .. } => {
            assert_eq!(slot, 0);
            assert!(coordinator, "first joiner is the coordinator");
            assert_eq!(members.len(), 1);
        }
        other => panic!("expected SessionStart, got {other:?}"),
    }

    join(&bob, "match").await;
    match next(&mut bob).await {
        ServerMsg::SessionStart { slot, coordinator, members, .. } => {
            assert_eq!(slot, 1);
            assert!(!coordinator);
            assert_eq!(members.len(), 2);
        }
        other => panic!("expected SessionStart, got {other:?}"),
    }
    // Alice (already in) learns Bob joined.
    match next(&mut alice).await {
        ServerMsg::MemberJoined { slot, id, .. } => {
            assert_eq!(slot, 1);
            assert_eq!(id, "bob");
        }
        other => panic!("expected MemberJoined, got {other:?}"),
    }
}

#[tokio::test]
async fn directional_relay_to_and_from_coordinator() {
    let hub = Hub::new();
    let mut alice = hub.connect(Principal::new("alice", None)).await; // coordinator
    let mut bob = hub.connect(Principal::new("bob", None)).await;
    join(&alice, "m").await;
    let _ = next(&mut alice).await; // SessionStart
    join(&bob, "m").await;
    let _ = next(&mut bob).await; // SessionStart
    let _ = next(&mut alice).await; // MemberJoined bob

    // Member → coordinator: bob's input reaches alice (host), not bob.
    bob.handle
        .dispatch(ClientMsg::ToCoordinator { session: "m".into(), payload: payload(r#"{"thrust":1}"#) })
        .await;
    match next(&mut alice).await {
        ServerMsg::FromMember { from, slot, payload, .. } => {
            assert_eq!(from, "bob");
            assert_eq!(slot, 1);
            assert_eq!(payload.get(), r#"{"thrust":1}"#);
        }
        other => panic!("expected FromMember, got {other:?}"),
    }
    expect_idle(&mut bob).await;

    // Coordinator → members: alice's snapshot reaches bob, not alice.
    alice
        .handle
        .dispatch(ClientMsg::ToMembers { session: "m".into(), payload: payload(r#"[1,2,3]"#) })
        .await;
    match next(&mut bob).await {
        ServerMsg::FromCoordinator { payload, .. } => assert_eq!(payload.get(), r#"[1,2,3]"#),
        other => panic!("expected FromCoordinator, got {other:?}"),
    }
    expect_idle(&mut alice).await;
}

#[tokio::test]
async fn to_members_from_non_coordinator_is_rejected() {
    let hub = Hub::new();
    let mut alice = hub.connect(Principal::new("alice", None)).await;
    let mut bob = hub.connect(Principal::new("bob", None)).await;
    join(&alice, "m").await;
    let _ = next(&mut alice).await;
    join(&bob, "m").await;
    let _ = next(&mut bob).await;
    let _ = next(&mut alice).await;

    bob.handle
        .dispatch(ClientMsg::ToMembers { session: "m".into(), payload: payload("true") })
        .await;
    match next(&mut bob).await {
        ServerMsg::Error { message } => assert!(message.contains("coordinator"), "got: {message}"),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[tokio::test]
async fn coordinator_disconnect_reelects_under_default_policy() {
    let hub = Hub::new(); // default = Reelect
    let mut alice = hub.connect(Principal::new("alice", None)).await; // coordinator
    let mut bob = hub.connect(Principal::new("bob", None)).await;
    join(&alice, "m").await;
    let _ = next(&mut alice).await;
    join(&bob, "m").await;
    let _ = next(&mut bob).await;
    let _ = next(&mut alice).await;

    alice.handle.disconnect().await;
    // Bob sees the host leave, then his own promotion.
    match next(&mut bob).await {
        ServerMsg::MemberLeft { id, .. } => assert_eq!(id, "alice"),
        other => panic!("expected MemberLeft, got {other:?}"),
    }
    match next(&mut bob).await {
        ServerMsg::CoordinatorChanged { id, slot, .. } => {
            assert_eq!(id, "bob");
            assert_eq!(slot, 1);
        }
        other => panic!("expected CoordinatorChanged, got {other:?}"),
    }
}

#[tokio::test]
async fn coordinator_disconnect_closes_session_under_close_policy() {
    let hub = Hub::with_config(HubConfig {
        coordinator_leave_policy: CoordinatorLeavePolicy::Close,
        ..Default::default()
    });
    let mut alice = hub.connect(Principal::new("alice", None)).await; // coordinator
    let mut bob = hub.connect(Principal::new("bob", None)).await;
    join(&alice, "m").await;
    let _ = next(&mut alice).await;
    join(&bob, "m").await;
    let _ = next(&mut bob).await;
    let _ = next(&mut alice).await;

    alice.handle.disconnect().await;
    match next(&mut bob).await {
        ServerMsg::SessionClosed { reason, .. } => assert_eq!(reason, "coordinator_left"),
        other => panic!("expected SessionClosed, got {other:?}"),
    }
}
