// ~/ag/backend/src/api/config_routes.rs  v1.0
// All /config/* endpoint handlers and related types

use super::*;

/// Get current prompt caching state
pub(crate) fn get_prompt_caching_enabled() -> bool {
    let state_arc = chat_state();
    let guard = state_arc.lock().expect("chat state lock");
    guard.prompt_caching_enabled
}

/// Set prompt caching state
pub(crate) fn set_prompt_caching_enabled(enabled: bool) -> bool {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    let previous = guard.prompt_caching_enabled;
    guard.prompt_caching_enabled = enabled;
    previous
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ChunkConfigCommitRequest {
    pub target_size: usize,
    pub min_size: usize,
    pub max_size: usize,
    pub overlap: usize,
    #[serde(default)]
    pub semantic_similarity_threshold: Option<f32>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub clean_html: Option<bool>,
    #[serde(default)]
    pub clean_unicode: Option<bool>,
    #[serde(default)]
    pub context_prefix_enabled: Option<bool>,
    #[serde(default)]
    pub context_prefix_tokens: Option<usize>,
    #[serde(default)]
    pub pipeline_stages: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct ChunkerConfigSnapshot {
    pub target_size: usize,
    pub min_size: usize,
    pub max_size: usize,
    pub overlap: usize,
    pub semantic_similarity_threshold: f32,
    pub mode: String,
    pub clean_html: bool,
    pub clean_unicode: bool,
    pub context_prefix_enabled: bool,
    pub context_prefix_tokens: usize,
    pub pipeline_stages: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChunkCommitResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub chunker_config: ChunkerConfigSnapshot,
    pub reindex_status: String,
    pub reindex_job_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct LlmConfigRequest {
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
    #[serde(default = "default_min_p")]
    pub min_p: f32,
    #[serde(default = "default_typical_p")]
    pub typical_p: f32,
    #[serde(default = "default_tfs_z")]
    pub tfs_z: f32,

    // Mirostat
    #[serde(default = "default_mirostat")]
    pub mirostat: i32,
    #[serde(default = "default_mirostat_eta")]
    pub mirostat_eta: f32,
    #[serde(default = "default_mirostat_tau")]
    pub mirostat_tau: f32,

    // Repetition control
    #[serde(default = "default_repeat_last_n")]
    pub repeat_last_n: usize,
    #[serde(default = "default_penalize_newline")]
    pub penalize_newline: bool,

    // Generation limits
    #[serde(default = "default_num_keep")]
    pub num_keep: i64,
    #[serde(default = "default_ignore_eos")]
    pub ignore_eos: bool,

    // DRY sampling
    #[serde(default = "default_dry_multiplier")]
    pub dry_multiplier: f32,
    #[serde(default = "default_dry_base")]
    pub dry_base: f32,
    #[serde(default = "default_dry_allowed_length")]
    pub dry_allowed_length: usize,

    // XTC sampling
    #[serde(default = "default_xtc_probability")]
    pub xtc_probability: f32,
    #[serde(default = "default_xtc_threshold")]
    pub xtc_threshold: f32,
}

pub(crate) fn default_min_p() -> f32 {
    llm_settings::DEFAULT_MIN_P
}

pub(crate) fn default_typical_p() -> f32 {
    llm_settings::DEFAULT_TYPICAL_P
}

pub(crate) fn default_tfs_z() -> f32 {
    llm_settings::DEFAULT_TFS_Z
}

pub(crate) fn default_mirostat() -> i32 {
    llm_settings::DEFAULT_MIROSTAT
}

pub(crate) fn default_mirostat_eta() -> f32 {
    llm_settings::DEFAULT_MIROSTAT_ETA
}

pub(crate) fn default_mirostat_tau() -> f32 {
    llm_settings::DEFAULT_MIROSTAT_TAU
}

pub(crate) fn default_repeat_last_n() -> usize {
    llm_settings::DEFAULT_REPEAT_LAST_N
}

pub(crate) fn default_num_keep() -> i64 {
    llm_settings::DEFAULT_NUM_KEEP
}

pub(crate) fn default_penalize_newline() -> bool {
    llm_settings::DEFAULT_PENALIZE_NEWLINE
}

pub(crate) fn default_ignore_eos() -> bool {
    llm_settings::DEFAULT_IGNORE_EOS
}

pub(crate) fn default_dry_multiplier() -> f32 {
    llm_settings::DEFAULT_DRY_MULTIPLIER
}

pub(crate) fn default_dry_base() -> f32 {
    llm_settings::DEFAULT_DRY_BASE
}

pub(crate) fn default_dry_allowed_length() -> usize {
    llm_settings::DEFAULT_DRY_ALLOWED_LENGTH
}

pub(crate) fn default_xtc_probability() -> f32 {
    llm_settings::DEFAULT_XTC_PROBABILITY
}

pub(crate) fn default_xtc_threshold() -> f32 {
    llm_settings::DEFAULT_XTC_THRESHOLD
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct HardwareConfigRequest {
    pub backend_type: String,
    pub model: String,

    // Model params
    pub gpu_layers: usize,
    pub main_gpu: usize,
    pub split_mode: String,
    pub tensor_split: Vec<f32>,
    pub use_mmap: bool,
    pub use_mlock: bool,
    pub vocab_only: bool,
    pub devices: Vec<crate::db::param_hardware::DeviceTarget>,
    pub kv_overrides: Vec<crate::db::param_hardware::KvOverride>,
    pub swa_full: bool,
    pub no_perf: bool,

    // Context params
    pub num_ctx: usize,
    pub num_batch: usize,
    pub num_ubatch: usize,
    pub num_seq_max: usize,
    pub rope_scaling_type: crate::db::param_hardware::RopeScalingType,
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
    pub type_k: crate::db::param_hardware::KvDataType,
    pub type_v: crate::db::param_hardware::KvDataType,
    pub embeddings: bool,
    pub offload_kqv: bool,
    pub defrag_thold: f32,
    pub logits_all: bool,
    pub f16_kv: bool,
    pub low_vram: bool,

    // CPU params
    pub num_thread: usize,
    pub num_thread_batch: usize,
    pub numa: bool,
    pub cpu_strict: bool,
    pub cpumask: crate::db::param_hardware::CpuMask,
    pub mask_valid: bool,
    pub poll: usize,
    pub priority: String,

    // Legacy/custom
    pub num_gpu: usize,
    pub llama_server_url: String,
}

pub(crate) fn backend_type_to_string(bt: &crate::db::param_hardware::BackendType) -> String {
    use crate::db::param_hardware::BackendType;
    match bt {
        BackendType::Ollama => "ollama".to_string(),
        BackendType::LlamaCpp => "llama_cpp".to_string(),
        BackendType::OpenAi => "openai".to_string(),
        BackendType::Anthropic => "anthropic".to_string(),
        BackendType::OpenRouter => "openrouter".to_string(),
        BackendType::Vllm => "vllm".to_string(),
        BackendType::Custom => "custom".to_string(),
    }
}

pub(crate) fn string_to_backend_type(s: &str) -> crate::db::param_hardware::BackendType {
    use crate::db::param_hardware::BackendType;
    match s {
        "ollama" => BackendType::Ollama,
        "llama_cpp" => BackendType::LlamaCpp,
        "openai" => BackendType::OpenAi,
        "anthropic" => BackendType::Anthropic,
        "openrouter" => BackendType::OpenRouter,
        "vllm" => BackendType::Vllm,
        "custom" => BackendType::Custom,
        _ => BackendType::Ollama, // default fallback
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct LlmConfigResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub config: LlmConfig,
}

#[derive(Debug, Serialize)]
pub(crate) struct HardwareConfigResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub config: HardwareConfigRequest,
}

#[derive(Debug, Serialize)]
pub(crate) struct OnnxConfigResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub config: OnnxConfigInfo,
}

#[derive(Debug, Serialize)]
pub(crate) struct OnnxConfigInfo {
    pub model_path: String,
    pub max_length: usize,
    pub embedding_dim: usize,
    pub num_threads: usize,
    pub inter_op_num_threads: usize,
    pub optimization_level: String,
    pub execution_mode: String,
    pub enable_mem_pattern: bool,
    pub enable_cpu_mem_arena: bool,
    pub deterministic_compute: bool,
    pub optimized_model_path: Option<String>,
    pub enable_profiling: bool,
    pub profiling_output_path: Option<String>,
    pub log_id: Option<String>,
    pub log_level: String,
    pub log_verbosity: i32,
    pub use_env_allocators: bool,
    pub denormal_as_zero: bool,
    pub enable_quant_qdq: bool,
    pub enable_double_qdq_remover: bool,
    pub enable_qdq_cleanup: bool,
    pub approximate_gelu: bool,
    pub enable_aot_inlining: bool,
    pub disabled_optimizers: Vec<String>,
    pub use_device_allocator_for_initializers: bool,
    pub allow_inter_op_spinning: bool,
    pub allow_intra_op_spinning: bool,
    pub use_prepacking: bool,
    pub independent_thread_pool: bool,
    pub no_env_execution_providers: bool,
}

pub(crate) fn validate_hardware_request(req: &HardwareConfigRequest) -> Result<(), String> {
    // Thread validation
    if req.num_thread == 0 {
        return Err("num_thread must be greater than 0".into());
    }
    if req.num_thread_batch == 0 {
        return Err("num_thread_batch must be greater than 0".into());
    }

    // GPU validation
    if req.num_gpu > 64 {
        return Err("num_gpu must be 64 or less".into());
    }
    if req.main_gpu > 64 {
        return Err("main_gpu index must be 64 or less".into());
    }
    if req.gpu_layers > 1000 {
        return Err("gpu_layers must be 1000 or less".into());
    }

    // RoPE validation
    if req.rope_frequency_base <= 0.0 {
        return Err("rope_frequency_base must be positive".into());
    }
    if req.rope_frequency_scale <= 0.0 {
        return Err("rope_frequency_scale must be positive".into());
    }

    // Context/batch validation
    if req.num_ctx == 0 {
        return Err("num_ctx must be greater than 0".into());
    }
    if req.num_batch == 0 {
        return Err("num_batch must be greater than 0".into());
    }
    if req.num_ubatch == 0 {
        return Err("num_ubatch must be greater than 0".into());
    }
    if req.num_ubatch > req.num_batch {
        return Err("num_ubatch must be <= num_batch".into());
    }
    if req.num_seq_max == 0 {
        return Err("num_seq_max must be greater than 0".into());
    }

    // CPU mask validation
    if req.mask_valid && req.cpumask.is_empty() {
        return Err("cpumask cannot be empty when mask_valid is true".into());
    }

    // Defrag threshold validation
    if req.defrag_thold < 0.0 || req.defrag_thold > 1.0 {
        return Err("defrag_thold must be between 0.0 and 1.0".into());
    }

    // Tensor split validation
    if !req.tensor_split.is_empty() {
        let sum: f32 = req.tensor_split.iter().sum();
        if (sum - 1.0).abs() > 0.01 && sum > 0.0 {
            // Allow sum != 1.0 only if all zeros (auto-split)
            let all_positive = req.tensor_split.iter().all(|&x| x > 0.0);
            if all_positive {
                return Err("tensor_split values should sum to approximately 1.0".into());
            }
        }
    }

    // Split mode validation
    let valid_split_modes = ["none", "layer", "row"];
    if !valid_split_modes.contains(&req.split_mode.as_str()) {
        return Err(format!(
            "split_mode must be one of: {}",
            valid_split_modes.join(", ")
        ));
    }

    // Priority validation
    let valid_priorities = ["low", "normal", "high", "realtime"];
    if !valid_priorities.contains(&req.priority.as_str()) {
        return Err(format!(
            "priority must be one of: {}",
            valid_priorities.join(", ")
        ));
    }

    Ok(())
}

/// POST /monitoring/io-uring
/// Save io_uring configuration to .env file
pub(crate) async fn save_io_uring_config(
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 1: QUEUE & BUFFERS
    // ═══════════════════════════════════════════════════════════════
    let ring_size = body
        .get("ring_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(256) as u32;
    let cq_size = body.get("cq_size").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let buffer_size = body
        .get("buffer_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(65536) as usize;
    let buffer_pool_size = body
        .get("buffer_pool_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(64) as usize;
    let clamp = body.get("clamp").and_then(|v| v.as_bool()).unwrap_or(false);

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 2: POLLING
    // ═══════════════════════════════════════════════════════════════
    let sqpoll = body
        .get("sqpoll")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let sqpoll_idle_ms = body
        .get("sqpoll_idle_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(1000) as u32;
    let sqpoll_cpu = body
        .get("sqpoll_cpu")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1) as i32;
    let iopoll = body
        .get("iopoll")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 3: OPTIMIZATION
    // ═══════════════════════════════════════════════════════════════
    let single_issuer = body
        .get("single_issuer")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let coop_taskrun = body
        .get("coop_taskrun")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let defer_taskrun = body
        .get("defer_taskrun")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let submit_all = body
        .get("submit_all")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let taskrun_flag = body
        .get("taskrun_flag")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 4: ADVANCED
    // ═══════════════════════════════════════════════════════════════
    let r_disabled = body
        .get("r_disabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let attach_wq_fd = body
        .get("attach_wq_fd")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1) as i32;
    let dontfork = body
        .get("dontfork")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Validate ring_size is power of 2
    if !ring_size.is_power_of_two() || ring_size < 1 || ring_size > 32768 {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "ring_size must be a power of 2 between 1 and 32768",
            "request_id": request_id
        })));
    }

    // Validate buffer_size
    if buffer_size < 4096 || buffer_size > 16 * 1024 * 1024 {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "buffer_size must be between 4096 and 16MB",
            "request_id": request_id
        })));
    }

    // Build env content with all parameters
    let env_content = format!(
        "# io_uring Configuration (saved by UI)\n\
         \n\
         # Category 1: Queue & Buffers\n\
         IO_URING_RING_SIZE={}\n\
         IO_URING_CQ_SIZE={}\n\
         IO_URING_BUFFER_SIZE={}\n\
         IO_URING_BUFFER_POOL_SIZE={}\n\
         IO_URING_CLAMP={}\n\
         \n\
         # Category 2: Polling\n\
         IO_URING_SQPOLL={}\n\
         IO_URING_SQPOLL_IDLE_MS={}\n\
         IO_URING_SQPOLL_CPU={}\n\
         IO_URING_IOPOLL={}\n\
         \n\
         # Category 3: Optimization\n\
         IO_URING_SINGLE_ISSUER={}\n\
         IO_URING_COOP_TASKRUN={}\n\
         IO_URING_DEFER_TASKRUN={}\n\
         IO_URING_SUBMIT_ALL={}\n\
         IO_URING_TASKRUN_FLAG={}\n\
         \n\
         # Category 4: Advanced\n\
         IO_URING_R_DISABLED={}\n\
         IO_URING_ATTACH_WQ_FD={}\n\
         IO_URING_DONTFORK={}\n",
        ring_size,
        cq_size,
        buffer_size,
        buffer_pool_size,
        clamp,
        sqpoll,
        sqpoll_idle_ms,
        sqpoll_cpu,
        iopoll,
        single_issuer,
        coop_taskrun,
        defer_taskrun,
        submit_all,
        taskrun_flag,
        r_disabled,
        attach_wq_fd,
        dontfork
    );

    // Save to .env.io_uring file
    let env_path = std::path::Path::new(".env.io_uring");
    match std::fs::write(env_path, &env_content) {
        Ok(_) => {
            info!("Saved io_uring config to .env.io_uring");
            Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "message": "io_uring configuration saved to .env.io_uring",
                "request_id": request_id,
                "config": {
                    "ring_size": ring_size,
                    "cq_size": cq_size,
                    "buffer_size": buffer_size,
                    "buffer_pool_size": buffer_pool_size,
                    "clamp": clamp,
                    "sqpoll": sqpoll,
                    "sqpoll_idle_ms": sqpoll_idle_ms,
                    "sqpoll_cpu": sqpoll_cpu,
                    "iopoll": iopoll,
                    "single_issuer": single_issuer,
                    "coop_taskrun": coop_taskrun,
                    "defer_taskrun": defer_taskrun,
                    "submit_all": submit_all,
                    "taskrun_flag": taskrun_flag,
                    "r_disabled": r_disabled,
                    "attach_wq_fd": attach_wq_fd,
                    "dontfork": dontfork
                },
                "note": "Restart backend to apply changes"
            })))
        }
        Err(e) => {
            error!("Failed to save io_uring config: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save config: {}", e),
                "request_id": request_id
            })))
        }
    }
}

