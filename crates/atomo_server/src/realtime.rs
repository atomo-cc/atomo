//! WebSocket transport for the ephemeral realtime tier.
//!
//! `atomo_realtime` is transport-agnostic (no HTTP, no auth). This module is the
//! "one backend" seam: it owns the `/realtime/ws` route, authenticates the
//! upgrade with the server's existing JWT path — exactly like
//! [`crate::handlers::graphql_ws_handler`] does for subscriptions — maps the
//! verified user onto an [`atomo_realtime::Principal`], and pumps frames between
//! the socket and the hub.
//!
//! Durable outcomes never flow through here: this is the in-memory, high-
//! frequency tier. Anything that must persist goes out via the normal
//! command/mutation path (later phases).

use atomo_realtime::{ClientMsg, Hub, Principal, ServerMsg};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, trace};

use crate::auth::HttpAuthService;

/// State shared by the realtime routes.
#[derive(Clone)]
pub struct RealtimeState {
    hub: Hub,
    auth: HttpAuthService,
}

/// Build the realtime router: `/realtime/health` and `/realtime/ws`.
///
/// The caller decides whether to mount it (gated by `enable_realtime`), and owns
/// the single [`Hub`] instance — so the hub is started once at boot and shared.
pub fn realtime_router(hub: Hub, auth: HttpAuthService) -> Router {
    Router::new()
        .route("/realtime/health", get(realtime_health))
        .route("/realtime/ws", get(realtime_ws))
        .route("/realtime/token", post(mint_realtime_token))
        .with_state(RealtimeState { hub, auth })
}

#[derive(Deserialize)]
struct MintRequest {
    /// Session to bind the token to (the matchmaker's assignment). Optional.
    #[serde(default)]
    session: Option<String>,
    /// Token lifetime in seconds (default 1h, capped at 24h).
    #[serde(default)]
    ttl_seconds: Option<i64>,
}

#[derive(Serialize)]
struct MintResponse {
    token: String,
}

/// Mint a short-lived, stateless token the caller hands to the standalone relay
/// (`atomo-realtime-server`). The caller authenticates here (full DB-backed
/// session check); the relay later verifies the minted token with signature +
/// expiry alone. This is the platform→relay handoff: matchmaking decides the
/// `session`, this endpoint signs it.
async fn mint_realtime_token(
    State(state): State<RealtimeState>,
    headers: HeaderMap,
    Json(req): Json<MintRequest>,
) -> Result<Json<MintResponse>, (StatusCode, &'static str)> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or((StatusCode::UNAUTHORIZED, "missing bearer token"))?;
    let user = state
        .auth
        .verify_token(token)
        .await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid or expired token"))?;

    let ttl = req.ttl_seconds.unwrap_or(3600).clamp(1, 86_400);
    let minted = state
        .auth
        .mint_realtime_token(&user.id, req.session.as_deref(), ttl)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to mint token"))?;
    Ok(Json(MintResponse { token: minted }))
}

/// Liveness + hub counters (channel/connection/fan-out/drop gauges).
async fn realtime_health(State(state): State<RealtimeState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "tier": "ephemeral-realtime",
        "stats": state.hub.stats(),
    }))
}

/// Whether unauthenticated connections are accepted. Off by default (secure);
/// a service opts in with `ATOMO_REALTIME_ALLOW_ANON=true` (RFC Phase 2).
fn allow_anonymous() -> bool {
    matches!(
        std::env::var("ATOMO_REALTIME_ALLOW_ANON").as_deref(),
        Ok("true") | Ok("1")
    )
}

/// Authenticate the upgrade, then hand the socket to the hub pump.
///
/// The JWT arrives as `?token=<jwt>` on the upgrade URL (browsers can't set
/// arbitrary headers on a `WebSocket`). Missing/invalid tokens are rejected
/// unless anonymous access is opted in.
async fn realtime_ws(
    State(state): State<RealtimeState>,
    Query(params): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> Response {
    let principal = match params.get("token") {
        Some(token) => match state.auth.verify_token(token).await {
            Ok(user) => Principal::new(user.id, Some(format!("{:?}", user.role))),
            Err(_) => {
                return (
                    axum::http::StatusCode::UNAUTHORIZED,
                    "realtime auth failed: invalid or expired token",
                )
                    .into_response();
            }
        },
        None if allow_anonymous() => {
            // Unique per connection so distinct anon clients don't collapse in
            // presence (which de-dups by principal id).
            Principal::anonymous(format!("anon:{}", uuid::Uuid::new_v4()))
        }
        None => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                "realtime auth required: pass ?token=<jwt> (or enable ATOMO_REALTIME_ALLOW_ANON)",
            )
                .into_response();
        }
    };

    let hub = state.hub;
    ws.on_upgrade(move |socket| pump(socket, hub, principal))
}

/// Bridge one socket to the hub: socket-in → `hub.dispatch`, hub-out → socket.
///
/// A single task multiplexes both directions with `select!` — no task spawn, no
/// stream splitting. When either side ends we disconnect, which drops the
/// client's subscriptions and fans out presence-leaves.
async fn pump(mut socket: WebSocket, hub: Hub, principal: Principal) {
    let mut conn = hub.connect(principal).await;
    let id = conn.id;
    debug!(client_id = id, "realtime connection up");

    loop {
        tokio::select! {
            inbound = socket.recv() => match inbound {
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMsg>(text.as_str()) {
                    Ok(msg) => conn.handle.dispatch(msg).await,
                    Err(e) => {
                        // Non-fatal: tell the client and keep the socket open.
                        let err = ServerMsg::Error { message: format!("bad frame: {e}") };
                        if send(&mut socket, &err).await.is_err() {
                            break;
                        }
                    }
                },
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(_)) => trace!(client_id = id, "ignoring non-text frame"),
                Some(Err(_)) => break,
            },
            outbound = conn.outbound.recv() => match outbound {
                Some(msg) => {
                    if send(&mut socket, &msg).await.is_err() {
                        break;
                    }
                }
                None => break, // hub closed this connection
            },
        }
    }

    conn.handle.disconnect().await;
    debug!(client_id = id, "realtime connection down");
}

/// Serialize a server frame and write it to the socket.
async fn send(socket: &mut WebSocket, msg: &ServerMsg) -> Result<(), axum::Error> {
    let json = serde_json::to_string(msg).unwrap_or_else(|_| "{\"type\":\"error\"}".to_string());
    socket.send(Message::Text(json.into())).await
}
