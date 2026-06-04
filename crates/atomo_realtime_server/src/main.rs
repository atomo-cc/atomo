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
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use atomo_realtime::hub::HubConfig;
use atomo_realtime::{
    ClientMsg, CoordinatorLeavePolicy, Hub, Principal, RateLimit, ServerMsg, StatsSnapshot,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, Query, State,
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
    conns: ConnLimiter,
}

static ANON_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Caps concurrent connections per client IP — basic DoS protection for the
/// internet-facing relay. `max == 0` disables the cap.
#[derive(Clone)]
struct ConnLimiter {
    inner: Arc<Mutex<HashMap<IpAddr, u32>>>,
    max: u32,
}

impl ConnLimiter {
    fn new(max: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            max,
        }
    }

    /// Reserve a connection slot for `ip`; `false` if the IP is at its cap.
    fn try_acquire(&self, ip: IpAddr) -> bool {
        if self.max == 0 {
            return true;
        }
        let mut map = self.inner.lock().unwrap();
        let count = map.entry(ip).or_insert(0);
        if *count >= self.max {
            false
        } else {
            *count += 1;
            true
        }
    }

    /// Release a slot when the connection closes.
    fn release(&self, ip: IpAddr) {
        if self.max == 0 {
            return;
        }
        let mut map = self.inner.lock().unwrap();
        if let Some(count) = map.get_mut(&ip) {
            *count -= 1;
            if *count == 0 {
                map.remove(&ip);
            }
        }
    }
}

/// Render hub counters in Prometheus text exposition format.
fn prometheus_text(s: &StatsSnapshot) -> String {
    let mut out = String::new();
    let mut metric = |name: &str, kind: &str, help: &str, value: u64| {
        out.push_str(&format!("# HELP atomo_realtime_{name} {help}\n"));
        out.push_str(&format!("# TYPE atomo_realtime_{name} {kind}\n"));
        out.push_str(&format!("atomo_realtime_{name} {value}\n"));
    };
    metric("active_clients", "gauge", "Currently connected clients", s.active_clients);
    metric("active_channels", "gauge", "Channels with members", s.active_channels);
    metric("active_sessions", "gauge", "Live coordinator sessions", s.active_sessions);
    metric("messages_published_total", "counter", "Frames accepted for fan-out", s.messages_published);
    metric("messages_delivered_total", "counter", "Frames delivered to clients", s.messages_delivered);
    metric("dropped_frames_total", "counter", "Frames shed to slow clients", s.dropped_frames);
    metric("connections_opened_total", "counter", "Connections opened", s.connections_opened);
    metric("connections_closed_total", "counter", "Connections closed", s.connections_closed);
    out
}

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
    let env_u32 = |key: &str, default: u32| -> u32 {
        std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    // Per-client join throttle (on by default for the exposed relay).
    let join_burst = env_u32("ATOMO_REALTIME_JOIN_BURST", 20);
    let join_per_sec: f64 = std::env::var("ATOMO_REALTIME_JOIN_PER_SEC")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10.0);
    let max_conn_per_ip = env_u32("ATOMO_REALTIME_MAX_CONN_PER_IP", 64);

    let hub = Hub::with_config(HubConfig {
        coordinator_leave_policy: policy,
        join_rate: Some(RateLimit::new(join_burst, join_per_sec)),
    });
    let state = AppState {
        hub,
        jwt_secret: Arc::new(jwt_secret),
        allow_anon,
        conns: ConnLimiter::new(max_conn_per_ip),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = SocketAddr::new(host.parse()?, port);
    let listener = TcpListener::bind(&addr).await?;
    info!(
        "⚡ atomo-realtime-server on ws://{}/ws (anon={}, max_conn_per_ip={})",
        addr, allow_anon, max_conn_per_ip
    );
    // ConnectInfo gives the per-connection peer address for the per-IP cap.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

/// Prometheus metrics (hub counters).
async fn metrics(State(state): State<AppState>) -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        prometheus_text(&state.hub.stats()),
    )
        .into_response()
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
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Query(params): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> Response {
    let ip = peer.ip();
    if !state.conns.try_acquire(ip) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "too many connections from this address",
        )
            .into_response();
    }
    let unauth = |conns: &ConnLimiter, msg: &'static str| -> Response {
        conns.release(ip); // don't leak the slot we reserved above
        (axum::http::StatusCode::UNAUTHORIZED, msg).into_response()
    };
    let (principal, session) = match params.get("token") {
        Some(token) => match verify(token, &state.jwt_secret) {
            Some(claims) => (Principal::new(claims.sub, None), claims.sid),
            None => return unauth(&state.conns, "realtime auth failed: invalid or expired token"),
        },
        None if state.allow_anon => {
            let n = ANON_COUNTER.fetch_add(1, Ordering::Relaxed);
            (Principal::anonymous(format!("anon:{n}")), None)
        }
        None => return unauth(&state.conns, "realtime auth required: pass ?token=<jwt>"),
    };

    let hub = state.hub;
    let conns = state.conns;
    ws.on_upgrade(move |socket| pump(socket, hub, principal, session, conns, ip))
}

/// Bridge one socket to the hub. If the token named a session, the matchmaker's
/// assignment is enforced by auto-joining it on connect. Releases the per-IP
/// connection slot when the socket closes.
async fn pump(
    mut socket: WebSocket,
    hub: Hub,
    principal: Principal,
    session: Option<String>,
    conns: ConnLimiter,
    ip: IpAddr,
) {
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
    conns.release(ip);
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

    #[test]
    fn conn_limiter_caps_per_ip_and_releases() {
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        let other: IpAddr = "10.0.0.2".parse().unwrap();
        let lim = ConnLimiter::new(2);
        assert!(lim.try_acquire(ip));
        assert!(lim.try_acquire(ip));
        assert!(!lim.try_acquire(ip), "third from same IP is rejected");
        assert!(lim.try_acquire(other), "a different IP is unaffected");

        lim.release(ip);
        assert!(lim.try_acquire(ip), "a freed slot can be reused");
    }

    #[test]
    fn conn_limiter_zero_means_unlimited() {
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        let lim = ConnLimiter::new(0);
        for _ in 0..1000 {
            assert!(lim.try_acquire(ip));
        }
    }

    #[test]
    fn prometheus_text_emits_named_metrics() {
        let stats = StatsSnapshot {
            messages_published: 5,
            messages_delivered: 4,
            dropped_frames: 1,
            connections_opened: 3,
            connections_closed: 1,
            active_clients: 2,
            active_channels: 1,
            active_sessions: 1,
        };
        let text = prometheus_text(&stats);
        assert!(text.contains("# TYPE atomo_realtime_active_clients gauge"));
        assert!(text.contains("atomo_realtime_active_clients 2"));
        assert!(text.contains("# TYPE atomo_realtime_messages_published_total counter"));
        assert!(text.contains("atomo_realtime_dropped_frames_total 1"));
    }
}