/// Returns the embedding dimension for a model name string.
fn embedding_model_dimension(model: &str) -> usize {
    match model {
        "bge-base-en-v1.5" => 768,
        _ => 384,
    }
}

/// Returns the HuggingFace model ID for a model name string.
fn embedding_model_hf_id(model: &str) -> &'static str {
    match model {
        "bge-small-en-v1.5q" => "BAAI/bge-small-en-v1.5",
        "all-minilm-l6-v2" => "sentence-transformers/all-MiniLM-L6-v2",
        "bge-base-en-v1.5" => "BAAI/bge-base-en-v1.5",
        "e5-small-v2" => "intfloat/e5-small-v2",
        _ => "BAAI/bge-small-en-v1.5",
    }
}

/// GET /config/embedding
/// Returns current embedding model configuration.
pub(crate) async fn get_embedding_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    let model = std::env::var("EMBEDDING_MODEL")
        .unwrap_or_else(|_| "bge-small-en-v1.5".to_string());
    let onnx_model_path = std::env::var("ONNX_MODEL_PATH")
        .unwrap_or_else(|_| "models/embedding_model.onnx".to_string());

    let model_exists = std::path::Path::new(&onnx_model_path).exists();
    let tokenizer_exists = std::path::Path::new(&onnx_model_path)
        .parent()
        .map(|d| d.join("tokenizer.json").exists())
        .unwrap_or(false);
    let dimension = embedding_model_dimension(&model);
    let hf_id = embedding_model_hf_id(&model);

    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "request_id": request_id,
        "provider": "onnx",
        "model": model,
        "model_path": onnx_model_path,
        "model_exists": model_exists,
        "tokenizer_exists": tokenizer_exists,
        "dimension": dimension,
        "hf_id": hf_id,
        "ready": model_exists,
    })))
}

