use crate::path_manager::PathManager;
use std::env;

#[derive(Debug, Clone, Copy)]
pub enum ChunkerMode {
    Fixed,
    Lightweight,
    Semantic,
}

impl ChunkerMode {
    pub fn from_env() -> Self {
        let raw = env::var("CHUNKER_MODE").unwrap_or_else(|_| "fixed".to_string());
        raw.parse().unwrap_or(ChunkerMode::Fixed)
    }
}

impl std::str::FromStr for ChunkerMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "fixed" => Ok(ChunkerMode::Fixed),
            "lightweight" => Ok(ChunkerMode::Lightweight),
            "semantic" => Ok(ChunkerMode::Semantic),
            other => Err(format!("unknown chunker mode: {}", other)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiConfig {
    // Network
    pub host: String,
    pub port: u16,

    // Phase 15 - Reliability & Observability
    pub skip_initial_indexing: bool,
    pub index_in_ram: bool,
    pub reindex_webhook_url: Option<String>,
    pub rate_limit_enabled: bool,
    pub rate_limit_qps: f64,
    pub rate_limit_burst: u32,
    pub trust_proxy: bool,
    pub rate_limit_search_qps: Option<f64>,
    pub rate_limit_search_burst: Option<u32>,
    pub rate_limit_upload_qps: Option<f64>,
    pub rate_limit_upload_burst: Option<u32>,
    pub rate_limit_lru_capacity: usize,

    // Chunker selection
    pub chunker_mode: ChunkerMode,

    // Path Management
    pub path_manager: PathManager,

    // Redis L3 Cache
    pub redis_enabled: bool,
    pub redis_url: Option<String>,
    pub redis_ttl: u64,

    // Chunking snapshot logging
    pub chunking_log_enabled: bool,
    pub admin_api_token: Option<String>,
}

impl ApiConfig {
    pub fn from_env() -> Self {
        // Allow tests to opt out of dotenv to avoid env contamination
        let no_dotenv = std::env::var("NO_DOTENV")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);
        if !no_dotenv {
            dotenvy::dotenv().ok();
        }

        // Network configuration
        let host = env::var("BACKEND_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

        let port = env::var("BACKEND_PORT")
            .unwrap_or_else(|_| "3010".to_string())
            .parse()
            .expect("BACKEND_PORT must be a valid u16");

        // Phase 15 - Reliability & Observability
        let skip_initial_indexing = env::var("SKIP_INITIAL_INDEXING")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        let index_in_ram = env::var("INDEX_IN_RAM")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        let reindex_webhook_url = env::var("REINDEX_WEBHOOK_URL").ok();

        let rate_limit_enabled = env::var("RATE_LIMIT_ENABLED")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);
        let rate_limit_qps = env::var("RATE_LIMIT_QPS")
            .unwrap_or_else(|_| "1.0".to_string())
            .parse()
            .unwrap_or(1.0);
        let rate_limit_burst = env::var("RATE_LIMIT_BURST")
            .unwrap_or_else(|_| "5".to_string())
            .parse()
            .unwrap_or(5);
        let trust_proxy = env::var("TRUST_PROXY")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);
        let rate_limit_search_qps = env::var("RATE_LIMIT_SEARCH_QPS")
            .ok()
            .and_then(|v| v.parse().ok());
        let rate_limit_search_burst = env::var("RATE_LIMIT_SEARCH_BURST")
            .ok()
            .and_then(|v| v.parse().ok());
        let rate_limit_upload_qps = env::var("RATE_LIMIT_UPLOAD_QPS")
            .ok()
            .and_then(|v| v.parse().ok());
        let rate_limit_upload_burst = env::var("RATE_LIMIT_UPLOAD_BURST")
            .ok()
            .and_then(|v| v.parse().ok());
        let rate_limit_lru_capacity = env::var("RATE_LIMIT_LRU_CAPACITY")
            .unwrap_or_else(|_| "1024".to_string())
            .parse()
            .unwrap_or(1024);

        // Chunker selection
        let chunker_mode = ChunkerMode::from_env();

        // Path Management
        let path_manager = PathManager::new().expect("Failed to initialize PathManager");

        // Redis L3 Cache
        let redis_enabled = env::var("REDIS_ENABLED")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        let redis_url = env::var("REDIS_URL").ok();

        let redis_ttl = env::var("REDIS_TTL")
            .unwrap_or_else(|_| "3600".to_string())
            .parse()
            .unwrap_or(3600);

        let chunking_log_enabled = env::var("CHUNKING_SNAPSHOT_LOGGING")
            .map(|v| v.to_lowercase() != "false" && v != "0")
            .unwrap_or(true);

        let admin_api_token = env::var("ADMIN_API_TOKEN").ok();

        Self {
            host,
            port,
            skip_initial_indexing,
            index_in_ram,
            reindex_webhook_url,
            rate_limit_enabled,
            rate_limit_qps,
            rate_limit_burst,
            trust_proxy,
            rate_limit_search_qps,
            rate_limit_search_burst,
            rate_limit_upload_qps,
            rate_limit_upload_burst,
            rate_limit_lru_capacity,
            chunker_mode,
            path_manager,
            redis_enabled,
            redis_url,
            redis_ttl,
            chunking_log_enabled,
            admin_api_token,
        }
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
