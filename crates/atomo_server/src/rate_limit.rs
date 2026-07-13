use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<HashMap<IpAddr, TokenBucket>>>,
    max_requests: u32,
    window_secs: u64,
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
        }
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
        Self::new(max, window)
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

pub async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let path = req.uri().path();
    if path.starts_with("/auth/") || path == "/auth" || path == "/health" || path == "/ready" {
        return Ok(next.run(req).await);
    }

    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

    match limiter.check(ip).await {
        Ok(()) => Ok(next.run(req).await),
        Err(retry_after) => {
            let val = HeaderValue::from_str(&retry_after.to_string()).unwrap();
            Err((StatusCode::TOO_MANY_REQUESTS, [("retry-after", val)]).into_response())
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
}
