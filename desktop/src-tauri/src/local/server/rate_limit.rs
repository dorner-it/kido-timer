//! Simple per-IP token bucket for the on-prem server (venue LAN; protects
//! the laptop from a misbehaving phone / script, not a security boundary).

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Burst size per IP (page load + a few submits).
pub const DEFAULT_CAPACITY: f64 = 60.0;
/// Sustained requests per second per IP (a judge polls every 4 s).
pub const DEFAULT_REFILL_PER_SEC: f64 = 10.0;
/// Buckets idle longer than this are dropped when the map grows.
const IDLE_EVICT: Duration = Duration::from_secs(600);
const MAX_BUCKETS: usize = 1024;

#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: f64,
    last: Instant,
}

#[derive(Debug)]
pub struct RateLimiter {
    capacity: f64,
    refill_per_sec: f64,
    buckets: HashMap<IpAddr, Bucket>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY, DEFAULT_REFILL_PER_SEC)
    }
}

impl RateLimiter {
    pub fn new(capacity: f64, refill_per_sec: f64) -> Self {
        Self {
            capacity,
            refill_per_sec,
            buckets: HashMap::new(),
        }
    }

    /// Take one token for `ip` at `now`. `false` = rate limited.
    pub fn check(&mut self, ip: IpAddr, now: Instant) -> bool {
        if self.buckets.len() >= MAX_BUCKETS {
            self.buckets
                .retain(|_, b| now.saturating_duration_since(b.last) < IDLE_EVICT);
        }
        let capacity = self.capacity;
        let refill = self.refill_per_sec;
        let bucket = self.buckets.entry(ip).or_insert(Bucket {
            tokens: capacity,
            last: now,
        });
        let elapsed = now.saturating_duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * refill).min(capacity);
        bucket.last = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub fn tracked(&self) -> usize {
        self.buckets.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn ip(n: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, n))
    }

    #[test]
    fn burst_then_limited() {
        let mut rl = RateLimiter::new(3.0, 1.0);
        let t = Instant::now();
        assert!(rl.check(ip(1), t));
        assert!(rl.check(ip(1), t));
        assert!(rl.check(ip(1), t));
        assert!(!rl.check(ip(1), t), "4th request in the same instant");
    }

    #[test]
    fn refills_over_time() {
        let mut rl = RateLimiter::new(2.0, 2.0);
        let t = Instant::now();
        assert!(rl.check(ip(1), t));
        assert!(rl.check(ip(1), t));
        assert!(!rl.check(ip(1), t));
        assert!(
            rl.check(ip(1), t + Duration::from_millis(500)),
            "1 token after 0.5 s"
        );
        assert!(!rl.check(ip(1), t + Duration::from_millis(500)));
        // Never above capacity.
        let later = t + Duration::from_secs(60);
        assert!(rl.check(ip(1), later));
        assert!(rl.check(ip(1), later));
        assert!(!rl.check(ip(1), later));
    }

    #[test]
    fn per_ip_isolation() {
        let mut rl = RateLimiter::new(1.0, 0.0);
        let t = Instant::now();
        assert!(rl.check(ip(1), t));
        assert!(!rl.check(ip(1), t));
        assert!(rl.check(ip(2), t), "other phone unaffected");
        assert_eq!(rl.tracked(), 2);
    }

    #[test]
    fn idle_buckets_are_evicted_when_full() {
        let mut rl = RateLimiter::new(1.0, 1.0);
        let t = Instant::now();
        for i in 0..MAX_BUCKETS {
            let a = IpAddr::V4(Ipv4Addr::from(0x0a00_0000u32 + i as u32));
            rl.check(a, t);
        }
        assert_eq!(rl.tracked(), MAX_BUCKETS);
        rl.check(ip(9), t + IDLE_EVICT + Duration::from_secs(1));
        assert_eq!(rl.tracked(), 1);
    }

    #[test]
    fn default_allows_a_normal_judge() {
        let mut rl = RateLimiter::default();
        let t = Instant::now();
        // Page load (4 requests) + 30 polls/submits in a burst.
        assert!((0..34).all(|_| rl.check(ip(1), t)));
    }
}
