//! ONNX Runtime Embedder

use serde::Serialize;
use std::path::Path;
use tracing::info;

#[cfg(feature = "onnx")]
use ort::logging::LogLevel as OrtLogLevel;
#[cfg(feature = "onnx")]
use ort::session::Session;
#[cfg(feature = "onnx")]
use ort::value::Tensor;

pub type EmbeddingVector = Vec<f32>;

/// Graph optimization level for ONNX Runtime
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OnnxOptimizationLevel {
    /// Disable all optimizations
    Disable,
    /// Basic optimizations (constant folding, redundant node elimination)
    Basic,
    /// Extended optimizations (includes Basic + more advanced fusions)
    Extended,
    /// All optimizations enabled (includes Extended + layout optimizations)
    #[default]
    All,
}

/// Execution mode for ONNX Runtime
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OnnxExecutionMode {
    /// Execute operators sequentially (default, lower memory usage)
    #[default]
    Sequential,
    /// Execute operators in parallel (may improve performance for models with many branches)
    Parallel,
}

/// Log level wrapper so we do not leak the ort type into API structs
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OnnxLogLevel {
    /// Verbose logging (most chatty)
    Verbose,
    /// Information messages
    #[default]
    Info,
    /// Warnings only
    Warning,
    /// Errors only
    Error,
    /// Fatal only
    Fatal,
}

#[cfg(feature = "onnx")]
impl From<OnnxLogLevel> for OrtLogLevel {
    fn from(level: OnnxLogLevel) -> Self {
        match level {
            OnnxLogLevel::Verbose => OrtLogLevel::Verbose,
            OnnxLogLevel::Info => OrtLogLevel::Info,
            OnnxLogLevel::Warning => OrtLogLevel::Warning,
            OnnxLogLevel::Error => OrtLogLevel::Error,
            OnnxLogLevel::Fatal => OrtLogLevel::Fatal,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OnnxConfig {
    pub model_path: String,
    pub max_length: usize,
    pub embedding_dim: usize,
    /// Number of threads for intra-op parallelism (within operators)
    pub num_threads: usize,
    /// Number of threads for inter-op parallelism (across operators, only used in Parallel mode)
    pub inter_op_num_threads: usize,
    /// Graph optimization level
    pub optimization_level: OnnxOptimizationLevel,
    /// Execution mode (Sequential or Parallel)
    pub execution_mode: OnnxExecutionMode,
    /// Enable memory pattern optimization
    pub enable_mem_pattern: bool,
    /// Enable CPU memory arena
    pub enable_cpu_mem_arena: bool,
    /// Force deterministic kernels when available
    pub deterministic_compute: bool,
    /// Optional path to write the optimized model
    pub optimized_model_path: Option<String>,
    /// Enable profiling (writes chrome trace JSON)
    pub enable_profiling: bool,
    /// Optional path for profiling output
    pub profiling_output_path: Option<String>,
    /// Optional custom logger id
    pub log_id: Option<String>,
    /// Minimum severity for ONNX Runtime logs
    pub log_level: OnnxLogLevel,
    /// Verbosity for verbose logging (>=0)
    pub log_verbosity: i32,
    /// Use environment allocators instead of session-specific ones
    pub use_env_allocators: bool,
    /// Enable flush-to-zero and denormal-as-zero
    pub denormal_as_zero: bool,
    /// Enable Quantize/Dequantize fusion optimizations
    pub enable_quant_qdq: bool,
    /// Enable the pass that removes double QDQ nodes
    pub enable_double_qdq_remover: bool,
    /// Remove QDQ nodes once processing completes
    pub enable_qdq_cleanup: bool,
    /// Enable GELU approximation for faster inference
    pub approximate_gelu: bool,
    /// Enable ahead-of-time function inlining
    pub enable_aot_inlining: bool,
    /// Disable specific graph optimizers by name
    pub disabled_optimizers: Vec<String>,
    /// Use device allocator when initializing tensors
    pub use_device_allocator_for_initializers: bool,
    /// Allow inter-op threads to spin briefly before blocking
    pub allow_inter_op_spinning: bool,
    /// Allow intra-op threads to spin briefly before blocking
    pub allow_intra_op_spinning: bool,
    /// Enable/disable prepacking optimizations
    pub use_prepacking: bool,
    /// Use an independent thread pool per session
    pub independent_thread_pool: bool,
    /// Stop inheriting execution providers registered on the Environment
    pub no_env_execution_providers: bool,
}

impl Default for OnnxConfig {
    fn default() -> Self {
        Self {
            model_path: "models/embedding_model.onnx".to_string(),
            max_length: 512,
            embedding_dim: 384,
            num_threads: 4,
            inter_op_num_threads: 1,
            optimization_level: OnnxOptimizationLevel::All,
            execution_mode: OnnxExecutionMode::Sequential,
            enable_mem_pattern: true,
            enable_cpu_mem_arena: true,
            deterministic_compute: false,
            optimized_model_path: None,
            enable_profiling: false,
            profiling_output_path: None,
            log_id: None,
            log_level: OnnxLogLevel::Info,
            log_verbosity: 0,
            use_env_allocators: false,
            denormal_as_zero: false,
            enable_quant_qdq: true,
            enable_double_qdq_remover: true,
            enable_qdq_cleanup: false,
            approximate_gelu: false,
            enable_aot_inlining: true,
            disabled_optimizers: Vec::new(),
            use_device_allocator_for_initializers: false,
            allow_inter_op_spinning: true,
            allow_intra_op_spinning: true,
            use_prepacking: true,
            independent_thread_pool: false,
            no_env_execution_providers: false,
        }
    }
}

#[derive(Debug)]
pub enum OnnxError {
    ModelNotFound(String),
    SessionCreationFailed(String),
    InferenceFailed(String),
}

impl std::fmt::Display for OnnxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelNotFound(p) => write!(f, "Model not found: {}", p),
            Self::SessionCreationFailed(e) => write!(f, "Session failed: {}", e),
            Self::InferenceFailed(e) => write!(f, "Inference failed: {}", e),
        }
    }
}

