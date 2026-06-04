//! A token-bucket rate limiter for gating bursty client operations.
//!
//! The hub keeps one per client to throttle *joins* (subscribe / session-join),
//! which are the cheap-to-send / expensive-to-handle frames an abusive client
//! would flood. It is deliberately tiny and not thread-safe: the hub owns it
//! inside its single task, and time is passed in so it is trivially testable.

use std::time::Instant;

/// Token-bucket parameters.
#[derive(Debug, Clone, Copy)]
pub struct RateLimit {
    /// Maximum burst — tokens available at once.
    pub burst: u32,
    /// Sustained rate — tokens refilled per second.
    pub per_sec: f64,
}

impl RateLimit {
    pub fn new(burst: u32, per_sec: f64) -> Self {
        Self { burst, per_sec }
    }
}

/// A single token bucket. Refills continuously; [`try_acquire`] takes one token.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    burst: f64,
    per_sec: f64,
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    /// Start full (a fresh client gets its whole burst immediately).
    pub fn new(limit: RateLimit, now: Instant) -> Self {
        Self {
            burst: limit.burst as f64,
            per_sec: limit.per_sec,
            tokens: limit.burst as f64,
            last: now,
        }
    }

    /// Refill for elapsed time, then take one token. Returns `false` (denied)
    /// when the bucket is empty.
    pub fn try_acquire(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.per_sec).min(self.burst);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn allows_up_to_the_burst_then_denies() {
        let t0 = Instant::now();
        let mut rl = RateLimiter::new(RateLimit::new(2, 1.0), t0);
        assert!(rl.try_acquire(t0));
        assert!(rl.try_acquire(t0));
        assert!(!rl.try_acquire(t0), "third in the same instant is denied");
    }

    #[test]
    fn refills_over_time() {
        let t0 = Instant::now();
        let mut rl = RateLimiter::new(RateLimit::new(1, 2.0), t0); // 2 tokens/sec
        assert!(rl.try_acquire(t0));
        assert!(!rl.try_acquire(t0));
        // After 0.5s, exactly one token has refilled.
        assert!(rl.try_acquire(t0 + Duration::from_millis(500)));
    }

    #[test]
    fn refill_is_capped_at_burst() {
        let t0 = Instant::now();
        let mut rl = RateLimiter::new(RateLimit::new(3, 100.0), t0);
        // Drain, then wait a long time — tokens cap at burst, not unbounded.
        for _ in 0..3 {
            assert!(rl.try_acquire(t0));
        }
        let later = t0 + Duration::from_secs(10);
        assert!(rl.try_acquire(later));
        assert!(rl.try_acquire(later));
        assert!(rl.try_acquire(later));
        assert!(!rl.try_acquire(later), "only `burst` tokens accumulate");
    }
}