/// POST /config/embedding-model
/// Persist a new embedding model selection to .env.embedding.
/// Takes effect on next restart.
pub(crate) async fn set_embedding_model(
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("bge-small-en-v1.5");

    // Validate against known models
    let known = ["bge-small-en-v1.5", "bge-small-en-v1.5q", "all-minilm-l6-v2",
                 "bge-base-en-v1.5", "e5-small-v2"];
    if !known.contains(&model) {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "error",
            "request_id": request_id,
            "message": format!("Unknown model '{}'. Valid options: {}", model, known.join(", "))
        })));
    }

    let model_path = std::env::var("ONNX_MODEL_PATH")
        .unwrap_or_else(|_| "models/embedding_model.onnx".to_string());
    let dimension = embedding_model_dimension(model);
    let hf_id = embedding_model_hf_id(model);

    let env_content = format!(
        "# Embedding model selection — written by /config/embedding-model\n\
         EMBEDDING_MODEL={}\n\
         ONNX_MODEL_PATH={}\n",
        model, model_path
    );

    let env_path = std::path::Path::new(".env.embedding");
    match std::fs::write(env_path, &env_content) {
        Ok(_) => {
            info!(model = %model, "Embedding model selection saved to .env.embedding");
            Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "request_id": request_id,
                "model": model,
                "dimension": dimension,
                "hf_id": hf_id,
                "message": "Saved. Restart the service for the new model to take effect."
            })))
        }
        Err(e) => {
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "request_id": request_id,
                "message": format!("Failed to write .env.embedding: {}", e)
            })))
        }
    }
}

/// POST /config/embedding/download-tokenizer
/// Downloads tokenizer.json from HuggingFace for the current EMBEDDING_MODEL and writes it
/// next to the ONNX model file. Idempotent — safe to call if the file already exists.
pub(crate) async fn download_tokenizer() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    let model = std::env::var("EMBEDDING_MODEL")
        .unwrap_or_else(|_| "bge-small-en-v1.5".to_string());
    let onnx_model_path = std::env::var("ONNX_MODEL_PATH")
        .unwrap_or_else(|_| "models/embedding_model.onnx".to_string());

    let model_dir = std::path::Path::new(&onnx_model_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("models"));

    // Create the directory if it doesn't exist yet
    if let Err(e) = std::fs::create_dir_all(model_dir) {
        return Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "request_id": request_id,
            "message": format!("Could not create model directory: {e}")
        })));
    }

    let tok_path = model_dir.join("tokenizer.json");
    let hf_id = embedding_model_hf_id(&model);
    let url = format!(
        "https://huggingface.co/{}/resolve/main/tokenizer.json",
        hf_id
    );

    info!(model = %model, hf_id = %hf_id, dest = %tok_path.display(), "Downloading tokenizer.json");

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "request_id": request_id,
            "message": format!("Failed to build HTTP client: {e}")
        }))),
    };

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return Ok(HttpResponse::BadGateway().json(json!({
            "status": "error",
            "request_id": request_id,
            "message": format!("HuggingFace request failed: {e}")
        }))),
    };

    if !resp.status().is_success() {
        return Ok(HttpResponse::BadGateway().json(json!({
            "status": "error",
            "request_id": request_id,
            "message": format!("HuggingFace returned HTTP {}", resp.status())
        })));
    }

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => return Ok(HttpResponse::BadGateway().json(json!({
            "status": "error",
            "request_id": request_id,
            "message": format!("Failed to read response body: {e}")
        }))),
    };

    if let Err(e) = std::fs::write(&tok_path, &bytes) {
        return Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "request_id": request_id,
            "message": format!("Failed to write tokenizer.json: {e}")
        })));
    }

    info!(dest = %tok_path.display(), bytes = bytes.len(), "tokenizer.json saved");

    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "request_id": request_id,
        "model": model,
        "hf_id": hf_id,
        "dest": tok_path.to_string_lossy(),
        "bytes": bytes.len(),
        "message": format!("tokenizer.json downloaded ({} bytes). Restart to activate.", bytes.len())
    })))
}

/// POST /config/embedding — legacy stub kept for route compatibility.
pub(crate) async fn set_embedding_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    Ok(HttpResponse::Ok().json(json!({
        "status": "info",
        "request_id": request_id,
        "message": "Use POST /config/embedding-model to change the active embedding model."
    })))
}

/// Self-contained UI metrics: HTTP Requests summary + chart
/// GET /monitoring/ui/requests
/// Returns: JSON with rate, p95 latency, error%, and recent points
pub(crate) async fn get_ui_requests() -> Result<HttpResponse, Error> {
    let snapshot = crate::monitoring::get_requests_snapshot();
    Ok(HttpResponse::Ok().json(snapshot))
}

pub(crate) async fn get_chunking_stats(
    query: web::Query<ChunkingQuery>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(new_cap) = query.capacity {
        let applied = crate::monitoring::set_chunking_history_capacity(new_cap);
        return Ok(HttpResponse::Ok().json(json!({
            "status": "ok",
            "request_id": request_id,
            "capacity_applied": applied,
            "message": "History capacity updated",
        })));
    }

    let limit = query.limit.unwrap_or(10);
    let history = crate::monitoring::chunking_snapshot_history(limit);

    if history.is_empty() {
        Ok(HttpResponse::Ok().json(json!({
            "status": "empty",
            "message": "No chunking stats recorded yet",
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::Ok().json(json!({
            "status": "ok",
            "request_id": request_id,
            "count": history.len(),
            "snapshots": history,
        })))
    }
}

pub(crate) async fn toggle_chunking_logging(
    query: web::Query<LoggingQuery>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(enabled) = query.enabled {
        crate::monitoring::set_chunking_logging_enabled(enabled);
        return Ok(HttpResponse::Ok().json(json!({
            "status": "ok",
            "request_id": request_id,
            "logging_enabled": enabled,
            "message": "Chunking snapshot logging updated",
        })));
    }

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "request_id": request_id,
        "logging_enabled": crate::monitoring::chunking_logging_enabled(),
    })))
}