impl std::error::Error for OnnxError {}

pub struct SimpleTokenizer {
    max_length: usize,
}

impl SimpleTokenizer {
    pub fn new(max_length: usize) -> Self {
        Self { max_length }
    }

    pub fn encode_i64(&self, text: &str) -> (Vec<i64>, Vec<i64>) {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut input_ids = vec![101i64]; // CLS
        let mut attention_mask = vec![1i64];

        for word in words.iter().take(self.max_length - 2) {
            let hash = seahash::hash(word.as_bytes());
            input_ids.push((hash % 29522 + 1000) as i64);
            attention_mask.push(1);
        }

        input_ids.push(102); // SEP
        attention_mask.push(1);

        input_ids.resize(self.max_length, 0);
        attention_mask.resize(self.max_length, 0);

        (input_ids, attention_mask)
    }
}

/// Tokenizer used for text encoding inside the ONNX embedder.
///
/// When a `tokenizer.json` is found next to the ONNX model file the real
/// HuggingFace tokenizer is used, producing proper sub-word encodings that
/// match what the model was trained on. If the file is absent the simple
/// hash-based fallback is used (lower quality but no extra files needed).
enum OnnxTokenizer {
    /// HuggingFace `tokenizers` — requires `tokenizer.json` alongside the model.
    Real(Box<tokenizers::Tokenizer>),
    /// Fallback: seahash word-to-id mapping (no vocabulary file needed).
    Simple(SimpleTokenizer),
}

