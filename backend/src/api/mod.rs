use crate::agent::{Agent, AgentResponse, AgentStep};
use crate::agent_memory::{AgentMemory, MemoryItem, MemorySearchResult};
use crate::config::ApiConfig;
use crate::db::chunk_settings;
use crate::db::llm_settings::{self, LlmConfig};
use crate::db::path_resolver;
use crate::index;
use crate::memory::agent::GoalStatus;
use crate::memory::chunker::ChunkerConfig;
use crate::monitoring::config::MonitoringConfig;
use crate::monitoring::metrics;
use crate::monitoring::rate_limit_middleware::{MatchKind, RateLimitOptions, RouteRule};
use crate::retriever::Retriever;
use crate::security::rate_limiter::{RateLimiter, RateLimiterState, RuntimeThresholds};
use crate::tools::calculator::CalculatorTool;
use crate::tools::entity_extractor::EntityExtractorTool;
use crate::tools::memory_tool::MemoryTool;
use crate::tools::scheduler::SchedulerTool;
use crate::tools::sentiment::SentimentAnalyzerTool;
use crate::tools::spell_checker::SpellCheckerTool;
use crate::tools::translator::TranslatorTool;
use crate::tools::url_fetch::URLFetchTool;
use crate::tools::web_search::WebSearchTool;
use crate::tools::Tool;
use actix_cors::Cors;
use actix_multipart::Multipart;
use actix_web::http::header::AUTHORIZATION;
use actix_web::{error, http::StatusCode, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Instant, SystemTime};
use tokio::time::{sleep, Duration};
use tracing::{error, info, info_span, warn};
use uuid::Uuid;

pub(crate) mod helpers;
use helpers::*;
pub(crate) mod agent_chat;
pub(crate) mod config_routes;
pub(crate) mod corpus_routes;
pub(crate) mod docker;
pub(crate) mod memory_routes;
pub(crate) mod monitor_routes;
pub(crate) mod training;
pub(crate) mod upload_search;
use agent_chat::*;
use config_routes::*;
use corpus_routes::*;
use docker::*;
use memory_routes::*;
use monitor_routes::*;
use training::*;
use upload_search::*;

/// Legacy relative path — kept so external tooling and tests that reference
/// `UPLOAD_DIR` by name still compile. At runtime, always use `default_upload_dir()`.
pub const UPLOAD_DIR: &str = "documents";

static DEFAULT_UPLOAD_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Set the upload directory for the default corpus.
/// Called once at startup (after the file migration) with the PathManager-derived path.
/// Creates the directory if it does not exist.
pub fn set_default_upload_dir(path: PathBuf) {
    let _ = std::fs::create_dir_all(&path);
    let _ = DEFAULT_UPLOAD_DIR.set(path);
}

/// Return the upload directory for the default corpus as a `String`.
/// Falls back to the legacy `"documents"` relative path if `set_default_upload_dir`
/// has not yet been called (e.g. in unit tests).
pub fn default_upload_dir() -> String {
    DEFAULT_UPLOAD_DIR
        .get()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| UPLOAD_DIR.to_string())
}

// Phase 15: Global reindex concurrency guard
static REINDEX_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Check if reindex is currently in progress
pub fn is_reindex_in_progress() -> bool {
    REINDEX_IN_PROGRESS.load(Ordering::SeqCst)
}

// Phase 15: Async job tracking
#[derive(Clone, Debug, serde::Serialize)]
struct AsyncJob {
    job_id: String,
    status: String, // "pending", "running", "completed", "failed"
    started_at: String,
    completed_at: Option<String>,
    vectors_indexed: Option<usize>,
    mappings_indexed: Option<usize>,
    error: Option<String>,
}

static ASYNC_JOBS: OnceLock<Arc<Mutex<HashMap<String, AsyncJob>>>> = OnceLock::new();