/// GET /config/chunk_size - Fetch current chunk configuration
pub(crate) async fn get_chunk_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = chunk_settings::global_config();
    let snapshot = ChunkerConfigSnapshot::from(&config);

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "message": "Chunk configuration loaded",
        "request_id": request_id,
        "chunker_config": snapshot
    })))
}

pub(crate) async fn commit_chunk_config(
    config: web::Data<ApiConfig>,
    payload: web::Json<ChunkConfigCommitRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let body = payload.into_inner();
    if let Err(msg) = validate_chunk_request(&body) {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "invalid",
            "message": msg,
            "request_id": request_id
        })));
    }

    let existing = chunk_settings::global_config();
    let new_cfg = ChunkerConfig {
        target_size: body.target_size,
        min_size: body.min_size,
        max_size: body.max_size,
        overlap: body.overlap,
        semantic_similarity_threshold: body
            .semantic_similarity_threshold
            .unwrap_or(existing.semantic_similarity_threshold),
        mode: body.mode.unwrap_or(existing.mode),
        clean_html: body.clean_html.unwrap_or(existing.clean_html),
        clean_unicode: body.clean_unicode.unwrap_or(existing.clean_unicode),
        context_prefix_enabled: body
            .context_prefix_enabled
            .unwrap_or(existing.context_prefix_enabled),
        context_prefix_tokens: body
            .context_prefix_tokens
            .unwrap_or(existing.context_prefix_tokens),
        pipeline_stages: body
            .pipeline_stages
            .unwrap_or(existing.pipeline_stages),
    };

    match chunk_settings::save_chunker_config_default_db(&new_cfg) {
        Ok(_) => {
            tracing::info!(
                request_id = %request_id,
                target = new_cfg.target_size,
                min = new_cfg.min_size,
                max = new_cfg.max_size,
                overlap = new_cfg.overlap,
                "Chunk config committed"
            );
        }
        Err(err) => {
            tracing::error!(
                request_id = %request_id,
                error = %err,
                "Failed to save chunk config"
            );
            return Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save chunk config: {}", err),
                "request_id": request_id
            })));
        }
    }

    let chunk_snapshot = ChunkerConfigSnapshot::from(&new_cfg);

    match launch_async_reindex_job(config) {
        Ok(job_id) => Ok(HttpResponse::Accepted().json(ChunkCommitResponse {
            status: "accepted".into(),
            message: "Chunk settings saved; reindex started".into(),
            request_id,
            chunker_config: chunk_snapshot,
            reindex_status: "accepted".into(),
            reindex_job_id: Some(job_id),
        })),
        Err((status, message)) => {
            let http_status = if status == StatusCode::TOO_MANY_REQUESTS {
                StatusCode::TOO_MANY_REQUESTS
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            tracing::warn!(
                request_id = %request_id,
                status = %http_status.as_u16(),
                message = %message,
                "Chunk commit applied but reindex not started"
            );
            Ok(HttpResponse::build(http_status).json(ChunkCommitResponse {
                status: "saved_pending_reindex".into(),
                message: format!("Settings saved, but reindex not started: {}", message),
                request_id,
                chunker_config: chunk_snapshot,
                reindex_status: "skipped".into(),
                reindex_job_id: None,
            }))
        }
    }
}

pub(crate) async fn get_llm_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = llm_settings::global_config();
    Ok(HttpResponse::Ok().json(LlmConfigResponse {
        status: "ok".into(),
        message: "Current LLM configuration".into(),
        request_id,
        config,
    }))
}

pub(crate) async fn commit_llm_config(
    payload: web::Json<LlmConfigRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let body = payload.into_inner();

    if let Err(msg) = validate_llm_request(&body) {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "invalid",
            "message": msg,
            "request_id": request_id
        })));
    }

    let new_cfg = LlmConfig {
        // Basic sampling
        temperature: body.temperature,
        top_p: body.top_p,
        top_k: body.top_k,
        max_tokens: body.max_tokens,
        repeat_penalty: body.repeat_penalty,
        frequency_penalty: body.frequency_penalty,
        presence_penalty: body.presence_penalty,
        stop_sequences: body.stop_sequences,
        seed: body.seed,
        min_p: body.min_p,
        typical_p: body.typical_p,
        tfs_z: body.tfs_z,
        // Mirostat
        mirostat: body.mirostat,
        mirostat_eta: body.mirostat_eta,
        mirostat_tau: body.mirostat_tau,
        // Repetition control
        repeat_last_n: body.repeat_last_n,
        penalize_newline: body.penalize_newline,
        // Generation limits
        num_keep: body.num_keep,
        ignore_eos: body.ignore_eos,
        // DRY sampling
        dry_multiplier: body.dry_multiplier,
        dry_base: body.dry_base,
        dry_allowed_length: body.dry_allowed_length,
        // XTC sampling
        xtc_probability: body.xtc_probability,
        xtc_threshold: body.xtc_threshold,
    };

    match llm_settings::save_llm_config_default_db(&new_cfg) {
        Ok(_) => {
            tracing::info!(
                request_id = %request_id,
                temperature = new_cfg.temperature,
                top_p = new_cfg.top_p,
                top_k = new_cfg.top_k,
                max_tokens = new_cfg.max_tokens,
                repeat_penalty = new_cfg.repeat_penalty,
                frequency_penalty = new_cfg.frequency_penalty,
                presence_penalty = new_cfg.presence_penalty,
                stop_sequences = ?new_cfg.stop_sequences,
                seed = ?new_cfg.seed,
                "LLM config committed"
            );
            Ok(HttpResponse::Ok().json(LlmConfigResponse {
                status: "ok".into(),
                message: "LLM settings saved".into(),
                request_id,
                config: new_cfg,
            }))
        }
        Err(err) => {
            tracing::error!(
                request_id = %request_id,
                error = %err,
                "Failed to save LLM config"
            );
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save LLM config: {}", err),
                "request_id": request_id
            })))
        }
    }
}

// ============================================================================
// PROMPT CACHING ENDPOINTS
// ============================================================================

#[derive(Debug, Serialize)]
pub(crate) struct PromptCachingResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub enabled: bool,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct PromptCachingRequest {
    pub enabled: bool,
}

/// Get current prompt caching state
/// When enabled, uses /api/chat (with KV caching) instead of /api/generate
pub(crate) async fn get_prompt_caching() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let enabled = get_prompt_caching_enabled();
    Ok(HttpResponse::Ok().json(PromptCachingResponse {
        status: "ok".into(),
        message: if enabled {
            "Prompt caching enabled (using /api/chat)".into()
        } else {
            "Prompt caching disabled (using /api/generate)".into()
        },
        request_id,
        enabled,
    }))
}

/// Set prompt caching state
pub(crate) async fn set_prompt_caching(
    payload: web::Json<PromptCachingRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let body = payload.into_inner();
    let previous = set_prompt_caching_enabled(body.enabled);

    tracing::info!(
        request_id = %request_id,
        enabled = body.enabled,
        previous = previous,
        "Prompt caching state changed"
    );

    Ok(HttpResponse::Ok().json(PromptCachingResponse {
        status: "ok".into(),
        message: if body.enabled {
            "Prompt caching enabled - using /api/chat for better KV cache reuse".into()
        } else {
            "Prompt caching disabled - using /api/generate".into()
        },
        request_id,
        enabled: body.enabled,
    }))
}

pub(crate) async fn get_hardware_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = crate::db::param_hardware::global_config().into();
    Ok(HttpResponse::Ok().json(HardwareConfigResponse {
        status: "ok".into(),
        message: "".into(),
        request_id,
        config,
    }))
}