impl OnnxTokenizer {
    /// Encode a single text into (input_ids, attention_mask), both padded /
    /// truncated to `max_length`.
    fn encode(&self, text: &str, max_length: usize) -> (Vec<i64>, Vec<i64>) {
        match self {
            OnnxTokenizer::Real(tok) => match tok.encode(text, true) {
                Ok(enc) => {
                    let mut ids: Vec<i64> = enc.get_ids().iter().map(|&x| x as i64).collect();
                    let mut mask: Vec<i64> =
                        enc.get_attention_mask().iter().map(|&x| x as i64).collect();
                    ids.truncate(max_length);
                    mask.truncate(max_length);
                    ids.resize(max_length, 0);
                    mask.resize(max_length, 0);
                    (ids, mask)
                }
                Err(e) => {
                    tracing::warn!("Tokenization failed: {e}; returning blank encoding");
                    (vec![0i64; max_length], vec![0i64; max_length])
                }
            },
            OnnxTokenizer::Simple(st) => st.encode_i64(text),
        }
    }
}

#[cfg(feature = "onnx")]
pub struct OnnxEmbedder {
    config: OnnxConfig,
    tokenizer: OnnxTokenizer,
    /// Whether the session's input list includes `token_type_ids`.
    needs_token_type_ids: bool,
    session: Session,
}

#[cfg(not(feature = "onnx"))]
pub struct OnnxEmbedder {
    config: OnnxConfig,
    tokenizer: OnnxTokenizer,
}

#[cfg(feature = "onnx")]
use ort::session::builder::GraphOptimizationLevel;