fn get_jobs_map() -> Arc<Mutex<HashMap<String, AsyncJob>>> {
    ASYNC_JOBS
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

// Global retriever handle
static RETRIEVER: OnceLock<Arc<Mutex<Retriever>>> = OnceLock::new();

// Global rate limiters — each server has its own LRU but shares RuntimeThresholds
static SHARED_RATE_LIMITER: OnceLock<Arc<RateLimiter>> = OnceLock::new();
static UPLOAD_RATE_LIMITER: OnceLock<Arc<RateLimiter>> = OnceLock::new();

pub fn get_upload_rate_limiter() -> Option<Arc<RateLimiter>> {
    UPLOAD_RATE_LIMITER.get().map(Arc::clone)
}

// Global upload concurrency semaphore — shared so /ready can query available permits
static UPLOAD_SEMAPHORE: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();

pub fn get_upload_semaphore() -> Option<Arc<tokio::sync::Semaphore>> {
    UPLOAD_SEMAPHORE.get().map(Arc::clone)
}

pub fn get_rate_limiter() -> Option<Arc<RateLimiter>> {
    SHARED_RATE_LIMITER.get().map(Arc::clone)
}

// Global EmbeddingService handle for cached query embedding
static EMBEDDING_SERVICE: OnceLock<Arc<crate::embedder::EmbeddingService>> = OnceLock::new();

pub fn set_embedding_service(svc: Arc<crate::embedder::EmbeddingService>) {
    let _ = EMBEDDING_SERVICE.set(svc);
}

pub fn get_embedding_service() -> Option<Arc<crate::embedder::EmbeddingService>> {
    EMBEDDING_SERVICE.get().map(Arc::clone)
}

// Global TokenCounterHandle for exact token counting from GGUF vocab
static TOKEN_COUNTER: OnceLock<Arc<crate::gguf_tokenizer::TokenCounterHandle>> = OnceLock::new();
pub fn set_token_counter(handle: Arc<crate::gguf_tokenizer::TokenCounterHandle>) {
    let _ = TOKEN_COUNTER.set(handle);
}
pub fn get_token_counter() -> Option<Arc<crate::gguf_tokenizer::TokenCounterHandle>> {
    TOKEN_COUNTER.get().map(Arc::clone)
}

pub fn set_retriever_handle(handle: Arc<Mutex<Retriever>>) {
    let _ = RETRIEVER.set(handle);
}

pub fn get_retriever_handle() -> Option<Arc<Mutex<Retriever>>> {
    RETRIEVER.get().map(Arc::clone)
}

/// Look up a retriever for a specific corpus slug.
/// Falls back to the global RETRIEVER for "default" when the registry is not yet populated.
pub fn get_corpus_retriever(slug: &str) -> Option<Arc<Mutex<Retriever>>> {
    if let Some(reg) = crate::corpus_registry::get_registry() {
        if let Some(h) = reg.get(slug) {
            return Some(h);
        }
    }
    if slug == "default" {
        return get_retriever_handle();
    }
    None
}

// Global Neo4j client handle (Phase 27)
#[cfg(feature = "neo4j")]
static NEO4J_CLIENT: std::sync::RwLock<Option<crate::graph::Neo4jClient>> =
    std::sync::RwLock::new(None);

#[cfg(feature = "neo4j")]
pub fn set_neo4j_client(client: crate::graph::Neo4jClient) {
    let mut lock = NEO4J_CLIENT.write().expect("NEO4J_CLIENT lock poisoned");
    *lock = Some(client);
}

#[cfg(feature = "neo4j")]
pub fn get_neo4j_client() -> Option<std::sync::Arc<crate::graph::Neo4jClient>> {
    NEO4J_CLIENT
        .read()
        .ok()?
        .as_ref()
        .map(|c| std::sync::Arc::new(c.clone()))
}

#[cfg(not(feature = "neo4j"))]
pub fn set_neo4j_client(_client: ()) {
    // No-op when neo4j feature is disabled
}

#[cfg(not(feature = "neo4j"))]
pub fn get_neo4j_client() -> Option<()> {
    None
}

// Global KnowledgeBuilder for graph integration during indexing
#[cfg(feature = "neo4j")]
static KNOWLEDGE_BUILDER: std::sync::RwLock<
    Option<std::sync::Arc<crate::graph::KnowledgeBuilder>>,
> = std::sync::RwLock::new(None);

#[cfg(feature = "neo4j")]
pub fn set_knowledge_builder(builder: std::sync::Arc<crate::graph::KnowledgeBuilder>) {
    let mut lock = KNOWLEDGE_BUILDER
        .write()
        .expect("KNOWLEDGE_BUILDER lock poisoned");
    *lock = Some(builder);
}

#[cfg(feature = "neo4j")]
pub fn get_knowledge_builder() -> Option<std::sync::Arc<crate::graph::KnowledgeBuilder>> {
    KNOWLEDGE_BUILDER.read().ok()?.clone()
}

#[cfg(not(feature = "neo4j"))]
pub fn get_knowledge_builder() -> Option<()> {
    None
}

/// Process a document and its chunks through the knowledge graph
/// This extracts entities and stores them in Neo4j
#[cfg(feature = "neo4j")]
pub async fn index_to_knowledge_graph(
    doc_id: &str,
    title: &str,
    source: &str,
    chunks: &[(String, String)], // (chunk_id, chunk_content)
) {
    use crate::graph::knowledge_builder::{ChunkMeta, DocumentMeta};
    use crate::tools::entity_extractor::EntityExtractorTool;
    use tracing::{debug, warn};

    let Some(kb) = get_knowledge_builder() else {
        return;
    };

    // Check if entity extraction is enabled
    let config = crate::graph::config::GraphConfig::from_env();
    if !config.entity_extraction.enabled {
        debug!("Entity extraction disabled, skipping knowledge graph indexing");
        return;
    }

    // Add document to graph
    let doc_meta = DocumentMeta {
        id: doc_id.to_string(),
        title: title.to_string(),
        source: source.to_string(),
        content_hash: {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            title.hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        },
        mime_type: "text/plain".to_string(),
        chunk_count: chunks.len(),
    };

    if let Err(e) = kb.add_document(&doc_meta).await {
        warn!(error = %e, doc_id = %doc_id, "Failed to add document to knowledge graph");
        return;
    }

    // Process each chunk
    let extractor = EntityExtractorTool::new();
    let confidence_threshold = config.entity_extraction.confidence_threshold;
    let ner_batch_size = crate::db::ner_settings::global_config().batch_size.max(1);

    // Phase 1: add all chunks to the graph, collect the ones that succeeded.
    let mut valid: Vec<(&str, &str)> = Vec::with_capacity(chunks.len());
    for (chunk_id, chunk_content) in chunks {
        tokio::task::yield_now().await;
        let chunk_meta = ChunkMeta {
            id: chunk_id.clone(),
            document_id: doc_id.to_string(),
            content: chunk_content.clone(),
            embedding_id: chunk_id.clone(),
            position: chunk_id
                .split('#')
                .next_back()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            token_count: chunk_content.split_whitespace().count(),
        };
        if let Err(e) = kb.add_chunk(&chunk_meta).await {
            warn!(error = %e, chunk_id = %chunk_id, "Failed to add chunk to knowledge graph");
        } else {
            valid.push((chunk_id, chunk_content));
        }
    }

    // Phase 2: batched NER — one ONNX call per batch of ner_batch_size chunks.
    let texts: Vec<&str> = valid.iter().map(|(_, c)| *c).collect();
    let ner_results: Vec<Vec<crate::tools::ner_extractor::NerEntity>> = texts
        .chunks(ner_batch_size)
        .flat_map(crate::tools::ner_extractor::extract_entities_batch)
        .collect();

    // Phase 3: entity mentions, regex fallback, co-occurrence links.
    for (i, (chunk_id, chunk_content)) in valid.iter().enumerate() {
        let ner_entities = &ner_results[i];
        let use_ner = !ner_entities.is_empty();
        if use_ner {
            for ner_entity in ner_entities {
                if let Err(e) = kb
                    .add_entity_mention(
                        chunk_id,
                        &ner_entity.text,
                        &ner_entity.label,
                        ner_entity.score,
                    )
                    .await
                {
                    debug!(error = %e, entity = %ner_entity.text, "Failed to add NER entity");
                }
            }
        }
        let extraction = extractor.extract(chunk_content);
        for entity in &extraction.entities {
            if !use_ner && entity.confidence >= confidence_threshold {
                if let Err(e) = kb
                    .add_entity_mention(
                        chunk_id,
                        &entity.text,
                        entity.entity_type.label(),
                        entity.confidence,
                    )
                    .await
                {
                    debug!(error = %e, entity = %entity.text, "Failed to add entity mention");
                }
            }
        }
        let high_confidence_entities: Vec<_> = extraction
            .entities
            .iter()
            .filter(|e| e.confidence >= confidence_threshold)
            .collect();
        for i in 0..high_confidence_entities.len() {
            for j in (i + 1)..high_confidence_entities.len() {
                let e1 = &high_confidence_entities[i];
                let e2 = &high_confidence_entities[j];
                let _ = kb
                    .link_entities(
                        &e1.text,
                        &e2.text,
                        "co_occurs_with",
                        (e1.confidence + e2.confidence) / 2.0,
                    )
                    .await;
            }
        }
    }

    debug!(
        doc_id = %doc_id,
        chunks = chunks.len(),
        "Indexed document to knowledge graph"
    );
}

#[cfg(not(feature = "neo4j"))]
pub async fn index_to_knowledge_graph(
    _doc_id: &str,
    _title: &str,
    _source: &str,
    _chunks: &[(String, String)],
) {
    // No-op when neo4j feature is disabled
}

static CHAT_STATE: OnceLock<Arc<Mutex<AgentChatState>>> = OnceLock::new();

const DEFAULT_LOG_LIMIT: usize = 200;
const MAX_LOG_LIMIT: usize = 500;

impl RateLimitSharedState {
    fn config_snapshot(&self, enabled: bool) -> RateLimitConfigSnapshot {
        let t = self.limiter.thresholds.snapshot();
        RateLimitConfigSnapshot {
            enabled,
            trust_proxy: self.opts.trust_proxy,
            search_qps: t.search_qps,
            search_burst: t.search_burst,
            upload_qps: t.upload_qps,
            upload_burst: t.upload_burst,
            exempt_prefixes: self.opts.exempt_prefixes.clone(),
            rules: self.opts.rules.clone(),
        }
    }
}

impl From<&ChunkerConfig> for ChunkerConfigSnapshot {
    fn from(cfg: &ChunkerConfig) -> Self {
        Self {
            target_size: cfg.target_size,
            min_size: cfg.min_size,
            max_size: cfg.max_size,
            overlap: cfg.overlap,
            semantic_similarity_threshold: cfg.semantic_similarity_threshold,
            mode: cfg.mode.clone(),
            clean_html: cfg.clean_html,
            clean_unicode: cfg.clean_unicode,
            context_prefix_enabled: cfg.context_prefix_enabled,
            context_prefix_tokens: cfg.context_prefix_tokens,
            pipeline_stages: cfg.pipeline_stages.clone(),
            snap_tolerance: cfg.snap_tolerance,
        }
    }
}

impl Default for HardwareConfigRequest {
    fn default() -> Self {
        crate::db::param_hardware::HardwareParams::default().into()
    }
}

impl From<crate::db::param_hardware::HardwareParams> for HardwareConfigRequest {
    fn from(params: crate::db::param_hardware::HardwareParams) -> Self {
        Self {
            backend_type: backend_type_to_string(&params.backend_type),
            model: params.model,

            // Model params
            gpu_layers: params.gpu_layers,
            main_gpu: params.main_gpu,
            split_mode: params.split_mode,
            tensor_split: params.tensor_split,
            use_mmap: params.use_mmap,
            use_mlock: params.use_mlock,
            vocab_only: params.vocab_only,
            devices: params.devices,
            kv_overrides: params.kv_overrides,
            swa_full: params.swa_full,
            no_perf: params.no_perf,

            // Context params
            num_ctx: params.num_ctx,
            num_batch: params.num_batch,
            num_ubatch: params.num_ubatch,
            num_seq_max: params.num_seq_max,
            rope_scaling_type: params.rope_scaling_type,
            rope_frequency_base: params.rope_frequency_base,
            rope_frequency_scale: params.rope_frequency_scale,
            yarn_ext_factor: params.yarn_ext_factor,
            yarn_attn_factor: params.yarn_attn_factor,
            yarn_beta_fast: params.yarn_beta_fast,
            yarn_beta_slow: params.yarn_beta_slow,
            yarn_orig_ctx: params.yarn_orig_ctx,
            pooling_type: params.pooling_type,
            attention_type: params.attention_type,
            flash_attn: params.flash_attn,
            type_k: params.type_k,
            type_v: params.type_v,
            embeddings: params.embeddings,
            offload_kqv: params.offload_kqv,
            defrag_thold: params.defrag_thold,
            logits_all: params.logits_all,
            f16_kv: params.f16_kv,
            low_vram: params.low_vram,

            // CPU params
            num_thread: params.num_thread,
            num_thread_batch: params.num_thread_batch,
            numa: params.numa,
            cpu_strict: params.cpu_strict,
            cpumask: params.cpumask,
            mask_valid: params.mask_valid,
            poll: params.poll,
            priority: params.priority,

            // Legacy/custom
            num_gpu: params.num_gpu,
            llama_server_url: params.llama_server_url,
        }
    }
}

impl From<HardwareConfigRequest> for crate::db::param_hardware::HardwareParams {
    fn from(req: HardwareConfigRequest) -> Self {
        Self {
            backend_type: string_to_backend_type(&req.backend_type),
            model: req.model,

            // Model params
            gpu_layers: req.gpu_layers,
            main_gpu: req.main_gpu,
            split_mode: req.split_mode,
            tensor_split: req.tensor_split,
            use_mmap: req.use_mmap,
            use_mlock: req.use_mlock,
            vocab_only: req.vocab_only,
            devices: req.devices,
            kv_overrides: req.kv_overrides,
            swa_full: req.swa_full,
            no_perf: req.no_perf,

            // Context params
            num_ctx: req.num_ctx,
            num_batch: req.num_batch,
            num_ubatch: req.num_ubatch,
            num_seq_max: req.num_seq_max,
            rope_scaling_type: req.rope_scaling_type,
            rope_frequency_base: req.rope_frequency_base,
            rope_frequency_scale: req.rope_frequency_scale,
            yarn_ext_factor: req.yarn_ext_factor,
            yarn_attn_factor: req.yarn_attn_factor,
            yarn_beta_fast: req.yarn_beta_fast,
            yarn_beta_slow: req.yarn_beta_slow,
            yarn_orig_ctx: req.yarn_orig_ctx,
            pooling_type: req.pooling_type,
            attention_type: req.attention_type,
            flash_attn: req.flash_attn,
            type_k: req.type_k,
            type_v: req.type_v,
            embeddings: req.embeddings,
            offload_kqv: req.offload_kqv,
            defrag_thold: req.defrag_thold,
            logits_all: req.logits_all,
            f16_kv: req.f16_kv,
            low_vram: req.low_vram,

            // CPU params
            num_thread: req.num_thread,
            num_thread_batch: req.num_thread_batch,
            numa: req.numa,
            cpu_strict: req.cpu_strict,
            cpumask: req.cpumask,
            mask_valid: req.mask_valid,
            poll: req.poll,
            priority: req.priority,

            // Legacy/custom
            num_gpu: req.num_gpu,
            llama_server_url: req.llama_server_url,
        }
    }
}

/// ONNX Runtime configuration request/response
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct OnnxConfigRequest {
    /// Path to the ONNX model file
    #[serde(default)]
    pub model_path: Option<String>,
    /// Maximum sequence length for tokenization
    #[serde(default)]
    pub max_length: Option<usize>,
    /// Embedding dimension
    #[serde(default)]
    pub embedding_dim: Option<usize>,
    /// Number of threads for intra-op parallelism (within operators)
    #[serde(default)]
    pub num_threads: Option<usize>,
    /// Number of threads for inter-op parallelism (across operators)
    #[serde(default)]
    pub inter_op_num_threads: Option<usize>,
    /// Graph optimization level: "disable", "basic", "extended", "all"
    #[serde(default)]
    pub optimization_level: Option<String>,
    /// Execution mode: "sequential" or "parallel"
    #[serde(default)]
    pub execution_mode: Option<String>,
    /// Enable memory pattern optimization
    #[serde(default)]
    pub enable_mem_pattern: Option<bool>,
    /// Enable CPU memory arena
    #[serde(default)]
    pub enable_cpu_mem_arena: Option<bool>,
    /// Enable deterministic compute
    #[serde(default)]
    pub deterministic_compute: Option<bool>,
    /// Optional path to serialize optimized models
    #[serde(default)]
    pub optimized_model_path: Option<Option<String>>,
    /// Enable profiling output
    #[serde(default)]
    pub enable_profiling: Option<bool>,
    /// Optional profiling output path
    #[serde(default)]
    pub profiling_output_path: Option<Option<String>>,
    /// Custom log id
    #[serde(default)]
    pub log_id: Option<Option<String>>,
    /// Log level string
    #[serde(default)]
    pub log_level: Option<String>,
    /// Verbosity for verbose logging
    #[serde(default)]
    pub log_verbosity: Option<i32>,
    /// Use environment allocators
    #[serde(default)]
    pub use_env_allocators: Option<bool>,
    /// Flush-to-zero / denormal-as-zero
    #[serde(default)]
    pub denormal_as_zero: Option<bool>,
    /// Enable Quantize/Dequantize fusion
    #[serde(default)]
    pub enable_quant_qdq: Option<bool>,
    /// Enable double QDQ remover
    #[serde(default)]
    pub enable_double_qdq_remover: Option<bool>,
    /// Enable QDQ cleanup
    #[serde(default)]
    pub enable_qdq_cleanup: Option<bool>,
    /// Enable GELU approximation
    #[serde(default)]
    pub approximate_gelu: Option<bool>,
    /// Enable ahead-of-time inlining
    #[serde(default)]
    pub enable_aot_inlining: Option<bool>,
    /// Optimizer passes to disable
    #[serde(default)]
    pub disabled_optimizers: Option<Vec<String>>,
    /// Allocate initializers using device allocator
    #[serde(default)]
    pub use_device_allocator_for_initializers: Option<bool>,
    /// Allow inter-op spinning
    #[serde(default)]
    pub allow_inter_op_spinning: Option<bool>,
    /// Allow intra-op spinning
    #[serde(default)]
    pub allow_intra_op_spinning: Option<bool>,
    /// Use prepacking optimizations
    #[serde(default)]
    pub use_prepacking: Option<bool>,
    /// Use independent thread pool per session
    #[serde(default)]
    pub independent_thread_pool: Option<bool>,
    /// Do not inherit execution providers from the environment
    #[serde(default)]
    pub no_env_execution_providers: Option<bool>,
    /// Chunks per ONNX inference pass — lower = less RAM, higher = more throughput
    #[serde(default)]
    pub embedding_batch_size: Option<usize>,
}

// ============================================================================
// TRAINING DATA COLLECTION ENDPOINTS (Phase 20)
// ============================================================================

use crate::training::{TrainingDataCollector, TrainingExample, TrainingStats};

static TRAINING_COLLECTOR: OnceLock<TrainingDataCollector> = OnceLock::new();
static LORA_EXPORT_STATE: OnceLock<Arc<Mutex<LoraExportState>>> = OnceLock::new();
static LORA_FILTER_OVERRIDE: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();
static AUTO_EXPORT_OVERRIDES: OnceLock<Arc<Mutex<AutoExportOverrides>>> = OnceLock::new();
static SYNTHETIC_QA_STATE: OnceLock<Arc<Mutex<SyntheticQaState>>> = OnceLock::new();

impl Default for SyntheticQaState {
    fn default() -> Self {
        Self {
            running: false,
            last_started: None,
            last_finished: None,
            last_error: None,
            examples_generated: None,
            questions_per_chunk: 3,
            max_chunks: None,
        }
    }
}

// ============================================================================
// ONNX CONFIG
// ============================================================================

use crate::perf::onnx_embedder::{
    OnnxConfig, OnnxExecutionMode, OnnxLogLevel, OnnxOptimizationLevel,
};

/// Global ONNX config storage (read at startup, can be modified via API)
static ONNX_CONFIG: OnceLock<std::sync::RwLock<OnnxConfig>> = OnceLock::new();

/// Chat mode: how to process queries
#[derive(serde::Deserialize, Clone, Copy, Default, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ChatMode {
    /// Only search documents (RAG)
    Rag,
    /// Only use LLM (no document search)
    Llm,
    /// Combine: search documents + LLM fallback/enhancement
    Hybrid,
    /// Prefer RAG, but fall back to Hybrid when retrieval confidence is low.
    #[default]
    Auto,
    /// Strict grounded RAG: LLM answers only from retrieved context.
    /// If no chunks are found, responds with "I don't know".
    #[serde(alias = "rag_strict")]
    RagStrict,
    /// Agentic mode: LLM-driven tool-use loop (Rig integration)
    Agentic,
}

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ManualObservationOrder {
    #[default]
    Relevance,
    Newest,
    Oldest,
}