pub(crate) async fn commit_hardware_config(
    payload: web::Json<HardwareConfigRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let body = payload.into_inner();

    if let Err(msg) = validate_hardware_request(&body) {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "invalid",
            "message": msg,
            "request_id": request_id
        })));
    }

    let params = crate::db::param_hardware::HardwareParams::from(body.clone());
    match crate::db::param_hardware::save_default_db(&params) {
        Ok(_) => {
            tracing::info!(
                request_id = %request_id,
                num_thread = params.num_thread,
                num_gpu = params.num_gpu,
                gpu_layers = params.gpu_layers,
                main_gpu = params.main_gpu,
                low_vram = params.low_vram,
                f16_kv = params.f16_kv,
                rope_frequency_base = params.rope_frequency_base,
                rope_frequency_scale = params.rope_frequency_scale,
                "Hardware config committed"
            );
            crate::api::sys_routes::reload_token_counter();
            Ok(HttpResponse::Ok().json(HardwareConfigResponse {
                status: "ok".into(),
                message: "Hardware settings saved".into(),
                request_id,
                config: body,
            }))
        }
        Err(err) => {
            tracing::error!(
                request_id = %request_id,
                error = %err,
                "Failed to save hardware config"
            );
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save hardware config: {}", err),
                "request_id": request_id
            })))
        }
    }
}

pub(crate) fn onnx_opt_level_to_str(level: OnnxOptimizationLevel) -> &'static str {
    match level {
        OnnxOptimizationLevel::Disable => "disable",
        OnnxOptimizationLevel::Basic => "basic",
        OnnxOptimizationLevel::Extended => "extended",
        OnnxOptimizationLevel::All => "all",
    }
}

pub(crate) fn onnx_exec_mode_to_str(mode: OnnxExecutionMode) -> &'static str {
    match mode {
        OnnxExecutionMode::Sequential => "sequential",
        OnnxExecutionMode::Parallel => "parallel",
    }
}

pub(crate) fn onnx_log_level_to_str(level: OnnxLogLevel) -> &'static str {
    match level {
        OnnxLogLevel::Verbose => "verbose",
        OnnxLogLevel::Info => "info",
        OnnxLogLevel::Warning => "warning",
        OnnxLogLevel::Error => "error",
        OnnxLogLevel::Fatal => "fatal",
    }
}

pub(crate) fn parse_log_level(input: &str) -> Option<OnnxLogLevel> {
    match input.to_lowercase().as_str() {
        "verbose" | "trace" => Some(OnnxLogLevel::Verbose),
        "info" => Some(OnnxLogLevel::Info),
        "warn" | "warning" => Some(OnnxLogLevel::Warning),
        "error" => Some(OnnxLogLevel::Error),
        "fatal" | "critical" => Some(OnnxLogLevel::Fatal),
        _ => None,
    }
}

pub(crate) fn apply_option_field<T>(target: &mut Option<T>, value: Option<Option<T>>) {
    if let Some(inner) = value {
        *target = inner;
    }
}

pub(crate) fn get_onnx_config_storage() -> &'static std::sync::RwLock<OnnxConfig> {
    ONNX_CONFIG.get_or_init(|| {
        // Initialize from environment or defaults
        let model_path = std::env::var("ONNX_MODEL_PATH")
            .unwrap_or_else(|_| "models/embedding_model.onnx".to_string());
        let num_threads = std::env::var("ONNX_NUM_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4);
        let inter_threads = std::env::var("ONNX_INTER_OP_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        std::sync::RwLock::new(OnnxConfig {
            model_path,
            num_threads,
            inter_op_num_threads: inter_threads,
            ..Default::default()
        })
    })
}

/// Get current ONNX configuration
pub(crate) async fn get_onnx_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = get_onnx_config_storage().read().unwrap();

    let opt_level_str = onnx_opt_level_to_str(config.optimization_level);
    let exec_mode_str = onnx_exec_mode_to_str(config.execution_mode);
    let log_level_str = onnx_log_level_to_str(config.log_level);

    Ok(HttpResponse::Ok().json(OnnxConfigResponse {
        status: "ok".into(),
        message: "".into(),
        request_id,
        config: OnnxConfigInfo {
            model_path: config.model_path.clone(),
            max_length: config.max_length,
            embedding_dim: config.embedding_dim,
            num_threads: config.num_threads,
            inter_op_num_threads: config.inter_op_num_threads,
            optimization_level: opt_level_str.to_string(),
            execution_mode: exec_mode_str.to_string(),
            enable_mem_pattern: config.enable_mem_pattern,
            enable_cpu_mem_arena: config.enable_cpu_mem_arena,
            deterministic_compute: config.deterministic_compute,
            optimized_model_path: config.optimized_model_path.clone(),
            enable_profiling: config.enable_profiling,
            profiling_output_path: config.profiling_output_path.clone(),
            log_id: config.log_id.clone(),
            log_level: log_level_str.to_string(),
            log_verbosity: config.log_verbosity,
            use_env_allocators: config.use_env_allocators,
            denormal_as_zero: config.denormal_as_zero,
            enable_quant_qdq: config.enable_quant_qdq,
            enable_double_qdq_remover: config.enable_double_qdq_remover,
            enable_qdq_cleanup: config.enable_qdq_cleanup,
            approximate_gelu: config.approximate_gelu,
            enable_aot_inlining: config.enable_aot_inlining,
            disabled_optimizers: config.disabled_optimizers.clone(),
            use_device_allocator_for_initializers: config.use_device_allocator_for_initializers,
            allow_inter_op_spinning: config.allow_inter_op_spinning,
            allow_intra_op_spinning: config.allow_intra_op_spinning,
            use_prepacking: config.use_prepacking,
            independent_thread_pool: config.independent_thread_pool,
            no_env_execution_providers: config.no_env_execution_providers,
        },
    }))
}

