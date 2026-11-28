use std::time::{Duration, Instant};

use crate::config::Limits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    
    Allow,
    
    Reject,
    
    Close,
}

#[derive(Debug)]
pub struct RateLimiter {
    per_second: f64,
    capacity: f64,
    tokens: f64,
    updated: Instant,
    strikes: u32,
    max_strikes: u32,
}

impl RateLimiter {
    
    pub fn new(limits: &Limits) -> Self {
        Self::with_clock(limits, Instant::now())
    }

    
    pub fn with_clock(limits: &Limits, now: Instant) -> Self {
        let per_second = f64::from(limits.messages_per_second);

        Self {
            per_second,
            capacity: per_second,
            tokens: per_second,
            updated: now,
            strikes: 0,
            max_strikes: limits.strikes_before_close,
        }
    }

    
    pub fn check(&mut self) -> Verdict {
        self.check_at(Instant::now())
    }

    
    pub fn check_at(&mut self, now: Instant) -> Verdict {
        self.refill(now);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            self.strikes = 0;
            return Verdict::Allow;
        }

        self.strikes += 1;
        if self.strikes >= self.max_strikes {
            Verdict::Close
        } else {
            Verdict::Reject
        }
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.updated);
        if elapsed == Duration::ZERO {
            return;
        }

        self.updated = now;
        self.tokens = (self.tokens + elapsed.as_secs_f64() * self.per_second).min(self.capacity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(per_second: u32, strikes: u32) -> Limits {
        Limits {
            messages_per_second: per_second,
            strikes_before_close: strikes,
            ..Limits::default()
        }
    }

    #[test]
    fn burst_within_limit_passes() {
        let now = Instant::now();
        let mut limiter = RateLimiter::with_clock(&limits(20, 40), now);

        for _ in 0..20 {
            assert_eq!(limiter.check_at(now), Verdict::Allow);
        }

        assert_eq!(limiter.check_at(now), Verdict::Reject);
    }

    #[test]
    fn tokens_come_back_over_time() {
        let now = Instant::now();
        let mut limiter = RateLimiter::with_clock(&limits(20, 40), now);

        for _ in 0..20 {
            limiter.check_at(now);
        }

        let later = now + Duration::from_millis(500);
        assert_eq!(limiter.check_at(later), Verdict::Allow);
    }

    #[test]
    fn stubborn_flood_closes_connection() {
        let now = Instant::now();
        let mut limiter = RateLimiter::with_clock(&limits(1, 3), now);

        assert_eq!(limiter.check_at(now), Verdict::Allow);
        assert_eq!(limiter.check_at(now), Verdict::Reject);
        assert_eq!(limiter.check_at(now), Verdict::Reject);
        assert_eq!(limiter.check_at(now), Verdict::Close);
    }

    #[test]
    fn allowed_message_resets_strikes() {
        let now = Instant::now();
        let mut limiter = RateLimiter::with_clock(&limits(1, 3), now);

        limiter.check_at(now);
        limiter.check_at(now);
        assert_eq!(limiter.check_at(now + Duration::from_secs(2)), Verdict::Allow);
        assert_eq!(limiter.check_at(now + Duration::from_secs(2)), Verdict::Reject);
    }
}
