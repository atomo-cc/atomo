use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
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

    async fn check(&self, ip: IpAddr) -> bool {
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
            true
        } else {
            false
        }
    }
}

pub async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

    if limiter.check(ip).await {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::TOO_MANY_REQUESTS)
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
        let rl = RateLimiter::new(2, 60); // 2 requests / 60s
        assert!(rl.check(ip()).await, "1st allowed");
        assert!(rl.check(ip()).await, "2nd allowed");
        assert!(!rl.check(ip()).await, "3rd over the limit -> blocked");
    }

    #[tokio::test]
    async fn refills_after_window() {
        let rl = RateLimiter::new(1, 0); // window 0s → refills every call
        assert!(rl.check(ip()).await, "1st allowed");
        // elapsed >= window(0) → bucket refilled before the check.
        assert!(rl.check(ip()).await, "allowed again after window elapsed");
    }

    #[tokio::test]
    async fn buckets_are_per_ip() {
        let rl = RateLimiter::new(1, 60);
        let a = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let b = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        assert!(rl.check(a).await);
        assert!(!rl.check(a).await, "A exhausted");
        assert!(rl.check(b).await, "B has its own bucket");
    }
}
