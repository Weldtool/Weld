//! Small in-process rate limiter for the unauthenticated endpoints.
//!
//! Login, registration and study provisioning are the only routes anyone on the
//! internet can hammer without a token, and nothing slowed them down: a password
//! could be guessed as fast as Argon2 would verify it. This is a fixed-window
//! counter keyed by client IP and, for login, by the account being attacked.
//!
//! In-memory and per-process, like the realtime `Hub`: right for a single server
//! process. Move it to Redis if the server is ever scaled horizontally.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;

use crate::errors::ApiError;

/// Entries beyond this trigger a sweep of expired windows, so the map cannot
/// grow without bound under a spray of distinct keys.
const SWEEP_THRESHOLD: usize = 10_000;

pub struct RateLimiter {
    max: u32,
    window: Duration,
    hits: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new(max: u32, window: Duration) -> Self {
        Self {
            max,
            window,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// Whether `key` has already used up its allowance for the current window.
    pub fn is_limited(&self, key: &str) -> bool {
        // A poisoned mutex only means another request panicked mid-update; the
        // counters are best-effort, so recover rather than fail the request.
        let hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        hits.get(key)
            .is_some_and(|(start, count)| start.elapsed() < self.window && *count >= self.max)
    }

    /// Count one event against `key`.
    pub fn record(&self, key: &str) {
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        if hits.len() >= SWEEP_THRESHOLD {
            let window = self.window;
            hits.retain(|_, (start, _)| start.elapsed() < window);
        }
        let entry = hits.entry(key.to_string()).or_insert((Instant::now(), 0));
        if entry.0.elapsed() >= self.window {
            *entry = (Instant::now(), 0);
        }
        entry.1 += 1;
    }

    /// Forget `key` — e.g. after a successful login.
    pub fn reset(&self, key: &str) {
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        hits.remove(key);
    }

    /// Count one event and reject it if the allowance is already spent.
    pub fn check(&self, key: &str) -> Result<(), ApiError> {
        if self.is_limited(key) {
            return Err(too_many());
        }
        self.record(key);
        Ok(())
    }
}

pub fn too_many() -> ApiError {
    ApiError::TooManyRequests("Too many attempts. Please wait a few minutes and try again.".into())
}

/// Login attempts per client IP.
pub static LOGIN_BY_IP: LazyLock<RateLimiter> =
    LazyLock::new(|| RateLimiter::new(20, Duration::from_secs(60)));

/// *Failed* logins per account. Counted on failure and cleared on success, so a
/// user who knows their password is not locked out by their own typos for long,
/// while a guesser gets ten tries per window however many IPs they use.
pub static LOGIN_FAILURES_BY_EMAIL: LazyLock<RateLimiter> =
    LazyLock::new(|| RateLimiter::new(10, Duration::from_secs(10 * 60)));

/// Account creation per client IP.
pub static REGISTER_BY_IP: LazyLock<RateLimiter> =
    LazyLock::new(|| RateLimiter::new(20, Duration::from_secs(60 * 60)));

/// Study provisioning per client IP. Each call clones a whole workspace and is
/// also the oracle for guessing access codes.
pub static PROVISION_BY_IP: LazyLock<RateLimiter> =
    LazyLock::new(|| RateLimiter::new(20, Duration::from_secs(60 * 60)));

/// The client's IP as reported by the reverse proxy, if there is one.
///
/// Prefers `X-Real-IP`, then the right-most `X-Forwarded-For` entry — the one
/// appended by the proxy closest to us, which a client cannot forge (the
/// left-most entries are whatever the client sent). Returns `None` when neither
/// header is present, i.e. when nothing sits in front of the server (local dev,
/// tests); per-IP limits are skipped then rather than lumping every caller into
/// one bucket.
pub fn client_ip(headers: &HeaderMap) -> Option<String> {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };

    header("x-real-ip")
        .map(str::to_string)
        .or_else(|| {
            header("x-forwarded-for")
                .and_then(|v| v.rsplit(',').next())
                .map(|v| v.trim().to_string())
        })
        .filter(|ip| !ip.is_empty())
}

/// Applies a per-IP limiter when the client IP is known.
pub fn check_ip(limiter: &RateLimiter, headers: &HeaderMap) -> Result<(), ApiError> {
    match client_ip(headers) {
        Some(ip) => limiter.check(&ip),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_max_then_rejects() {
        let limiter = RateLimiter::new(3, Duration::from_secs(60));
        for _ in 0..3 {
            assert!(limiter.check("k").is_ok());
        }
        assert!(limiter.check("k").is_err());
        // A different key has its own allowance.
        assert!(limiter.check("other").is_ok());
    }

    #[test]
    fn window_expiry_restores_the_allowance() {
        let limiter = RateLimiter::new(1, Duration::from_millis(20));
        assert!(limiter.check("k").is_ok());
        assert!(limiter.check("k").is_err());
        std::thread::sleep(Duration::from_millis(30));
        assert!(limiter.check("k").is_ok());
    }

    #[test]
    fn reset_clears_recorded_failures() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        limiter.record("k");
        limiter.record("k");
        assert!(limiter.is_limited("k"));
        limiter.reset("k");
        assert!(!limiter.is_limited("k"));
    }

    #[test]
    fn client_ip_prefers_real_ip_then_rightmost_forwarded() {
        let mut headers = HeaderMap::new();
        assert_eq!(client_ip(&headers), None);

        headers.insert("x-forwarded-for", "6.6.6.6, 10.0.0.1".parse().unwrap());
        assert_eq!(client_ip(&headers).as_deref(), Some("10.0.0.1"));

        headers.insert("x-real-ip", "203.0.113.7".parse().unwrap());
        assert_eq!(client_ip(&headers).as_deref(), Some("203.0.113.7"));
    }
}