#[cfg(feature = "onnx")]
impl OnnxEmbedder {
    pub fn new(config: OnnxConfig) -> Result<Self, OnnxError> {
        eprintln!("[ONNX] OnnxEmbedder::new called");

        if !Path::new(&config.model_path).exists() {
            eprintln!("[ONNX] Model not found: {}", config.model_path);
            return Err(OnnxError::ModelNotFound(config.model_path.clone()));
        }

        eprintln!("[ONNX] Model file exists, initializing...");

        // ── Tokenizer ───────────────────────────────────────────────────────
        // Look for tokenizer.json in the same directory as the ONNX model.
        let tokenizer = {
            let model_dir = Path::new(&config.model_path)
                .parent()
                .unwrap_or_else(|| Path::new("."));
            let tok_path = model_dir.join("tokenizer.json");
            if tok_path.exists() {
                match tokenizers::Tokenizer::from_file(&tok_path) {
                    Ok(t) => {
                        eprintln!(
                            "[ONNX] Loaded real HuggingFace tokenizer from {:?}",
                            tok_path
                        );
                        OnnxTokenizer::Real(Box::new(t))
                    }
                    Err(e) => {
                        eprintln!(
                            "[ONNX] tokenizer.json found but failed to load ({e}); \
                             falling back to SimpleTokenizer"
                        );
                        OnnxTokenizer::Simple(SimpleTokenizer::new(config.max_length))
                    }
                }
            } else {
                eprintln!(
                    "[ONNX] No tokenizer.json at {:?} — using SimpleTokenizer \
                     (degraded quality). Place tokenizer.json next to the ONNX \
                     model for proper sub-word encoding.",
                    tok_path
                );
                OnnxTokenizer::Simple(SimpleTokenizer::new(config.max_length))
            }
        };

        info!(model = %config.model_path, "Initializing ONNX embedder");

        eprintln!("[ONNX] Calling ort::init()...");
        let _ = ort::init().with_name("ag").commit();
        eprintln!("[ONNX] ort::init() complete");

        // Convert our optimization level to ort's GraphOptimizationLevel
        let opt_level = match config.optimization_level {
            OnnxOptimizationLevel::Disable => GraphOptimizationLevel::Disable,
            OnnxOptimizationLevel::Basic => GraphOptimizationLevel::Level1,
            OnnxOptimizationLevel::Extended => GraphOptimizationLevel::Level2,
            OnnxOptimizationLevel::All => GraphOptimizationLevel::Level3,
        };

        eprintln!("[ONNX] Creating Session::builder()...");
        eprintln!("[ONNX] SessionOptions: intra_threads={}, inter_threads={}, opt_level={:?}, mem_pattern={}, cpu_arena={}",
            config.num_threads, config.inter_op_num_threads, config.optimization_level,
            config.enable_mem_pattern, config.enable_cpu_mem_arena);

        let mut builder =
            Session::builder().map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;

        // Configure threading + execution basics
        builder = builder
            .with_intra_threads(config.num_threads)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        builder = builder
            .with_inter_threads(config.inter_op_num_threads)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        builder = builder
            .with_parallel_execution(matches!(config.execution_mode, OnnxExecutionMode::Parallel))
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;

        // Optimization & layout
        builder = builder
            .with_optimization_level(opt_level)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        if let Some(path) = &config.optimized_model_path {
            builder = builder
                .with_optimized_model_path(path)
                .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        }
        builder = builder
            .with_memory_pattern(config.enable_mem_pattern)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        builder = builder
            .with_deterministic_compute(config.deterministic_compute)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        // with_denormal_as_zero was removed in ORT rc.12; skip if not available.
        let _ = config.denormal_as_zero;
        builder = builder
            .with_quant_qdq(config.enable_quant_qdq)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        builder = builder
            .with_double_qdq_remover(config.enable_double_qdq_remover)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        if config.enable_qdq_cleanup {
            builder = builder
                .with_qdq_cleanup()
                .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        }
        if config.approximate_gelu {
            builder = builder
                .with_approximate_gelu()
                .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        }
        builder = builder
            .with_aot_inlining(config.enable_aot_inlining)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        if !config.disabled_optimizers.is_empty() {
            let disabled = config.disabled_optimizers.join(",");
            builder = builder
                .with_disabled_optimizers(disabled.as_str())
                .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        }
        if config.use_device_allocator_for_initializers {
            builder = builder
                .with_device_allocated_initializers()
                .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        }
        builder = builder
            .with_inter_op_spinning(config.allow_inter_op_spinning)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        builder = builder
            .with_intra_op_spinning(config.allow_intra_op_spinning)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        builder = builder
            .with_prepacking(config.use_prepacking)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        if config.independent_thread_pool {
            builder = builder
                .with_independent_thread_pool()
                .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        }
        if config.no_env_execution_providers {
            builder = builder
                .with_no_environment_execution_providers()
                .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        }
        if config.use_env_allocators {
            builder = builder
                .with_env_allocators()
                .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        }

        // Profiling
        if config.enable_profiling {
            let path = config
                .profiling_output_path
                .clone()
                .unwrap_or_else(|| "onnx_profile.json".to_string());
            builder = builder
                .with_profiling(path)
                .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        }
        if let Some(log_id) = &config.log_id {
            builder = builder
                .with_log_id(log_id)
                .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        }
        builder = builder
            .with_log_level(config.log_level.into())
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;
        builder = builder
            .with_log_verbosity(config.log_verbosity)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;

        // Commit the session from file
        let session = builder
            .commit_from_file(&config.model_path)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;

        // Detect whether this model expects token_type_ids as an input
        let needs_token_type_ids = session
            .inputs()
            .iter()
            .any(|inp| inp.name() == "token_type_ids");

        eprintln!(
            "[ONNX] Session created (token_type_ids: {})",
            needs_token_type_ids
        );
        info!(
            optimization_level = ?config.optimization_level,
            intra_threads = config.num_threads,
            inter_threads = config.inter_op_num_threads,
            token_type_ids = needs_token_type_ids,
            "ONNX session created"
        );

        Ok(Self {
            tokenizer,
            config,
            needs_token_type_ids,
            session,
        })
    }

    pub fn embed(&mut self, texts: &[&str]) -> Result<Vec<EmbeddingVector>, OnnxError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = texts.len();
        let seq_len = self.config.max_length;

        // Tokenize
        let mut all_input_ids = Vec::with_capacity(batch_size * seq_len);
        let mut all_attention_mask = Vec::with_capacity(batch_size * seq_len);

        for text in texts {
            let (ids, mask) = self.tokenizer.encode(text, seq_len);
            all_input_ids.extend(ids);
            all_attention_mask.extend(mask);
        }

        let shape = vec![batch_size as i64, seq_len as i64];

