// File: tests/rate_limit_middleware_integration_test.rs
// Version: 1.3.0 - updated for RuntimeThresholds API
// Purpose: Integration tests for rate limiting middleware
// Location: tests/rate_limit_middleware_integration_test.rs
//
// Run with: cargo test --test rate_limit_middleware_integration_test -- --nocapture

use actix_web::{http::StatusCode, test, web, App, HttpResponse};
use std::sync::Arc;

// Import from your ag crate - ACTUAL structure
use ag::monitoring::rate_limit_middleware::{RateLimitMiddleware, RateLimitOptions};
use ag::security::rate_limiter::{RateLimiter, RateLimiterConfig, RuntimeThresholds};

// ───────────────────────────────────────────────────────────────────────────
// Test Handler
// ───────────────────────────────────────────────────────────────────────────

/// Simple test handler that returns 200 OK
async fn test_handler() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

// ───────────────────────────────────────────────────────────────────────────
// Helpers
// ───────────────────────────────────────────────────────────────────────────

fn default_config() -> RateLimiterConfig {
    RateLimiterConfig {
        enabled: true,
        qps: 10.0,
        burst: 20.0,
        max_ips: 100,
    }
}

fn make_thresholds(
    search_qps: f64,
    search_burst: f64,
    upload_qps: f64,
    upload_burst: f64,
) -> Arc<RuntimeThresholds> {
    RuntimeThresholds::new(search_qps, search_burst, upload_qps, upload_burst)
}

fn default_opts(
    search_qps: f64,
    search_burst: f64,
    upload_qps: f64,
    upload_burst: f64,
    trust_proxy: bool,
) -> RateLimitOptions {
    RateLimitOptions {
        thresholds: make_thresholds(search_qps, search_burst, upload_qps, upload_burst),
        rules: vec![],
        exempt_prefixes: vec![],
        trust_proxy,
    }
}

// ───────────────────────────────────────────────────────────────────────────
// TEST 1: Middleware allows first request
// ───────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_middleware_allows_first_request() {
    let thresholds = make_thresholds(10.0, 20.0, 5.0, 10.0);
    let limiter = Arc::new(RateLimiter::new(default_config(), thresholds.clone()));
    let opts = RateLimitOptions {
        thresholds,
        rules: vec![],
        exempt_prefixes: vec![],
        trust_proxy: false,
    };

    let app = test::init_service(
        App::new()
            .wrap(RateLimitMiddleware::new_with_options(limiter, opts))
            .route("/search", web::get().to(test_handler)),
    )
    .await;

    let req = test::TestRequest::get().uri("/search?q=test").to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// ───────────────────────────────────────────────────────────────────────────
// TEST 2: Middleware blocks excess requests
// ───────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_middleware_blocks_excess_requests() {
    let config = RateLimiterConfig {
        enabled: true,
        qps: 1.0,
        burst: 1.0,
        max_ips: 100,
    };
    let thresholds = make_thresholds(1.0, 1.0, 5.0, 10.0);
    let limiter = Arc::new(RateLimiter::new(config, thresholds.clone()));
    let opts = default_opts(1.0, 1.0, 5.0, 10.0, false);

    let app = test::init_service(
        App::new()
            .wrap(RateLimitMiddleware::new_with_options(limiter, opts))
            .route("/search", web::get().to(test_handler)),
    )
    .await;

    // First request should succeed
    let req1 = test::TestRequest::get().uri("/search?q=test1").to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), StatusCode::OK);

    // Second immediate request should be rate limited
    let req2 = test::TestRequest::get().uri("/search?q=test2").to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), StatusCode::TOO_MANY_REQUESTS);
}

// ───────────────────────────────────────────────────────────────────────────
// TEST 3: Retry-After header is set
// ───────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_middleware_sets_retry_after_header() {
    let config = RateLimiterConfig {
        enabled: true,
        qps: 1.0,
        burst: 1.0,
        max_ips: 100,
    };
    let thresholds = make_thresholds(1.0, 1.0, 5.0, 10.0);
    let limiter = Arc::new(RateLimiter::new(config, thresholds.clone()));
    let opts = default_opts(1.0, 1.0, 5.0, 10.0, false);

    let app = test::init_service(
        App::new()
            .wrap(RateLimitMiddleware::new_with_options(limiter, opts))
            .route("/search", web::get().to(test_handler)),
    )
    .await;

    // Exhaust rate limit
    let req1 = test::TestRequest::get().uri("/search?q=test1").to_request();
    let _ = test::call_service(&app, req1).await;

    // Next request should fail and have Retry-After header
    let req2 = test::TestRequest::get().uri("/search?q=test2").to_request();
    let resp = test::call_service(&app, req2).await;

    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(resp.headers().contains_key("retry-after"));

    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    assert!(retry_after.is_some());
    assert!(retry_after.unwrap() > 0);
}