/// Valid RAG memory types
/// Core types: fact, preference, instruction, context, summary, task
/// Extended types: conversation, decision, correction, feedback, persona, note
const VALID_MEMORY_TYPES: &[&str] = &[
    // Core types
    "fact",        // What is true (project facts, user info)
    "preference",  // What user likes/dislikes
    "instruction", // What to do/not do
    "context",     // Background information
    "summary",     // Condensed past interactions
    "task",        // Current work context
    // Extended types
    "conversation", // Past exchanges/dialogue
    "decision",     // Past decisions made
    "correction",   // Corrections to previous responses
    "feedback",     // User feedback on responses
    "persona",      // Personality/style preferences
    "note",         // User-added notes
];

pub mod agentic_monitor_routes;
pub mod graph_routes;
pub mod sys_routes;
pub mod tool_routes;

pub fn start_api_server(
    config: &ApiConfig,
) -> impl std::future::Future<Output = std::io::Result<()>> {
    // Snapshot needed config values to satisfy 'static factory closure
    let bind_addr = config.bind_addr();
    let upload_bind_addr = config.upload_bind_addr();
    let search_workers = config.search_workers;
    let upload_workers = config.upload_workers;
    let search_max_connections = config.search_max_connections;
    let upload_max_connections = config.upload_max_connections;
    let search_max_body_bytes = config.search_max_body_kb * 1024;
    let search_timeout = std::time::Duration::from_secs(config.search_timeout_secs);
    let upload_timeout = std::time::Duration::from_secs(config.upload_timeout_secs);
    let _trust_proxy = config.trust_proxy;
    let trust_proxy_search = config.trust_proxy_search;
    let trust_proxy_upload = config.trust_proxy_upload;
    let upload_rate_limit_lru_capacity = config.upload_rate_limit_lru_capacity;

    let upload_semaphore = Arc::new(tokio::sync::Semaphore::new(config.upload_max_concurrent));
    let _ = UPLOAD_SEMAPHORE.set(Arc::clone(&upload_semaphore));
    info!(
        max_concurrent = config.upload_max_concurrent,
        "Upload concurrency semaphore initialized"
    );

    // CORS: search is always permissive; upload restricts when UPLOAD_CORS_ORIGINS is set.
    let upload_cors_origins: Option<Arc<Vec<String>>> =
        std::env::var("UPLOAD_CORS_ORIGINS")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|s| Arc::new(s.split(',').map(|o| o.trim().to_string()).collect()));

    if let Some(ref origins) = upload_cors_origins {
        info!(origins = ?origins, "Upload CORS restricted to explicit origin list");
    } else {
        info!("Upload CORS: allow_any_origin (set UPLOAD_CORS_ORIGINS to restrict)");
    }
    let rate_limit_enabled = config.rate_limit_enabled;
    let rate_limit_qps = config.rate_limit_qps;
    let rate_limit_burst = config.rate_limit_burst as f64;
    let rate_limit_lru_capacity = config.rate_limit_lru_capacity;
    let search_qps = config.rate_limit_search_qps.unwrap_or(rate_limit_qps);
    let search_burst = config
        .rate_limit_search_burst
        .unwrap_or(config.rate_limit_burst) as f64;
    let upload_qps = config.rate_limit_upload_qps.unwrap_or(rate_limit_qps);
    let upload_burst = config
        .rate_limit_upload_burst
        .unwrap_or(config.rate_limit_burst) as f64;

    let force_single_worker = std::env::var("NO_DOTENV")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);

    // Shared thresholds — both limiters reference the same Arc so runtime updates
    // via /config/rate-limits apply to both servers simultaneously.
    let rl_thresholds = RuntimeThresholds::new(
        search_qps.max(0.0),
        search_burst,
        upload_qps.max(0.0),
        upload_burst,
    );

    let search_rl_cfg = crate::security::rate_limiter::RateLimiterConfig {
        enabled: rate_limit_enabled,
        qps: rate_limit_qps.max(0.0),
        burst: rate_limit_burst,
        max_ips: rate_limit_lru_capacity,
    };
    let upload_rl_cfg = crate::security::rate_limiter::RateLimiterConfig {
        enabled: rate_limit_enabled,
        qps: upload_qps.max(0.0),
        burst: upload_burst,
        max_ips: upload_rate_limit_lru_capacity,
    };
    let shared_rl = Arc::new(RateLimiter::new(search_rl_cfg, Arc::clone(&rl_thresholds)));
    let upload_rl = Arc::new(RateLimiter::new(upload_rl_cfg, Arc::clone(&rl_thresholds)));
    let _ = SHARED_RATE_LIMITER.set(Arc::clone(&shared_rl));
    let _ = UPLOAD_RATE_LIMITER.set(Arc::clone(&upload_rl));

    // Search server opts: only search-relevant route rules; trust_proxy_search.
    let search_opts = RateLimitOptions {
        trust_proxy: trust_proxy_search,
        thresholds: Arc::clone(&rl_thresholds),
        rules: vec![],
        exempt_prefixes: vec![
            "/".into(),
            "/health".into(),
            "/ready".into(),
            "/metrics".into(),
            "/monitoring".into(),
            "/monitor".into(),
        ],
    }
    .with_env_overrides();

    // Upload server opts: all non-monitoring routes are upload class by default; trust_proxy_upload.
    let upload_opts = RateLimitOptions {
        trust_proxy: trust_proxy_upload,
        thresholds: Arc::clone(&rl_thresholds),
        rules: vec![
            RouteRule {
                pattern: "/reindex".into(),
                match_kind: MatchKind::Exact,
                qps: 0.5,
                burst: 2.0,
                label: Some("admin-reindex".into()),
            },
        ],
        exempt_prefixes: vec![
            "/monitoring".into(),
        ],
    };

    info!(
        trust_proxy_search,
        trust_proxy_upload,
        search_qps = rl_thresholds.get_search_qps(),
        search_burst = rl_thresholds.get_search_burst(),
        upload_qps = rl_thresholds.get_upload_qps(),
        upload_burst = rl_thresholds.get_upload_burst(),
        search_lru = rate_limit_lru_capacity,
        upload_lru = upload_rate_limit_lru_capacity,
        "Rate limit options initialized"
    );

    let shared_rate_limit_state = web::Data::new(RateLimitSharedState {
        limiter: Arc::clone(&shared_rl),
        opts: search_opts.clone(),
    });
    let upload_rate_limit_state = web::Data::new(RateLimitSharedState {
        limiter: Arc::clone(&upload_rl),
        opts: upload_opts.clone(),
    });

    let upload_max_bytes = std::env::var("UPLOAD_MAX_MB")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(150)
        * 1024
        * 1024;

    // ── Search server ─────────────────────────────────────────────────────────
    let api_config_search = config.clone();
    let shared_rl_search = Arc::clone(&shared_rl);
    let search_opts_search = search_opts.clone();
    let shared_rate_limit_state_search = shared_rate_limit_state.clone();
    let mut search_server = HttpServer::new(move || {
        let api_config = api_config_search.clone();
        let rl = Arc::clone(&shared_rl_search);
        let opts = search_opts_search.clone();
        let rate_limit_state_data = shared_rate_limit_state_search.clone();

        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST", "DELETE", "PATCH"])
            .allowed_headers(vec![
                actix_web::http::header::CONTENT_TYPE,
                actix_web::http::header::AUTHORIZATION,
            ])
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(api_config.clone()))
            .app_data(rate_limit_state_data.clone())
            .app_data(web::PayloadConfig::default().limit(search_max_body_bytes))
            .wrap(crate::trace_middleware::TraceMiddleware::new_with_server("search"))
            .wrap(
                crate::monitoring::rate_limit_middleware::RateLimitMiddleware::new_with_options(
                    rl.clone(),
                    opts.clone(),
                ),
            )
            // ============================================================================
            // MONITORING ROUTES (Phase 16 Step 3 - OTLP Exporting)
            // Exports metrics in Prometheus text format for Prometheus scraping
            // ============================================================================
            .service(
                web::scope("/monitoring")
                    .route("/health", web::get().to(health_check))
                    .route("/ready", web::get().to(ready_check))
                    .route("/status-log/{status}", web::get().to(get_status_log))
                    .route("/systemd/logs", web::get().to(get_systemd_logs))
                    .route("/metrics", web::get().to(get_metrics)) // ← Prometheus format
                    .route("/optimizations", web::get().to(get_optimization_stats)) // ← Performance optimization stats
                    .route("/io-uring", web::get().to(get_io_uring_stats)) // ← io_uring async I/O stats
                    .route("/io-uring", web::post().to(save_io_uring_config)) // ← save io_uring config
                    .route("/log-frontend-error", web::post().to(log_frontend_error)) // ← Log frontend errors
                    .route(
                        "/optimizations/build-hnsw",
                        web::post().to(build_hnsw_index),
                    )
                    .route("/optimizations/build-pq", web::post().to(build_pq_index))
                    .route(
                        "/optimizations/build-fp16",
                        web::post().to(build_fp16_store),
                    )
                    .route(
                        "/optimizations/build-all",
                        web::post().to(build_all_indexes),
                    )
                    .route("/ui/requests", web::get().to(get_ui_requests))
                    .route("/ui/requests/search", web::get().to(get_ui_requests_search))
                    .route("/ui/requests/upload", web::get().to(get_ui_requests_upload))
                    .route("/chunking/latest", web::get().to(get_chunking_stats))
                    .route("/chunking/logging", web::get().to(toggle_chunking_logging))
                    // Agentic monitoring routes
                    .route(
                        "/agents/stats",
                        web::get().to(agentic_monitor_routes::get_agent_stats),
                    )
                    .route(
                        "/agents/episodes",
                        web::get().to(agentic_monitor_routes::get_recent_episodes),
                    )
                    .route(
                        "/agents/goals",
                        web::get().to(agentic_monitor_routes::get_goals),
                    )
                    .route(
                        "/agents/reflections",
                        web::get().to(agentic_monitor_routes::get_reflections),
                    )
                    .route(
                        "/memory/stats",
                        web::get().to(agentic_monitor_routes::get_memory_stats),
                    )
                    .route(
                        "/tools/stats",
                        web::get().to(agentic_monitor_routes::get_tool_stats),
                    )
                    .route(
                        "/tools/executions",
                        web::get().to(agentic_monitor_routes::get_tool_executions),
                    )
                    .route(
                        "/tools/available",
                        web::get().to(agentic_monitor_routes::get_available_tools),
                    )
                    .route(
                        "/tools/cache",
                        web::get().to(agentic_monitor_routes::get_tool_cache_stats_endpoint),
                    )
                    .route(
                        "/tools/rate-limits",
                        web::get().to(agentic_monitor_routes::get_tool_rate_limits_endpoint),
                    )
                    .route(
                        "/tools/costs",
                        web::get().to(agentic_monitor_routes::get_tool_costs_endpoint),
                    )
                    .route(
                        "/tools/trends",
                        web::get().to(agentic_monitor_routes::get_tool_trends_endpoint),
                    )
                    .route(
                        "/tools/dependencies",
                        web::get().to(agentic_monitor_routes::get_tool_dependencies_endpoint),
                    )
                    .route(
                        "/observations/metrics",
                        web::get().to(get_manual_observation_metrics),
                    )
                    .route(
                        "/observations/recent",
                        web::get().to(get_recent_observations),
                    )
                    .route(
                        "/memory/search/stats",
                        web::get().to(get_memory_search_layer_stats),
                    )
                    .route("/memories/rag", web::get().to(get_recent_rag_memories))
                    // Docker monitoring
                    .route("/docker", web::get().to(get_docker_status))
                    .route("/docker/action", web::post().to(docker_action))
                    // LLM runtime control (Ollama)
                    .route("/runtime/action", web::post().to(runtime_action))
                    .route("/ollama", web::get().to(get_ollama_status))
                    .route("/onnx", web::get().to(get_onnx_status)),
            )
            // ============================================================================
            // CORPUS ROUTES (search + metadata only)
            // ============================================================================
            .service(
                web::scope("/corpora")
                    .route("", web::post().to(create_corpus_handler))
                    .route("", web::get().to(list_corpora_handler))
                    .route("/{slug}", web::get().to(get_corpus_handler))
                    .route("/{slug}", web::patch().to(rename_corpus_handler))
                    .route("/{slug}", web::delete().to(delete_corpus_handler))
                    .route(
                        "/{slug}/description",
                        web::patch().to(update_corpus_description_handler),
                    )
                    .route(
                        "/{slug}/documents",
                        web::get().to(corpus_list_documents_handler),
                    )
                    .route("/{slug}/search", web::get().to(corpus_search_handler))
                    .route(
                        "/{slug}/settings",
                        web::get().to(get_corpus_settings_handler),
                    )
                    .route(
                        "/{slug}/settings",
                        web::patch().to(patch_corpus_settings_handler),
                    ),
            )
            .route(
                "/agent/memory/settings",
                web::get().to(get_agent_memory_settings_handler),
            )
            .route(
                "/agent/memory/settings",
                web::patch().to(patch_agent_memory_settings_handler),
            )
            // ============================================================================
            // ROOT & CORE ROUTES
            // ============================================================================
            .route("/", web::get().to(root_handler))
            .route("/documents", web::get().to(list_documents))
            .route("/config/chunk_size", web::get().to(get_chunk_config))
            .route("/config/chunk_size", web::post().to(commit_chunk_config))
            .route("/config/index_in_ram", web::post().to(set_index_in_ram))
            .route("/config/servers", web::get().to(get_server_config))
            .route("/config/servers", web::post().to(save_server_config))
            .route("/chunk/preview", web::post().to(chunk_preview_handler))
            .route("/config/embedding", web::get().to(get_embedding_config))
            .route("/config/embedding", web::post().to(set_embedding_config))
            .route(
                "/config/embedding-model",
                web::post().to(set_embedding_model),
            )
            .route(
                "/config/embedding/download-tokenizer",
                web::post().to(download_tokenizer),
            )
            .route("/config/llm", web::get().to(get_llm_config))
            .route("/config/llm", web::post().to(commit_llm_config))
            .route("/config/prompt_caching", web::get().to(get_prompt_caching))
            .route("/config/prompt_caching", web::post().to(set_prompt_caching))
            .route("/config/hardware", web::get().to(get_hardware_config))
            .route("/config/hardware", web::post().to(commit_hardware_config))
            .route("/config/ner", web::get().to(get_ner_config))
            .route("/config/ner", web::post().to(set_ner_config))
            .route("/config/onnx", web::get().to(get_onnx_config))
            .route("/config/onnx", web::post().to(set_onnx_config))
            // Neo4j Knowledge Graph config (Phase 27)
            .route("/config/neo4j", web::get().to(get_neo4j_config))
            .route("/config/neo4j", web::post().to(save_neo4j_config))
            .route("/config/neo4j/test", web::post().to(test_neo4j_connection))
            .route("/config/api_keys", web::get().to(get_api_keys))
            .route("/config/api_keys", web::post().to(save_api_keys))
            .route(
                "/config/api_keys/{provider}",
                web::delete().to(delete_api_key),
            )
            .route(
                "/extract_entities",
                web::post().to(extract_entities_handler),
            )
            // Entity Terms config (Step 1 v1.0)
            .route("/config/entity_terms", web::get().to(get_entity_terms))
            .route("/config/entity_terms", web::post().to(save_entity_terms))
            .route("/index/info", web::get().to(index_info_handler))
            .route("/search", web::get().to(search_documents_inner))
            .route("/rerank", web::post().to(rerank))
            .route("/summarize", web::post().to(summarize))
            .route("/monitor/cache/info", web::get().to(get_cache_monitor_info))
            .route("/cache/clear", web::post().to(clear_cache))
            .route(
                "/monitor/rate_limits/info",
                web::get().to(get_rate_limit_monitor_info),
            )
            .route(
                "/monitor/rate_limits/enabled",
                web::post().to(set_rate_limit_enabled),
            )
            .route(
                "/monitor/rate_limits/thresholds",
                web::patch().to(update_rate_limit_thresholds),
            )
            .route(
                "/monitor/inference_gateway",
                web::get().to(get_inference_gateway_stats),
            )
            .route("/monitor/logs/recent", web::get().to(get_recent_logs))
            .route("/monitor/parser/stats", web::get().to(get_parser_stats))
            .route("/monitor/canon/stats", web::get().to(get_canon_stats))
            .route(
                "/monitor/preprocess/stats",
                web::get().to(get_preprocess_stats),
            )
            .route(
                "/monitor/chunk-meta/stats",
                web::get().to(get_chunk_meta_stats),
            )
            .route("/monitor/golden-sample", web::get().to(get_golden_sample))
            .route(
                "/monitor/golden-sample/recapture",
                web::post().to(post_golden_sample_recapture),
            )
            .route(
                "/monitor/tokenizer/diff",
                web::post().to(post_tokenizer_diff),
            )
            // ============================================================================
            // RAG MEMORY ROUTES
            // ============================================================================
            .route("/memory/types", web::get().to(list_memory_types))
            .route("/memory/search_rag", web::post().to(search_rag_memory))
            .route("/memory/recall_rag", web::post().to(recall_rag_memory))
            .route("/memory/delete_rag", web::post().to(delete_rag_memory))
            // Manual observation CRUD + search
            .route(
                "/memory/observations",
                web::post().to(create_manual_observation),
            )
            .route(
                "/memory/observations",
                web::get().to(list_manual_observations),
            )
            .route(
                "/memory/observations/search",
                web::post().to(search_manual_observations),
            )
            .route(
                "/memory/observations/timeline",
                web::post().to(manual_observation_timeline),
            )
            .route(
                "/memory/observations/fetch",
                web::post().to(fetch_manual_observations),
            )
            .route(
                "/memory/observations/{id}",
                web::get().to(get_manual_observation),
            )
            .route(
                "/memory/observations/{id}",
                web::put().to(update_manual_observation),
            )
            .route(
                "/memory/observations/{id}",
                web::delete().to(delete_manual_observation),
            )
            // ============================================================================
            // TRAINING DATA COLLECTION ROUTES (Phase 20)
            // ============================================================================
            .route(
                "/training/feedback",
                web::post().to(submit_training_feedback),
            )
            .route("/training/stats", web::get().to(get_training_stats))
            .route("/training/export", web::post().to(export_training_data))
            .route(
                "/training/export_snapshot",
                web::post().to(export_lora_snapshot),
            )
            .route(
                "/training/export_snapshot/status",
                web::get().to(export_snapshot_status),
            )
            .route(
                "/training/export_snapshot/config",
                web::get().to(export_snapshot_config),
            )
            .route(
                "/training/export_snapshot/filter",
                web::post().to(set_export_snapshot_filter),
            )
            .route(
                "/training/export_snapshot/config",
                web::post().to(save_export_snapshot_config),
            )
            .route("/training/clear", web::post().to(clear_training_data))
            // Synthetic Q&A generation
            .route(
                "/training/synthetic_qa",
                web::post().to(generate_synthetic_qa),
            )
            .route(
                "/training/synthetic_qa/status",
                web::get().to(synthetic_qa_status),
            )
            .route(
                "/training/synthetic_qa/examples",
                web::get().to(synthetic_qa_examples),
            )
            // ============================================================================
            // AGENT ROUTES
            // ============================================================================
            .route("/agent", web::post().to(run_agent))
            .route("/agent/stream", web::post().to(run_agent_stream))
            .route("/agent/chat", web::get().to(run_agent_get))
            // Goal management routes
            .route(
                "/agent/goals",
                web::post().to(agentic_monitor_routes::create_goal),
            )
            .route(
                "/agent/goals",
                web::get().to(agentic_monitor_routes::get_active_goals),
            )
            .route(
                "/agent/goals/{goal_id}/complete",
                web::post().to(agentic_monitor_routes::complete_goal),
            )
            .route(
                "/agent/goals/{goal_id}/fail",
                web::post().to(agentic_monitor_routes::fail_goal),
            )
            .service(web::scope("/sys").configure(sys_routes::sys_routes))
            .configure(tool_routes::configure_tool_routes)
            .configure(graph_routes::configure_graph_routes)
    });
    if force_single_worker {
        search_server = search_server.workers(1);
    } else {
        search_server = search_server.workers(search_workers);
    }
    if let Err(e) = std::net::TcpListener::bind(&bind_addr) {
        error!(
            addr = %bind_addr,
            error = %e,
            "Search server port is already in use — set BACKEND_PORT to a free port"
        );
        std::process::exit(1);
    }
    let search_server = search_server
        .max_connections(search_max_connections)
        .client_request_timeout(search_timeout)
        .keep_alive(std::time::Duration::from_secs(20))
        .bind(bind_addr.clone())
        .unwrap_or_else(|e| panic!("Failed to bind search server to {}: {}", bind_addr, e));

    info!("Search server bound to http://{}", bind_addr);

    // ── Upload server ─────────────────────────────────────────────────────────
    let api_config_upload = config.clone();
    let upload_rl_ref = Arc::clone(&upload_rl);
    let upload_opts_ref = upload_opts.clone();
    let upload_rate_limit_state_ref = upload_rate_limit_state.clone();
    let upload_semaphore_ref = Arc::clone(&upload_semaphore);
    let upload_cors_origins_ref = upload_cors_origins.clone();
    let mut upload_server = HttpServer::new(move || {
        let api_config = api_config_upload.clone();
        let rl = Arc::clone(&upload_rl_ref);
        let opts = upload_opts_ref.clone();
        let rate_limit_state_data = upload_rate_limit_state_ref.clone();
        let sem = Arc::clone(&upload_semaphore_ref);
        let origins = upload_cors_origins_ref.clone();

            // CORS voor upload + monitoring server
    let upload_cors = Cors::default()
        .allow_any_origin()
        .allowed_methods(vec!["GET", "POST"])
        .allowed_headers(vec![
            actix_web::http::header::CONTENT_TYPE,
            actix_web::http::header::AUTHORIZATION,
        ])
        .max_age(3600);

   

        let _cors = {
            let mut c = Cors::default()
                .allowed_methods(vec!["GET", "POST", "DELETE", "PATCH"])
                .allowed_headers(vec![
                    actix_web::http::header::CONTENT_TYPE,
                    actix_web::http::header::AUTHORIZATION,
                ])
                .max_age(3600);
            if let Some(ref list) = origins {
                for origin in list.iter() {
                    c = c.allowed_origin(origin.as_str());
                }
            } else {
                c = c.allow_any_origin();
            }
            c
        };

        App::new()
            .app_data(web::Data::new(api_config.clone()))
            .app_data(rate_limit_state_data.clone())
            .app_data(web::PayloadConfig::default().limit(upload_max_bytes))
            .wrap(crate::trace_middleware::TraceMiddleware::new_with_server("upload"))
            .wrap(
                crate::monitoring::rate_limit_middleware::RateLimitMiddleware::new_with_options(
                    rl.clone(),
                    opts.clone(),
                )
                .with_server("upload"),
            )
            .wrap(
                crate::monitoring::upload_concurrency_middleware::UploadConcurrencyMiddleware::new(
                    Arc::clone(&sem),
                ),
            )
            // Upload-only corpus routes
            
            .wrap(upload_cors)
            .service(
                web::scope("/corpora")
                    .route("/{slug}/upload", web::post().to(corpus_upload_handler))
                    .route("/{slug}/reindex", web::post().to(corpus_reindex_handler))
                    .route(
                        "/{slug}/documents/{filename}",
                        web::delete().to(corpus_delete_document_handler),
                    ),
            )
            .route("/upload", web::post().to(upload_document_inner))
            .route("/documents/{filename}", web::delete().to(delete_document))
            .route("/save_vectors", web::post().to(save_vectors_handler))
            .route("/reindex", web::post().to(reindex_handler))
            .route("/reindex/async", web::post().to(reindex_async_handler))
            .route(
                "/reindex/status/{job_id}",
                web::get().to(reindex_status_handler),
            )
            .route("/memory/store_rag", web::post().to(store_rag_memory))
            .service(
                web::scope("/monitoring")
                    .route("/health", web::get().to(upload_health_check))
                    .route("/ready", web::get().to(upload_ready_check))
                    .route("/metrics", web::get().to(get_metrics))
                    .route("/ui/requests", web::get().to(get_ui_requests_upload)),
            )
    });
    if force_single_worker {
        upload_server = upload_server.workers(1);
    } else {
        upload_server = upload_server.workers(upload_workers);
    }
    if let Err(e) = std::net::TcpListener::bind(&upload_bind_addr) {
        error!(
            addr = %upload_bind_addr,
            error = %e,
            "Upload server port is already in use — set UPLOAD_PORT to a free port"
        );
        std::process::exit(1);
    }
    let upload_server = upload_server
        .max_connections(upload_max_connections)
        .client_request_timeout(upload_timeout)
        .keep_alive(std::time::Duration::from_secs(75))
        .bind(upload_bind_addr.clone())
        .unwrap_or_else(|e| panic!("Failed to bind upload server to {}: {}", upload_bind_addr, e));

    info!("Upload server bound to http://{}", upload_bind_addr);

    // Call .run() here (on the calling thread) so the returned future captures only
    // the Send `Server` handles, not the !Send `HttpServer` factory closures.
    let search_run = search_server.run();
    let upload_run = upload_server.run();
    async move {
        tokio::try_join!(search_run, upload_run)?;
        Ok(())
    }
}
