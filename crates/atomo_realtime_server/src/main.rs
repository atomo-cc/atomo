//! `atomo-realtime-server` — a standalone, lightweight relay.
//!
//! This is the **ephemeral tier** deployed on its own: the [`atomo_realtime`] hub
//! over a WebSocket, with **stateless JWT verification** and **no database**. It
//! is meant to run as a fleet of small, edge/region-local processes (a game's
//! relay servers), separate from the heavy `atomo_server` (which owns users,
//! auth sessions, matchmaking, and persistence).
//!
//! ## Auth boundary
//!
//! The relay does **not** manage users. The platform (`atomo_server`) mints a
//! short-lived token (`POST /realtime/token`) signed with the shared `JWT_SECRET`
//! carrying `{ sub, sid?, exp }`. The relay verifies the **signature + expiry**
//! only — a stateless crypto check, no DB round-trip — and, if the token names a
//! session (`sid`), binds the connection to it (the matchmaker's assignment).
//!
//! ## Config (env)
//!
//! - `HOST` (default `0.0.0.0`), `PORT` (default `9100`)
//! - `JWT_SECRET` — shared with `atomo_server`; required in production
//! - `ATOMO_REALTIME_ALLOW_ANON` — accept tokenless connections (default off)
//! - `ATOMO_REALTIME_COORDINATOR_POLICY` — `reelect` (default) or `close`

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use atomo_realtime::hub::HubConfig;
use atomo_realtime::{ClientMsg, CoordinatorLeavePolicy, Hub, Principal, ServerMsg};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::net::TcpListener;
use tracing::{debug, info, warn};

/// Claims the relay reads from a realtime token. `exp` is validated by the
/// library; extra claims (e.g. role) are ignored — the relay only needs identity
/// and the optional session assignment.
#[derive(Debug, Deserialize)]
struct RealtimeClaims {
    sub: String,
    #[serde(default)]
    sid: Option<String>,
}

#[derive(Clone)]
struct AppState {
    hub: Hub,
    jwt_secret: Arc<String>,
    allow_anon: bool,
}

static ANON_COUNTER: AtomicU64 = AtomicU64::new(1);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9100);

    let is_prod = matches!(std::env::var("ATOMO_ENV").as_deref(), Ok("production"));
    let jwt_secret = match std::env::var("JWT_SECRET") {
        Ok(s) => s,
        Err(_) if is_prod => anyhow::bail!("JWT_SECRET must be set in production"),
        Err(_) => {
            warn!("JWT_SECRET not set; using an insecure development default");
            "dev-insecure-secret".to_string()
        }
    };
    let allow_anon = matches!(
        std::env::var("ATOMO_REALTIME_ALLOW_ANON").as_deref(),
        Ok("true") | Ok("1")
    );
    let policy = match std::env::var("ATOMO_REALTIME_COORDINATOR_POLICY").as_deref() {
        Ok("close") => CoordinatorLeavePolicy::Close,
        _ => CoordinatorLeavePolicy::Reelect,
    };

    let hub = Hub::with_config(HubConfig {
        coordinator_leave_policy: policy,
    });
    let state = AppState {
        hub,
        jwt_secret: Arc::new(jwt_secret),
        allow_anon,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = SocketAddr::new(host.parse()?, port);
    let listener = TcpListener::bind(&addr).await?;
    info!("⚡ atomo-realtime-server on ws://{}/ws (anon={})", addr, allow_anon);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "tier": "ephemeral-realtime",
        "stats": state.hub.stats(),
    }))
}

/// Stateless verification: signature + `exp` against the shared secret. No DB.
fn verify(token: &str, secret: &str) -> Option<RealtimeClaims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_required_spec_claims(&["sub", "exp"]);
    decode::<RealtimeClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .ok()
    .map(|data| data.claims)
}

async fn ws_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> Response {
    let (principal, session) = match params.get("token") {
        Some(token) => match verify(token, &state.jwt_secret) {
            Some(claims) => (Principal::new(claims.sub, None), claims.sid),
            None => {
                return (
                    axum::http::StatusCode::UNAUTHORIZED,
                    "realtime auth failed: invalid or expired token",
                )
                    .into_response();
            }
        },
        None if state.allow_anon => {
            let n = ANON_COUNTER.fetch_add(1, Ordering::Relaxed);
            (Principal::anonymous(format!("anon:{n}")), None)
        }
        None => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                "realtime auth required: pass ?token=<jwt>",
            )
                .into_response();
        }
    };

    let hub = state.hub;
    ws.on_upgrade(move |socket| pump(socket, hub, principal, session))
}

/// Bridge one socket to the hub. If the token named a session, the matchmaker's
/// assignment is enforced by auto-joining it on connect.
async fn pump(mut socket: WebSocket, hub: Hub, principal: Principal, session: Option<String>) {
    let mut conn = hub.connect(principal).await;
    let id = conn.id;
    if let Some(session) = session {
        conn.handle.dispatch(ClientMsg::SessionJoin { session }).await;
    }
    debug!(client_id = id, "relay connection up");

    loop {
        tokio::select! {
            inbound = socket.recv() => match inbound {
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMsg>(text.as_str()) {
                    Ok(msg) => conn.handle.dispatch(msg).await,
                    Err(e) => {
                        let err = ServerMsg::Error { message: format!("bad frame: {e}") };
                        if send(&mut socket, &err).await.is_err() {
                            break;
                        }
                    }
                },
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(_)) => {}
                Some(Err(_)) => break,
            },
            outbound = conn.outbound.recv() => match outbound {
                Some(msg) => {
                    if send(&mut socket, &msg).await.is_err() {
                        break;
                    }
                }
                None => break,
            },
        }
    }

    conn.handle.disconnect().await;
    debug!(client_id = id, "relay connection down");
}

async fn send(socket: &mut WebSocket, msg: &ServerMsg) -> Result<(), axum::Error> {
    let json = serde_json::to_string(msg).unwrap_or_else(|_| "{\"type\":\"error\"}".to_string());
    socket.send(Message::Text(json.into())).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;

    #[derive(Serialize)]
    struct Mint {
        sub: String,
        sid: Option<String>,
        exp: i64,
        iat: i64,
    }

    fn mint(secret: &str, sub: &str, sid: Option<&str>, exp: i64) -> String {
        let claims = Mint {
            sub: sub.to_string(),
            sid: sid.map(String::from),
            exp,
            iat: 0,
        };
        encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
    }

    #[test]
    fn verifies_a_valid_token_and_reads_claims() {
        let token = mint("s3cret", "user-1", Some("match-9"), 9_999_999_999);
        let claims = verify(&token, "s3cret").expect("valid token");
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.sid.as_deref(), Some("match-9"));
    }

    #[test]
    fn rejects_wrong_secret() {
        let token = mint("right", "u", None, 9_999_999_999);
        assert!(verify(&token, "wrong").is_none());
    }

    #[test]
    fn rejects_expired_token() {
        let token = mint("s", "u", None, 1); // exp in 1970
        assert!(verify(&token, "s").is_none());
    }

    #[test]
    fn session_id_is_optional() {
        let token = mint("s", "u", None, 9_999_999_999);
        let claims = verify(&token, "s").unwrap();
        assert!(claims.sid.is_none());
    }
}
