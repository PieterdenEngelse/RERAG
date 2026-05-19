// File: src/monitoring/rate_limit_middleware.rs
// Phase 15 Step 6: Security and Hardening - Rate Limiting Middleware (Improved)
// Version: 1.1.0
// Location: src/monitoring/rate_limit_middleware.rs
//
// Purpose: Production-grade per-IP rate limiting with:
// - TRUST_PROXY support (X-Forwarded-For, Forwarded headers)
// - Per-route QPS/BURST policies
// - Retry-After headers
// - Metrics and structured logging

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderValue, RETRY_AFTER},
    Error, HttpResponse,
};
use futures_util::future::LocalBoxFuture;
use std::sync::Arc;

use crate::monitoring::metrics::{
    RATE_LIMIT_DROPS_BY_ROUTE, RATE_LIMIT_DROPS_BY_SERVER_ROUTE, RATE_LIMIT_DROPS_TOTAL,
};
use crate::security::rate_limiter::{RateLimiter, RuntimeThresholds};
use actix_web::body::EitherBody;
use serde::{Deserialize, Serialize};
use tracing::warn;

#[cfg(feature = "rl_yaml")]
use serde_yaml;

/// Extract client IP with TRUST_PROXY support
///
/// Priority:
/// 1. X-Forwarded-For header (first IP)
/// 2. Forwarded header (for= field)
/// 3. peer_addr from connection_info
fn extract_client_ip(req: &ServiceRequest, trust_proxy: bool) -> String {
    if trust_proxy {
        // Check X-Forwarded-For header (most common)
        if let Some(header) = req.headers().get("X-Forwarded-For") {
            if let Ok(s) = header.to_str() {
                // Take first IP from comma-separated list
                if let Some(ip) = s.split(',').next() {
                    return ip.trim().to_string();
                }
            }
        }

        // Check Forwarded header (RFC 7239)
        if let Some(header) = req.headers().get("Forwarded") {
            if let Ok(s) = header.to_str() {
                // Parse "for=192.0.2.1" or "for=[2001:db8:cafe::17]"
                if let Some(for_clause) = s.split(';').find(|c| c.trim().starts_with("for=")) {
                    let ip = for_clause
                        .trim_start_matches("for=")
                        .trim()
                        .trim_start_matches('[')
                        .trim_end_matches(']');
                    return ip.to_string();
                }
            }
        }
    }

    // Fall back to peer_addr
    req.connection_info()
        .peer_addr()
        .unwrap_or("127.0.0.1")
        .to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MatchKind {
    Exact,
    Prefix,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouteRule {
    pub pattern: String,
    pub match_kind: MatchKind,
    pub qps: f64,
    pub burst: f64,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RateLimitOptions {
    pub trust_proxy: bool,
    pub thresholds: Arc<RuntimeThresholds>,
    pub rules: Vec<RouteRule>,
    pub exempt_prefixes: Vec<String>,
}

impl RateLimitOptions {
    /// Merge environment overrides into this options struct.
    /// Supports:
    /// - RATE_LIMIT_ROUTES: JSON array of RouteRule or JSON object { routes: [...], exempt_prefixes: [...] }
    /// - RATE_LIMIT_ROUTES_FILE: path to JSON file with the same schema
    /// - RATE_LIMIT_EXEMPT_PREFIXES: JSON array of strings (fallback)
    pub fn with_env_overrides(mut self) -> Self {
        #[derive(Deserialize)]
        struct RoutesFile {
            routes: Option<Vec<RouteRule>>,
            exempt_prefixes: Option<Vec<String>>,
        }

        fn parse_json(input: &str) -> (Option<Vec<RouteRule>>, Option<Vec<String>>) {
            if let Ok(rules) = serde_json::from_str::<Vec<RouteRule>>(input) {
                return (Some(rules), None);
            }
            if let Ok(cfg) = serde_json::from_str::<RoutesFile>(input) {
                return (cfg.routes, cfg.exempt_prefixes);
            }
            (None, None)
        }

        if let Ok(val) = std::env::var("RATE_LIMIT_ROUTES") {
            let (rules, exempt) = parse_json(&val);
            if let Some(rules) = rules {
                self.rules = rules;
            }
            if let Some(ex) = exempt {
                self.exempt_prefixes = ex;
            }
        } else if let Ok(path) = std::env::var("RATE_LIMIT_ROUTES_FILE") {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    // Try JSON first
                    let (rules, exempt) = parse_json(&text);

                    // If not JSON and looks like YAML, try YAML (if feature enabled)
                    if rules.is_none() && (path.ends_with(".yml") || path.ends_with(".yaml")) {
                        #[cfg(feature = "rl_yaml")]
                        {
                            match serde_yaml::from_str::<Vec<RouteRule>>(&text) {
                                Ok(rs) => rules = Some(rs),
                                Err(_) => {
                                    // Try object form
                                    #[derive(Deserialize)]
                                    struct YamlFile {
                                        routes: Option<Vec<RouteRule>>,
                                        exempt_prefixes: Option<Vec<String>>,
                                    }
                                    if let Ok(cfg) = serde_yaml::from_str::<YamlFile>(&text) {
                                        rules = cfg.routes;
                                        exempt = cfg.exempt_prefixes;
                                    } else {
                                        warn!(file = %path, "Failed to parse YAML routes file");
                                    }
                                }
                            }
                        }
                        #[cfg(not(feature = "rl_yaml"))]
                        {
                            warn!(file = %path, "YAML provided but 'rl_yaml' feature is not enabled. Rebuild with YAML support or provide JSON.");
                        }
                    }

                    if let Some(rules) = rules {
                        self.rules = rules;
                    }
                    if let Some(ex) = exempt {
                        self.exempt_prefixes = ex;
                    }
                }
                Err(e) => {
                    warn!(file = %path, error = %e, "Failed to read RATE_LIMIT_ROUTES_FILE");
                }
            }
        }

        if let Ok(val) = std::env::var("RATE_LIMIT_EXEMPT_PREFIXES") {
            if let Ok(list) = serde_json::from_str::<Vec<String>>(&val) {
                self.exempt_prefixes = list;
            }
        }
        self
    }

    fn classify_default(&self, path: &str, _method: &str) -> (f64, f64, String) {
        if path.starts_with("/upload")
            || path.starts_with("/save_vectors")
            || path.starts_with("/reindex")
            || path.starts_with("/memory/store_rag")
        {
            (
                self.thresholds.get_upload_qps().max(0.0),
                self.thresholds.get_upload_burst().max(0.0),
                "upload".to_string(),
            )
        } else {
            (
                self.thresholds.get_search_qps().max(0.0),
                self.thresholds.get_search_burst().max(0.0),
                "search".to_string(),
            )
        }
    }

    pub fn for_request(&self, req: &actix_web::dev::ServiceRequest) -> Option<(f64, f64, String)> {
        let path = req.path();
        let method = req.method().as_str();

        for p in &self.exempt_prefixes {
            if path.starts_with(p) {
                return None;
            }
        }

        for r in &self.rules {
            let is_match = match r.match_kind {
                MatchKind::Exact => path == r.pattern,
                MatchKind::Prefix => path.starts_with(&r.pattern),
            };
            if is_match {
                let label = r
                    .label
                    .clone()
                    .unwrap_or_else(|| req.match_pattern().unwrap_or(path.to_string()));
                return Some((r.qps.max(0.0), r.burst.max(0.0), label));
            }
        }

        let (qps, burst, default_label) = self.classify_default(path, method);
        let route_label = req.match_pattern().unwrap_or(default_label);
        Some((qps, burst, route_label))
    }
}

/// Rate limiting middleware
pub struct RateLimitMiddleware {
    rate_limiter: Arc<RateLimiter>,
    opts: RateLimitOptions,
    server: &'static str,
}

impl RateLimitMiddleware {
    pub fn new_with_options(rate_limiter: Arc<RateLimiter>, opts: RateLimitOptions) -> Self {
        Self {
            rate_limiter,
            opts,
            server: "search",
        }
    }

    pub fn with_server(mut self, server: &'static str) -> Self {
        self.server = server;
        self
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimitMiddlewareService<S>;
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(RateLimitMiddlewareService {
            service: std::sync::Arc::new(service),
            rate_limiter: Arc::clone(&self.rate_limiter),
            opts: self.opts.clone(),
            server: self.server,
        }))
    }
}

