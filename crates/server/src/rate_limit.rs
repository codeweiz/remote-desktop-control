//! Per-IP token-bucket rate limiter.
//!
//! Each unique client IP gets its own [`TokenBucket`] that refills at a steady
//! rate. When the bucket is empty the request is rejected with `429 Too Many
//! Requests`.
//!
//! Four separate limiters cover different endpoint categories:
//! - **auth** — unauthenticated auth endpoints (5 req/min)
//! - **ws** — WebSocket upgrade requests (10 req/min)
//! - **get** — authenticated GET requests (200 req/min)
//! - **post** — authenticated POST/PUT/DELETE requests (60 req/min)

use std::net::IpAddr;
use std::time::Instant;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Token bucket
// ---------------------------------------------------------------------------

struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Try to consume one token. Returns `true` if the request is allowed.
    fn try_consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Per-category limiter
// ---------------------------------------------------------------------------

/// A rate limiter that maintains per-IP token buckets for a single category.
struct CategoryLimiter {
    buckets: DashMap<IpAddr, TokenBucket>,
    max_tokens: f64,
    refill_rate: f64,
}

impl CategoryLimiter {
    fn new(requests_per_minute: u32) -> Self {
        let max = requests_per_minute as f64;
        Self {
            buckets: DashMap::new(),
            max_tokens: max,
            refill_rate: max / 60.0,
        }
    }

    /// Returns `true` if the request is allowed.
    fn check(&self, ip: &IpAddr) -> bool {
        let mut entry = self
            .buckets
            .entry(*ip)
            .or_insert_with(|| TokenBucket::new(self.max_tokens, self.refill_rate));
        entry.try_consume()
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Request category for rate limiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestCategory {
    /// Unauthenticated auth endpoints (login, token validation).
    Auth,
    /// WebSocket upgrade requests.
    WebSocket,
    /// Authenticated GET requests.
    AuthGet,
    /// Authenticated POST/PUT/DELETE requests.
    AuthPost,
}

/// Composite rate limiter holding one [`CategoryLimiter`] per endpoint class.
pub struct RateLimiter {
    auth: CategoryLimiter,
    ws: CategoryLimiter,
    get: CategoryLimiter,
    post: CategoryLimiter,
}

impl RateLimiter {
    /// Create a new `RateLimiter` with the default quotas from the spec.
    pub fn new() -> Self {
        Self {
            auth: CategoryLimiter::new(5),   // 5 req/min
            ws: CategoryLimiter::new(10),     // 10 req/min
            get: CategoryLimiter::new(200),   // 200 req/min
            post: CategoryLimiter::new(60),   // 60 req/min
        }
    }

    /// Check whether a request from `ip` in the given `category` is allowed.
    pub fn check(&self, ip: &IpAddr, category: RequestCategory) -> bool {
        match category {
            RequestCategory::Auth => self.auth.check(ip),
            RequestCategory::WebSocket => self.ws.check(ip),
            RequestCategory::AuthGet => self.get.check(ip),
            RequestCategory::AuthPost => self.post.check(ip),
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Axum middleware
// ---------------------------------------------------------------------------

/// Extract the client IP address from the request.
///
/// Checks `X-Forwarded-For`, `X-Real-Ip`, and falls back to `127.0.0.1`.
pub fn extract_client_ip(req: &Request<Body>) -> IpAddr {
    // Try X-Forwarded-For first (first IP in the list)
    if let Some(xff) = req.headers().get("x-forwarded-for") {
        if let Ok(value) = xff.to_str() {
            if let Some(first) = value.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }
    // Try X-Real-Ip
    if let Some(xri) = req.headers().get("x-real-ip") {
        if let Ok(value) = xri.to_str() {
            if let Ok(ip) = value.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }
    // Fallback — localhost
    IpAddr::from([127, 0, 0, 1])
}

/// Classify a request into a [`RequestCategory`].
fn classify(req: &Request<Body>) -> RequestCategory {
    let path = req.uri().path();

    // WebSocket upgrade requests
    if path.starts_with("/ws/") {
        return RequestCategory::WebSocket;
    }

    // Health / unauthenticated endpoints treated as Auth category to keep
    // the public surface rate-limited.
    if path == "/health" {
        return RequestCategory::Auth;
    }

    // Authenticated API
    match *req.method() {
        Method::GET | Method::HEAD | Method::OPTIONS => RequestCategory::AuthGet,
        _ => RequestCategory::AuthPost,
    }
}

/// Axum middleware that enforces per-IP rate limits.
///
/// Must be installed as an outer layer so it runs before auth.
pub async fn rate_limit_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let ip = extract_client_ip(&request);
    let category = classify(&request);

    // Check blocklist first
    if state.blocklist.is_banned(&ip) {
        return (StatusCode::FORBIDDEN, "Forbidden: IP banned").into_response();
    }

    // Check rate limit
    if !state.rate_limiter.check(&ip, category) {
        tracing::warn!(ip = %ip, category = ?category, "rate limit exceeded");
        return (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests").into_response();
    }

    next.run(request).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_bucket_allows_up_to_max() {
        let mut bucket = TokenBucket::new(3.0, 3.0 / 60.0);
        assert!(bucket.try_consume());
        assert!(bucket.try_consume());
        assert!(bucket.try_consume());
        // Fourth request should be denied
        assert!(!bucket.try_consume());
    }

    #[test]
    fn rate_limiter_per_ip() {
        let limiter = RateLimiter::new();
        let ip1: IpAddr = "1.2.3.4".parse().unwrap();
        let ip2: IpAddr = "5.6.7.8".parse().unwrap();

        // Auth limit is 5/min — consume all for ip1
        for _ in 0..5 {
            assert!(limiter.check(&ip1, RequestCategory::Auth));
        }
        assert!(!limiter.check(&ip1, RequestCategory::Auth));

        // ip2 should still have its own bucket
        assert!(limiter.check(&ip2, RequestCategory::Auth));
    }
}
