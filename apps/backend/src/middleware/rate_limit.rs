use axum::extract::ConnectInfo;
use axum::{
    Json,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Above this many tracked keys, a `check()` call first sweeps expired entries
/// to bound memory (the map is otherwise unbounded and a DoS vector).
const EVICTION_THRESHOLD: usize = 10_000;

#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<DashMap<String, RateLimitEntry>>,
    max_requests: u32,
    window: Duration,
    trust_proxy_headers: bool,
}

struct RateLimitEntry {
    count: u32,
    window_start: Instant,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64, trust_proxy_headers: bool) -> Self {
        Self {
            state: Arc::new(DashMap::new()),
            max_requests,
            window: Duration::from_secs(window_secs),
            trust_proxy_headers,
        }
    }

    pub fn check(&self, key: &str) -> Result<(), ()> {
        let now = Instant::now();

        // Bound memory: drop expired buckets once the map grows large. Done
        // before taking a shard lock on `key` to avoid a self-deadlock.
        if self.state.len() > EVICTION_THRESHOLD {
            self.state
                .retain(|_, e| now.duration_since(e.window_start) <= self.window);
        }

        let mut entry = self.state.entry(key.to_string()).or_insert(RateLimitEntry {
            count: 0,
            window_start: now,
        });

        if now.duration_since(entry.window_start) > self.window {
            entry.count = 0;
            entry.window_start = now;
        }
        entry.count += 1;

        if entry.count > self.max_requests {
            Err(())
        } else {
            Ok(())
        }
    }

    /// Derive the rate-limit bucket key for a request. When `trust_proxy_headers`
    /// is enabled, the first `X-Forwarded-For` / `X-Real-IP` value is used;
    /// otherwise the TCP peer IP (via `ConnectInfo`) is used so clients cannot
    /// spoof their bucket. Never keys on the `Authorization` header, which would
    /// leak bearer tokens into server state and let a stolen token grief its owner.
    fn extract_client_key(&self, req: &Request) -> String {
        if self.trust_proxy_headers {
            if let Some(ip) = req
                .headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(',').next())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                return ip.to_string();
            }
            if let Some(ip) = req
                .headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                return ip.to_string();
            }
        }

        if let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
            return addr.ip().to_string();
        }

        "unknown".to_string()
    }
}

pub async fn rate_limit_middleware(
    limiter: axum::extract::Extension<RateLimiter>,
    req: Request,
    next: Next,
) -> Response {
    let key = limiter.extract_client_key(&req);

    match limiter.check(&key) {
        Ok(()) => next.run(req).await,
        Err(()) => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Too many requests. Please try again later."})),
        )
            .into_response(),
    }
}
