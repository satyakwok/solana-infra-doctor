//! Per-endpoint resilience policy for HTTP RPC calls:
//!
//! - a **token-bucket rate limiter** ([`reliakit_ratelimit`]) so we stay polite
//!   toward public RPCs (important when `--samples` fans out many calls), and
//! - **exponential backoff retry** ([`reliakit_backoff`]) on transient failures
//!   (timeouts, connection errors, HTTP 429 / 5xx).
//!
//! Both reliakit crates are `no_std` and clock-agnostic — they compute durations
//! and bucket state but never sleep. This module drives their clocks with
//! `tokio` (a monotonic `Instant` for the limiter, `tokio::time::sleep` for both
//! the limiter wait and the retry backoff). No error or URL is ever logged here,
//! so redaction is preserved.

use reliakit_backoff::Backoff;
use reliakit_ratelimit::RateLimiter;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Exponential backoff: up to 2 retries at ~200ms then ~400ms, capped at 2s.
fn default_backoff() -> Backoff {
    Backoff::exponential(Duration::from_millis(200), 2)
        .with_max_delay(Duration::from_secs(2))
        .with_max_retries(2)
}

/// A burst of up to 20 requests, then ~40 requests/second sustained. Generous
/// enough that ordinary diagnostics are never throttled; it only paces heavy
/// `--samples` runs.
fn default_limiter() -> RateLimiter {
    // capacity, refill_amount, refill_interval (in the same unit as `now` — ms).
    RateLimiter::new(20, 1, 25)
}

/// Rate-limit + retry policy bound to a single endpoint, with a shared monotonic
/// clock for the rate limiter.
#[derive(Debug)]
pub struct Resilience {
    backoff: Backoff,
    limiter: Mutex<RateLimiter>,
    started: Instant,
}

impl Resilience {
    /// Build the policy with the project defaults.
    pub fn new() -> Self {
        Self {
            backoff: default_backoff(),
            limiter: Mutex::new(default_limiter()),
            started: Instant::now(),
        }
    }

    /// The backoff delay for a 0-indexed retry `attempt`, or `None` once retries
    /// are exhausted (callers stop retrying and surface the error).
    pub fn retry_delay(&self, attempt: u32) -> Option<Duration> {
        self.backoff.delay(attempt)
    }

    /// Wait asynchronously until a rate-limit token is available, then consume it.
    pub async fn acquire(&self) {
        loop {
            let wait_ms = {
                let now = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
                // Poison only happens if a holder panicked; recover rather than
                // propagate a panic onto the network path.
                let mut limiter = self.limiter.lock().unwrap_or_else(PoisonError::into_inner);
                if limiter.try_acquire_one(now) {
                    return;
                }
                limiter.retry_after(now, 1).unwrap_or(0)
            };
            tokio::time::sleep(Duration::from_millis(wait_ms.max(1))).await;
        }
    }
}

impl Default for Resilience {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether an HTTP error is worth retrying. Deliberately narrow — timeouts,
/// connection errors, and HTTP 429 (Too Many Requests) — so that a real
/// application-level error (a 4xx/5xx that won't change on retry) fails fast and
/// is classified, rather than being retried pointlessly.
pub fn is_transient(error: &reqwest::Error) -> bool {
    error.is_timeout()
        || error.is_connect()
        || error.status().map(|status| status.as_u16()) == Some(429)
}
