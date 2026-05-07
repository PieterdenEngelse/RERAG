use std::{env, time::Duration};
use tokio::time::sleep;

#[tokio::test]
async fn rate_limit_read_and_write_buckets() {
    let port: u16 = 40124;
    // Deterministic per-route configuration and stable test env
    env::set_var("NO_DOTENV", "true");
    env::set_var("RATE_LIMIT_EXEMPT_PREFIXES", "[]");
    env::set_var(
        "RATE_LIMIT_ROUTES",
        r#"[{"pattern":"/search","match_kind":"Prefix","qps":0.0,"burst":2.0,"label":"search"},
            {"pattern":"/api/search","match_kind":"Prefix","qps":0.0,"burst":2.0,"label":"search"},
            {"pattern":"/upload","match_kind":"Prefix","qps":1.0,"burst":2.0,"label":"upload"},
            {"pattern":"/api/upload","match_kind":"Prefix","qps":1.0,"burst":2.0,"label":"upload"}]"#,
    );
    // Global enable
    env::set_var("BACKEND_HOST", "127.0.0.1");
    env::set_var("BACKEND_PORT", port.to_string());
    env::set_var("RATE_LIMIT_ENABLED", "true");
    env::set_var("TRUST_PROXY", "true");
    // Route-specific: make read burst and write burst small to assert quickly
    env::set_var("RATE_LIMIT_SEARCH_QPS", "1");
    env::set_var("RATE_LIMIT_SEARCH_BURST", "3");
    env::set_var("RATE_LIMIT_UPLOAD_QPS", "1");
    env::set_var("RATE_LIMIT_UPLOAD_BURST", "2");
    env::set_var("SKIP_INITIAL_INDEXING", "true");

    // Start server
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

    // Health warm-up with different IP to avoid draining /search bucket
    let _ = client
        .get(format!("http://127.0.0.1:{}/health", port))
        .header("X-Forwarded-For", "9.9.9.9")
        .send()
        .await
        .unwrap();

    // 1) /search (read bucket: burst=3)
    let mut search_codes = Vec::new();
    for _ in 0..6 {
        // 3 ok + 3 drops likely
        let r = client
            .get(format!("http://127.0.0.1:{}/search?q=t", port))
            .header("X-Forwarded-For", "1.2.3.4")
            .send()
            .await
            .unwrap();
        search_codes.push(r.status().as_u16());
    }
    assert_eq!(search_codes[0], 200);
    assert_eq!(search_codes[1], 200);
    assert_eq!(search_codes[2], 429);
    for i in 3..search_codes.len() {
        assert_eq!(
            search_codes[i], 429,
            "search idx {} codes {:?}",
            i, search_codes
        );
    }

    // 2) /rerank (read bucket applies)
    let mut rerank_codes = Vec::new();
    for _ in 0..4 {
        // 3 ok + 1 drop expected under tight loop for the same read bucket IP
        let r = client
            .post(format!("http://127.0.0.1:{}/rerank", port))
            .header("X-Forwarded-For", "1.2.3.4")
            .json(&serde_json::json!({"query":"q","candidates":["a","b"]}))
            .send()
            .await
            .unwrap();
        rerank_codes.push(r.status().as_u16());
    }
    // Since same IP and tight loop, it's possible all are 429 due to exhausted tokens.
    // Ensure at least one 429 occurred (rate limiting engaged)
    assert!(
        rerank_codes.contains(&429),
        "rerank not limited: {:?}",
        rerank_codes
    );

    // 3) /upload (write bucket: burst=2)
    // Build forms with explicit unique filenames to avoid collisions
    let part1 = reqwest::multipart::Part::file(test_file("one"))
        .await
        .unwrap()
        .file_name("test_1.txt");
    let part2 = reqwest::multipart::Part::file(test_file("two"))
        .await
        .unwrap()
        .file_name("test_2.txt");
    let part3 = reqwest::multipart::Part::file(test_file("three"))
        .await
        .unwrap()
        .file_name("test_3.txt");
    let form1 = reqwest::multipart::Form::new().part("file", part1);
    let form2 = reqwest::multipart::Form::new().part("file", part2);
    let form3 = reqwest::multipart::Form::new().part("file", part3);

    let c1 = client
        .post(format!("http://127.0.0.1:{}/upload", port))
        .header("X-Forwarded-For", "1.2.3.4")
        .multipart(form1)
        .send()
        .await
        .unwrap()
        .status()
        .as_u16();
    let c2 = client
        .post(format!("http://127.0.0.1:{}/upload", port))
        .header("X-Forwarded-For", "1.2.3.4")
        .multipart(form2)
        .send()
        .await
        .unwrap()
        .status()
        .as_u16();
    // Small delay before third upload to reduce race potential
    sleep(std::time::Duration::from_millis(30)).await;
    let mut resp3 = client
        .post(format!("http://127.0.0.1:{}/upload", port))
        .header("X-Forwarded-For", "1.2.3.4")
        .multipart(form3)
        .send()
        .await
        .unwrap();
    let mut c3 = resp3.status().as_u16();
    if c3 == 400 {
        // Retry once if multipart parsing had a transient issue
        let part_retry = reqwest::multipart::Part::file(test_file("retry"))
            .await
            .unwrap()
            .file_name("test_retry.txt");
        let form_retry = reqwest::multipart::Form::new().part("file", part_retry);
        resp3 = client
            .post(format!("http://127.0.0.1:{}/upload", port))
            .header("X-Forwarded-For", "1.2.3.4")
            .multipart(form_retry)
            .send()
            .await
            .unwrap();
        c3 = resp3.status().as_u16();
    }
    if c3 == 429 {
        // Validate Retry-After present and >= 1
        let ra = resp3
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        assert!(ra >= 1, "Retry-After should be >= 1, got {}", ra);
    }

    assert_eq!(c1, 200);
    assert_eq!(c2, 200);
    assert_eq!(c3, 429);
}

fn test_file(contents: &str) -> String {
    use std::io::Write;
    let mut f = tempfile::NamedTempFile::new().expect("tmp file");
    write!(f, "{}", contents).unwrap();
    let path = f.into_temp_path();
    // leak the path so it survives until upload completes; temp path deletes on drop otherwise
    let s = path.to_string_lossy().to_string();
    std::mem::forget(path);
    s
}
