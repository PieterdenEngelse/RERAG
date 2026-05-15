use std::{env, time::Duration};
use tokio::time::sleep;

#[tokio::test]
async fn rate_limit_lru_eviction_behavior() {
    let port: u16 = 40125;
    env::set_var("BACKEND_HOST", "127.0.0.1");
    env::set_var("BACKEND_PORT", port.to_string());
    env::set_var("UPLOAD_PORT", "40128");
    env::set_var("RATE_LIMIT_ENABLED", "true");
    env::set_var("TRUST_PROXY", "true");
    env::set_var("RATE_LIMIT_SEARCH_QPS", "100"); // high so refill not a factor
    env::set_var("RATE_LIMIT_SEARCH_BURST", "1"); // 1 token per new IP
    env::set_var("RATE_LIMIT_LRU_CAPACITY", "3"); // tiny to force eviction
    env::set_var("SKIP_INITIAL_INDEXING", "true");

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

    sleep(Duration::from_millis(800)).await;

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/search?q=evict", port);

    // Hit with 3 distinct IPs to fill LRU (burst=1 means first request should be 200)
    for ip in ["1.1.1.1", "2.2.2.2", "3.3.3.3"] {
        let code = client
            .get(&url)
            .header("X-Forwarded-For", ip)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16();
        assert_eq!(code, 200, "initial token for {} should succeed", ip);
    }

    // Now introduce a 4th IP -> should evict the LRU oldest (1.1.1.1)
    let code4 = client
        .get(&url)
        .header("X-Forwarded-For", "4.4.4.4")
        .send()
        .await
        .unwrap()
        .status()
        .as_u16();
    assert_eq!(code4, 200);

    // The oldest (1.1.1.1) should have been evicted; when it comes back it should behave as new -> 200 again
    let code1_again = client
        .get(&url)
        .header("X-Forwarded-For", "1.1.1.1")
        .send()
        .await
        .unwrap()
        .status()
        .as_u16();
    assert_eq!(
        code1_again, 200,
        "after eviction, 1.1.1.1 should be treated as new with fresh token"
    );
}
