use serde::{Deserialize, Serialize};

/// Default base URL used when running outside of a browser (e.g., dx serve)
pub const API_BASE_URL: &str = "http://127.0.0.1:3010";

pub fn resolve_api_base_url() -> String {
    if let Some(window) = web_sys::window() {
        let location = window.location();
        if let Ok(origin) = location.origin() {
            let is_loopback = origin.contains("127.0.0.1") || origin.contains("localhost");
            if !is_loopback {
                return origin;
            }

            let hostname = location
                .hostname()
                .unwrap_or_else(|_| "127.0.0.1".into())
                .trim()
                .to_string();
            let scheme = location
                .protocol()
                .unwrap_or_else(|_| "http:".into())
                .trim_end_matches(':')
                .to_string();

            if hostname.is_empty() {
                return API_BASE_URL.to_string();
            }

            return format!("{}://{}:3010", scheme, hostname);
        }
    }

    API_BASE_URL.to_string()
}

pub fn api_url(path: &str) -> String {
    format!("{}{}", resolve_api_base_url(), path)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthResponse {
    pub status: String,
    pub documents: Option<usize>,
    pub vectors: Option<usize>,
    pub index_path: Option<String>,
    pub message: Option<String>,
    pub load: Option<LoadMetrics>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoadMetrics {
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub active_tasks: u32,
    pub queue_depth: u32,
    pub indexing: bool,
    pub llm_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResult {
    pub content: String,
    pub score: f32,
    pub document: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResponse {
    pub status: String,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocumentsResponse {
    pub status: String,
    pub documents: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestChartPoint {
    pub ts: i64,
    pub latency_ms: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct LatencyBreakdown {
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct StatusBreakdown {
    pub success_rate: f64,
    pub client_error_rate: f64,
    pub server_error_rate: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestsSnapshot {
    pub request_rate_rps: f64,
    pub latency_p95_ms: f64,
    pub error_rate_percent: f64,
    #[serde(default)]
    pub latency_breakdown: LatencyBreakdown,
    #[serde(default)]
    pub status_breakdown: StatusBreakdown,
    pub points: Vec<RequestChartPoint>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RerankRequest {
    pub query: String,
    pub candidates: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SummarizeRequest {
    pub query: String,
    pub candidates: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexInfoResponse {
    pub index_in_ram: bool,
    pub mode: String,
    pub warning: Option<String>,
    pub total_documents: usize,
    pub total_vectors: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkingLoggingResponse {
    pub status: String,
    pub request_id: String,
    pub logging_enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReindexAsyncResponse {
    pub status: String,
    pub job_id: String,
    pub request_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReindexStatusResponse {
    pub status: String,
    pub job_id: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub vectors_indexed: Option<usize>,
    pub mappings_indexed: Option<usize>,
    pub error: Option<String>,
    pub request_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkerConfigSnapshot {
    pub target_size: usize,
    pub min_size: usize,
    pub max_size: usize,
    pub overlap: usize,
    pub semantic_similarity_threshold: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkCommitResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub chunker_config: ChunkerConfigSnapshot,
    pub reindex_status: String,
    pub reindex_job_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkCommitRequest {
    pub target_size: usize,
    pub min_size: usize,
    pub max_size: usize,
    pub overlap: usize,
    pub semantic_similarity_threshold: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct LlmConfig {
    // Basic sampling
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: usize,
    pub max_tokens: usize,
    pub repeat_penalty: f32,
    pub frequency_penalty: f32,
    pub presence_penalty: f32,
    pub stop_sequences: Vec<String>,
    pub seed: Option<i64>,
    pub min_p: f32,
    pub typical_p: f32,
    pub tfs_z: f32,

    // Mirostat (adaptive sampling)
    pub mirostat: i32,
    pub mirostat_eta: f32,
    pub mirostat_tau: f32,

    // Repetition control
    pub repeat_last_n: usize,
    pub penalize_newline: bool,

    // Generation limits
    pub num_predict: i64,
    pub num_keep: i64,
    pub ignore_eos: bool,

    // DRY (Don't Repeat Yourself) sampling
    pub dry_multiplier: f32,
    pub dry_base: f32,
    pub dry_allowed_length: usize,

    // XTC (eXtreme Token Control) sampling
    pub xtc_probability: f32,
    pub xtc_threshold: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            // Basic sampling
            temperature: 0.7,
            top_p: 0.95,
            top_k: 40,
            max_tokens: 1024,
            repeat_penalty: 1.1,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            stop_sequences: Vec::new(),
            seed: None,
            min_p: 0.0,
            typical_p: 1.0,
            tfs_z: 1.0,

            // Mirostat
            mirostat: 0,
            mirostat_eta: 0.1,
            mirostat_tau: 5.0,

            // Repetition control
            repeat_last_n: 64,
            penalize_newline: true,

            // Generation limits
            num_predict: 1024, // Match max_tokens default, -1 means unlimited
            num_keep: 0,
            ignore_eos: false,

            // DRY sampling (disabled by default)
            dry_multiplier: 0.0,
            dry_base: 1.75,
            dry_allowed_length: 2,

            // XTC sampling (disabled by default)
            xtc_probability: 0.0,
            xtc_threshold: 0.1,
        }
    }
}

/// Supported LLM inference backends
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
    #[default]
    Ollama,
    LlamaCpp,
    #[serde(rename = "openai")]
    OpenAi,
    Anthropic,
    Vllm,
    Custom,
}

impl BackendType {
    /// Returns true if this backend supports local hardware configuration
    pub fn supports_hardware_config(&self) -> bool {
        matches!(self, Self::LlamaCpp | Self::Vllm)
    }

    /// Returns true if this backend supports thread configuration
    pub fn supports_thread_config(&self) -> bool {
        matches!(self, Self::Ollama | Self::LlamaCpp)
    }

    /// Returns true if this backend supports GPU configuration (num_gpu)
    pub fn supports_gpu_config(&self) -> bool {
        matches!(self, Self::Ollama | Self::LlamaCpp | Self::Vllm)
    }

    /// Returns true if this backend supports GPU layer offloading (n_gpu_layers)
    pub fn supports_gpu_layers(&self) -> bool {
        matches!(self, Self::LlamaCpp | Self::Vllm)
    }

    /// Returns true if this backend supports RoPE configuration
    pub fn supports_rope_config(&self) -> bool {
        matches!(self, Self::LlamaCpp)
    }

    /// Returns true if this backend supports low_vram and f16_kv options
    pub fn supports_memory_options(&self) -> bool {
        matches!(self, Self::LlamaCpp)
    }

    /// Returns true if this is a cloud/API-based backend
    pub fn is_cloud_backend(&self) -> bool {
        matches!(self, Self::OpenAi | Self::Anthropic)
    }

    /// Human-readable label for the backend
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ollama => "Ollama 0.12.6",
            Self::LlamaCpp => "llama.cpp",
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Vllm => "vLLM",
            Self::Custom => "Custom",
        }
    }

    /// All available backend types
    pub fn all() -> Vec<BackendType> {
        vec![
            Self::Ollama,
            Self::LlamaCpp,
            Self::OpenAi,
            Self::Anthropic,
            Self::Vllm,
            Self::Custom,
        ]
    }

    /// Convert to API string representation
    pub fn to_api_string(&self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::LlamaCpp => "llama_cpp",
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Vllm => "vllm",
            Self::Custom => "custom",
        }
    }

    /// Parse from API string representation
    pub fn from_api_string(s: &str) -> Self {
        match s {
            "ollama" => Self::Ollama,
            "llama_cpp" => Self::LlamaCpp,
            "openai" => Self::OpenAi,
            "anthropic" => Self::Anthropic,
            "vllm" => Self::Vllm,
            "custom" => Self::Custom,
            _ => Self::Ollama,
        }
    }
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Override a single model metadata entry (for llama.cpp kv_overrides)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct KvOverride {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct HardwareConfig {
    pub backend_type: String,
    pub model: String,

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 1: MODEL PARAMS (requires restart)
    // ═══════════════════════════════════════════════════════════════
    pub gpu_layers: usize,
    pub main_gpu: usize,
    pub split_mode: String,
    pub tensor_split: Vec<f32>,
    pub use_mmap: bool,
    pub use_mlock: bool,
    pub vocab_only: bool,
    pub devices: Vec<String>,
    pub kv_overrides: Vec<KvOverride>,
    pub swa_full: bool,
    pub no_perf: bool,

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 2: CONTEXT PARAMS (requires new context)
    // ═══════════════════════════════════════════════════════════════
    pub num_ctx: usize,
    pub num_batch: usize,
    pub num_ubatch: usize,
    pub num_seq_max: usize,
    pub rope_scaling_type: String,
    pub rope_frequency_base: f32,
    pub rope_frequency_scale: f32,
    pub yarn_ext_factor: f32,
    pub yarn_attn_factor: f32,
    pub yarn_beta_fast: f32,
    pub yarn_beta_slow: f32,
    pub yarn_orig_ctx: usize,
    pub pooling_type: String,
    pub attention_type: String,
    pub flash_attn: bool,
    pub type_k: String,
    pub type_v: String,
    pub embeddings: bool,
    pub offload_kqv: bool,
    pub defrag_thold: f32,
    pub logits_all: bool,
    pub f16_kv: bool,
    pub low_vram: bool,

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 3: CPU PARAMS (requires new context)
    // ═══════════════════════════════════════════════════════════════
    pub num_thread: usize,
    pub num_thread_batch: usize,
    pub numa: bool,
    pub cpu_strict: bool,
    pub cpumask: Vec<bool>,
    pub mask_valid: bool,
    pub poll: usize,
    pub priority: String,

    // Legacy/custom
    pub num_gpu: usize,
}

impl Default for HardwareConfig {
    fn default() -> Self {
        Self {
            backend_type: "ollama".to_string(),
            model: String::new(),

            // Model params
            gpu_layers: 0,
            main_gpu: 0,
            split_mode: "layer".to_string(),
            tensor_split: Vec::new(),
            use_mmap: true,
            use_mlock: false,
            vocab_only: false,
            devices: Vec::new(),
            kv_overrides: Vec::new(),
            swa_full: false,
            no_perf: false,

            // Context params
            num_ctx: 2048,
            num_batch: 512,
            num_ubatch: 512,
            num_seq_max: 1,
            rope_scaling_type: "unspecified".to_string(),
            rope_frequency_base: 10_000.0,
            rope_frequency_scale: 1.0,
            yarn_ext_factor: -1.0,
            yarn_attn_factor: 1.0,
            yarn_beta_fast: 32.0,
            yarn_beta_slow: 1.0,
            yarn_orig_ctx: 0,
            pooling_type: "unspecified".to_string(),
            attention_type: "unspecified".to_string(),
            flash_attn: false,
            type_k: "f16".to_string(),
            type_v: "f16".to_string(),
            embeddings: false,
            offload_kqv: true,
            defrag_thold: 0.1,
            logits_all: false,
            f16_kv: true,
            low_vram: false,

            // CPU params
            num_thread: 1,
            num_thread_batch: 1,
            numa: false,
            cpu_strict: false,
            cpumask: Vec::new(),
            mask_valid: false,
            poll: 50,
            priority: "normal".to_string(),

            // Legacy
            num_gpu: 0,
        }
    }
}

impl HardwareConfig {
    /// Get the backend type as enum
    pub fn get_backend_type(&self) -> BackendType {
        BackendType::from_api_string(&self.backend_type)
    }

    /// Set the backend type from enum
    pub fn set_backend_type(&mut self, bt: BackendType) {
        self.backend_type = bt.to_api_string().to_string();
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LlmConfigResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub config: LlmConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HardwareConfigResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub config: HardwareConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ApiKeysRequest {
    #[serde(default)]
    pub openai_api_key: String,
    #[serde(default)]
    pub anthropic_api_key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ApiKeysResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub has_openai_key: bool,
    pub has_anthropic_key: bool,
    pub openai_key_masked: String,
    pub anthropic_key_masked: String,
    pub openai_from_env: bool,
    pub anthropic_from_env: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheLayerStats {
    pub enabled: bool,
    pub total_searches: u64,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheL2Stats {
    pub enabled: bool,
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub total_items: u64,
    pub hit_rate: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheCountersSnapshot {
    pub hits_total: i64,
    pub misses_total: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RedisSummary {
    pub enabled: bool,
    pub connected: bool,
    pub ttl_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheInfoResponse {
    pub request_id: String,
    pub l1: CacheLayerStats,
    pub l2: CacheL2Stats,
    pub redis: RedisSummary,
    pub counters: CacheCountersSnapshot,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RouteDropStat {
    pub route: String,
    pub drops: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimitConfigSnapshot {
    pub enabled: bool,
    pub trust_proxy: bool,
    pub search_qps: f64,
    pub search_burst: f64,
    pub upload_qps: f64,
    pub upload_burst: f64,
    pub exempt_prefixes: Vec<String>,
    pub rules: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimiterState {
    pub enabled: bool,
    pub active_keys: usize,
    pub capacity: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimitInfoResponse {
    pub request_id: String,
    pub total_drops: i64,
    pub drops_by_route: Vec<RouteDropStat>,
    pub config: RateLimitConfigSnapshot,
    pub limiter_state: RateLimiterState,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogEntry {
    pub timestamp: Option<String>,
    pub level: Option<String>,
    pub target: Option<String>,
    pub message: Option<String>,
    pub raw: String,
    pub fields: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogsResponse {
    pub request_id: String,
    pub file: Option<String>,
    pub entries: Vec<LogEntry>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct GpuInfo {
    pub index: usize,
    pub name: String,
    pub vendor: String,
    pub backend: String,
    pub device_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SystemInfo {
    pub os: String,
    pub os_family: String,
    pub arch: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
}

/// Model information returned by the models endpoint
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ModelInfo {
    pub name: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub modified_at: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_custom: bool,
    #[serde(default)]
    pub is_active: bool,
}

impl ModelInfo {
    /// Format size in human-readable format (e.g., "3.8 GB")
    pub fn size_display(&self) -> String {
        match self.size {
            Some(bytes) => {
                let gb = bytes as f64 / 1_073_741_824.0;
                if gb >= 1.0 {
                    format!("{:.1} GB", gb)
                } else {
                    let mb = bytes as f64 / 1_048_576.0;
                    format!("{:.1} MB", mb)
                }
            }
            None => String::new(),
        }
    }
}

pub async fn fetch_physical_cores() -> Result<usize, String> {
    fetch_json::<usize>("/sys/cores").await
}

/// Fetch detailed GPU information
pub async fn fetch_gpus() -> Result<Vec<GpuInfo>, String> {
    fetch_json::<Vec<GpuInfo>>("/sys/gpus").await
}

/// Fetch simple GPU names list (backward compatible)
pub async fn fetch_gpu_names() -> Result<Vec<String>, String> {
    fetch_json::<Vec<String>>("/sys/gpu-names").await
}

/// Fetch system information including OS, architecture, and CPU cores
pub async fn fetch_system_info() -> Result<SystemInfo, String> {
    fetch_json::<SystemInfo>("/sys/info").await
}

/// Fetch available models for a given backend type
pub async fn fetch_models(backend: &str) -> Result<Vec<ModelInfo>, String> {
    let url = format!("/sys/models?backend={}", backend);
    fetch_json::<Vec<ModelInfo>>(&url).await
}

/// Fetch custom models discovered on this host
pub async fn fetch_custom_models() -> Result<Vec<ModelInfo>, String> {
    fetch_json::<Vec<ModelInfo>>("/sys/models/custom").await
}

// ============================================================================
// PROMPT CACHING API
// ============================================================================

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PromptCachingResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub enabled: bool,
}

/// Get current prompt caching state
pub async fn get_prompt_caching() -> Result<PromptCachingResponse, String> {
    fetch_json::<PromptCachingResponse>("/config/prompt_caching").await
}

/// Set prompt caching state
pub async fn set_prompt_caching(enabled: bool) -> Result<PromptCachingResponse, String> {
    let url = api_url("/config/prompt_caching");
    let body = serde_json::json!({ "enabled": enabled });

    gloo_net::http::Request::post(&url)
        .json(&body)
        .map_err(|e| format!("Failed to create request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

// ============================================================================
// TRAINING DATA COLLECTION API (Phase 20)
// ============================================================================

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TrainingFeedbackRequest {
    pub query: String,
    pub response: String,
    pub context: Option<String>,
    pub quality_score: u8,
    pub conversation_id: Option<String>,
    pub mode: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TrainingFeedbackResponse {
    pub status: String,
    pub example_id: String,
    pub message: String,
    pub request_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TrainingStats {
    pub total_examples: usize,
    pub high_quality_count: usize,
    pub usable_count: usize,
    pub average_quality: f32,
    pub ready_for_export: bool,
    #[serde(default)]
    pub by_mode: std::collections::HashMap<String, usize>,
    pub last_collected: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TrainingStatsResponse {
    pub status: String,
    pub request_id: String,
    pub stats: TrainingStats,
    pub collection_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TrainingExportResponse {
    pub status: String,
    pub request_id: String,
    pub exported_count: usize,
    pub output_path: String,
    pub message: String,
}

/// Submit feedback for training data collection
pub async fn submit_training_feedback(
    feedback: TrainingFeedbackRequest,
) -> Result<TrainingFeedbackResponse, String> {
    let url = api_url("/training/feedback");

    gloo_net::http::Request::post(&url)
        .json(&feedback)
        .map_err(|e| format!("Failed to create request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Get training data collection statistics
pub async fn get_training_stats() -> Result<TrainingStatsResponse, String> {
    fetch_json::<TrainingStatsResponse>("/training/stats").await
}

/// Export training data for Unsloth
pub async fn export_training_data() -> Result<TrainingExportResponse, String> {
    let url = api_url("/training/export");

    gloo_net::http::Request::post(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Clear all training data
pub async fn clear_training_data() -> Result<serde_json::Value, String> {
    let url = api_url("/training/clear");

    gloo_net::http::Request::post(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Check backend health
pub async fn health_check() -> Result<HealthResponse, String> {
    let url = api_url("/monitoring/health");

    gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Search documents
pub async fn search(query: &str) -> Result<SearchResponse, String> {
    let url = format!(
        "{}/search?q={}",
        resolve_api_base_url(),
        urlencoding::encode(query)
    );

    gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// List all documents
pub async fn list_documents() -> Result<DocumentsResponse, String> {
    let url = api_url("/documents");

    gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Delete a document
pub async fn delete_document(filename: &str) -> Result<serde_json::Value, String> {
    let url = format!("{}/documents/{}", resolve_api_base_url(), filename);

    gloo_net::http::Request::delete(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Trigger reindexing
pub async fn reindex() -> Result<serde_json::Value, String> {
    let url = api_url("/reindex");

    gloo_net::http::Request::post(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

pub async fn reindex_async() -> Result<ReindexAsyncResponse, String> {
    let url = api_url("/reindex/async");

    gloo_net::http::Request::post(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkConfigResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub chunker_config: ChunkerConfigSnapshot,
}

pub async fn fetch_chunk_config() -> Result<ChunkConfigResponse, String> {
    fetch_json::<ChunkConfigResponse>("/config/chunk_size").await
}

pub async fn commit_chunk_config(
    payload: &ChunkCommitRequest,
) -> Result<ChunkCommitResponse, String> {
    let url = api_url("/config/chunk_size");
    gloo_net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(
            serde_json::to_string(payload)
                .map_err(|e| format!("Failed to serialize payload: {}", e))?,
        )
        .map_err(|e| format!("Failed to build request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

pub async fn fetch_llm_config() -> Result<LlmConfigResponse, String> {
    fetch_json::<LlmConfigResponse>("/config/llm").await
}

pub async fn commit_llm_config(payload: &LlmConfig) -> Result<LlmConfigResponse, String> {
    let url = api_url("/config/llm");
    gloo_net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(
            serde_json::to_string(payload)
                .map_err(|e| format!("Failed to serialize payload: {}", e))?,
        )
        .map_err(|e| format!("Failed to build request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

pub async fn fetch_hardware_config() -> Result<HardwareConfigResponse, String> {
    fetch_json::<HardwareConfigResponse>("/config/hardware").await
}

pub async fn fetch_hardware_config_with_origin(
    origin: &str,
) -> Result<HardwareConfigResponse, String> {
    let url = format!("{}/config/hardware", origin.trim_end_matches('/'));
    let response = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    if !(200..=299).contains(&status) {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, body));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

pub async fn commit_hardware_config(
    payload: &HardwareConfig,
) -> Result<HardwareConfigResponse, String> {
    let url = api_url("/config/hardware");
    gloo_net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(
            serde_json::to_string(payload)
                .map_err(|e| format!("Failed to serialize payload: {}", e))?,
        )
        .map_err(|e| format!("Failed to build request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

pub async fn fetch_api_keys() -> Result<ApiKeysResponse, String> {
    fetch_json::<ApiKeysResponse>("/config/api_keys").await
}

pub async fn save_api_keys(payload: &ApiKeysRequest) -> Result<ApiKeysResponse, String> {
    let url = api_url("/config/api_keys");
    gloo_net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(
            serde_json::to_string(payload)
                .map_err(|e| format!("Failed to serialize payload: {}", e))?,
        )
        .map_err(|e| format!("Failed to build request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

pub async fn delete_api_key(provider: &str) -> Result<serde_json::Value, String> {
    let url = format!("{}/config/api_keys/{}", resolve_api_base_url(), provider);
    gloo_net::http::Request::delete(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

pub async fn fetch_reindex_status(job_id: &str) -> Result<ReindexStatusResponse, String> {
    let url = format!("{}/reindex/status/{}", resolve_api_base_url(), job_id);

    gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Fetch request metrics snapshot for the Monitor UI
pub async fn fetch_requests_snapshot() -> Result<RequestsSnapshot, String> {
    let url = api_url("/monitoring/ui/requests");

    gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Fetch index info for the monitor page
pub async fn fetch_index_info() -> Result<IndexInfoResponse, String> {
    fetch_json::<IndexInfoResponse>("/index/info").await
}

pub async fn get_chunking_logging() -> Result<ChunkingLoggingResponse, String> {
    fetch_json::<ChunkingLoggingResponse>("/monitoring/chunking/logging").await
}

pub async fn set_chunking_logging(enabled: bool) -> Result<ChunkingLoggingResponse, String> {
    let url = format!("/monitoring/chunking/logging?enabled={}", enabled);
    fetch_json::<ChunkingLoggingResponse>(&url).await
}

pub async fn fetch_cache_info() -> Result<CacheInfoResponse, String> {
    let url = api_url("/monitor/cache/info");
    let response = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if response.status() == 204 {
        return Err("Backend returned 204 No Content for cache info".into());
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

pub async fn fetch_rate_limit_info() -> Result<RateLimitInfoResponse, String> {
    let url = api_url("/monitor/rate_limits/info");
    gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetRateLimitEnabledResponse {
    pub request_id: String,
    pub enabled: bool,
    pub message: String,
}

pub async fn set_rate_limit_enabled(enabled: bool) -> Result<SetRateLimitEnabledResponse, String> {
    let url = api_url("/monitor/rate_limits/enabled");
    gloo_net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(serde_json::json!({ "enabled": enabled }).to_string())
        .map_err(|e| format!("Failed to create request: {:?}", e))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

pub async fn fetch_recent_logs(limit: usize) -> Result<LogsResponse, String> {
    let url = format!(
        "{}/monitor/logs/recent?limit={}",
        resolve_api_base_url(),
        limit
    );
    gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexedFile {
    pub file: String,
    pub chunks_indexed: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexError {
    pub file: Option<String>,
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UploadResponse {
    pub status: String,
    #[serde(default)]
    pub uploaded_files: Vec<String>,
    #[serde(default)]
    pub indexed_files: Vec<IndexedFile>,
    #[serde(default)]
    pub index_errors: Vec<IndexError>,
    #[serde(default)]
    pub request_id: Option<String>,
}

pub async fn upload_document(filename: &str, data: &[u8]) -> Result<UploadResponse, String> {
    use gloo_net::http::Request;
    use js_sys::{Array, Uint8Array};
    use web_sys::{Blob, BlobPropertyBag, FormData};

    let url = api_url("/upload");

    // Create a Uint8Array from the data
    let uint8_array = Uint8Array::new_with_length(data.len() as u32);
    uint8_array.copy_from(data);

    // Create blob from the array
    let array = Array::new();
    array.push(&uint8_array);
    let blob_options = BlobPropertyBag::new();
    blob_options.set_type("application/octet-stream");
    let blob = Blob::new_with_u8_array_sequence_and_options(&array, &blob_options)
        .map_err(|_| "Failed to create blob".to_string())?;

    // Create FormData and append the blob with filename
    let form_data = FormData::new().map_err(|_| "Failed to create FormData".to_string())?;
    form_data
        .append_with_blob_and_filename("file", &blob, filename)
        .map_err(|_| "Failed to append file to FormData".to_string())?;

    // Send the request
    let response = Request::post(&url)
        .body(form_data)
        .map_err(|e| format!("Failed to create request: {:?}", e))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    if !(200..=299).contains(&status) {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, body));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

async fn fetch_json<T>(path: &str) -> Result<T, String>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let url = api_url(path);
    let response = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    if !(200..=299).contains(&status) {
        let body = match response.text().await {
            Ok(body) => body.trim().to_string(),
            Err(_) => String::new(),
        };
        let detail = if body.is_empty() {
            "(empty response)".to_string()
        } else {
            body
        };
        return Err(format!("HTTP {} {}", status, detail));
    }

    response
        .json::<T>()
        .await
        .map_err(|e| format!("Failed to parse JSON: {}", e))
}

// ============================================================================
// AGENTIC MONITORING API
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AgentStatsResponse {
    pub active_agents: usize,
    pub episodes_total: usize,
    pub episodes_last_hour: usize,
    pub success_rate: f64,
    pub active_goals: usize,
    pub completed_goals: usize,
    pub failed_goals: usize,
    pub total_reflections: usize,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpisodeEntry {
    pub id: String,
    pub agent_id: String,
    pub query: String,
    pub response: String,
    pub context_chunks_used: usize,
    pub success: bool,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EpisodesResponse {
    pub episodes: Vec<EpisodeEntry>,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GoalEntry {
    pub id: String,
    pub agent_id: String,
    pub goal: String,
    pub status: String,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GoalsResponse {
    pub goals: Vec<GoalEntry>,
    pub active: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReflectionEntry {
    pub id: String,
    pub agent_id: String,
    pub reflection_type: String,
    pub insight: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ReflectionsResponse {
    pub reflections: Vec<ReflectionEntry>,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MemoryStatsResponse {
    pub total_episodes: usize,
    pub total_rag_memories: usize,
    pub unique_agents: usize,
    pub oldest_episode_timestamp: Option<i64>,
    pub newest_episode_timestamp: Option<i64>,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AvailableTool {
    pub name: String,
    pub description: String,
    pub status: String,
    pub icon: String,
    pub category: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AvailableToolsResponse {
    pub request_id: String,
    pub tools: Vec<AvailableTool>,
    pub total: usize,
    pub active: usize,
    pub placeholder: usize,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolUsageEntry {
    pub tool_name: String,
    pub count: usize,
    pub percentage: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolStatsResponse {
    pub tool_executions: usize,
    pub avg_confidence: f64,
    pub fallback_rate: f64,
    pub tool_distribution: Vec<ToolUsageEntry>,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolCacheStats {
    pub tool_type: String,
    pub enabled: bool,
    pub ttl_secs: u64,
    pub max_entries: usize,
    pub current_entries: usize,
    pub hits: usize,
    pub misses: usize,
    pub hit_rate: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolCacheResponse {
    pub request_id: String,
    pub caches: Vec<ToolCacheStats>,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolRateLimitStatus {
    pub tool_type: String,
    pub enabled: bool,
    pub max_requests: usize,
    pub window_secs: u64,
    pub tokens_available: f64,
    pub tokens_max: f64,
    pub utilization: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolRateLimitResponse {
    pub request_id: String,
    pub statuses: Vec<ToolRateLimitStatus>,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolCostEntry {
    pub tool_type: String,
    pub total_cost: f32,
    pub executions: usize,
    pub avg_cost: f32,
    pub last_updated: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolCostResponse {
    pub request_id: String,
    pub timestamp: String,
    pub costs: Vec<ToolCostEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolTrendBucket {
    pub timestamp: String,
    pub executions: usize,
    pub successes: usize,
    pub failures: usize,
    pub avg_latency_ms: f64,
    pub avg_confidence: f32,
    pub total_cost: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolTrendSummary {
    pub total_executions: usize,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub avg_confidence: f32,
    pub total_cost: f64,
    pub trend_direction: String,
    pub latency_trend: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolTrend {
    pub tool_type: String,
    pub window: String,
    pub buckets: Vec<ToolTrendBucket>,
    pub summary: ToolTrendSummary,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolTrendsResponse {
    pub request_id: String,
    pub window: String,
    pub trends: Vec<ToolTrend>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolDependencyNode {
    pub tool_type: String,
    pub executions: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolDependencyEdge {
    pub from: String,
    pub to: String,
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolDependencyGraph {
    pub nodes: Vec<ToolDependencyNode>,
    pub edges: Vec<ToolDependencyEdge>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolDependencyResponse {
    pub request_id: String,
    pub graph: ToolDependencyGraph,
}

/// Fetch agent statistics
pub async fn fetch_agent_stats() -> Result<AgentStatsResponse, String> {
    fetch_json("/monitoring/agents/stats").await
}

/// Fetch recent episodes
pub async fn fetch_recent_episodes(limit: usize) -> Result<EpisodesResponse, String> {
    fetch_json(&format!("/monitoring/agents/episodes?limit={}", limit)).await
}

/// Fetch goals
pub async fn fetch_goals() -> Result<GoalsResponse, String> {
    fetch_json("/monitoring/agents/goals").await
}

/// Fetch reflections
pub async fn fetch_reflections(limit: usize) -> Result<ReflectionsResponse, String> {
    fetch_json(&format!("/monitoring/agents/reflections?limit={}", limit)).await
}

/// Fetch memory statistics
pub async fn fetch_memory_stats() -> Result<MemoryStatsResponse, String> {
    fetch_json("/monitoring/memory/stats").await
}

/// Fetch tool statistics
pub async fn fetch_tool_stats() -> Result<ToolStatsResponse, String> {
    fetch_json("/monitoring/tools/stats").await
}

/// Fetch available tools list
pub async fn fetch_available_tools() -> Result<AvailableToolsResponse, String> {
    fetch_json("/monitoring/tools/available").await
}

pub async fn fetch_tool_cache_stats() -> Result<ToolCacheResponse, String> {
    fetch_json("/monitoring/tools/cache").await
}

pub async fn fetch_tool_rate_limits() -> Result<ToolRateLimitResponse, String> {
    fetch_json("/monitoring/tools/rate-limits").await
}

pub async fn fetch_tool_costs() -> Result<ToolCostResponse, String> {
    fetch_json("/monitoring/tools/costs").await
}

pub async fn fetch_tool_dependencies() -> Result<ToolDependencyResponse, String> {
    fetch_json("/monitoring/tools/dependencies").await
}

pub async fn fetch_tool_trends(window: &str) -> Result<ToolTrendsResponse, String> {
    fetch_json(&format!("/monitoring/tools/trends?window={}", window)).await
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ManualObservationMetric {
    pub endpoint: String,
    pub ok: u64,
    pub err: u64,
    pub latency_p50: f64,
    pub latency_p90: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ManualObservationMetricsResponse {
    pub metrics: Vec<ManualObservationMetric>,
    pub request_id: String,
}

pub async fn fetch_manual_observation_metrics() -> Result<ManualObservationMetricsResponse, String>
{
    fetch_json("/monitoring/observations/metrics").await
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ManualObservationSummary {
    pub id: String,
    pub entry_type: String,
    pub title: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RecentObservationsResponse {
    pub observations: Vec<ManualObservationSummary>,
    pub request_id: String,
}

pub async fn fetch_recent_observations(limit: usize) -> Result<RecentObservationsResponse, String> {
    fetch_json(&format!("/monitoring/observations/recent?limit={}", limit)).await
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct RagMemoryItem {
    pub id: i64,
    pub agent_id: String,
    pub memory_type: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RagMemoriesResponse {
    pub memories: Vec<RagMemoryItem>,
    pub request_id: String,
}

pub async fn fetch_rag_memories(limit: usize) -> Result<RagMemoriesResponse, String> {
    fetch_json(&format!("/monitoring/memories/rag?limit={}", limit)).await
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MemoryTypesResponse {
    pub core: Vec<String>,
    pub extended: Vec<String>,
    pub all: Vec<String>,
    pub request_id: String,
}

pub async fn fetch_memory_types() -> Result<MemoryTypesResponse, String> {
    fetch_json("/memory/types").await
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoreRagRequest {
    pub agent_id: String,
    pub memory_type: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoreRagResponse {
    pub status: String,
    pub request_id: String,
}

pub async fn store_rag_memory(req: &StoreRagRequest) -> Result<StoreRagResponse, String> {
    let url = api_url("/memory/store_rag");
    let response = gloo_net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(req).map_err(|e| format!("Failed to serialize: {}", e))?)
        .map_err(|e| format!("Failed to build request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    if !(200..=299).contains(&status) {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, body));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeleteRagRequest {
    pub agent_id: String,
    pub ids: Vec<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeleteRagResponse {
    pub status: String,
    pub deleted: usize,
    pub request_id: String,
}

pub async fn delete_rag_memories(req: &DeleteRagRequest) -> Result<DeleteRagResponse, String> {
    let url = api_url("/memory/delete_rag");
    let response = gloo_net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(req).map_err(|e| format!("Failed to serialize: {}", e))?)
        .map_err(|e| format!("Failed to build request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    if !(200..=299).contains(&status) {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, body));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

// ═══════════════════════════════════════════════════════════════════════════
// Chunking Stats API - Detection Observability
// ═══════════════════════════════════════════════════════════════════════════

/// Detection info for observability - tracks raw inputs vs derived conclusions
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DetectionInfo {
    /// Raw input: MIME type from magic bytes (if detected)
    pub mime_type: Option<String>,
    /// Raw input: File extension
    pub extension: Option<String>,
    /// Derived conclusion: Detected content type
    pub detected_format: String,
    /// Derived conclusion: Chosen chunking strategy
    pub chosen_strategy: String,
    /// Detection method used (magic_bytes, extension, heuristic)
    pub detection_method: String,
}

/// Chunking stats from semantic chunker
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ChunkingStats {
    pub semantic_similarity_threshold: f32,
    pub semantic_flushes: usize,
    pub heading_flushes: usize,
    pub size_flushes: usize,
    pub total_segments: usize,
    pub similarity_sum: f32,
    pub similarity_count: usize,
}

/// Snapshot of chunking operation with detection info
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkingStatsSnapshot {
    pub recorded_at: String,
    pub file: String,
    pub chunker_mode: String,
    pub chunks: usize,
    pub tokens: usize,
    pub duration_ms: u64,
    pub stats: Option<ChunkingStats>,
    pub detection: Option<DetectionInfo>,
}

/// Response from chunking stats endpoint
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkingStatsResponse {
    pub status: String,
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub snapshots: Vec<ChunkingStatsSnapshot>,
    #[serde(default)]
    pub count: Option<usize>,
    #[serde(default)]
    pub message: Option<String>,
}

/// Fetch chunking stats history for observability
pub async fn fetch_chunking_stats(limit: usize) -> Result<ChunkingStatsResponse, String> {
    fetch_json(&format!("/monitoring/chunking/latest?limit={}", limit)).await
}

// ============ Tool Execution Monitoring ============

/// Record of a single tool execution
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ToolExecution {
    pub tool_type: String,
    pub query: String,
    pub success: bool,
    pub result_preview: String,
    pub execution_time_ms: u64,
    pub confidence: f32,
    pub timestamp: String,
    #[serde(default)]
    pub source: Option<String>,
}

/// Response from tool executions endpoint
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolExecutionResponse {
    pub status: String,
    #[serde(default)]
    pub request_id: String,
    pub executions: Vec<ToolExecution>,
    pub count: usize,
}

/// Fetch recent tool executions
pub async fn fetch_tool_executions(limit: usize) -> Result<ToolExecutionResponse, String> {
    fetch_json(&format!("/monitoring/tools/executions?limit={}", limit)).await
}

// ============ Embedding Configuration (ONNX only) ============

/// ONNX status info
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OnnxStatus {
    #[serde(default)]
    pub model_path: String,
    #[serde(default)]
    pub model_exists: bool,
    #[serde(default)]
    pub ready: bool,
}

/// Response from embedding config endpoint
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmbeddingConfigResponse {
    pub status: String,
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default, alias = "model_path")]
    pub onnx: OnnxStatus,
    #[serde(default)]
    pub note: Option<String>,
}

/// Fetch current embedding configuration
pub async fn fetch_embedding_config() -> Result<EmbeddingConfigResponse, String> {
    // Parse the flat response into our struct
    let url = api_url("/config/embedding");
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    
    if response.status().is_success() {
        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;
        
        // Map flat response to our struct
        Ok(EmbeddingConfigResponse {
            status: json["status"].as_str().unwrap_or("unknown").to_string(),
            request_id: json["request_id"].as_str().unwrap_or("").to_string(),
            provider: json["provider"].as_str().unwrap_or("onnx").to_string(),
            onnx: OnnxStatus {
                model_path: json["model_path"].as_str().unwrap_or("").to_string(),
                model_exists: json["model_exists"].as_bool().unwrap_or(false),
                ready: json["ready"].as_bool().unwrap_or(false),
            },
            note: json["note"].as_str().map(|s| s.to_string()),
        })
    } else {
        Err(format!("Server error: {}", response.status()))
    }
}