        // Create tensors using Tensor::from_array
        let input_ids_tensor = Tensor::from_array((shape.clone(), all_input_ids))
            .map_err(|e| OnnxError::InferenceFailed(e.to_string()))?;

        let attention_mask_tensor = Tensor::from_array((shape.clone(), all_attention_mask))
            .map_err(|e| OnnxError::InferenceFailed(e.to_string()))?;

        // Run inference — conditionally include token_type_ids (all zeros)
        let outputs = if self.needs_token_type_ids {
            let token_type_ids_tensor =
                Tensor::from_array((shape, vec![0i64; batch_size * seq_len]))
                    .map_err(|e| OnnxError::InferenceFailed(e.to_string()))?;
            self.session
                .run(ort::inputs![
                    "input_ids" => input_ids_tensor,
                    "attention_mask" => attention_mask_tensor,
                    "token_type_ids" => token_type_ids_tensor
                ])
                .map_err(|e| OnnxError::InferenceFailed(e.to_string()))?
        } else {
            self.session
                .run(ort::inputs![
                    "input_ids" => input_ids_tensor,
                    "attention_mask" => attention_mask_tensor
                ])
                .map_err(|e| OnnxError::InferenceFailed(e.to_string()))?
        };

        // Get first output
        let output = &outputs[0];
        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| OnnxError::InferenceFailed(e.to_string()))?;

        let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();

        // Extract embeddings
        let embeddings = match dims.as_slice() {
            [b, _s, h] => {
                // [batch, seq, hidden] - take CLS token
                (0..*b)
                    .map(|i| {
                        let start = i * dims[1] * dims[2];
                        data[start..start + *h].to_vec()
                    })
                    .collect()
            }
            [b, h] => {
                // [batch, hidden]
                (0..*b)
                    .map(|i| {
                        let start = i * *h;
                        data[start..start + *h].to_vec()
                    })
                    .collect()
            }
            _ => return Err(OnnxError::InferenceFailed(format!("Bad shape: {:?}", dims))),
        };

        Ok(embeddings)
    }

    pub fn embed_one(&mut self, text: &str) -> Result<EmbeddingVector, OnnxError> {
        self.embed(&[text]).map(|mut v| v.pop().unwrap_or_default())
    }

    pub fn dimension(&self) -> usize {
        self.config.embedding_dim
    }
    pub fn model_path(&self) -> &str {
        &self.config.model_path
    }
}

#[cfg(not(feature = "onnx"))]
impl OnnxEmbedder {
    pub fn new(config: OnnxConfig) -> Result<Self, OnnxError> {
        if !Path::new(&config.model_path).exists() {
            return Err(OnnxError::ModelNotFound(config.model_path.clone()));
        }
        warn!("ONNX feature not enabled");
        Ok(Self {
            tokenizer: OnnxTokenizer::Simple(SimpleTokenizer::new(config.max_length)),
            config,
        })
    }

    pub fn embed(&self, texts: &[&str]) -> Result<Vec<EmbeddingVector>, OnnxError> {
        Ok(texts
            .iter()
            .map(|t| {
                let h = seahash::hash(t.as_bytes());
                let mut v = vec![0f32; self.config.embedding_dim];
                for i in 0..v.len() {
                    v[i] = ((seahash::hash(&[h.to_le_bytes(), (i as u64).to_le_bytes()].concat())
                        as f32)
                        / u64::MAX as f32)
                        * 2.0
                        - 1.0;
                }
                let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                if n > 0.0 {
                    v.iter_mut().for_each(|x| *x /= n);
                }
                v
            })
            .collect())
    }

    pub fn embed_one(&self, text: &str) -> Result<EmbeddingVector, OnnxError> {
        self.embed(&[text]).map(|mut v| v.pop().unwrap_or_default())
    }

    pub fn dimension(&self) -> usize {
        self.config.embedding_dim
    }
    pub fn model_path(&self) -> &str {
        &self.config.model_path
    }
}

pub fn is_onnx_enabled() -> bool {
    cfg!(feature = "onnx")
}