/// Update ONNX configuration (requires restart to take effect for embedder)
pub(crate) async fn set_onnx_config(
    payload: web::Json<OnnxConfigRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let body = payload.into_inner();

    let mut config = get_onnx_config_storage().write().unwrap();

    // Update only provided fields
    if let Some(path) = body.model_path {
        config.model_path = path;
    }
    if let Some(len) = body.max_length {
        config.max_length = len;
    }
    if let Some(dim) = body.embedding_dim {
        config.embedding_dim = dim;
    }
    if let Some(threads) = body.num_threads {
        if threads == 0 {
            return Ok(HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "num_threads must be greater than 0",
                "request_id": request_id
            })));
        }
        config.num_threads = threads;
    }
    if let Some(threads) = body.inter_op_num_threads {
        config.inter_op_num_threads = threads;
    }
    if let Some(level) = body.optimization_level {
        config.optimization_level = match level.to_lowercase().as_str() {
            "disable" | "0" => OnnxOptimizationLevel::Disable,
            "basic" | "1" => OnnxOptimizationLevel::Basic,
            "extended" | "2" => OnnxOptimizationLevel::Extended,
            "all" | "3" => OnnxOptimizationLevel::All,
            _ => {
                return Ok(HttpResponse::BadRequest().json(json!({
                    "status": "error",
                    "message": "Invalid optimization_level. Use: disable, basic, extended, all",
                    "request_id": request_id
                })));
            }
        };
    }
    if let Some(mode) = body.execution_mode {
        config.execution_mode = match mode.to_lowercase().as_str() {
            "sequential" => OnnxExecutionMode::Sequential,
            "parallel" => OnnxExecutionMode::Parallel,
            _ => {
                return Ok(HttpResponse::BadRequest().json(json!({
                    "status": "error",
                    "message": "Invalid execution_mode. Use: sequential, parallel",
                    "request_id": request_id
                })));
            }
        };
    }
    if let Some(enabled) = body.enable_mem_pattern {
        config.enable_mem_pattern = enabled;
    }
    if let Some(enabled) = body.enable_cpu_mem_arena {
        config.enable_cpu_mem_arena = enabled;
    }
    if let Some(flag) = body.deterministic_compute {
        config.deterministic_compute = flag;
    }
    apply_option_field(&mut config.optimized_model_path, body.optimized_model_path);
    if let Some(flag) = body.enable_profiling {
        config.enable_profiling = flag;
    }
    apply_option_field(
        &mut config.profiling_output_path,
        body.profiling_output_path,
    );
    apply_option_field(&mut config.log_id, body.log_id);
    if let Some(level) = body.log_level {
        match parse_log_level(&level) {
            Some(parsed) => config.log_level = parsed,
            None => {
                return Ok(HttpResponse::BadRequest().json(json!({
                    "status": "error",
                    "message": format!("Invalid log_level '{}'. Use verbose, info, warning, error, fatal", level),
                    "request_id": request_id
                })));
            }
        }
    }
    if let Some(verbosity) = body.log_verbosity {
        if verbosity < 0 {
            return Ok(HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "log_verbosity must be >= 0",
                "request_id": request_id
            })));
        }
        config.log_verbosity = verbosity;
    }
    if let Some(flag) = body.use_env_allocators {
        config.use_env_allocators = flag;
    }
    if let Some(flag) = body.denormal_as_zero {
        config.denormal_as_zero = flag;
    }
    if let Some(flag) = body.enable_quant_qdq {
        config.enable_quant_qdq = flag;
    }
    if let Some(flag) = body.enable_double_qdq_remover {
        config.enable_double_qdq_remover = flag;
    }
    if let Some(flag) = body.enable_qdq_cleanup {
        config.enable_qdq_cleanup = flag;
    }
    if let Some(flag) = body.approximate_gelu {
        config.approximate_gelu = flag;
    }
    if let Some(flag) = body.enable_aot_inlining {
        config.enable_aot_inlining = flag;
    }
    if let Some(list) = body.disabled_optimizers {
        config.disabled_optimizers = list;
    }
    if let Some(flag) = body.use_device_allocator_for_initializers {
        config.use_device_allocator_for_initializers = flag;
    }
    if let Some(flag) = body.allow_inter_op_spinning {
        config.allow_inter_op_spinning = flag;
    }
    if let Some(flag) = body.allow_intra_op_spinning {
        config.allow_intra_op_spinning = flag;
    }
    if let Some(flag) = body.use_prepacking {
        config.use_prepacking = flag;
    }
    if let Some(flag) = body.independent_thread_pool {
        config.independent_thread_pool = flag;
    }
    if let Some(flag) = body.no_env_execution_providers {
        config.no_env_execution_providers = flag;
    }

    let opt_level_str = onnx_opt_level_to_str(config.optimization_level);
    let exec_mode_str = onnx_exec_mode_to_str(config.execution_mode);
    let log_level_str = onnx_log_level_to_str(config.log_level);

    tracing::info!(
        request_id = %request_id,
        num_threads = config.num_threads,
        inter_op_threads = config.inter_op_num_threads,
        optimization_level = opt_level_str,
        execution_mode = exec_mode_str,
        deterministic_compute = config.deterministic_compute,
        enable_profiling = config.enable_profiling,
        log_level = log_level_str,
        "ONNX config updated (restart required to apply)"
    );

    Ok(HttpResponse::Ok().json(OnnxConfigResponse {
        status: "ok".into(),
        message: "ONNX config updated. Restart backend to apply changes to embedder.".into(),
        request_id,
        config: OnnxConfigInfo {
            model_path: config.model_path.clone(),
            max_length: config.max_length,
            embedding_dim: config.embedding_dim,
            num_threads: config.num_threads,
            inter_op_num_threads: config.inter_op_num_threads,
            optimization_level: opt_level_str.to_string(),
            execution_mode: exec_mode_str.to_string(),
            enable_mem_pattern: config.enable_mem_pattern,
            enable_cpu_mem_arena: config.enable_cpu_mem_arena,
            deterministic_compute: config.deterministic_compute,
            optimized_model_path: config.optimized_model_path.clone(),
            enable_profiling: config.enable_profiling,
            profiling_output_path: config.profiling_output_path.clone(),
            log_id: config.log_id.clone(),
            log_level: log_level_str.to_string(),
            log_verbosity: config.log_verbosity,
            use_env_allocators: config.use_env_allocators,
            denormal_as_zero: config.denormal_as_zero,
            enable_quant_qdq: config.enable_quant_qdq,
            enable_double_qdq_remover: config.enable_double_qdq_remover,
            enable_qdq_cleanup: config.enable_qdq_cleanup,
            approximate_gelu: config.approximate_gelu,
            enable_aot_inlining: config.enable_aot_inlining,
            disabled_optimizers: config.disabled_optimizers.clone(),
            use_device_allocator_for_initializers: config.use_device_allocator_for_initializers,
            allow_inter_op_spinning: config.allow_inter_op_spinning,
            allow_intra_op_spinning: config.allow_intra_op_spinning,
            use_prepacking: config.use_prepacking,
            independent_thread_pool: config.independent_thread_pool,
            no_env_execution_providers: config.no_env_execution_providers,
        },
    }))
}

/// Get the current ONNX config for use by embedder initialization
pub fn get_current_onnx_config() -> OnnxConfig {
    get_onnx_config_storage().read().unwrap().clone()
}

// ============================================================================
// NER CONFIG
// ============================================================================

use crate::db::ner_settings::NerConfig;

#[derive(Debug, serde::Deserialize)]
pub(crate) struct NerConfigRequest {
    pub extraction_enabled: Option<bool>,
    pub type_allowlist: Option<String>,
    pub confidence_threshold: Option<f64>,
    pub type_thresholds: Option<String>,
    pub fuzzy_threshold: Option<f64>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub dedup_case_insensitive: Option<bool>,
    pub nesting_strategy: Option<String>,
    pub batch_size: Option<usize>,
    pub quantization_enabled: Option<bool>,
    pub model_cache_enabled: Option<bool>,
    pub graph_storage_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NerConfigResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub config: NerConfig,
}

pub(crate) async fn get_ner_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = crate::db::ner_settings::global_config();
    Ok(HttpResponse::Ok().json(NerConfigResponse {
        status: "ok".into(),
        message: "".into(),
        request_id,
        config,
    }))
}

pub(crate) async fn set_ner_config(
    payload: web::Json<NerConfigRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let body = payload.into_inner();
    let mut config = crate::db::ner_settings::global_config();

    if let Some(v) = body.extraction_enabled {
        config.extraction_enabled = v;
    }
    if let Some(v) = body.type_allowlist {
        config.type_allowlist = v;
    }
    if let Some(v) = body.confidence_threshold {
        config.confidence_threshold = v.clamp(0.0, 1.0);
    }
    if let Some(v) = body.type_thresholds {
        config.type_thresholds = v;
    }
    if let Some(v) = body.fuzzy_threshold {
        config.fuzzy_threshold = v.clamp(0.0, 1.0);
    }
    if let Some(v) = body.min_length {
        config.min_length = v;
    }
    if let Some(v) = body.max_length {
        config.max_length = v;
    }
    if let Some(v) = body.dedup_case_insensitive {
        config.dedup_case_insensitive = v;
    }
    if let Some(v) = body.nesting_strategy {
        match v.as_str() {
            "KeepLongest" | "KeepAll" | "KeepShortest" => config.nesting_strategy = v,
            _ => {
                return Ok(HttpResponse::BadRequest().json(json!({
                    "status": "error",
                    "message": "Invalid nesting_strategy. Use: KeepLongest, KeepAll, KeepShortest",
                    "request_id": request_id
                })));
            }
        }
    }
    if let Some(v) = body.batch_size {
        if v == 0 {
            return Ok(HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": "batch_size must be greater than 0",
                "request_id": request_id
            })));
        }
        config.batch_size = v;
    }
    if let Some(v) = body.quantization_enabled {
        config.quantization_enabled = v;
    }
    if let Some(v) = body.model_cache_enabled {
        config.model_cache_enabled = v;
    }
    if let Some(v) = body.graph_storage_enabled {
        config.graph_storage_enabled = v;
    }

    if let Err(e) = crate::db::ner_settings::save_ner_config_default_db(&config) {
        tracing::error!(request_id = %request_id, error = %e, "Failed to persist NER config");
        return Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": format!("Failed to save NER config: {e}"),
            "request_id": request_id
        })));
    }

    tracing::info!(
        request_id = %request_id,
        extraction_enabled = config.extraction_enabled,
        batch_size = config.batch_size,
        "NER config saved to database"
    );

    Ok(HttpResponse::Ok().json(NerConfigResponse {
        status: "ok".into(),
        message: "NER config saved.".into(),
        request_id,
        config,
    }))
}

// ============================================================================
// API KEYS CONFIG
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ApiKeysRequest {
    #[serde(default)]
    pub openai_api_key: String,
    #[serde(default)]
    pub anthropic_api_key: String,
    #[serde(default)]
    pub openrouter_api_key: String,
}

// ============================================================================
// NEO4J KNOWLEDGE GRAPH CONFIG (Phase 27)
// ============================================================================

