use crate::path_manager::PathManager;
use std::env;

#[derive(Debug, Clone, Copy)]
pub enum ChunkerMode {
    Fixed,
    Lightweight,
    Semantic,
    Sentence,
    Pipeline,
}

impl ChunkerMode {
    pub fn from_env() -> Self {
        let raw = crate::settings::effective_or("CHUNKER_MODE", "fixed");
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
            "sentence" => Ok(ChunkerMode::Sentence),
            "pipeline" => Ok(ChunkerMode::Pipeline),
            other => Err(format!("unknown chunker mode: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum DistanceMetric {
    #[default]
    Cosine,
    DotProduct,
    Euclidean,
}

impl std::fmt::Display for DistanceMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistanceMetric::Cosine => write!(f, "cosine"),
            DistanceMetric::DotProduct => write!(f, "dotproduct"),
            DistanceMetric::Euclidean => write!(f, "euclidean"),
        }
    }
}

impl std::str::FromStr for DistanceMetric {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cosine" => Ok(DistanceMetric::Cosine),
            "dotproduct" | "dot" | "dot_product" => Ok(DistanceMetric::DotProduct),
            "euclidean" | "l2" => Ok(DistanceMetric::Euclidean),
            other => Err(format!("unknown distance metric: {}", other)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiConfig {
    // Network
    pub host: String,
    pub port: u16,
    pub upload_host: String,
    pub upload_port: u16,
    pub search_workers: usize,
    pub upload_workers: usize,
    pub search_max_connections: usize,
    pub upload_max_connections: usize,
    pub upload_max_concurrent: usize,
    pub search_max_body_kb: usize,
    pub search_timeout_secs: u64,
    pub upload_timeout_secs: u64,

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
    pub trust_proxy_search: bool,
    pub trust_proxy_upload: bool,
    pub upload_rate_limit_lru_capacity: usize,

    // Chunker selection
    pub chunker_mode: ChunkerMode,

    // Path Management
    pub path_manager: PathManager,

    // Redis L3 Cache
    pub redis_enabled: bool,
    pub redis_url: Option<String>,
    pub redis_ttl: u64,

    // Retrieval tuning
    pub search_top_k: usize,

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

        let port: u16 = crate::settings::effective_u64("BACKEND_PORT", 3010)
            .try_into()
            .expect("BACKEND_PORT must fit in u16");

        let upload_host = env::var("UPLOAD_HOST").unwrap_or_else(|_| host.clone());

        let upload_port = env::var("UPLOAD_PORT")
            .unwrap_or_else(|_| "3011".to_string())
            .parse()
            .expect("UPLOAD_PORT must be a valid u16");

        let default_search_workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .saturating_sub(2)
            .max(1);
        let search_workers = env::var("SEARCH_WORKERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default_search_workers);

        let upload_workers = env::var("UPLOAD_WORKERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        let search_max_connections = env::var("SEARCH_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);

        let upload_max_connections = env::var("UPLOAD_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);

        let upload_max_concurrent = env::var("UPLOAD_MAX_CONCURRENT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4);

        let search_max_body_kb = env::var("SEARCH_MAX_BODY_KB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(64);

        let search_timeout_secs = env::var("SEARCH_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let upload_timeout_secs = env::var("UPLOAD_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);

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
        let trust_proxy = crate::settings::effective_bool("TRUST_PROXY", false);
        let trust_proxy_search = env::var("TRUST_PROXY_SEARCH")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(trust_proxy);
        let trust_proxy_upload = env::var("TRUST_PROXY_UPLOAD")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(trust_proxy);
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
        let upload_rate_limit_lru_capacity = env::var("UPLOAD_RATE_LIMIT_LRU_CAPACITY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(256usize);

        // Chunker selection
        let chunker_mode = ChunkerMode::from_env();

        // Path Management
        let path_manager = PathManager::new().expect("Failed to initialize PathManager");

        // Redis L3 Cache — read through the settings layer so a runtime
        // override in <base_dir>/overrides.json takes precedence over the
        // env file. Falls back to env::var when the global isn't installed
        // yet (e.g. unit tests that bypass main).
        let redis_enabled = crate::settings::effective_bool("REDIS_ENABLED", false);

        let redis_url = crate::settings::global()
            .and_then(|s| s.effective("REDIS_URL"))
            .or_else(|| env::var("REDIS_URL").ok());

        let redis_ttl = crate::settings::effective_u64("REDIS_TTL", 3600);

        let search_top_k = (crate::settings::effective_u64("SEARCH_TOP_K", 10) as usize).max(1);

        // Permissive parse — preserves original "anything but false/0 is on"
        // behavior; only goes off for explicit "false"/"0".
        let chunking_log_enabled = crate::settings::global()
            .and_then(|s| s.effective("CHUNKING_SNAPSHOT_LOGGING"))
            .map(|v| v.to_lowercase() != "false" && v != "0")
            .unwrap_or(true);

        let admin_api_token = env::var("ADMIN_API_TOKEN").ok();

        Self {
            host,
            port,
            upload_host,
            upload_port,
            search_workers,
            upload_workers,
            search_max_connections,
            upload_max_connections,
            upload_max_concurrent,
            search_max_body_kb,
            search_timeout_secs,
            upload_timeout_secs,
            skip_initial_indexing,
            index_in_ram,
            reindex_webhook_url,
            rate_limit_enabled,
            trust_proxy_search,
            trust_proxy_upload,
            upload_rate_limit_lru_capacity,
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
            search_top_k,
            chunking_log_enabled,
            admin_api_token,
        }
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn upload_bind_addr(&self) -> String {
        format!("{}:{}", self.upload_host, self.upload_port)
    }
}
