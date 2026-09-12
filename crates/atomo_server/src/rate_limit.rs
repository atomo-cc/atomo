use axum::{
    extract::{ConnectInfo, Request},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<HashMap<IpAddr, TokenBucket>>>,
    max_requests: u32,
    window_secs: u64,
    trust_xff: bool,
}

struct TokenBucket {
    tokens: u32,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window_secs,
            trust_xff: true,
        }
    }

    /// Whether `x-forwarded-for` is trusted as the client identity. Default true —
    /// proxied deployments (the control-plane gateway) depend on it. Directly-exposed
    /// servers should set `ATOMO_TRUST_X_FORWARDED_FOR=false` so a spoofed header
    /// can't mint fresh buckets.
    fn with_xff_trust(mut self, trust: bool) -> Self {
        self.trust_xff = trust;
        self
    }

    /// Create from environment variables (RATE_LIMIT_RPS, RATE_LIMIT_WINDOW_SECS)
    /// Defaults: 100 requests per 60 seconds
    pub fn from_env() -> Self {
        let max = std::env::var("RATE_LIMIT_RPS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);
        let window = std::env::var("RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        let trust_xff = std::env::var("ATOMO_TRUST_X_FORWARDED_FOR")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);
        Self::new(max, window).with_xff_trust(trust_xff)
    }

    /// Returns `Ok(())` if the request is allowed, or `Err(retry_after_secs)` if
    /// the bucket is exhausted.
    pub async fn check(&self, ip: IpAddr) -> Result<(), u64> {
        let mut state = self.state.lock().await;
        let now = Instant::now();
        let bucket = state.entry(ip).or_insert(TokenBucket {
            tokens: self.max_requests,
            last_refill: now,
        });

        let elapsed = now.duration_since(bucket.last_refill).as_secs();
        if elapsed >= self.window_secs {
            bucket.tokens = self.max_requests;
            bucket.last_refill = now;
        }

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            Ok(())
        } else {
            let retry_after = self.window_secs.saturating_sub(elapsed);
            Err(retry_after.max(1))
        }
    }
}

// Both variants are complete HTTP responses at the middleware boundary. Keep
// the existing public signature; boxing only the 429 response adds no value.
#[allow(clippy::result_large_err)]
pub async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let path = req.uri().path();
    if path.starts_with("/auth/") || path == "/auth" || path == "/health" || path == "/ready" {
        return Ok(next.run(req).await);
    }

    // Client identity: trusted XFF when configured (proxied deployments), else the
    // real peer from ConnectInfo (wired at serve time). Without either, direct clients
    // would all collapse into one shared 127.0.0.1 bucket — the "everything 429s on a
    // busy dev backend" failure mode.
    let ip = (if limiter.trust_xff {
        req.headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.trim().parse::<IpAddr>().ok())
    } else {
        None
    })
    .or_else(|| {
        req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip())
    })
    .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

    match limiter.check(ip).await {
        Ok(()) => Ok(next.run(req).await),
        Err(retry_after) => {
            let val = HeaderValue::from_str(&retry_after.to_string()).unwrap();
            // A bare 429 is indistinguishable from a transport error for clients that
            // only read the body — send a machine-readable payload.
            Err((
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", val)],
                Json(json!({ "error": "rate_limited", "retryAfter": retry_after })),
            )
                .into_response())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn ip() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))
    }

    #[tokio::test]
    async fn allows_up_to_limit_then_blocks() {
        let rl = RateLimiter::new(2, 60);
        assert!(rl.check(ip()).await.is_ok(), "1st allowed");
        assert!(rl.check(ip()).await.is_ok(), "2nd allowed");
        let err = rl.check(ip()).await.unwrap_err();
        assert!(err >= 1, "3rd over the limit -> blocked with retry_after");
    }

    #[tokio::test]
    async fn refills_after_window() {
        let rl = RateLimiter::new(1, 0); // window 0s → refills every call
        assert!(rl.check(ip()).await.is_ok(), "1st allowed");
        assert!(
            rl.check(ip()).await.is_ok(),
            "allowed again after window elapsed"
        );
    }

    #[tokio::test]
    async fn buckets_are_per_ip() {
        let rl = RateLimiter::new(1, 60);
        let a = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let b = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        assert!(rl.check(a).await.is_ok());
        assert!(rl.check(a).await.is_err(), "A exhausted");
        assert!(rl.check(b).await.is_ok(), "B has its own bucket");
    }

    use axum::{middleware, routing::get, Router};
    use tower::ServiceExt;

    fn limited_app(limiter: RateLimiter) -> Router {
        Router::new()
            .route("/x", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(
                limiter,
                rate_limit_middleware,
            ))
    }

    fn req(xff: Option<&str>, peer: Option<IpAddr>) -> Request {
        let mut b = Request::builder().uri("/x");
        if let Some(xff) = xff {
            b = b.header("x-forwarded-for", xff);
        }
        let mut r = b.body(axum::body::Body::empty()).unwrap();
        if let Some(ip) = peer {
            r.extensions_mut()
                .insert(ConnectInfo(SocketAddr::new(ip, 4000)));
        }
        r
    }

    #[tokio::test]
    async fn middleware_429_carries_json_body_and_retry_after() {
        let app = limited_app(RateLimiter::new(1, 60));
        let peer = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7));

        assert_eq!(
            app.clone()
                .oneshot(req(None, Some(peer)))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        let resp = app.oneshot(req(None, Some(peer))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(resp.headers().contains_key("retry-after"));
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "rate_limited");
        assert!(json["retryAfter"].as_u64().unwrap() >= 1);
    }

    #[tokio::test]
    async fn middleware_uses_peer_addr_when_xff_absent() {
        // No XFF, distinct peers → distinct buckets (previously all direct
        // clients shared the single 127.0.0.1 fallback bucket).
        let app = limited_app(RateLimiter::new(1, 60));
        let a = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 8));
        let b = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));

        assert_eq!(
            app.clone()
                .oneshot(req(None, Some(a)))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            app.clone()
                .oneshot(req(None, Some(a)))
                .await
                .unwrap()
                .status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            app.oneshot(req(None, Some(b))).await.unwrap().status(),
            StatusCode::OK,
            "peer B keeps its own bucket"
        );
    }

    #[tokio::test]
    async fn untrusted_xff_does_not_mint_fresh_buckets() {
        // XFF distrusted: forged headers can't escape the peer's bucket.
        let app = limited_app(RateLimiter::new(1, 60).with_xff_trust(false));
        let peer = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 11));

        assert_eq!(
            app.clone()
                .oneshot(req(Some("203.0.113.1"), Some(peer)))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            app.oneshot(req(Some("203.0.113.2"), Some(peer)))
                .await
                .unwrap()
                .status(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[tokio::test]
    async fn trusted_xff_wins_over_peer_addr() {
        // Default: proxied deployments rate-limit on the real client, not the proxy.
        let app = limited_app(RateLimiter::new(1, 60));
        let proxy = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 12));

        assert_eq!(
            app.clone()
                .oneshot(req(Some("203.0.113.1"), Some(proxy)))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            app.oneshot(req(Some("203.0.113.2"), Some(proxy)))
                .await
                .unwrap()
                .status(),
            StatusCode::OK,
            "different XFF client → different bucket"
        );
    }
}