#[derive(Debug, Serialize)]
pub(crate) struct Neo4jConfigResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub feature_compiled: bool,
    pub enabled: bool,
    pub connected: bool,
    pub uri: String,
    pub user: String,
    pub database: String,
    pub max_connections: usize,
    pub connection_timeout_ms: u64,
    // Graph expansion settings
    pub expansion_enabled: bool,
    pub max_hops: usize,
    pub max_chunks: usize,
    pub entity_weight: f32,
    pub concept_weight: f32,
    pub min_relationship_strength: f32,
    // Entity extraction settings
    pub extraction_enabled: bool,
    pub confidence_threshold: f32,
    pub fuzzy_threshold: f32,
    // Stats (if connected)
    pub stats: Option<Neo4jStats>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Neo4jStats {
    pub total_nodes: usize,
    pub total_relationships: usize,
    pub documents: usize,
    pub chunks: usize,
    pub entities: usize,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct Neo4jConfigRequest {
    pub enabled: Option<bool>,
    pub uri: Option<String>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub database: Option<String>,
    pub max_connections: Option<usize>,
    pub connection_timeout_ms: Option<u64>,
    // Graph expansion
    pub expansion_enabled: Option<bool>,
    pub max_hops: Option<usize>,
    pub max_chunks: Option<usize>,
    pub entity_weight: Option<f32>,
    pub concept_weight: Option<f32>,
    pub min_relationship_strength: Option<f32>,
    // Entity extraction
    pub extraction_enabled: Option<bool>,
    pub confidence_threshold: Option<f32>,
    pub fuzzy_threshold: Option<f32>,
}

pub(crate) async fn get_neo4j_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    let feature_compiled = crate::graph::is_neo4j_compiled();
    let config = crate::graph::config::GraphConfig::from_env();

    // Check if connected
    #[cfg(feature = "neo4j")]
    let (connected, stats) = {
        if let Some(client) = get_neo4j_client() {
            match client.health_check().await {
                Ok(true) => {
                    // Get stats
                    let stats = client.get_stats().await.ok().map(|s| Neo4jStats {
                        total_nodes: s.total_nodes,
                        total_relationships: s.total_relationships,
                        documents: *s.node_counts.get("Document").unwrap_or(&0),
                        chunks: *s.node_counts.get("Chunk").unwrap_or(&0),
                        entities: *s.node_counts.get("Entity").unwrap_or(&0),
                    });
                    (true, stats)
                }
                _ => (false, None),
            }
        } else {
            (false, None)
        }
    };

    #[cfg(not(feature = "neo4j"))]
    let (connected, stats): (bool, Option<Neo4jStats>) = (false, None);

    Ok(HttpResponse::Ok().json(Neo4jConfigResponse {
        status: "ok".into(),
        message: if connected {
            "Connected to Neo4j".into()
        } else {
            "Not connected".into()
        },
        request_id,
        feature_compiled,
        enabled: config.enabled,
        connected,
        uri: config.uri,
        user: config.user,
        database: config.database,
        max_connections: config.max_connections,
        connection_timeout_ms: config.connection_timeout_ms,
        expansion_enabled: config.expansion.enabled,
        max_hops: config.expansion.max_hops,
        max_chunks: config.expansion.max_chunks,
        entity_weight: config.expansion.entity_weight,
        concept_weight: config.expansion.concept_weight,
        min_relationship_strength: config.expansion.min_relationship_strength,
        extraction_enabled: config.entity_extraction.enabled,
        confidence_threshold: config.entity_extraction.confidence_threshold,
        fuzzy_threshold: config.entity_extraction.fuzzy_threshold,
        stats,
    }))
}

pub(crate) async fn save_neo4j_config(
    payload: web::Json<Neo4jConfigRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let Neo4jConfigRequest {
        enabled,
        uri,
        user,
        password,
        database,
        max_connections,
        connection_timeout_ms,
        expansion_enabled,
        max_hops,
        max_chunks,
        entity_weight,
        concept_weight,
        min_relationship_strength,
        extraction_enabled,
        confidence_threshold,
        fuzzy_threshold,
    } = payload.into_inner();

    // Capture the requested changes so operators can see what was submitted even
    // though we still require editing .env + restart for them to take effect.
    let requested_changes = json!({
        "enabled": enabled,
        "uri": uri,
        "user": user,
        "password": password.as_ref().map(|_| "***"),
        "database": database,
        "max_connections": max_connections,
        "connection_timeout_ms": connection_timeout_ms,
        "expansion": {
            "enabled": expansion_enabled,
            "max_hops": max_hops,
            "max_chunks": max_chunks,
            "entity_weight": entity_weight,
            "concept_weight": concept_weight,
            "min_relationship_strength": min_relationship_strength,
        },
        "entity_extraction": {
            "enabled": extraction_enabled,
            "confidence_threshold": confidence_threshold,
            "fuzzy_threshold": fuzzy_threshold,
        }
    });

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "message": "Neo4j configuration is read from environment variables. Update .env and restart the application to apply changes.",
        "request_id": request_id,
        "restart_required": true,
        "requested_changes": requested_changes
    })))
}

pub(crate) async fn test_neo4j_connection() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    #[cfg(feature = "neo4j")]
    {
        if let Some(client) = get_neo4j_client() {
            match client.health_check().await {
                Ok(true) => {
                    return Ok(HttpResponse::Ok().json(json!({
                        "status": "ok",
                        "message": "Successfully connected to Neo4j",
                        "request_id": request_id,
                        "connected": true
                    })));
                }
                Ok(false) => {
                    return Ok(HttpResponse::Ok().json(json!({
                        "status": "error",
                        "message": "Neo4j health check failed",
                        "request_id": request_id,
                        "connected": false
                    })));
                }
                Err(e) => {
                    return Ok(HttpResponse::Ok().json(json!({
                        "status": "error",
                        "message": format!("Neo4j connection error: {}", e),
                        "request_id": request_id,
                        "connected": false
                    })));
                }
            }
        } else {
            return Ok(HttpResponse::Ok().json(json!({
                "status": "error",
                "message": "Neo4j client not initialized. Check NEO4J_ENABLED=true in .env and restart.",
                "request_id": request_id,
                "connected": false
            })));
        }
    }

    #[cfg(not(feature = "neo4j"))]
    {
        Ok(HttpResponse::Ok().json(json!({
            "status": "error",
            "message": "Neo4j feature not compiled. Build with: cargo build --features neo4j",
            "request_id": request_id,
            "connected": false,
            "feature_compiled": false
        })))
    }
}

// ============================================================================
// API KEYS CONFIG
// ============================================================================