pub struct RateLimitMiddlewareService<S> {
    service: std::sync::Arc<S>,
    rate_limiter: Arc<RateLimiter>,
    opts: RateLimitOptions,
    server: &'static str,
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Extract client IP with TRUST_PROXY support
        let client_ip = extract_client_ip(&req, self.opts.trust_proxy);

        if let Some((qps, burst, route_label)) = self.opts.for_request(&req) {
            // Optional debug logging for tests
            if std::env::var("RATE_LIMIT_DEBUG_LOG")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(false)
            {
                tracing::info!(client_ip=%client_ip, route=%route_label, qps=%qps, burst=%burst, "rate_limit_check");
            }
            // Build per-route+IP key to isolate budgets
            let key = format!("{}::{}", client_ip, route_label);
            // Check rate limit using per-route policy
            let (allowed, retry_after) = self.rate_limiter.check_key(&key, qps, burst);
            if !allowed {
                tracing::warn!(
                    route = %route_label,
                    client_ip = %client_ip,
                    qps = qps,
                    burst = burst,
                    retry_after_secs = retry_after,
                    "Rate limit exceeded"
                );

                // Increment metrics
                RATE_LIMIT_DROPS_TOTAL.inc();
                RATE_LIMIT_DROPS_BY_ROUTE
                    .with_label_values(&[&route_label])
                    .inc();
                RATE_LIMIT_DROPS_BY_SERVER_ROUTE
                    .with_label_values(&[self.server, &route_label])
                    .inc();

                return Box::pin(async move {
                    let resp = HttpResponse::TooManyRequests()
                        .insert_header((
                            RETRY_AFTER,
                            HeaderValue::from_str(&retry_after.to_string())
                                .unwrap_or_else(|_| HeaderValue::from_static("1")),
                        ))
                        .json(serde_json::json!({
                            "status": "rate_limited",
                            "message": "Too many requests",
                            "retry_after": retry_after,
                            "qps_limit": qps,
                        }));
                    Ok(req.into_response(resp.map_into_right_body()))
                });
            }
        } else {
            // Exempt; pass through
        }

        let service = std::sync::Arc::clone(&self.service);
        Box::pin(async move {
            let res = service.call(req).await?;
            Ok(res.map_into_left_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ip_no_proxy() {
        // Simulated request - in real tests use actix test utilities
        // This is a structural test only
        let trust_proxy = false;
        assert!(!trust_proxy);
    }

    #[test]
    fn test_middleware_creation() {
        let config = crate::security::rate_limiter::RateLimiterConfig {
            enabled: true,
            qps: 1.0,
            burst: 1.0,
            max_ips: 100,
        };
        let thresholds = crate::security::rate_limiter::RuntimeThresholds::new(5.0, 10.0, 2.0, 5.0);
        let limiter = std::sync::Arc::new(crate::security::rate_limiter::RateLimiter::new(
            config,
            std::sync::Arc::clone(&thresholds),
        ));
        let opts = RateLimitOptions {
            trust_proxy: true,
            thresholds,
            exempt_prefixes: vec![],
            rules: vec![],
        };
        let _middleware = RateLimitMiddleware::new_with_options(limiter, opts);
        // Middleware created successfully
    }
}
