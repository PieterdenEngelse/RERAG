use std::{env, time::Duration};
use tokio::time::sleep;

#[tokio::test]
async fn rate_limit_per_ip_token_bucket() {
    // Initialize logging for capturing middleware debug logs in tests
    let _ = tracing_subscriber::fmt::try_init();
    // Choose a port; if busy, adjust
    let port: u16 = 40123;
    env::set_var("BACKEND_HOST", "127.0.0.1");
    env::set_var("BACKEND_PORT", port.to_string());
    env::set_var("UPLOAD_PORT", "40124");
    env::set_var("RATE_LIMIT_ENABLED", "true");
    env::set_var("RATE_LIMIT_QPS", "1");
    env::set_var("RATE_LIMIT_BURST", "2");
    // Trust proxy headers so X-Forwarded-For is honored
    env::set_var("TRUST_PROXY", "true");
    env::set_var("SKIP_INITIAL_INDEXING", "true");
    // Avoid loading .env during test to prevent config contamination
    env::set_var("NO_DOTENV", "true");
    // Allow switching between deterministic (Option 1) and realistic (Option 2) behavior via env
    let mode =
        std::env::var("RATE_LIMIT_TEST_MODE").unwrap_or_else(|_| "deterministic".to_string());
    if mode == "deterministic" {
        // Option 1: Strict determinism – consume burst then block all
        env::set_var(
            "RATE_LIMIT_ROUTES",
            r#"[{"pattern":"/search","match_kind":"Prefix","qps":0.0,"burst":2.0,"label":"search"}]"#,
        );
        // No discrete refill needed here
        env::remove_var("RATE_LIMIT_DISCRETE_REFILL");
        env::remove_var("RATE_LIMIT_REFILL_INTERVAL_MS");
    } else {
        // Option 2: Realistic policy, discrete refill, but assertions tolerant
        env::set_var(
            "RATE_LIMIT_ROUTES",
            r#"[{"pattern":"/search","match_kind":"Prefix","qps":1.0,"burst":2.0,"label":"search"}]"#,
        );
        env::set_var("RATE_LIMIT_DISCRETE_REFILL", "true");
        env::set_var("RATE_LIMIT_REFILL_INTERVAL_MS", "3600000"); // large interval to avoid refills during loop
    }
    // Ensure no global exemptions (default had "/" which exempts everything)
    env::set_var("RATE_LIMIT_EXEMPT_PREFIXES", "[]");
    // Force a single worker to reduce scheduling variance
    env::set_var("ACTIX_WORKERS", "1");

    // Start server in background
    tokio::spawn(async move {
        let config = ag::config::ApiConfig::from_env();
        let pm = &config.path_manager;
        let retriever =
            ag::Retriever::new_with_paths(pm.index_path("tantivy"), pm.vector_store_path())
                .expect("retriever init");
        let retriever = std::sync::Arc::new(std::sync::Mutex::new(retriever));
        ag::api::set_retriever_handle(std::sync::Arc::clone(&retriever));
        ag::api::start_api_server(&config).await.unwrap();
    });

    // Wait a bit for server to bind
    sleep(Duration::from_millis(800)).await;

    let client = reqwest::Client::new();

    // Health warmup
    let _ = client
        .get(format!("http://127.0.0.1:{}/health", port))
        // Use a different client IP for warmup to avoid draining tokens for /search
        .header("X-Forwarded-For", "9.9.9.9")
        .send()
        .await
        .unwrap();

    // Fire 8 requests
    let mut codes = Vec::new();
    for _ in 0..8 {
        let resp = client
            .get(format!("http://127.0.0.1:{}/search?q=hi", port))
            .header("X-Forwarded-For", "1.2.3.4")
            .send()
            .await
            .unwrap();
        codes.push(resp.status().as_u16());
    }

    assert_eq!(codes[0], 200);
    assert_eq!(codes[1], 200);

    if mode == "deterministic" {
        // Strict: after burst is consumed, everything else must be 429
        for i in 2..codes.len() {
            assert_eq!(codes[i], 429, "Unexpected code at {}: {:?}", i, codes);
        }
    } else {
        // Tolerant: allow at most one 200 among the remaining due to potential discrete refill crossing
        let mut extra_ok = 0;
        for i in 2..codes.len() {
            if codes[i] == 200 {
                extra_ok += 1;
                assert!(extra_ok <= 1, "Too many 200s after burst: {:?}", codes);
            } else {
                assert_eq!(codes[i], 429, "Unexpected code at {}: {:?}", i, codes);
            }
        }
    }
}