// ── Entity Terms API types (Step 1 v1.0) ─────────────────────────────
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct EntityTermEntry {
    pub category: String,
    pub term: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct EntityTermsResponse {
    pub status: String,
    pub terms: Vec<EntityTermEntry>,
    pub file_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SaveEntityTermsRequest {
    pub terms: Vec<EntityTermEntry>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiKeysResponse {
    pub status: String,
    pub message: String,
    pub request_id: String,
    pub has_openai_key: bool,
    pub has_anthropic_key: bool,
    pub has_openrouter_key: bool,
    pub openai_key_masked: String,
    pub anthropic_key_masked: String,
    pub openrouter_key_masked: String,
    pub openai_from_env: bool,
    pub anthropic_from_env: bool,
    pub openrouter_from_env: bool,
}

pub(crate) async fn get_api_keys() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let keys = crate::db::api_keys::global_config();

    let openai_from_env = std::env::var("OPENAI_API_KEY").is_ok();
    let anthropic_from_env = std::env::var("ANTHROPIC_API_KEY").is_ok();
    let openrouter_from_env = std::env::var("OPENROUTER_API_KEY").is_ok();

    let openai_key_masked = if openai_from_env {
        "[from environment]".to_string()
    } else if !keys.openai_api_key.is_empty() {
        crate::db::api_keys::ApiKeys::mask_key(&keys.openai_api_key)
    } else {
        String::new()
    };

    let anthropic_key_masked = if anthropic_from_env {
        "[from environment]".to_string()
    } else if !keys.anthropic_api_key.is_empty() {
        crate::db::api_keys::ApiKeys::mask_key(&keys.anthropic_api_key)
    } else {
        String::new()
    };

    let openrouter_key_masked = if openrouter_from_env {
        "[from environment]".to_string()
    } else if !keys.openrouter_api_key.is_empty() {
        crate::db::api_keys::ApiKeys::mask_key(&keys.openrouter_api_key)
    } else {
        String::new()
    };

    Ok(HttpResponse::Ok().json(ApiKeysResponse {
        status: "ok".into(),
        message: "API keys status".into(),
        request_id,
        has_openai_key: keys.has_openai_key(),
        has_anthropic_key: keys.has_anthropic_key(),
        has_openrouter_key: keys.has_openrouter_key(),
        openai_key_masked,
        anthropic_key_masked,
        openrouter_key_masked,
        openai_from_env,
        anthropic_from_env,
        openrouter_from_env,
    }))
}

pub(crate) async fn save_api_keys(
    payload: web::Json<ApiKeysRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let body = payload.into_inner();

    // Get current keys and update only non-empty values
    let mut keys = crate::db::api_keys::global_config();

    if !body.openai_api_key.is_empty() {
        keys.openai_api_key = body.openai_api_key;
    }
    if !body.anthropic_api_key.is_empty() {
        keys.anthropic_api_key = body.anthropic_api_key;
    }
    if !body.openrouter_api_key.is_empty() {
        keys.openrouter_api_key = body.openrouter_api_key;
    }

    match crate::db::api_keys::update_config(keys.clone()) {
        Ok(_) => {
            tracing::info!(
                request_id = %request_id,
                has_openai = keys.has_openai_key(),
                has_anthropic = keys.has_anthropic_key(),
                has_openrouter = keys.has_openrouter_key(),
                "API keys saved"
            );

            let openai_from_env = std::env::var("OPENAI_API_KEY").is_ok();
            let anthropic_from_env = std::env::var("ANTHROPIC_API_KEY").is_ok();
            let openrouter_from_env = std::env::var("OPENROUTER_API_KEY").is_ok();

            Ok(HttpResponse::Ok().json(ApiKeysResponse {
                status: "ok".into(),
                message: "API keys saved".into(),
                request_id,
                has_openai_key: keys.has_openai_key(),
                has_anthropic_key: keys.has_anthropic_key(),
                has_openrouter_key: keys.has_openrouter_key(),
                openai_key_masked: if openai_from_env {
                    "[from environment]".to_string()
                } else if !keys.openai_api_key.is_empty() {
                    crate::db::api_keys::ApiKeys::mask_key(&keys.openai_api_key)
                } else {
                    String::new()
                },
                anthropic_key_masked: if anthropic_from_env {
                    "[from environment]".to_string()
                } else if !keys.anthropic_api_key.is_empty() {
                    crate::db::api_keys::ApiKeys::mask_key(&keys.anthropic_api_key)
                } else {
                    String::new()
                },
                openrouter_key_masked: if openrouter_from_env {
                    "[from environment]".to_string()
                } else if !keys.openrouter_api_key.is_empty() {
                    crate::db::api_keys::ApiKeys::mask_key(&keys.openrouter_api_key)
                } else {
                    String::new()
                },
                openai_from_env,
                anthropic_from_env,
                openrouter_from_env,
            }))
        }
        Err(err) => {
            tracing::error!(
                request_id = %request_id,
                error = %err,
                "Failed to save API keys"
            );
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save API keys: {}", err),
                "request_id": request_id
            })))
        }
    }
}

pub(crate) async fn delete_api_key(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let provider = path.into_inner();

    let mut keys = crate::db::api_keys::global_config();

    match provider.as_str() {
        "openai" => {
            keys.openai_api_key = String::new();
        }
        "anthropic" => {
            keys.anthropic_api_key = String::new();
        }
        _ => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "status": "error",
                "message": format!("Unknown provider: {}. Use 'openai' or 'anthropic'", provider),
                "request_id": request_id
            })));
        }
    }

    match crate::db::api_keys::update_config(keys) {
        Ok(_) => {
            tracing::info!(
                request_id = %request_id,
                provider = %provider,
                "API key deleted"
            );
            Ok(HttpResponse::Ok().json(json!({
                "status": "ok",
                "message": format!("{} API key deleted", provider),
                "request_id": request_id
            })))
        }
        Err(err) => Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": format!("Failed to delete API key: {}", err),
            "request_id": request_id
        }))),
    }
}

// ── POST /extract_entities — standalone NER test endpoint (v1.0) ─────
pub(crate) async fn extract_entities_handler(
    payload: web::Json<serde_json::Value>,
) -> Result<HttpResponse, Error> {
    let text = payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if text.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "Provide a 'text' field with content to extract entities from"
        })));
    }

    let extractor = crate::tools::entity_extractor::EntityExtractorTool::new();
    let result = extractor.extract(&text);

    let entities: Vec<serde_json::Value> = result
        .entities
        .iter()
        .map(|e| {
            json!({
                "text": e.text,
                "type": e.entity_type.label(),
                "start": e.start,
                "end": e.end,
                "confidence": e.confidence
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "entity_count": entities.len(),
        "entities": entities,
        "counts": result.entity_counts
    })))
}

// ── GET /config/entity_terms (Step 1 v1.0) ───────────────────────────
pub(crate) async fn get_entity_terms() -> Result<HttpResponse, Error> {
    let terms_path = std::env::var("AG_ENTITY_TERMS_FILE").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/pde".to_string());
        format!("{}/.config/ag/entity_terms.txt", home)
    });

    let mut terms = Vec::new();

    if let Ok(content) = std::fs::read_to_string(&terms_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((prefix, term)) = line.split_once(':') {
                let cat = prefix.trim().to_uppercase();
                let term = term.trim().to_string();
                if !term.is_empty() {
                    terms.push(EntityTermEntry {
                        category: cat,
                        term,
                    });
                }
            } else {
                terms.push(EntityTermEntry {
                    category: "TECH".to_string(),
                    term: line.to_string(),
                });
            }
        }
    }

    Ok(HttpResponse::Ok().json(EntityTermsResponse {
        status: "ok".into(),
        terms,
        file_path: terms_path,
    }))
}

// ── POST /config/entity_terms (Step 1 v1.0) ─────────────────────────
pub(crate) async fn save_entity_terms(
    payload: web::Json<SaveEntityTermsRequest>,
) -> Result<HttpResponse, Error> {
    let terms_path = std::env::var("AG_ENTITY_TERMS_FILE").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/pde".to_string());
        format!("{}/.config/ag/entity_terms.txt", home)
    });

    if let Some(parent) = std::path::Path::new(&terms_path).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Cannot create config dir: {}", e)
            })));
        }
    }

    let body = payload.into_inner();
    let valid_categories = ["MED", "TECH", "ORG", "LOC", "PERSON", "PRODUCT", "EVENT"];

    let mut lines = Vec::new();
    lines.push("# AG Entity Terms — managed via UI".to_string());
    lines.push(format!(
        "# Last saved: {}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    lines.push("# Format: CATEGORY:term".to_string());
    lines.push(String::new());

    let mut by_cat: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for entry in &body.terms {
        let cat = entry.category.trim().to_uppercase();
        let cat = if valid_categories.contains(&cat.as_str()) {
            cat
        } else {
            "TECH".to_string()
        };
        let term = entry.term.trim().to_string();
        if !term.is_empty() {
            by_cat.entry(cat).or_default().push(term);
        }
    }

    for (cat, terms) in &by_cat {
        lines.push(format!("# ── {} ──", cat));
        for term in terms {
            lines.push(format!("{}:{}", cat, term));
        }
        lines.push(String::new());
    }

    let content = lines.join("\n");

    match std::fs::write(&terms_path, &content) {
        Ok(_) => {
            let count: usize = by_cat.values().map(|v| v.len()).sum();
            tracing::info!(
                path = %terms_path,
                count = count,
                "Entity terms saved"
            );
            Ok(HttpResponse::Ok().json(json!({
                "status": "ok",
                "message": format!("Saved {} terms to {}", count, terms_path),
                "count": count
            })))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to save entity terms");
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to write {}: {}", terms_path, e)
            })))
        }
    }
}