// ───────────────────────────────────────────────────────────────────────────
// TEST 4: Search route respects search_qps limit
// ───────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_middleware_per_route_policies() {
    let config = RateLimiterConfig {
        enabled: true,
        qps: 10.0,
        burst: 20.0,
        max_ips: 100,
    };
    let thresholds = make_thresholds(1.0, 1.0, 100.0, 200.0);
    let limiter = Arc::new(RateLimiter::new(config, thresholds.clone()));
    let opts = RateLimitOptions {
        thresholds,
        rules: vec![],
        exempt_prefixes: vec![],
        trust_proxy: false,
    };

    let app = test::init_service(
        App::new()
            .wrap(RateLimitMiddleware::new_with_options(limiter, opts))
            .route("/search", web::get().to(test_handler)),
    )
    .await;

    // /search should be rate limited (1.0 QPS)
    let search1 = test::TestRequest::get().uri("/search").to_request();
    let search2 = test::TestRequest::get().uri("/search").to_request();

    let _ = test::call_service(&app, search1).await;
    let resp = test::call_service(&app, search2).await;
    assert_eq!(
        resp.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "search should be rate limited at 1.0 QPS"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// TEST 5: X-Forwarded-For header respected
// ───────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_middleware_trust_proxy_x_forwarded_for() {
    let config = RateLimiterConfig {
        enabled: true,
        qps: 1.0,
        burst: 1.0,
        max_ips: 100,
    };
    let thresholds = make_thresholds(1.0, 1.0, 5.0, 10.0);
    let limiter = Arc::new(RateLimiter::new(config, thresholds.clone()));
    let opts = default_opts(1.0, 1.0, 5.0, 10.0, true);

    let app = test::init_service(
        App::new()
            .wrap(RateLimitMiddleware::new_with_options(limiter, opts))
            .route("/search", web::get().to(test_handler)),
    )
    .await;

    // First request from 192.0.2.1
    let req1 = test::TestRequest::get()
        .uri("/search?q=test1")
        .insert_header(("X-Forwarded-For", "192.0.2.1"))
        .to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), StatusCode::OK);

    // Second request from same IP should be rate limited
    let req2 = test::TestRequest::get()
        .uri("/search?q=test2")
        .insert_header(("X-Forwarded-For", "192.0.2.1"))
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), StatusCode::TOO_MANY_REQUESTS);

    // Request from different IP should succeed
    let req3 = test::TestRequest::get()
        .uri("/search?q=test3")
        .insert_header(("X-Forwarded-For", "192.0.2.2"))
        .to_request();
    let resp3 = test::call_service(&app, req3).await;
    assert_eq!(resp3.status(), StatusCode::OK);
}

// ───────────────────────────────────────────────────────────────────────────
// TEST 6: Forwarded header (RFC 7239) respected
// ───────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_middleware_trust_proxy_forwarded_header() {
    let thresholds = make_thresholds(1.0, 1.0, 5.0, 10.0);
    let limiter = Arc::new(RateLimiter::new(default_config(), thresholds.clone()));
    let opts = default_opts(1.0, 1.0, 5.0, 10.0, true);

    let app = test::init_service(
        App::new()
            .wrap(RateLimitMiddleware::new_with_options(limiter, opts))
            .route("/search", web::get().to(test_handler)),
    )
    .await;

    // Request with RFC 7239 Forwarded header
    let req1 = test::TestRequest::get()
        .uri("/search?q=test1")
        .insert_header(("Forwarded", "for=192.0.2.10"))
        .to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), StatusCode::OK);

    // Second request with same Forwarded IP should be rate limited
    let req2 = test::TestRequest::get()
        .uri("/search?q=test2")
        .insert_header(("Forwarded", "for=192.0.2.10"))
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), StatusCode::TOO_MANY_REQUESTS);
}

// ───────────────────────────────────────────────────────────────────────────
// TEST 7: Exempt prefixes bypass rate limiting
// ───────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_middleware_exempt_prefixes() {
    let thresholds = make_thresholds(1.0, 1.0, 5.0, 10.0);
    let limiter = Arc::new(RateLimiter::new(default_config(), thresholds.clone()));
    let opts = RateLimitOptions {
        thresholds,
        rules: vec![],
        exempt_prefixes: vec!["/health".to_string(), "/ready".to_string()],
        trust_proxy: false,
    };

    let app = test::init_service(
        App::new()
            .wrap(RateLimitMiddleware::new_with_options(limiter, opts))
            .route("/search", web::get().to(test_handler))
            .route("/health", web::get().to(test_handler)),
    )
    .await;

    // /health should NOT be rate limited (exempt prefix)
    for i in 0..10 {
        let req = test::TestRequest::get()
            .uri(&format!("/health?check={}", i))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // /search should still be rate limited
    let req1 = test::TestRequest::get().uri("/search").to_request();
    let _ = test::call_service(&app, req1).await;

    let req2 = test::TestRequest::get().uri("/search").to_request();
    let resp = test::call_service(&app, req2).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

// ───────────────────────────────────────────────────────────────────────────
// TEST 8: 429 response body contains valid JSON
// ───────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_middleware_429_response_format() {
    use actix_web::body::to_bytes;

    let config = RateLimiterConfig {
        enabled: true,
        qps: 1.0,
        burst: 1.0,
        max_ips: 100,
    };
    let thresholds = make_thresholds(1.0, 1.0, 5.0, 10.0);
    let limiter = Arc::new(RateLimiter::new(config, thresholds.clone()));
    let opts = default_opts(1.0, 1.0, 5.0, 10.0, false);

    let app = test::init_service(
        App::new()
            .wrap(RateLimitMiddleware::new_with_options(limiter, opts))
            .route("/search", web::get().to(test_handler)),
    )
    .await;

    // Exhaust limit
    let req1 = test::TestRequest::get().uri("/search").to_request();
    let _ = test::call_service(&app, req1).await;

    // Get 429 response
    let req2 = test::TestRequest::get().uri("/search").to_request();
    let resp = test::call_service(&app, req2).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    // Verify response body
    let body = to_bytes(resp.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "rate_limited");
    assert!(json["message"].is_string());
    assert!(json["retry_after"].is_number());
}

// ───────────────────────────────────────────────────────────────────────────
// TEST 9: Different IPs are tracked independently
// ───────────────────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_middleware_per_ip_isolation() {
    let config = RateLimiterConfig {
        enabled: true,
        qps: 1.0,
        burst: 1.0,
        max_ips: 100,
    };
    let thresholds = make_thresholds(1.0, 1.0, 5.0, 10.0);
    let limiter = Arc::new(RateLimiter::new(config, thresholds.clone()));
    let opts = default_opts(1.0, 1.0, 5.0, 10.0, true);

    let app = test::init_service(
        App::new()
            .wrap(RateLimitMiddleware::new_with_options(limiter, opts))
            .route("/search", web::get().to(test_handler)),
    )
    .await;

    // IP1: first request OK
    let req1 = test::TestRequest::get()
        .uri("/search")
        .insert_header(("X-Forwarded-For", "192.0.2.100"))
        .to_request();
    let resp = test::call_service(&app, req1).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // IP1: second request rate limited
    let req2 = test::TestRequest::get()
        .uri("/search")
        .insert_header(("X-Forwarded-For", "192.0.2.100"))
        .to_request();
    let resp = test::call_service(&app, req2).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    // IP2: first request OK (different bucket)
    let req3 = test::TestRequest::get()
        .uri("/search")
        .insert_header(("X-Forwarded-For", "192.0.2.101"))
        .to_request();
    let resp = test::call_service(&app, req3).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // IP2: second request rate limited (different bucket than IP1)
    let req4 = test::TestRequest::get()
        .uri("/search")
        .insert_header(("X-Forwarded-For", "192.0.2.101"))
        .to_request();
    let resp = test::call_service(&app, req4).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}
