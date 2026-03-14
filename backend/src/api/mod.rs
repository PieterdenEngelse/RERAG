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
use crate::security::rate_limiter::{RateLimiter, RateLimiterState};
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

pub const UPLOAD_DIR: &str = "documents";

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

pub fn set_retriever_handle(handle: Arc<Mutex<Retriever>>) {
    let _ = RETRIEVER.set(handle);
}

pub fn get_retriever_handle() -> Option<Arc<Mutex<Retriever>>> {
    RETRIEVER.get().map(|h| Arc::clone(h))
}

// Global Neo4j client handle (Phase 27)
#[cfg(feature = "neo4j")]
static NEO4J_CLIENT: OnceLock<crate::graph::Neo4jClient> = OnceLock::new();

#[cfg(feature = "neo4j")]
pub fn set_neo4j_client(client: crate::graph::Neo4jClient) {
    let _ = NEO4J_CLIENT.set(client);
}

#[cfg(feature = "neo4j")]
pub fn get_neo4j_client() -> Option<&'static crate::graph::Neo4jClient> {
    NEO4J_CLIENT.get()
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
static KNOWLEDGE_BUILDER: OnceLock<std::sync::Arc<crate::graph::KnowledgeBuilder>> =
    OnceLock::new();

#[cfg(feature = "neo4j")]
pub fn set_knowledge_builder(builder: std::sync::Arc<crate::graph::KnowledgeBuilder>) {
    let _ = KNOWLEDGE_BUILDER.set(builder);
}

#[cfg(feature = "neo4j")]
pub fn get_knowledge_builder() -> Option<&'static std::sync::Arc<crate::graph::KnowledgeBuilder>> {
    KNOWLEDGE_BUILDER.get()
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

    for (chunk_id, chunk_content) in chunks {
        // Add chunk to graph
        let chunk_meta = ChunkMeta {
            id: chunk_id.clone(),
            document_id: doc_id.to_string(),
            content: chunk_content.clone(),
            embedding_id: chunk_id.clone(),
            position: chunk_id
                .split('#')
                .last()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            token_count: chunk_content.split_whitespace().count(),
        };

        if let Err(e) = kb.add_chunk(&chunk_meta).await {
            warn!(error = %e, chunk_id = %chunk_id, "Failed to add chunk to knowledge graph");
            continue;
        }

        // Extract entities from chunk
        let extraction = extractor.extract(chunk_content);

        for entity in &extraction.entities {
            if entity.confidence >= confidence_threshold {
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

        // Link co-occurring entities (entities in the same chunk are related)
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

// Shared agent session state for chat commands
#[derive(Default, Clone)]
struct AgentChatState {
    focus_topic: Option<String>,
    persona: Option<String>,
    verbosity: Verbosity,
    preferred_model: Option<String>,
    temperature: Option<f32>,
    last_query: Option<String>,
    last_response: Option<String>,
    last_steps: Vec<AgentStep>,
    last_sources: Vec<String>,
    #[allow(dead_code)]
    last_tool: Option<String>,
    last_token_usage: Option<usize>,
    undo_stack: Vec<CommandAction>,
    dry_run_plan: Option<String>,
    /// Enable prompt caching (uses /api/chat instead of /api/generate for Ollama)
    prompt_caching_enabled: bool,
}

#[derive(Clone)]
enum CommandAction {
    FocusSet(Option<String>),
    PersonaSet(Option<String>),
    VerbosityChanged(Verbosity),
    ModelChanged(Option<String>),
    TemperatureChanged(Option<f32>),
    NoteAdded(#[allow(dead_code)] String),
}

#[derive(Clone, Copy, Default)]
enum Verbosity {
    Brief,
    #[default]
    Normal,
    Verbose,
}

impl Verbosity {
    fn label(&self) -> &'static str {
        match self {
            Verbosity::Brief => "brief",
            Verbosity::Normal => "normal",
            Verbosity::Verbose => "verbose",
        }
    }
}

static CHAT_STATE: OnceLock<Arc<Mutex<AgentChatState>>> = OnceLock::new();

fn chat_state() -> Arc<Mutex<AgentChatState>> {
    CHAT_STATE
        .get_or_init(|| Arc::new(Mutex::new(AgentChatState::default())))
        .clone()
}

fn update_last_agent_run(query: String, response: &AgentResponse) {
    let state_arc = chat_state();
    let mut state = state_arc.lock().expect("chat state lock");
    state.last_query = Some(query.clone());
    state.last_response = Some(response.answer.clone());
    state.last_steps = response.steps.clone();
    state.last_sources = response.used_chunks.clone();
    let token_estimate = response.answer.split_whitespace().count();
    state.last_token_usage = Some(token_estimate.max(response.used_chunks.len()));
}

fn record_focus_change(new_focus: Option<String>) -> Option<String> {
    let state_arc = chat_state();
    let mut state = state_arc.lock().expect("chat state lock");
    let previous = state.focus_topic.clone();
    state
        .undo_stack
        .push(CommandAction::FocusSet(previous.clone()));
    state.focus_topic = new_focus;
    previous
}

fn record_persona_change(new_persona: Option<String>) -> Option<String> {
    let state_arc = chat_state();
    let mut state = state_arc.lock().expect("chat state lock");
    let previous = state.persona.clone();
    state
        .undo_stack
        .push(CommandAction::PersonaSet(previous.clone()));
    state.persona = new_persona;
    previous
}

fn record_verbosity_change(new_mode: Verbosity) -> Verbosity {
    let state_arc = chat_state();
    let mut state = state_arc.lock().expect("chat state lock");
    let previous = state.verbosity;
    state
        .undo_stack
        .push(CommandAction::VerbosityChanged(previous));
    state.verbosity = new_mode;
    previous
}

fn push_note_action(note: String) {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    guard.undo_stack.push(CommandAction::NoteAdded(note));
}

fn record_model_change(new_model: Option<String>) -> Option<String> {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    let previous = guard.preferred_model.clone();
    guard
        .undo_stack
        .push(CommandAction::ModelChanged(previous.clone()));
    guard.preferred_model = new_model.clone();
    previous
}

fn record_temperature_change(new_temp: Option<f32>) -> Option<f32> {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    let previous = guard.temperature;
    guard
        .undo_stack
        .push(CommandAction::TemperatureChanged(previous));
    guard.temperature = new_temp;
    previous
}

/// Get current prompt caching state
fn get_prompt_caching_enabled() -> bool {
    let state_arc = chat_state();
    let guard = state_arc.lock().expect("chat state lock");
    guard.prompt_caching_enabled
}

/// Set prompt caching state
fn set_prompt_caching_enabled(enabled: bool) -> bool {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    let previous = guard.prompt_caching_enabled;
    guard.prompt_caching_enabled = enabled;
    previous
}

fn pop_undo_action() -> Option<CommandAction> {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    guard.undo_stack.pop()
}

#[allow(dead_code)]
fn snapshots_for_debug() -> (Option<String>, Option<String>, Verbosity, Option<String>) {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    (
        state.focus_topic.clone(),
        state.persona.clone(),
        state.verbosity,
        state.last_query.clone(),
    )
}

/// Get current chat settings for the agent, including RAG memories
fn get_current_chat_settings() -> crate::agent::ChatSettings {
    use crate::agent::{load_categorized_memories, ChatSettings, Verbosity as AgentVerbosity};

    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");

    let verbosity = match state.verbosity {
        Verbosity::Brief => AgentVerbosity::Brief,
        Verbosity::Normal => AgentVerbosity::Normal,
        Verbosity::Verbose => AgentVerbosity::Verbose,
    };

    // Load RAG memories from database (limit to 20 most recent)
    let memories = load_categorized_memories(path_resolver::agent_db_path_str(), "default", 20);

    ChatSettings::new()
        .with_focus(state.focus_topic.clone())
        .with_persona(state.persona.clone())
        .with_verbosity(verbosity)
        .with_temperature(state.temperature)
        .with_model(state.preferred_model.clone())
        .with_memories(memories)
}

fn store_dry_run_plan(plan: String) {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    guard.dry_run_plan = Some(plan);
}

#[allow(dead_code)]
fn fetch_dry_run_plan() -> Option<String> {
    let state_arc = chat_state();
    let guard = state_arc.lock().expect("chat state lock");
    guard.dry_run_plan.clone()
}

// Rate limiting is enforced by middleware (see monitoring/rate_limit_middleware.rs).
// The per-handler token-bucket implementation was removed to avoid double-limiting.

#[derive(serde::Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(serde::Deserialize)]
pub struct RerankRequest {
    pub query: String,
    pub candidates: Vec<String>,
}

#[derive(serde::Deserialize)]
pub struct SummarizeRequest {
    pub query: String,
    pub candidates: Vec<String>,
}

const DEFAULT_LOG_LIMIT: usize = 200;
const MAX_LOG_LIMIT: usize = 500;
const LOG_FILE_PREFIX: &str = "backend.log";

#[derive(Clone)]
struct RateLimitSharedState {
    limiter: Arc<RateLimiter>,
    opts: RateLimitOptions,
}

impl RateLimitSharedState {
    fn config_snapshot(&self, enabled: bool) -> RateLimitConfigSnapshot {
        RateLimitConfigSnapshot {
            enabled,
            trust_proxy: self.opts.trust_proxy,
            search_qps: self.opts.search_qps,
            search_burst: self.opts.search_burst,
            upload_qps: self.opts.upload_qps,
            upload_burst: self.opts.upload_burst,
            exempt_prefixes: self.opts.exempt_prefixes.clone(),
            rules: self.opts.rules.clone(),
        }
    }
}

#[derive(Serialize)]
struct L1CacheSnapshot {
    enabled: bool,
    total_searches: u64,
    hits: u64,
    misses: u64,
    hit_rate: f64,
}

#[derive(Serialize)]
struct L2CacheSnapshot {
    enabled: bool,
    l1_hits: u64,
    l1_misses: u64,
    l2_hits: u64,
    l2_misses: u64,
    total_items: u64,
    hit_rate: f64,
}

#[derive(Serialize)]
struct CacheCountersSnapshot {
    hits_total: i64,
    misses_total: i64,
}

#[derive(Serialize)]
struct CacheMonitorResponse {
    request_id: String,
    l1: L1CacheSnapshot,
    l2: L2CacheSnapshot,
    redis: crate::cache::redis_cache::RedisCacheSummary,
    counters: CacheCountersSnapshot,
}

#[derive(Serialize)]
struct RouteDropStat {
    route: String,
    drops: i64,
}

#[derive(Serialize)]
struct RateLimitConfigSnapshot {
    enabled: bool,
    trust_proxy: bool,
    search_qps: f64,
    search_burst: f64,
    upload_qps: f64,
    upload_burst: f64,
    exempt_prefixes: Vec<String>,
    rules: Vec<RouteRule>,
}

#[derive(Serialize)]
struct RateLimitMonitorResponse {
    request_id: String,
    total_drops: i64,
    drops_by_route: Vec<RouteDropStat>,
    config: RateLimitConfigSnapshot,
    limiter_state: RateLimiterState,
}

#[derive(serde::Deserialize)]
struct LogsQuery {
    limit: Option<usize>,
}

#[derive(serde::Deserialize)]
struct ChunkingQuery {
    limit: Option<usize>,
    capacity: Option<usize>,
}

#[derive(serde::Deserialize)]
struct LoggingQuery {
    enabled: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
struct ChunkConfigCommitRequest {
    target_size: usize,
    min_size: usize,
    max_size: usize,
    overlap: usize,
    #[serde(default)]
    semantic_similarity_threshold: Option<f32>,
}

#[derive(Debug, Serialize, Clone)]
struct ChunkerConfigSnapshot {
    target_size: usize,
    min_size: usize,
    max_size: usize,
    overlap: usize,
    semantic_similarity_threshold: f32,
}

impl From<&ChunkerConfig> for ChunkerConfigSnapshot {
    fn from(cfg: &ChunkerConfig) -> Self {
        Self {
            target_size: cfg.target_size,
            min_size: cfg.min_size,
            max_size: cfg.max_size,
            overlap: cfg.overlap,
            semantic_similarity_threshold: cfg.semantic_similarity_threshold,
        }
    }
}

#[derive(Debug, Serialize)]
struct ChunkCommitResponse {
    status: String,
    message: String,
    request_id: String,
    chunker_config: ChunkerConfigSnapshot,
    reindex_status: String,
    reindex_job_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct LlmConfigRequest {
    // Basic sampling
    temperature: f32,
    top_p: f32,
    top_k: usize,
    max_tokens: usize,
    repeat_penalty: f32,
    frequency_penalty: f32,
    presence_penalty: f32,
    stop_sequences: Vec<String>,
    seed: Option<i64>,
    #[serde(default = "default_min_p")]
    min_p: f32,
    #[serde(default = "default_typical_p")]
    typical_p: f32,
    #[serde(default = "default_tfs_z")]
    tfs_z: f32,

    // Mirostat
    #[serde(default = "default_mirostat")]
    mirostat: i32,
    #[serde(default = "default_mirostat_eta")]
    mirostat_eta: f32,
    #[serde(default = "default_mirostat_tau")]
    mirostat_tau: f32,

    // Repetition control
    #[serde(default = "default_repeat_last_n")]
    repeat_last_n: usize,
    #[serde(default = "default_penalize_newline")]
    penalize_newline: bool,

    // Generation limits
    #[serde(default = "default_num_keep")]
    num_keep: i64,
    #[serde(default = "default_ignore_eos")]
    ignore_eos: bool,

    // DRY sampling
    #[serde(default = "default_dry_multiplier")]
    dry_multiplier: f32,
    #[serde(default = "default_dry_base")]
    dry_base: f32,
    #[serde(default = "default_dry_allowed_length")]
    dry_allowed_length: usize,

    // XTC sampling
    #[serde(default = "default_xtc_probability")]
    xtc_probability: f32,
    #[serde(default = "default_xtc_threshold")]
    xtc_threshold: f32,
}

fn default_min_p() -> f32 {
    llm_settings::DEFAULT_MIN_P
}
fn default_typical_p() -> f32 {
    llm_settings::DEFAULT_TYPICAL_P
}
fn default_tfs_z() -> f32 {
    llm_settings::DEFAULT_TFS_Z
}
fn default_mirostat() -> i32 {
    llm_settings::DEFAULT_MIROSTAT
}
fn default_mirostat_eta() -> f32 {
    llm_settings::DEFAULT_MIROSTAT_ETA
}
fn default_mirostat_tau() -> f32 {
    llm_settings::DEFAULT_MIROSTAT_TAU
}
fn default_repeat_last_n() -> usize {
    llm_settings::DEFAULT_REPEAT_LAST_N
}
fn default_num_keep() -> i64 {
    llm_settings::DEFAULT_NUM_KEEP
}
fn default_penalize_newline() -> bool {
    llm_settings::DEFAULT_PENALIZE_NEWLINE
}
fn default_ignore_eos() -> bool {
    llm_settings::DEFAULT_IGNORE_EOS
}
fn default_dry_multiplier() -> f32 {
    llm_settings::DEFAULT_DRY_MULTIPLIER
}
fn default_dry_base() -> f32 {
    llm_settings::DEFAULT_DRY_BASE
}
fn default_dry_allowed_length() -> usize {
    llm_settings::DEFAULT_DRY_ALLOWED_LENGTH
}
fn default_xtc_probability() -> f32 {
    llm_settings::DEFAULT_XTC_PROBABILITY
}
fn default_xtc_threshold() -> f32 {
    llm_settings::DEFAULT_XTC_THRESHOLD
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct HardwareConfigRequest {
    backend_type: String,
    model: String,

    // Model params
    gpu_layers: usize,
    main_gpu: usize,
    split_mode: String,
    tensor_split: Vec<f32>,
    use_mmap: bool,
    use_mlock: bool,
    vocab_only: bool,
    devices: Vec<crate::db::param_hardware::DeviceTarget>,
    kv_overrides: Vec<crate::db::param_hardware::KvOverride>,
    swa_full: bool,
    no_perf: bool,

    // Context params
    num_ctx: usize,
    num_batch: usize,
    num_ubatch: usize,
    num_seq_max: usize,
    rope_scaling_type: crate::db::param_hardware::RopeScalingType,
    rope_frequency_base: f32,
    rope_frequency_scale: f32,
    yarn_ext_factor: f32,
    yarn_attn_factor: f32,
    yarn_beta_fast: f32,
    yarn_beta_slow: f32,
    yarn_orig_ctx: usize,
    pooling_type: String,
    attention_type: String,
    flash_attn: bool,
    type_k: crate::db::param_hardware::KvDataType,
    type_v: crate::db::param_hardware::KvDataType,
    embeddings: bool,
    offload_kqv: bool,
    defrag_thold: f32,
    logits_all: bool,
    f16_kv: bool,
    low_vram: bool,

    // CPU params
    num_thread: usize,
    num_thread_batch: usize,
    numa: bool,
    cpu_strict: bool,
    cpumask: crate::db::param_hardware::CpuMask,
    mask_valid: bool,
    poll: usize,
    priority: String,

    // Legacy/custom
    num_gpu: usize,
    llama_server_url: String,
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

fn backend_type_to_string(bt: &crate::db::param_hardware::BackendType) -> String {
    use crate::db::param_hardware::BackendType;
    match bt {
        BackendType::Ollama => "ollama".to_string(),
        BackendType::LlamaCpp => "llama_cpp".to_string(),
        BackendType::OpenAi => "openai".to_string(),
        BackendType::Anthropic => "anthropic".to_string(),
        BackendType::Vllm => "vllm".to_string(),
        BackendType::Custom => "custom".to_string(),
    }
}

fn string_to_backend_type(s: &str) -> crate::db::param_hardware::BackendType {
    use crate::db::param_hardware::BackendType;
    match s {
        "ollama" => BackendType::Ollama,
        "llama_cpp" => BackendType::LlamaCpp,
        "openai" => BackendType::OpenAi,
        "anthropic" => BackendType::Anthropic,
        "vllm" => BackendType::Vllm,
        "custom" => BackendType::Custom,
        _ => BackendType::Ollama, // default fallback
    }
}

#[derive(Debug, Serialize)]
struct LlmConfigResponse {
    status: String,
    message: String,
    request_id: String,
    config: LlmConfig,
}

#[derive(Debug, Serialize)]
struct HardwareConfigResponse {
    status: String,
    message: String,
    request_id: String,
    config: HardwareConfigRequest,
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
}

#[derive(Debug, Serialize)]
struct OnnxConfigResponse {
    status: String,
    message: String,
    request_id: String,
    config: OnnxConfigInfo,
}

#[derive(Debug, Serialize)]
struct OnnxConfigInfo {
    model_path: String,
    max_length: usize,
    embedding_dim: usize,
    num_threads: usize,
    inter_op_num_threads: usize,
    optimization_level: String,
    execution_mode: String,
    enable_mem_pattern: bool,
    enable_cpu_mem_arena: bool,
    deterministic_compute: bool,
    optimized_model_path: Option<String>,
    enable_profiling: bool,
    profiling_output_path: Option<String>,
    log_id: Option<String>,
    log_level: String,
    log_verbosity: i32,
    use_env_allocators: bool,
    denormal_as_zero: bool,
    enable_quant_qdq: bool,
    enable_double_qdq_remover: bool,
    enable_qdq_cleanup: bool,
    approximate_gelu: bool,
    enable_aot_inlining: bool,
    disabled_optimizers: Vec<String>,
    use_device_allocator_for_initializers: bool,
    allow_inter_op_spinning: bool,
    allow_intra_op_spinning: bool,
    use_prepacking: bool,
    independent_thread_pool: bool,
    no_env_execution_providers: bool,
}

#[derive(Serialize)]
struct LogEntry {
    timestamp: Option<String>,
    level: Option<String>,
    target: Option<String>,
    message: Option<String>,
    raw: String,
    fields: Option<Value>,
}

#[derive(Serialize)]
struct LogsResponse {
    request_id: String,
    file: Option<String>,
    entries: Vec<LogEntry>,
    note: Option<String>,
}

/// Generate a short request ID for correlation
fn generate_request_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

fn validate_chunk_request(req: &ChunkConfigCommitRequest) -> Result<(), String> {
    if req.min_size == 0 {
        return Err("min_size must be greater than 0".into());
    }
    if req.min_size > req.target_size {
        return Err("min_size cannot exceed target_size".into());
    }
    if req.target_size > req.max_size {
        return Err("target_size cannot exceed max_size".into());
    }
    if req.overlap >= req.target_size {
        return Err("overlap must be smaller than target_size".into());
    }
    if req.max_size == 0 {
        return Err("max_size must be greater than 0".into());
    }
    if req
        .semantic_similarity_threshold
        .map_or(false, |v| !(0.0..=1.0).contains(&v))
    {
        return Err("semantic_similarity_threshold must be between 0 and 1".into());
    }
    Ok(())
}

fn validate_llm_request(req: &LlmConfigRequest) -> Result<(), String> {
    if !(0.0..=2.0).contains(&req.temperature) {
        return Err("temperature must be between 0 and 2".into());
    }
    if !(0.0..=1.0).contains(&req.top_p) {
        return Err("top_p must be between 0 and 1".into());
    }
    if req.top_k == 0 {
        return Err("top_k must be greater than 0".into());
    }
    if req.max_tokens == 0 {
        return Err("max_tokens must be greater than 0".into());
    }
    if req.repeat_penalty < 1.0 {
        return Err("repeat_penalty must be at least 1.0".into());
    }
    if !(0.0..=2.0).contains(&req.frequency_penalty) {
        return Err("frequency_penalty must be between 0 and 2".into());
    }
    if !(0.0..=2.0).contains(&req.presence_penalty) {
        return Err("presence_penalty must be between 0 and 2".into());
    }
    if !(0.0..=1.0).contains(&req.min_p) {
        return Err("min_p must be between 0 and 1".into());
    }
    if !(0.0..=1.0).contains(&req.typical_p) {
        return Err("typical_p must be between 0 and 1".into());
    }
    if !(0.0..=1.0).contains(&req.tfs_z) {
        return Err("tfs_z must be between 0 and 1".into());
    }
    if !(0..=2).contains(&req.mirostat) {
        return Err("mirostat must be 0, 1, or 2".into());
    }
    if !(0.0..=1.0).contains(&req.mirostat_eta) {
        return Err("mirostat_eta must be between 0 and 1".into());
    }
    if !(0.0..=10.0).contains(&req.mirostat_tau) {
        return Err("mirostat_tau must be between 0 and 10".into());
    }
    if req.repeat_last_n == 0 {
        return Err("repeat_last_n must be greater than 0".into());
    }
    Ok(())
}

fn validate_hardware_request(req: &HardwareConfigRequest) -> Result<(), String> {
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

// Track previous health status for change detection
static LAST_HEALTH_STATUS: std::sync::OnceLock<std::sync::Mutex<String>> =
    std::sync::OnceLock::new();

/// Write to status-specific log file
fn write_status_log(status: &str, reason: &str, is_change: bool) {
    use std::io::Write;

    // Get log directory
    let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/.agentic-rag/logs", home)
    });

    // Create log directory if needed
    let _ = std::fs::create_dir_all(&log_dir);

    // Status to filename mapping
    let filename = match status {
        "healthy" => "status_healthy.log",
        "busy" => "status_busy.log",
        "degraded" => "status_degraded.log",
        "unhealthy" => "status_unhealthy.log",
        "offline" => "status_offline.log",
        "checking" => "status_checking.log",
        _ => "status_unknown.log",
    };

    let log_path = format!("{}/{}", log_dir, filename);

    // Format log entry
    let timestamp = chrono::Utc::now().to_rfc3339();
    let change_type = if is_change { "CHANGED" } else { "INIT" };
    let entry = format!(
        "[{}] [{}] {} | {}\n",
        timestamp, change_type, status, reason
    );

    // Append to status-specific log file
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = file.write_all(entry.as_bytes());
    }
}

fn log_status_change(new_status: &str, reason: &str) {
    let status_lock = LAST_HEALTH_STATUS.get_or_init(|| std::sync::Mutex::new(String::new()));
    let mut last_status = status_lock.lock().unwrap();

    if *last_status != new_status {
        let is_change = !last_status.is_empty();

        // Write to status-specific log file
        write_status_log(new_status, reason, is_change);

        // Also log to main log
        if is_change {
            warn!(
                "Health status changed: {} -> {} | Reason: {}",
                last_status, new_status, reason
            );
        } else {
            info!(
                "Health status initialized: {} | Reason: {}",
                new_status, reason
            );
        }
        *last_status = new_status.to_string();
    }
}

/// Get status log file content
pub async fn get_systemd_logs(
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let unit = query.get("unit").cloned().unwrap_or_else(|| "ag-full-stack.service".to_string());
    let limit = query.get("limit").and_then(|l| l.parse::<usize>().ok()).unwrap_or(100);

    // Validate unit name to prevent injection
    if unit.contains("..") || unit.contains('/') || unit.contains(';') {
        return Ok(HttpResponse::BadRequest().json(json!({"error": "Invalid unit name"})));
    }

    let output = tokio::process::Command::new("journalctl")
        .args(["--user", "-u", &unit, "-n", &limit.to_string(), "--no-pager", "--output=short-iso"])
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let content = if stdout.is_empty() { stderr } else { stdout };
            let lines: Vec<&str> = content.lines().collect();
            Ok(HttpResponse::Ok().json(json!({
                "unit": unit,
                "limit": limit,
                "total_lines": lines.len(),
                "content": content,
            })))
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to run journalctl: {}", e)
        }))),
    }
}

pub async fn get_status_log(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let status = path.into_inner();

    // Validate status name to prevent path traversal
    let valid_statuses = [
        "healthy",
        "busy",
        "degraded",
        "unhealthy",
        "offline",
        "checking",
        "unknown",
        "initial",
    ];
    if !valid_statuses.contains(&status.as_str()) {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "Invalid status name",
            "valid_statuses": valid_statuses
        })));
    }

    // Get log directory
    let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/.agentic-rag/logs", home)
    });

    let filename = format!("status_{}.log", status);
    let log_path = format!("{}/{}", log_dir, filename);

    // Read log file
    match std::fs::read_to_string(&log_path) {
        Ok(content) => {
            // Return last 100 lines (most recent entries)
            let lines: Vec<&str> = content.lines().collect();
            let start = if lines.len() > 100 {
                lines.len() - 100
            } else {
                0
            };
            let recent_lines = lines[start..].join("\n");

            Ok(HttpResponse::Ok().json(json!({
                "status": status,
                "log_path": log_path,
                "total_lines": lines.len(),
                "showing_lines": lines.len() - start,
                "content": recent_lines
            })))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HttpResponse::Ok().json(json!({
            "status": status,
            "log_path": log_path,
            "total_lines": 0,
            "showing_lines": 0,
            "content": "",
            "message": "No log entries yet for this status"
        }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to read log file: {}", e),
            "log_path": log_path
        }))),
    }
}

pub async fn health_check() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    // Check ONNX model exists
    let onnx_model_path = std::env::var("ONNX_MODEL_PATH")
        .unwrap_or_else(|_| "models/embedding_model.onnx".to_string());
    let onnx_ready = std::path::Path::new(&onnx_model_path).exists();

    if !onnx_ready {
        let reason = format!("ONNX model not found at: {}", onnx_model_path);
        log_status_change("unhealthy", &reason);
        return Ok(HttpResponse::ServiceUnavailable().json(json!({
            "status": "unhealthy",
            "error": reason,
            "request_id": request_id
        })));
    }

    // Get load metrics from global health tracker
    let (load, is_busy, message) = if let Some(tracker) = crate::monitoring::get_health_tracker() {
        let load = tracker.get_load_metrics();
        let is_busy = tracker.is_busy();
        let message = if is_busy {
            Some(format!(
                "System busy: {} active tasks{}{}",
                load.active_tasks,
                if load.indexing { ", indexing" } else { "" },
                if load.llm_active {
                    ", LLM processing"
                } else {
                    ""
                }
            ))
        } else {
            None
        };
        (Some(load), is_busy, message)
    } else {
        (None, false, None)
    };

    // Check Neo4j status if enabled
    #[cfg(feature = "neo4j")]
    let neo4j_status: Option<(bool, bool)> = {
        let config = crate::graph::config::GraphConfig::from_env();
        if config.enabled {
            if let Some(client) = get_neo4j_client() {
                match client.health_check().await {
                    Ok(connected) => Some((true, connected)),
                    Err(_) => Some((true, false)),
                }
            } else {
                Some((true, false)) // Enabled but client not initialized
            }
        } else {
            None // Disabled
        }
    };

    #[cfg(not(feature = "neo4j"))]
    let neo4j_status: Option<(bool, bool)> = None;

    if let Some(retriever) = RETRIEVER.get() {
        let retriever = retriever.lock().unwrap();
        match retriever.health_check() {
            Ok(()) => {
                // Check if Neo4j is enabled but not connected
                let neo4j_issue = match neo4j_status {
                    Some((true, false)) => true, // Enabled but not connected
                    _ => false,
                };

                let status = if neo4j_issue {
                    "degraded"
                } else if is_busy {
                    "busy"
                } else {
                    "healthy"
                };

                let reason = if neo4j_issue {
                    "Neo4j enabled but not connected".to_string()
                } else if is_busy {
                    message.clone().unwrap_or_else(|| "System busy".to_string())
                } else {
                    format!(
                        "All systems operational ({} docs, {} vectors)",
                        retriever.metrics.total_documents_indexed, retriever.metrics.total_vectors
                    )
                };
                log_status_change(status, &reason);

                let mut response = json!({
                    "status": status,
                    "documents": retriever.metrics.total_documents_indexed,
                    "vectors": retriever.metrics.total_vectors,
                    "index_path": retriever.metrics.index_path,
                    "request_id": request_id
                });

                // Add load metrics if available
                if let Some(load) = load {
                    response["load"] = json!({
                        "cpu_percent": load.cpu_percent,
                        "memory_percent": load.memory_percent,
                        "active_tasks": load.active_tasks,
                        "queue_depth": load.queue_depth,
                        "indexing": load.indexing,
                        "llm_active": load.llm_active
                    });
                }

                // Add message if busy
                if let Some(msg) = message {
                    response["message"] = json!(msg);
                }

                // Add Neo4j status
                if let Some((enabled, connected)) = neo4j_status {
                    response["neo4j"] = json!({
                        "enabled": enabled,
                        "connected": connected
                    });
                }

                Ok(HttpResponse::Ok().json(response))
            }
            Err(e) => {
                let reason = format!("Retriever health check failed: {}", e);
                log_status_change("unhealthy", &reason);
                error!("[{}] {}", request_id, reason);
                Ok(HttpResponse::ServiceUnavailable().json(json!({
                    "status": "unhealthy",
                    "error": e.to_string(),
                    "request_id": request_id
                })))
            }
        }
    } else {
        let reason = "Retriever not initialized";
        log_status_change("unhealthy", reason);
        error!("[{}] Health check failed: {}", request_id, reason);
        Ok(HttpResponse::ServiceUnavailable().json(json!({
            "status": "unhealthy",
            "error": reason,
            "request_id": request_id
        })))
    }
}

async fn root_handler() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body("✅ Backend is running (Actix Web)\n\nTry /health or /ready\n"))
}

async fn ready_check() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    if let Some(retriever) = RETRIEVER.get() {
        match retriever.lock() {
            Ok(retriever) => match retriever.ready_check() {
                Ok(_) => Ok(HttpResponse::Ok().json(json!({
                    "status": "ready",
                    "timestamp": Utc::now().to_rfc3339(),
                    "request_id": request_id
                }))),
                Err(e) => Ok(HttpResponse::ServiceUnavailable().json(json!({
                    "status": "not ready",
                    "error": e.to_string(),
                    "timestamp": Utc::now().to_rfc3339(),
                    "request_id": request_id
                }))),
            },
            Err(e) => Ok(HttpResponse::ServiceUnavailable().json(json!({
                "status": "not ready",
                "error": format!("Failed to acquire lock: {}", e),
                "timestamp": Utc::now().to_rfc3339(),
                "request_id": request_id
            }))),
        }
    } else {
        Ok(HttpResponse::ServiceUnavailable().json(json!({
            "status": "not ready",
            "message": "Retriever not initialized",
            "timestamp": Utc::now().to_rfc3339(),
            "request_id": request_id
        })))
    }
}

/// Phase 16: Export metrics in Prometheus text format
/// GET /monitoring/metrics
/// Returns: Prometheus-compliant text format metrics
async fn get_metrics() -> Result<HttpResponse, Error> {
    // Export metrics in Prometheus text format (not JSON)
    // Phase 16 Step 3: OTLP Exporting - Prometheus format compliance
    let prometheus_text = crate::monitoring::metrics::export_prometheus();

    Ok(HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(prometheus_text))
}

/// GET /monitoring/optimizations
/// Returns: Statistics about all performance optimizations
async fn get_optimization_stats() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(retriever) = RETRIEVER.get() {
        let retriever = retriever.lock().unwrap();
        let stats = retriever.get_optimization_stats();

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "request_id": request_id,
            "optimizations": stats,
            "modules": {
                "simd": "4-8x faster cosine similarity",
                "bloom_filter": "O(1) document existence checks",
                "hnsw_index": "O(log n) approximate nearest neighbor",
                "semantic_cache": "Cache similar queries",
                "hybrid_search": "BM25 + vector fusion",
                "sqlite_wal": "10-100x faster concurrent writes",
                "mmap": "Zero-copy vector loading",
                "rkyv": "20-40x faster serialization",
                "lz4": "2x compression for vectors",
            }
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

/// GET /monitoring/io-uring
/// Returns: io_uring async I/O statistics, configuration, and availability
async fn get_io_uring_stats() -> Result<HttpResponse, Error> {
    use crate::perf::io_uring as async_io;

    let request_id = generate_request_id();
    let stats = async_io::get_stats();
    let config = async_io::get_config();

    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "request_id": request_id,
        "io_uring": {
            "available": async_io::is_available(),
            "feature_enabled": async_io::is_feature_enabled(),
            "backend": async_io::backend_name(),
            "config": {
                // Category 1: Queue & Buffers
                "ring_size": config.ring_size,
                "cq_size": config.cq_size,
                "buffer_size": config.buffer_size,
                "buffer_pool_size": config.buffer_pool_size,
                "clamp": config.clamp,
                // Category 2: Polling
                "sqpoll": config.sqpoll,
                "sqpoll_idle_ms": config.sqpoll_idle_ms,
                "sqpoll_cpu": config.sqpoll_cpu,
                "iopoll": config.iopoll,
                // Category 3: Optimization
                "single_issuer": config.single_issuer,
                "coop_taskrun": config.coop_taskrun,
                "defer_taskrun": config.defer_taskrun,
                "submit_all": config.submit_all,
                "taskrun_flag": config.taskrun_flag,
                // Category 4: Advanced
                "r_disabled": config.r_disabled,
                "attach_wq_fd": config.attach_wq_fd,
                "dontfork": config.dontfork
            },
            "stats": {
                "reads": stats.get_reads(),
                "writes": stats.get_writes(),
                "bytes_read": stats.get_bytes_read(),
                "bytes_written": stats.get_bytes_written(),
                "read_errors": stats.get_read_errors(),
                "write_errors": stats.get_write_errors(),
                "total_errors": stats.get_total_errors()
            },
            "env_vars": {
                "IO_URING_RING_SIZE": "Submission/completion queue size (1-32768, power of 2)",
                "IO_URING_BUFFER_SIZE": "Read/write buffer size in bytes (4096-16MB)",
                "IO_URING_SQPOLL": "Enable kernel SQ polling thread (true/false)",
                "IO_URING_SQPOLL_IDLE_MS": "SQ poll thread idle timeout in ms",
                "IO_URING_BUFFER_POOL_SIZE": "Number of pre-allocated buffers (1-4096)",
                "IO_URING_SINGLE_ISSUER": "Single issuer optimization (true/false)"
            },
            "description": "io_uring provides 2-3x faster file I/O on Linux 5.1+",
            "available_functions": {
                "vector_loading": "load_vectors_rkyv_async() / load_vectors_auto_async()",
                "cache_loading": "load_search_cache_async()",
                "document_ingestion": "index_file_async() / extract_text_async()",
                "file_read": "perf::io_uring::read_file()",
                "file_write": "perf::io_uring::write_file()",
                "batch_read": "perf::io_uring::read_files()"
            },
            "current_usage": {
                "startup_vector_load": "mmap (zero-copy, already optimal)",
                "upload_indexing": "io_uring via extract_text_async()",
                "reindex": "io_uring via index_all_documents_async()",
                "note": "All file reads now use io_uring on Linux 5.1+ for 2-3x speedup"
            }
        }
    })))
}

/// POST /monitoring/log-frontend-error
/// Log frontend errors so they appear in the log viewer
/// This allows page errors to be visible when filtering logs by color (red for errors)
async fn log_frontend_error(body: web::Json<serde_json::Value>) -> Result<HttpResponse, Error> {
    let page = body
        .get("page")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let error = body
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown error");
    let level = body
        .get("level")
        .and_then(|v| v.as_str())
        .unwrap_or("error");

    // Log at the appropriate level so it appears in log filtering
    match level {
        "warn" | "warning" => {
            tracing::warn!(page = %page, "Frontend error: {}", error);
        }
        _ => {
            tracing::error!(page = %page, "Frontend error: {}", error);
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "logged",
        "page": page,
        "level": level
    })))
}

// ============================================================================
// DOCKER MONITORING
// ============================================================================

/// Docker container info
#[derive(Debug, Clone, Serialize)]
struct DockerContainer {
    name: String,
    image: String,
    status: String,
    state: String,
    ports: Vec<String>,
    created: String,
    health: Option<String>,
}

/// Docker stats for a container
#[derive(Debug, Clone, Serialize)]
struct DockerStats {
    name: String,
    cpu_percent: f64,
    memory_usage: String,
    memory_limit: String,
    memory_percent: f64,
    network_rx: String,
    network_tx: String,
}

/// GET /monitoring/docker
/// Returns Docker container status and stats for ag infrastructure
async fn get_docker_status() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    // Try to get Docker container info by running docker commands
    // This runs docker ps and docker stats to get container info

    let containers = get_docker_containers().await;
    let stats = get_docker_stats().await;
    let docker_available = !containers.is_empty() || check_docker_available().await;

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "request_id": request_id,
        "docker_available": docker_available,
        "containers": containers,
        "stats": stats
    })))
}

/// Check if Docker is available
async fn check_docker_available() -> bool {
    match tokio::process::Command::new("docker")
        .args(["info"])
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .output()
        .await
    {
        Ok(output) => {
            if output.status.success() {
                return true;
            }
            // Check if it's a permission error
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("permission denied") {
                warn!("Docker permission denied. Add user to docker group: sudo usermod -aG docker $USER");
            }
            false
        }
        Err(_) => false,
    }
}

/// Get Docker container list
async fn get_docker_containers() -> Vec<DockerContainer> {
    // Run: docker ps -a --filter "name=ag-" --format json
    let output = match tokio::process::Command::new("docker")
        .args(["ps", "-a", "--filter", "name=ag-", "--format", "{{json .}}"])
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to run docker ps: {}", e);
            return Vec::new();
        }
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut containers = Vec::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            let name = json["Names"].as_str().unwrap_or("").to_string();
            let image = json["Image"].as_str().unwrap_or("").to_string();
            let status = json["Status"].as_str().unwrap_or("").to_string();
            let state = json["State"].as_str().unwrap_or("").to_string();
            let ports_str = json["Ports"].as_str().unwrap_or("");
            let created = json["CreatedAt"].as_str().unwrap_or("").to_string();

            // Parse ports
            let ports: Vec<String> = if ports_str.is_empty() {
                Vec::new()
            } else {
                ports_str
                    .split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            };

            // Extract health from status if present
            let health = if status.contains("(healthy)") {
                Some("healthy".to_string())
            } else if status.contains("(unhealthy)") {
                Some("unhealthy".to_string())
            } else if status.contains("(health: starting)") {
                Some("starting".to_string())
            } else {
                None
            };

            containers.push(DockerContainer {
                name,
                image,
                status,
                state,
                ports,
                created,
                health,
            });
        }
    }

    containers
}

/// Get Docker container stats
async fn get_docker_stats() -> Vec<DockerStats> {
    // Run: docker stats --no-stream --format json for ag containers
    let output = match tokio::process::Command::new("docker")
        .args(["stats", "--no-stream", "--format", "{{json .}}"])
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to run docker stats: {}", e);
            return Vec::new();
        }
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut stats = Vec::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            let name = json["Name"].as_str().unwrap_or("").to_string();

            // Only include ag containers
            if !name.starts_with("ag-") {
                continue;
            }

            let cpu_str = json["CPUPerc"].as_str().unwrap_or("0%");
            let cpu_percent = cpu_str.trim_end_matches('%').parse::<f64>().unwrap_or(0.0);

            let mem_usage = json["MemUsage"].as_str().unwrap_or("0B / 0B").to_string();
            let (memory_usage, memory_limit) = if mem_usage.contains(" / ") {
                let parts: Vec<&str> = mem_usage.split(" / ").collect();
                (
                    parts.get(0).unwrap_or(&"0B").to_string(),
                    parts.get(1).unwrap_or(&"0B").to_string(),
                )
            } else {
                (mem_usage.clone(), "0B".to_string())
            };

            let mem_perc_str = json["MemPerc"].as_str().unwrap_or("0%");
            let memory_percent = mem_perc_str
                .trim_end_matches('%')
                .parse::<f64>()
                .unwrap_or(0.0);

            let net_io = json["NetIO"].as_str().unwrap_or("0B / 0B").to_string();
            let (network_rx, network_tx) = if net_io.contains(" / ") {
                let parts: Vec<&str> = net_io.split(" / ").collect();
                (
                    parts.get(0).unwrap_or(&"0B").to_string(),
                    parts.get(1).unwrap_or(&"0B").to_string(),
                )
            } else {
                (net_io.clone(), "0B".to_string())
            };

            stats.push(DockerStats {
                name,
                cpu_percent,
                memory_usage,
                memory_limit,
                memory_percent,
                network_rx,
                network_tx,
            });
        }
    }

    stats
}

// ============================================================================
// RUNTIME ACTIONS (LLM runtime control)
// ============================================================================

#[derive(Debug, serde::Deserialize)]
struct RuntimeActionRequest {
    action: String,
}

/// POST /monitoring/runtime/action
/// Stop/start the LLM runtime (currently Ollama via systemd).
///
/// Notes:
/// - This requires the backend process user to have permission to run `systemctl` for ollama
///   without an interactive password prompt.
async fn runtime_action(body: web::Json<RuntimeActionRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let action = body.action.as_str();

    let args: Vec<&str> = match action {
        "stop" => vec!["stop", "ollama.service"],
        "start" => vec!["start", "ollama.service"],
        _ => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "status": "error",
                "request_id": request_id,
                "error": format!("Unknown runtime action: {}", action),
            })));
        }
    };

    // Use the user systemd manager so we can control user-level runtimes without sudo.
    let output = tokio::process::Command::new("systemctl")
        .arg("--user")
        .args(&args)
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            if out.status.success() {
                Ok(HttpResponse::Ok().json(json!({
                    "status": "ok",
                    "request_id": request_id,
                    "action": action,
                    "stdout": stdout,
                    "stderr": stderr,
                })))
            } else {
                Ok(HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "request_id": request_id,
                    "action": action,
                    "stdout": stdout,
                    "stderr": stderr,
                })))
            }
        }
        Err(err) => Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "request_id": request_id,
            "action": action,
            "error": format!("Failed to execute systemctl: {}", err),
        }))),
    }
}

// ============================================================================
// DOCKER ACTIONS
// ============================================================================

/// Docker action request
#[derive(Debug, serde::Deserialize)]
struct DockerActionRequest {
    action: String,
    container: Option<String>,
}

/// POST /monitoring/docker/action
/// Execute docker compose actions (restart, stop, start, logs)
async fn docker_action(body: web::Json<DockerActionRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let action = &body.action;
    let container = body.container.as_deref();

    info!(
        "Docker action requested: {} container={:?}",
        action, container
    );

    let (cmd, args): (&str, Vec<&str>) = match action.as_str() {
        "restart" => {
            if let Some(c) = container {
                ("docker", vec!["restart", c])
            } else {
                (
                    "docker",
                    vec!["compose", "-f", "docker-compose.yml", "restart"],
                )
            }
        }
        "stop" => {
            if let Some(c) = container {
                ("docker", vec!["stop", c])
            } else {
                (
                    "docker",
                    vec!["compose", "-f", "docker-compose.yml", "stop"],
                )
            }
        }
        "start" => {
            if let Some(c) = container {
                ("docker", vec!["start", c])
            } else {
                (
                    "docker",
                    vec!["compose", "-f", "docker-compose.yml", "up", "-d"],
                )
            }
        }
        "down" => (
            "docker",
            vec!["compose", "-f", "docker-compose.yml", "down"],
        ),
        "up" => (
            "docker",
            vec!["compose", "-f", "docker-compose.yml", "up", "-d"],
        ),
        _ => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "status": "error",
                "request_id": request_id,
                "error": format!("Unknown action: {}", action)
            })));
        }
    };

    // Execute the command
    let output = tokio::process::Command::new(cmd)
        .args(&args)
        .current_dir("/home/pde/ag")
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let success = output.status.success();

            if success {
                info!("Docker action {} completed successfully", action);
                Ok(HttpResponse::Ok().json(json!({
                    "status": "ok",
                    "request_id": request_id,
                    "action": action,
                    "success": true,
                    "stdout": stdout,
                    "stderr": stderr
                })))
            } else {
                warn!("Docker action {} failed: {}", action, stderr);
                Ok(HttpResponse::Ok().json(json!({
                    "status": "error",
                    "request_id": request_id,
                    "action": action,
                    "success": false,
                    "stdout": stdout,
                    "stderr": stderr
                })))
            }
        }
        Err(e) => {
            error!("Failed to execute docker action {}: {}", action, e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "request_id": request_id,
                "error": format!("Failed to execute: {}", e)
            })))
        }
    }
}

/// POST /monitoring/io-uring
/// Save io_uring configuration to .env file
async fn save_io_uring_config(body: web::Json<serde_json::Value>) -> Result<HttpResponse, Error> {
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

/// POST /monitoring/optimizations/build-hnsw
/// Build HNSW index for O(log n) search
async fn build_hnsw_index() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        let start = std::time::Instant::now();
        retriever.build_hnsw_index();
        let elapsed = start.elapsed().as_millis();

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "request_id": request_id,
            "message": "HNSW index built",
            "index_size": retriever.hnsw_index_size(),
            "build_time_ms": elapsed
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

/// POST /monitoring/optimizations/build-pq
/// Build Product Quantization index for 16x memory reduction
async fn build_pq_index() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        let start = std::time::Instant::now();
        retriever.build_pq_index();
        let elapsed = start.elapsed().as_millis();

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "request_id": request_id,
            "message": "PQ index built (16x compression)",
            "build_time_ms": elapsed
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

/// POST /monitoring/optimizations/build-fp16
/// Build FP16 vector store for 2x memory reduction
async fn build_fp16_store() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        let start = std::time::Instant::now();
        retriever.build_fp16_store();
        let elapsed = start.elapsed().as_millis();

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "request_id": request_id,
            "message": "FP16 store built (2x compression)",
            "build_time_ms": elapsed
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

/// POST /monitoring/optimizations/build-all
/// Build all optimization indexes
async fn build_all_indexes() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        let start = std::time::Instant::now();

        retriever.build_hnsw_index();
        retriever.build_pq_index();
        retriever.build_fp16_store();

        let elapsed = start.elapsed().as_millis();
        let stats = retriever.get_optimization_stats();

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "request_id": request_id,
            "message": "All optimization indexes built",
            "build_time_ms": elapsed,
            "stats": stats
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

/// GET /config/embedding
/// Returns current embedding configuration (ONNX only)
async fn get_embedding_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    let onnx_model_path = std::env::var("ONNX_MODEL_PATH")
        .unwrap_or_else(|_| "models/embedding_model.onnx".to_string());
    let onnx_available = std::path::Path::new(&onnx_model_path).exists();

    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "request_id": request_id,
        "provider": "onnx",
        "model_path": onnx_model_path,
        "model_exists": onnx_available,
        "ready": onnx_available,
        "note": "ONNX is the only supported embedding provider"
    })))
}

/// POST /config/embedding - No longer needed (ONNX only)
async fn set_embedding_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    Ok(HttpResponse::Ok().json(json!({
        "status": "info",
        "request_id": request_id,
        "message": "ONNX is the only supported embedding provider. No configuration needed."
    })))
}

/// Self-contained UI metrics: HTTP Requests summary + chart
/// GET /monitoring/ui/requests
/// Returns: JSON with rate, p95 latency, error%, and recent points
async fn get_ui_requests() -> Result<HttpResponse, Error> {
    let snapshot = crate::monitoring::get_requests_snapshot();
    Ok(HttpResponse::Ok().json(snapshot))
}

async fn get_chunking_stats(query: web::Query<ChunkingQuery>) -> Result<HttpResponse, Error> {
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

async fn toggle_chunking_logging(query: web::Query<LoggingQuery>) -> Result<HttpResponse, Error> {
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
async fn get_chunk_config() -> Result<HttpResponse, Error> {
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

async fn commit_chunk_config(
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

    let new_cfg = ChunkerConfig {
        target_size: body.target_size,
        min_size: body.min_size,
        max_size: body.max_size,
        overlap: body.overlap,
        semantic_similarity_threshold: body
            .semantic_similarity_threshold
            .unwrap_or_else(|| chunk_settings::global_config().semantic_similarity_threshold),
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

async fn get_llm_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = llm_settings::global_config();
    Ok(HttpResponse::Ok().json(LlmConfigResponse {
        status: "ok".into(),
        message: "Current LLM configuration".into(),
        request_id,
        config,
    }))
}

async fn commit_llm_config(payload: web::Json<LlmConfigRequest>) -> Result<HttpResponse, Error> {
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
struct PromptCachingResponse {
    status: String,
    message: String,
    request_id: String,
    enabled: bool,
}

#[derive(Debug, serde::Deserialize)]
struct PromptCachingRequest {
    enabled: bool,
}

/// Get current prompt caching state
/// When enabled, uses /api/chat (with KV caching) instead of /api/generate
async fn get_prompt_caching() -> Result<HttpResponse, Error> {
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
async fn set_prompt_caching(
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

// ============================================================================
// TRAINING DATA COLLECTION ENDPOINTS (Phase 20)
// ============================================================================

use crate::training::{TrainingDataCollector, TrainingExample, TrainingStats};

static TRAINING_COLLECTOR: OnceLock<TrainingDataCollector> = OnceLock::new();
static LORA_EXPORT_STATE: OnceLock<Arc<Mutex<LoraExportState>>> = OnceLock::new();
static LORA_FILTER_OVERRIDE: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();
static AUTO_EXPORT_OVERRIDES: OnceLock<Arc<Mutex<AutoExportOverrides>>> = OnceLock::new();
static SYNTHETIC_QA_STATE: OnceLock<Arc<Mutex<SyntheticQaState>>> = OnceLock::new();

fn training_collector() -> &'static TrainingDataCollector {
    TRAINING_COLLECTOR.get_or_init(TrainingDataCollector::default)
}

fn lora_export_state() -> Arc<Mutex<LoraExportState>> {
    LORA_EXPORT_STATE
        .get_or_init(|| {
            Arc::new(Mutex::new(LoraExportState {
                running: false,
                last_started: None,
                last_finished: None,
                last_error: None,
            }))
        })
        .clone()
}

fn lora_filter_override() -> Arc<Mutex<Option<String>>> {
    LORA_FILTER_OVERRIDE
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone()
}

#[derive(Debug)]
struct LoraExportState {
    running: bool,
    last_started: Option<DateTime<Utc>>,
    last_finished: Option<DateTime<Utc>>,
    last_error: Option<String>,
}

#[derive(Debug)]
struct SyntheticQaState {
    running: bool,
    last_started: Option<DateTime<Utc>>,
    last_finished: Option<DateTime<Utc>>,
    last_error: Option<String>,
    examples_generated: Option<usize>,
    questions_per_chunk: u32,
    max_chunks: Option<usize>,
}

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

fn synthetic_qa_state() -> Arc<Mutex<SyntheticQaState>> {
    SYNTHETIC_QA_STATE
        .get_or_init(|| Arc::new(Mutex::new(SyntheticQaState::default())))
        .clone()
}

#[derive(Debug, Default)]
struct AutoExportOverrides {
    auto_export_enabled: Option<bool>,
    debounce_ms: Option<u64>,
}

#[derive(Debug, serde::Deserialize)]
struct TrainingFeedbackRequest {
    query: String,
    response: String,
    context: Option<String>,
    quality_score: u8,
    conversation_id: Option<String>,
    mode: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Serialize)]
struct TrainingFeedbackResponse {
    status: String,
    example_id: String,
    message: String,
    request_id: String,
}

/// POST /training/feedback - Submit user feedback for training data collection
async fn submit_training_feedback(
    payload: web::Json<TrainingFeedbackRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let body = payload.into_inner();
    let collector = training_collector();

    if !collector.is_enabled() {
        return Ok(HttpResponse::Ok().json(TrainingFeedbackResponse {
            status: "skipped".into(),
            example_id: String::new(),
            message: "Training data collection is disabled".into(),
            request_id,
        }));
    }

    let example_id = uuid::Uuid::new_v4().to_string();
    let example = TrainingExample {
        id: example_id.clone(),
        instruction: body.query,
        context: body.context,
        response: body.response,
        quality_score: Some(body.quality_score.clamp(1, 5)),
        timestamp: chrono::Utc::now(),
        conversation_id: body.conversation_id,
        mode: body.mode,
        model: body.model,
    };

    match collector.add_example(example) {
        Ok(_) => {
            tracing::info!(
                example_id = %example_id,
                quality = body.quality_score,
                "Training feedback collected"
            );
            Ok(HttpResponse::Ok().json(TrainingFeedbackResponse {
                status: "collected".into(),
                example_id,
                message: "Thank you for your feedback!".into(),
                request_id,
            }))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to collect training feedback");
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save feedback: {}", e),
                "request_id": request_id
            })))
        }
    }
}

#[derive(Debug, Serialize)]
struct TrainingStatsResponse {
    status: String,
    request_id: String,
    stats: TrainingStats,
    collection_enabled: bool,
}

/// GET /training/stats - Get training data collection statistics
async fn get_training_stats() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let collector = training_collector();

    match collector.get_stats() {
        Ok(stats) => Ok(HttpResponse::Ok().json(TrainingStatsResponse {
            status: "ok".into(),
            request_id,
            stats,
            collection_enabled: collector.is_enabled(),
        })),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get training stats");
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to get stats: {}", e),
                "request_id": request_id
            })))
        }
    }
}

#[derive(Debug, Serialize)]
struct TrainingExportResponse {
    status: String,
    request_id: String,
    exported_count: usize,
    output_path: String,
    message: String,
}

/// POST /training/export - Export collected data for Unsloth training
async fn export_training_data() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let collector = training_collector();

    // Determine export path
    let export_path = std::env::var("TRAINING_EXPORT_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("ag")
                .join("training")
                .join("training_data.jsonl")
        });

    // Ensure parent directory exists
    if let Some(parent) = export_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match collector.export_for_unsloth(&export_path) {
        Ok(count) => {
            tracing::info!(
                count = count,
                path = ?export_path,
                "Training data exported"
            );
            Ok(HttpResponse::Ok().json(TrainingExportResponse {
                status: "ok".into(),
                request_id,
                exported_count: count,
                output_path: export_path.to_string_lossy().to_string(),
                message: format!("Exported {} examples for Unsloth training", count),
            }))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to export training data");
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to export: {}", e),
                "request_id": request_id
            })))
        }
    }
}

#[derive(Debug, Serialize)]
struct SnapshotExportResponse {
    status: String,
    request_id: String,
    message: String,
}

/// POST /training/export_snapshot - Run LoRA dataset export + normalization scripts
async fn export_lora_snapshot() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    match spawn_lora_export_job(true).await {
        Ok(()) => Ok(HttpResponse::Ok().json(SnapshotExportResponse {
            status: "ok".into(),
            request_id,
            message: "LoRA snapshot export started".into(),
        })),
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(SnapshotExportResponse {
                status: "error".into(),
                request_id,
                message: e,
            }),
        ),
    }
}

async fn spawn_lora_export_job(force: bool) -> Result<(), String> {
    use tokio::task;

    let state_handle = lora_export_state();

    {
        let mut state = state_handle
            .lock()
            .map_err(|_| "Failed to acquire export state".to_string())?;

        if state.running {
            if force {
                return Err("LoRA export already in progress".to_string());
            } else {
                return Err("running".to_string());
            }
        }

        state.running = true;
        state.last_started = Some(Utc::now());
        state.last_error = None;
    }

    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let scripts_dir = workspace_root
        .join("tools")
        .join("lora_training")
        .join("scripts");
    let export_script = scripts_dir.join("export_docs.py");
    let normalize_script = scripts_dir.join("normalize_dataset.py");
    let state_for_task = state_handle.clone();
    let filter = current_lora_filter();

    let job = task::spawn_blocking(move || {
        if let Some(ref value) = filter {
            std::env::set_var("LORA_EXPORT_ONLY", value);
        } else {
            std::env::remove_var("LORA_EXPORT_ONLY");
        }

        let result = run_script(&workspace_root, &export_script)
            .and_then(|_| run_script(&workspace_root, &normalize_script));

        let mut state = state_for_task.lock().expect("export state poisoned");
        state.running = false;
        state.last_finished = Some(Utc::now());
        if let Err(ref err) = result {
            state.last_error = Some(err.clone());
        } else {
            state.last_error = None;
        }

        result
    })
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Export snapshot task panicked");
        let mut state = state_handle.lock().expect("export state poisoned");
        state.running = false;
        state.last_finished = Some(Utc::now());
        state.last_error = Some("task panicked".into());
        "Export task failed".to_string()
    })?;

    job
}

#[derive(Debug, Serialize)]
struct SnapshotStatusResponse {
    status: String,
    running: bool,
    last_started: Option<String>,
    last_finished: Option<String>,
    last_error: Option<String>,
}

#[derive(Debug, Serialize)]
struct SnapshotConfigResponse {
    status: String,
    auto_export_enabled: bool,
    default_debounce_ms: u64,
    export_filter: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateExportConfigRequest {
    auto_export_enabled: Option<bool>,
    default_debounce_ms: Option<u64>,
}

async fn export_snapshot_status() -> Result<HttpResponse, Error> {
    let state_handle = lora_export_state();
    let state = state_handle
        .lock()
        .map_err(|_| error::ErrorInternalServerError("Failed to acquire export state"))?;

    Ok(HttpResponse::Ok().json(SnapshotStatusResponse {
        status: "ok".into(),
        running: state.running,
        last_started: state.last_started.map(|dt| dt.to_rfc3339()),
        last_finished: state.last_finished.map(|dt| dt.to_rfc3339()),
        last_error: state.last_error.clone(),
    }))
}

#[derive(Debug, Deserialize)]
struct SetExportFilterRequest {
    filter: Option<String>,
}

async fn set_export_snapshot_filter(
    payload: web::Json<SetExportFilterRequest>,
) -> Result<HttpResponse, Error> {
    if let Ok(mut guard) = lora_filter_override().lock() {
        *guard = payload.filter.clone();
    }
    export_snapshot_config().await
}

async fn export_snapshot_config() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().json(SnapshotConfigResponse {
        status: "ok".into(),
        auto_export_enabled: env_auto_export_enabled(),
        default_debounce_ms: env_auto_export_debounce_ms(),
        export_filter: current_lora_filter(),
    }))
}

async fn save_export_snapshot_config(
    payload: web::Json<UpdateExportConfigRequest>,
) -> Result<HttpResponse, Error> {
    let body = payload.into_inner();
    if let Some(enabled) = body.auto_export_enabled {
        set_auto_export_override(enabled);
    }
    if let Some(ms) = body.default_debounce_ms {
        set_auto_debounce_override(ms);
    }
    export_snapshot_config().await
}

fn auto_export_overrides() -> Arc<Mutex<AutoExportOverrides>> {
    AUTO_EXPORT_OVERRIDES
        .get_or_init(|| Arc::new(Mutex::new(AutoExportOverrides::default())))
        .clone()
}

fn set_auto_export_override(enabled: bool) {
    if let Ok(mut guard) = auto_export_overrides().lock() {
        guard.auto_export_enabled = Some(enabled);
    }
}

fn set_auto_debounce_override(ms: u64) {
    if let Ok(mut guard) = auto_export_overrides().lock() {
        guard.debounce_ms = Some(ms);
    }
}

fn env_auto_export_enabled() -> bool {
    if let Ok(guard) = auto_export_overrides().lock() {
        if let Some(value) = guard.auto_export_enabled {
            return value;
        }
    }
    std::env::var("AUTO_EXPORT_ON_UPLOAD")
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(true)
}

fn env_auto_export_debounce_ms() -> u64 {
    if let Ok(guard) = auto_export_overrides().lock() {
        if let Some(value) = guard.debounce_ms {
            return value;
        }
    }
    std::env::var("AUTO_EXPORT_DEBOUNCE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
}

fn env_lora_export_filter() -> Option<String> {
    std::env::var("LORA_EXPORT_ONLY").ok().and_then(|v| {
        if v.trim().is_empty() {
            None
        } else {
            Some(v)
        }
    })
}

fn current_lora_filter() -> Option<String> {
    if let Ok(guard) = lora_filter_override().lock() {
        if let Some(value) = guard.clone() {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }
    env_lora_export_filter()
}

fn trigger_auto_export_after_upload(upload_count: usize) {
    if upload_count == 0 || !env_auto_export_enabled() {
        return;
    }
    let debounce = env_auto_export_debounce_ms();
    tokio::spawn(async move {
        if debounce > 0 {
            sleep(Duration::from_millis(debounce)).await;
        }
        if let Err(err) = spawn_lora_export_job(false).await {
            tracing::warn!(error = %err, "Auto export skipped");
        }
    });
}

fn run_script(
    workspace_root: &std::path::Path,
    script_path: &std::path::Path,
) -> Result<(), String> {
    if !script_path.exists() {
        return Err(format!("Script not found: {}", script_path.display()));
    }

    let status = std::process::Command::new("python3")
        .arg(script_path)
        .current_dir(workspace_root)
        .status()
        .map_err(|e| format!("Failed to spawn {}: {}", script_path.display(), e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Script {} exited with status {:?}",
            script_path.display(),
            status.code()
        ))
    }
}

fn run_script_with_args(
    workspace_root: &std::path::Path,
    script_path: &std::path::Path,
    args: &[&str],
) -> Result<String, String> {
    if !script_path.exists() {
        return Err(format!("Script not found: {}", script_path.display()));
    }

    let output = std::process::Command::new("python3")
        .arg(script_path)
        .args(args)
        .current_dir(workspace_root)
        .output()
        .map_err(|e| format!("Failed to spawn {}: {}", script_path.display(), e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Script {} failed: {}",
            script_path.display(),
            stderr
        ))
    }
}

// ============================================================================
// SYNTHETIC Q&A GENERATION
// ============================================================================

#[derive(Debug, Deserialize)]
struct SyntheticQaRequest {
    questions_per_chunk: Option<u32>,
    max_chunks: Option<usize>,
    ollama_model: Option<String>,
}

#[derive(Debug, Serialize)]
struct SyntheticQaResponse {
    status: String,
    request_id: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct SyntheticQaStatusResponse {
    status: String,
    running: bool,
    last_started: Option<String>,
    last_finished: Option<String>,
    last_error: Option<String>,
    examples_generated: Option<usize>,
    questions_per_chunk: u32,
    max_chunks: Option<usize>,
}

/// POST /training/synthetic_qa - Generate synthetic Q&A training data
async fn generate_synthetic_qa(
    payload: Option<web::Json<SyntheticQaRequest>>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    let (questions_per_chunk, max_chunks, ollama_model) = if let Some(p) = payload {
        (
            p.questions_per_chunk.unwrap_or(3),
            p.max_chunks,
            p.ollama_model.clone(),
        )
    } else {
        (3, None, None)
    };

    match spawn_synthetic_qa_job(questions_per_chunk, max_chunks, ollama_model).await {
        Ok(()) => Ok(HttpResponse::Ok().json(SyntheticQaResponse {
            status: "ok".into(),
            request_id,
            message: "Synthetic Q&A generation started".into(),
        })),
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(SyntheticQaResponse {
                status: "error".into(),
                request_id,
                message: e,
            }),
        ),
    }
}

async fn spawn_synthetic_qa_job(
    questions_per_chunk: u32,
    max_chunks: Option<usize>,
    ollama_model: Option<String>,
) -> Result<(), String> {
    use tokio::task;

    let state_handle = synthetic_qa_state();

    {
        let mut state = state_handle
            .lock()
            .map_err(|_| "Failed to acquire synthetic QA state".to_string())?;

        if state.running {
            return Err("Synthetic Q&A generation already in progress".to_string());
        }

        state.running = true;
        state.last_started = Some(Utc::now());
        state.last_error = None;
        state.examples_generated = None;
        state.questions_per_chunk = questions_per_chunk;
        state.max_chunks = max_chunks;
    }

    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let scripts_dir = workspace_root
        .join("tools")
        .join("lora_training")
        .join("scripts");
    let export_script = scripts_dir.join("export_docs.py");
    let synthetic_script = scripts_dir.join("generate_synthetic_qa.py");
    let state_for_task = state_handle.clone();
    let model = ollama_model.unwrap_or_else(|| "phi3.5:latest".to_string());

    let job = task::spawn_blocking(move || {
        // First run export_docs.py to ensure we have fresh document data
        tracing::info!("Running export_docs.py...");
        if let Err(e) = run_script(&workspace_root, &export_script) {
            return Err(format!("Export failed: {}", e));
        }

        // Build args for synthetic generation
        let mut args = vec![
            "--questions-per-chunk".to_string(),
            questions_per_chunk.to_string(),
            "--ollama-model".to_string(),
            model,
        ];

        if let Some(max) = max_chunks {
            args.push("--max-chunks".to_string());
            args.push(max.to_string());
        }

        tracing::info!(args = ?args, "Running generate_synthetic_qa.py...");

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let result = run_script_with_args(&workspace_root, &synthetic_script, &args_refs);

        // Parse output to get examples count
        let examples_count = if let Ok(ref output) = result {
            // Look for "Total examples: N" in output
            output
                .lines()
                .find(|line| line.contains("Total examples:"))
                .and_then(|line| {
                    line.split(':')
                        .nth(1)
                        .and_then(|s| s.trim().parse::<usize>().ok())
                })
        } else {
            None
        };

        let mut state = state_for_task.lock().expect("synthetic QA state poisoned");
        state.running = false;
        state.last_finished = Some(Utc::now());
        state.examples_generated = examples_count;

        if let Err(ref err) = result {
            state.last_error = Some(err.clone());
        } else {
            state.last_error = None;
        }

        result.map(|_| ())
    })
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Synthetic QA task panicked");
        let mut state = state_handle.lock().expect("synthetic QA state poisoned");
        state.running = false;
        state.last_finished = Some(Utc::now());
        state.last_error = Some("task panicked".into());
        "Synthetic QA task failed".to_string()
    })?;

    job
}

/// GET /training/synthetic_qa/status - Get synthetic Q&A generation status
async fn synthetic_qa_status() -> Result<HttpResponse, Error> {
    let state_handle = synthetic_qa_state();
    let state = state_handle
        .lock()
        .map_err(|_| error::ErrorInternalServerError("Failed to acquire synthetic QA state"))?;

    Ok(HttpResponse::Ok().json(SyntheticQaStatusResponse {
        status: "ok".into(),
        running: state.running,
        last_started: state.last_started.map(|t| t.to_rfc3339()),
        last_finished: state.last_finished.map(|t| t.to_rfc3339()),
        last_error: state.last_error.clone(),
        examples_generated: state.examples_generated,
        questions_per_chunk: state.questions_per_chunk,
        max_chunks: state.max_chunks,
    }))
}

#[derive(Debug, Deserialize)]
struct SyntheticQaExamplesQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Serialize)]
struct SyntheticQaExample {
    instruction: String,
    context: String,
    response: String,
    source: Option<String>,
    timestamp: Option<String>,
}

#[derive(Debug, Serialize)]
struct SyntheticQaExamplesResponse {
    status: String,
    total: usize,
    offset: usize,
    limit: usize,
    examples: Vec<SyntheticQaExample>,
}

/// GET /training/synthetic_qa/examples - Get generated synthetic Q&A examples
async fn synthetic_qa_examples(
    query: web::Query<SyntheticQaExamplesQuery>,
) -> Result<HttpResponse, Error> {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let qa_file = workspace_root
        .join("tools")
        .join("lora_training")
        .join("data")
        .join("synthetic_qa.jsonl");

    if !qa_file.exists() {
        return Ok(HttpResponse::Ok().json(SyntheticQaExamplesResponse {
            status: "ok".into(),
            total: 0,
            offset: 0,
            limit: query.limit.unwrap_or(10),
            examples: vec![],
        }));
    }

    let content = std::fs::read_to_string(&qa_file)
        .map_err(|e| error::ErrorInternalServerError(format!("Failed to read QA file: {}", e)))?;

    let all_examples: Vec<SyntheticQaExample> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .map(|v| SyntheticQaExample {
                    instruction: v
                        .get("instruction")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    context: v
                        .get("context")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    response: v
                        .get("response")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    source: v.get("source").and_then(|v| v.as_str()).map(String::from),
                    timestamp: v
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                })
        })
        .collect();

    let total = all_examples.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(10).min(100); // Cap at 100

    let examples: Vec<SyntheticQaExample> =
        all_examples.into_iter().skip(offset).take(limit).collect();

    Ok(HttpResponse::Ok().json(SyntheticQaExamplesResponse {
        status: "ok".into(),
        total,
        offset,
        limit,
        examples,
    }))
}

/// POST /training/clear - Clear all collected training data
async fn clear_training_data() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let collector = training_collector();

    match collector.clear() {
        Ok(_) => {
            tracing::info!("Training data cleared");
            Ok(HttpResponse::Ok().json(json!({
                "status": "ok",
                "message": "Training data cleared",
                "request_id": request_id
            })))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to clear training data");
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to clear: {}", e),
                "request_id": request_id
            })))
        }
    }
}

async fn get_hardware_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = crate::db::param_hardware::global_config().into();
    Ok(HttpResponse::Ok().json(HardwareConfigResponse {
        status: "ok".into(),
        message: "".into(),
        request_id,
        config,
    }))
}

async fn commit_hardware_config(
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

// ============================================================================
// ONNX CONFIG
// ============================================================================

use crate::perf::onnx_embedder::{
    OnnxConfig, OnnxExecutionMode, OnnxLogLevel, OnnxOptimizationLevel,
};

fn onnx_opt_level_to_str(level: OnnxOptimizationLevel) -> &'static str {
    match level {
        OnnxOptimizationLevel::Disable => "disable",
        OnnxOptimizationLevel::Basic => "basic",
        OnnxOptimizationLevel::Extended => "extended",
        OnnxOptimizationLevel::All => "all",
    }
}

fn onnx_exec_mode_to_str(mode: OnnxExecutionMode) -> &'static str {
    match mode {
        OnnxExecutionMode::Sequential => "sequential",
        OnnxExecutionMode::Parallel => "parallel",
    }
}

fn onnx_log_level_to_str(level: OnnxLogLevel) -> &'static str {
    match level {
        OnnxLogLevel::Verbose => "verbose",
        OnnxLogLevel::Info => "info",
        OnnxLogLevel::Warning => "warning",
        OnnxLogLevel::Error => "error",
        OnnxLogLevel::Fatal => "fatal",
    }
}

fn parse_log_level(input: &str) -> Option<OnnxLogLevel> {
    match input.to_lowercase().as_str() {
        "verbose" | "trace" => Some(OnnxLogLevel::Verbose),
        "info" => Some(OnnxLogLevel::Info),
        "warn" | "warning" => Some(OnnxLogLevel::Warning),
        "error" => Some(OnnxLogLevel::Error),
        "fatal" | "critical" => Some(OnnxLogLevel::Fatal),
        _ => None,
    }
}

fn apply_option_field<T>(target: &mut Option<T>, value: Option<Option<T>>) {
    if let Some(inner) = value {
        *target = inner;
    }
}

/// Global ONNX config storage (read at startup, can be modified via API)
static ONNX_CONFIG: OnceLock<std::sync::RwLock<OnnxConfig>> = OnceLock::new();

fn get_onnx_config_storage() -> &'static std::sync::RwLock<OnnxConfig> {
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
async fn get_onnx_config() -> Result<HttpResponse, Error> {
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
async fn set_onnx_config(payload: web::Json<OnnxConfigRequest>) -> Result<HttpResponse, Error> {
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
// API KEYS CONFIG
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ApiKeysRequest {
    #[serde(default)]
    openai_api_key: String,
    #[serde(default)]
    anthropic_api_key: String,
}

// ============================================================================
// NEO4J KNOWLEDGE GRAPH CONFIG (Phase 27)
// ============================================================================

#[derive(Debug, Serialize)]
struct Neo4jConfigResponse {
    status: String,
    message: String,
    request_id: String,
    feature_compiled: bool,
    enabled: bool,
    connected: bool,
    uri: String,
    user: String,
    database: String,
    max_connections: usize,
    connection_timeout_ms: u64,
    // Graph expansion settings
    expansion_enabled: bool,
    max_hops: usize,
    max_chunks: usize,
    entity_weight: f32,
    concept_weight: f32,
    min_relationship_strength: f32,
    // Entity extraction settings
    extraction_enabled: bool,
    confidence_threshold: f32,
    fuzzy_threshold: f32,
    // Stats (if connected)
    stats: Option<Neo4jStats>,
}

#[derive(Debug, Serialize)]
struct Neo4jStats {
    total_nodes: usize,
    total_relationships: usize,
    documents: usize,
    chunks: usize,
    entities: usize,
}

#[derive(Debug, serde::Deserialize)]
struct Neo4jConfigRequest {
    enabled: Option<bool>,
    uri: Option<String>,
    user: Option<String>,
    password: Option<String>,
    database: Option<String>,
    max_connections: Option<usize>,
    connection_timeout_ms: Option<u64>,
    // Graph expansion
    expansion_enabled: Option<bool>,
    max_hops: Option<usize>,
    max_chunks: Option<usize>,
    entity_weight: Option<f32>,
    concept_weight: Option<f32>,
    min_relationship_strength: Option<f32>,
    // Entity extraction
    extraction_enabled: Option<bool>,
    confidence_threshold: Option<f32>,
    fuzzy_threshold: Option<f32>,
}

async fn get_neo4j_config() -> Result<HttpResponse, Error> {
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

async fn save_neo4j_config(payload: web::Json<Neo4jConfigRequest>) -> Result<HttpResponse, Error> {
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

async fn test_neo4j_connection() -> Result<HttpResponse, Error> {
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

#[derive(Debug, Serialize)]
struct ApiKeysResponse {
    status: String,
    message: String,
    request_id: String,
    has_openai_key: bool,
    has_anthropic_key: bool,
    openai_key_masked: String,
    anthropic_key_masked: String,
    openai_from_env: bool,
    anthropic_from_env: bool,
}

async fn get_api_keys() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let keys = crate::db::api_keys::global_config();

    let openai_from_env = std::env::var("OPENAI_API_KEY").is_ok();
    let anthropic_from_env = std::env::var("ANTHROPIC_API_KEY").is_ok();

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

    Ok(HttpResponse::Ok().json(ApiKeysResponse {
        status: "ok".into(),
        message: "API keys status".into(),
        request_id,
        has_openai_key: keys.has_openai_key(),
        has_anthropic_key: keys.has_anthropic_key(),
        openai_key_masked,
        anthropic_key_masked,
        openai_from_env,
        anthropic_from_env,
    }))
}

async fn save_api_keys(payload: web::Json<ApiKeysRequest>) -> Result<HttpResponse, Error> {
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

    match crate::db::api_keys::update_config(keys.clone()) {
        Ok(_) => {
            tracing::info!(
                request_id = %request_id,
                has_openai = keys.has_openai_key(),
                has_anthropic = keys.has_anthropic_key(),
                "API keys saved"
            );

            let openai_from_env = std::env::var("OPENAI_API_KEY").is_ok();
            let anthropic_from_env = std::env::var("ANTHROPIC_API_KEY").is_ok();

            Ok(HttpResponse::Ok().json(ApiKeysResponse {
                status: "ok".into(),
                message: "API keys saved".into(),
                request_id,
                has_openai_key: keys.has_openai_key(),
                has_anthropic_key: keys.has_anthropic_key(),
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
                openai_from_env,
                anthropic_from_env,
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

async fn delete_api_key(path: web::Path<String>) -> Result<HttpResponse, Error> {
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

async fn get_cache_monitor_info() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let retriever = match RETRIEVER.get() {
        Some(handle) => handle,
        None => {
            return Ok(HttpResponse::ServiceUnavailable().json(json!({
                "status": "unavailable",
                "error": "Retriever not initialized",
                "request_id": request_id,
            })));
        }
    };

    let (metrics_snapshot, l2_stats, redis_summary, l1_enabled, l2_enabled) = {
        let guard = retriever.lock().unwrap();
        (
            guard.metrics.clone(),
            guard.get_l2_cache_stats(),
            guard.get_l3_cache_summary(),
            guard.l1_cache_enabled(),
            guard.l2_cache_enabled(),
        )
    };

    let l1_snapshot = L1CacheSnapshot {
        enabled: l1_enabled,
        total_searches: metrics_snapshot.total_searches as u64,
        hits: metrics_snapshot.cache_hits as u64,
        misses: metrics_snapshot.cache_misses as u64,
        hit_rate: metrics_snapshot.cache_hit_rate(),
    };
    let l2_snapshot = L2CacheSnapshot {
        enabled: l2_enabled,
        l1_hits: l2_stats.l1_hits,
        l1_misses: l2_stats.l1_misses,
        l2_hits: l2_stats.l2_hits,
        l2_misses: l2_stats.l2_misses,
        total_items: l2_stats.total_items as u64,
        hit_rate: l2_stats.hit_rate(),
    };
    let counters = metrics::cache_hit_miss_counts();
    let counters_snapshot = CacheCountersSnapshot {
        hits_total: counters.0,
        misses_total: counters.1,
    };

    let response = CacheMonitorResponse {
        request_id,
        l1: l1_snapshot,
        l2: l2_snapshot,
        redis: redis_summary,
        counters: counters_snapshot,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// POST /cache/clear
/// Clear all caches (L1, L2, and optionally L3/Redis)
async fn clear_cache() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    let retriever = match RETRIEVER.get() {
        Some(handle) => handle,
        None => {
            return Ok(HttpResponse::ServiceUnavailable().json(json!({
                "status": "error",
                "error": "Retriever not initialized",
                "request_id": request_id,
            })));
        }
    };

    // Clear caches
    {
        let mut guard = retriever.lock().unwrap();
        guard.clear_cache();
        guard.clear_l2_cache();
    }

    info!("[{}] Cache cleared via API", request_id);

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "message": "Cache cleared",
        "request_id": request_id,
    })))
}

async fn get_rate_limit_monitor_info(
    state: web::Data<RateLimitSharedState>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let limiter_state = state.limiter.snapshot();
    let total_drops = metrics::rate_limit_drop_total();
    let drops_by_route = metrics::rate_limit_drops_by_route_snapshot()
        .into_iter()
        .map(|(route, drops)| RouteDropStat { route, drops })
        .collect();
    let config = state.config_snapshot(limiter_state.enabled);

    let response = RateLimitMonitorResponse {
        request_id,
        total_drops,
        drops_by_route,
        config,
        limiter_state,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Get inference gateway statistics
async fn get_inference_gateway_stats() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let stats = crate::inference_gateway::gateway_stats();

    // Also refresh the Prometheus gauges
    metrics::refresh_inference_gateway_gauges();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "request_id": request_id,
        "gateway": stats
    })))
}

#[derive(Debug, serde::Deserialize)]
struct SetRateLimitEnabledRequest {
    enabled: bool,
}

#[derive(Debug, Serialize)]
struct SetRateLimitEnabledResponse {
    request_id: String,
    enabled: bool,
    message: String,
}

async fn set_rate_limit_enabled(
    state: web::Data<RateLimitSharedState>,
    body: web::Json<SetRateLimitEnabledRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let new_state = state.limiter.set_enabled(body.enabled);

    let message = if new_state {
        "Rate limiter enabled".to_string()
    } else {
        "Rate limiter disabled".to_string()
    };

    tracing::info!("[{}] Rate limiter set to: {}", request_id, new_state);

    Ok(HttpResponse::Ok().json(SetRateLimitEnabledResponse {
        request_id,
        enabled: new_state,
        message,
    }))
}

async fn get_recent_logs(query: web::Query<LogsQuery>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let limit = query
        .limit
        .unwrap_or(DEFAULT_LOG_LIMIT)
        .clamp(1, MAX_LOG_LIMIT);
    let config = MonitoringConfig::from_env();
    let log_dir = config.log_dir;

    let file = latest_log_file(&log_dir);
    let (entries, note) = if let Some(path) = file.clone() {
        match read_recent_lines(&path, limit) {
            Ok(lines) => {
                let entries = lines
                    .into_iter()
                    .map(|line| parse_log_line(&line))
                    .collect();
                (entries, None)
            }
            Err(err) => {
                warn!(error = %err, path = %path.display(), "Failed to read logs");
                (Vec::new(), Some(format!("Failed to read logs: {}", err)))
            }
        }
    } else {
        (Vec::new(), Some("No backend log files found".to_string()))
    };

    let response = LogsResponse {
        request_id,
        file: file.and_then(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        }),
        entries,
        note,
    };

    Ok(HttpResponse::Ok().json(response))
}

async fn upload_document_inner(
    mut payload: Multipart,
    config: web::Data<ApiConfig>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    fs::create_dir_all(UPLOAD_DIR).ok();
    let mut uploaded_files = Vec::new();

    while let Some(item) = payload.next().await {
        let mut field = item?;
        let filename = field
            .content_disposition()
            .as_ref()
            .and_then(|cd| cd.get_filename())
            .ok_or_else(|| actix_web::error::ErrorBadRequest("No filename"))?
            .to_string();

        let ext = Path::new(&filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Allow documents and code files that mime_detect.rs supports
        let allowed_extensions = [
            // Documents
            "pdf", "txt", "text", "md", "markdown", "html", "htm", "xhtml", "xml", "json",
            // Code files
            "rs", "py", "pyw", "js", "mjs", "cjs", "ts", "tsx", "go", "java", "cs", "cpp", "cc",
            "cxx", "hpp", "c", "h", "rb", "php", "sh", "bash", "zsh", "sql", "yaml", "yml", "toml",
        ];

        if !allowed_extensions.contains(&ext.as_str()) {
            return Ok(HttpResponse::BadRequest().body(format!(
                "File type '{}' not supported. Allowed: documents (pdf, txt, md, html, xml, json) and code files (rs, py, js, ts, go, java, etc.)",
                ext
            )));
        }

        let filepath = format!("{}/{}", UPLOAD_DIR, filename);
        let mut f = web::block(move || std::fs::File::create(&filepath)).await??;
        while let Some(chunk) = field.next().await {
            let data = chunk?;
            f = web::block(move || f.write_all(&data).map(|_| f)).await??;
        }

        uploaded_files.push(filename);
    }

    let mut indexed_files = Vec::new();
    let mut index_errors = Vec::new();
    let io_backend = crate::perf::io_uring::backend_name();

    if !uploaded_files.is_empty() {
        if is_reindex_in_progress() {
            index_errors.push(json!({
                "file": null,
                "error": "Reindex already in progress; automatic indexing skipped",
            }));
        } else if let Some(handle) = RETRIEVER.get() {
            // Phase 1: Read all files asynchronously using io_uring (outside mutex)
            // This is where io_uring provides 2-3x speedup on Linux
            let mut file_contents: Vec<(String, std::path::PathBuf, Option<String>)> = Vec::new();

            for filename in &uploaded_files {
                let path = Path::new(UPLOAD_DIR).join(filename);
                // Use io_uring async read
                let content = index::extract_text_async(&path).await;
                file_contents.push((filename.clone(), path, content));
            }

            // Phase 2: Index with mutex (brief lock, no I/O)
            // Collect chunks for graph indexing (done outside mutex)
            let mut graph_index_tasks: Vec<(String, String, Vec<(String, String)>)> = Vec::new();

            match handle.lock() {
                Ok(mut retriever) => {
                    let chunker = crate::index::default_chunker(config.chunker_mode);

                    for (filename, path, content_opt) in file_contents {
                        match content_opt {
                            Some(content) => {
                                // Index the pre-read content and get chunks for graph
                                match index::index_content_with_graph(
                                    &mut *retriever,
                                    &path,
                                    &content,
                                    config.chunker_mode,
                                    chunker.as_ref(),
                                ) {
                                    Ok((chunk_count, graph_chunks)) => {
                                        indexed_files.push(json!({
                                            "file": filename.clone(),
                                            "chunks_indexed": chunk_count,
                                            "io_backend": io_backend,
                                        }));
                                        // Queue for graph indexing (outside mutex)
                                        if !graph_chunks.is_empty() {
                                            graph_index_tasks.push((
                                                filename.clone(),
                                                path.to_string_lossy().to_string(),
                                                graph_chunks,
                                            ));
                                        }
                                    }
                                    Err(err) => index_errors.push(json!({
                                        "file": filename,
                                        "error": err,
                                    })),
                                }
                            }
                            None => {
                                index_errors.push(json!({
                                    "file": filename,
                                    "error": "Failed to extract text from file",
                                }));
                            }
                        }
                    }

                    if let Err(err) = retriever.commit() {
                        index_errors.push(json!({
                            "file": null,
                            "error": format!("commit failed: {}", err),
                        }));
                    }
                }
                Err(_) => {
                    index_errors.push(json!({
                        "file": null,
                        "error": "Failed to lock retriever for indexing",
                    }));
                }
            }

            // Phase 3: Index to knowledge graph (outside mutex, async)
            for (filename, source, chunks) in graph_index_tasks {
                index_to_knowledge_graph(&filename, &filename, &source, &chunks).await;
            }
        } else {
            index_errors.push(json!({
                "file": null,
                "error": "Retriever not initialized; run /reindex manually",
            }));
        }
    }

    trigger_auto_export_after_upload(uploaded_files.len());

    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "uploaded_files": uploaded_files,
        "indexed_files": indexed_files,
        "index_errors": index_errors,
        "request_id": request_id
    })))
}

pub async fn list_documents() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(UPLOAD_DIR) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                if let Some(filename) = entry.file_name().to_str() {
                    files.push(filename.to_string());
                }
            }
        }
    }
    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "documents": files,
        "count": files.len(),
        "request_id": request_id
    })))
}

pub async fn delete_document(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let filename = path.into_inner();
    let filepath = format!("{}/{}", UPLOAD_DIR, filename);

    match fs::remove_file(&filepath) {
        Ok(_) => {
            // Incrementally delete from the live index so the deleted document
            // does not keep showing up in search results.
            let mut deleted_chunks: Option<usize> = None;
            if let Some(retriever) = RETRIEVER.get() {
                if let Ok(mut retriever) = retriever.lock() {
                    match retriever.delete_document_by_filename(&filename) {
                        Ok(count) => {
                            deleted_chunks = Some(count);
                        }
                        Err(e) => {
                            warn!(error = %e, filename, "Failed to delete document chunks from index");
                        }
                    }
                }
            }

            Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "message": format!("Deleted {}", filename),
                "deleted_chunks": deleted_chunks,
                "request_id": request_id
            })))
        }
        Err(_) => Ok(HttpResponse::NotFound().json(json!({
            "status": "error",
            "message": "File not found",
            "request_id": request_id
        }))),
    }
}

pub async fn reindex_handler(config: web::Data<ApiConfig>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let start = std::time::Instant::now();

    // Phase 15: Check concurrency
    if REINDEX_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(HttpResponse::TooManyRequests().json(json!({
            "status": "busy",
            "message": "Reindex already in progress",
            "request_id": request_id
        })));
    }

    // Alerting config
    let hooks = crate::monitoring::alerting_hooks::AlertingHooksConfig::from_env();

    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        let chunker = crate::index::default_chunker(config.chunker_mode);
        let res = index::index_all_documents(
            &mut *retriever,
            UPLOAD_DIR,
            config.chunker_mode,
            chunker.as_ref(),
        );
        let duration_ms = start.elapsed().as_millis() as u64;
        let vectors = retriever.metrics.total_vectors as u64;
        let mappings = retriever.metrics.total_documents_indexed as u64;
        REINDEX_IN_PROGRESS.store(false, Ordering::SeqCst);

        // Fire webhook (non-blocking)
        let event = match res {
            Ok(_) => crate::monitoring::alerting_hooks::ReindexCompletionEvent::success(
                duration_ms,
                vectors,
                mappings,
            ),
            Err(_) => crate::monitoring::alerting_hooks::ReindexCompletionEvent::error(
                duration_ms,
                vectors,
                mappings,
            ),
        };
        actix_web::rt::spawn(async move {
            crate::monitoring::alerting_hooks::send_alert(&hooks, event).await;
        });

        match res {
            Ok(_) => Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "message": "Reindexing complete",
                "request_id": request_id
            }))),
            Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Reindex failed: {}", e),
                "request_id": request_id
            }))),
        }
    } else {
        REINDEX_IN_PROGRESS.store(false, Ordering::SeqCst);
        // Fire error webhook for missing retriever
        let hooks2 = crate::monitoring::alerting_hooks::AlertingHooksConfig::from_env();
        let event = crate::monitoring::alerting_hooks::ReindexCompletionEvent::error(0, 0, 0);
        actix_web::rt::spawn(async move {
            crate::monitoring::alerting_hooks::send_alert(&hooks2, event).await;
        });
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

fn launch_async_reindex_job(config: web::Data<ApiConfig>) -> Result<String, (StatusCode, String)> {
    if REINDEX_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Reindex already in progress".to_string(),
        ));
    }

    let job_id = Uuid::new_v4().to_string();
    let job = AsyncJob {
        job_id: job_id.clone(),
        status: "pending".to_string(),
        started_at: Utc::now().to_rfc3339(),
        completed_at: None,
        vectors_indexed: None,
        mappings_indexed: None,
        error: None,
    };

    let jobs = get_jobs_map();
    jobs.lock().unwrap().insert(job_id.clone(), job);

    let job_id_clone = job_id.clone();
    let jobs_map = jobs.clone();
    let retriever_handle = RETRIEVER.get().map(|h| Arc::clone(h));
    let config_clone = config.clone();

    actix_web::rt::spawn(async move {
        let start = std::time::Instant::now();
        let hooks = crate::monitoring::alerting_hooks::AlertingHooksConfig::from_env();
        if let Some(retriever) = retriever_handle {
            let mut retriever = retriever.lock().unwrap();
            {
                let mut job = jobs_map
                    .lock()
                    .unwrap()
                    .get(&job_id_clone)
                    .cloned()
                    .unwrap();
                job.status = "running".to_string();
                jobs_map.lock().unwrap().insert(job_id_clone.clone(), job);
            }

            let chunker = crate::index::default_chunker(config_clone.chunker_mode);
            let res = index::index_all_documents(
                &mut *retriever,
                UPLOAD_DIR,
                config_clone.chunker_mode,
                chunker.as_ref(),
            );

            let mut job = jobs_map
                .lock()
                .unwrap()
                .get(&job_id_clone)
                .cloned()
                .unwrap();
            let duration_ms = start.elapsed().as_millis() as u64;
            let vectors = retriever.metrics.total_vectors as u64;
            let mappings = retriever.metrics.total_documents_indexed as u64;
            drop(retriever); // Release lock before async graph rebuild

            match res {
                Ok(_) => {
                    job.status = "completed".to_string();
                    job.completed_at = Some(Utc::now().to_rfc3339());
                    job.vectors_indexed = Some(vectors as usize);
                    job.mappings_indexed = Some(mappings as usize);
                    let event = crate::monitoring::alerting_hooks::ReindexCompletionEvent::success(
                        duration_ms,
                        vectors,
                        mappings,
                    );
                    crate::monitoring::alerting_hooks::send_alert(&hooks, event).await;
                }
                Err(ref e) => {
                    job.status = "failed".to_string();
                    job.completed_at = Some(Utc::now().to_rfc3339());
                    job.error = Some(e.to_string());
                    let event = crate::monitoring::alerting_hooks::ReindexCompletionEvent::error(
                        duration_ms,
                        vectors,
                        mappings,
                    );
                    crate::monitoring::alerting_hooks::send_alert(&hooks, event).await;
                }
            }
            jobs_map.lock().unwrap().insert(job_id_clone.clone(), job);
            // v1.3.0: Rebuild knowledge graph after successful reindex
            if res.is_ok() {
                let graph_result = crate::api::graph_routes::rebuild_graph_from_index().await;
                info!(
                    "Post-reindex graph rebuild: {} docs, {} chunks",
                    graph_result.documents_processed, graph_result.chunks_processed
                );
            }
        } else {
            let mut job = jobs_map
                .lock()
                .unwrap()
                .get(&job_id_clone)
                .cloned()
                .unwrap();
            job.status = "failed".to_string();
            job.completed_at = Some(Utc::now().to_rfc3339());
            job.error = Some("Retriever not initialized".to_string());
            jobs_map
                .lock()
                .unwrap()
                .insert(job_id_clone.clone(), job.clone());
            let event = crate::monitoring::alerting_hooks::ReindexCompletionEvent::error(0, 0, 0);
            crate::monitoring::alerting_hooks::send_alert(&hooks, event).await;
        }
        REINDEX_IN_PROGRESS.store(false, Ordering::SeqCst);
    });

    Ok(job_id)
}

/// Phase 15: Async reindex endpoint
pub async fn reindex_async_handler(config: web::Data<ApiConfig>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    match launch_async_reindex_job(config) {
        Ok(job_id) => Ok(HttpResponse::Accepted().json(json!({
            "status": "accepted",
            "job_id": job_id,
            "request_id": request_id
        }))),
        Err((status, message)) => Ok(HttpResponse::build(status).json(json!({
            "status": "busy",
            "message": message,
            "request_id": request_id
        }))),
    }
}

/// Phase 15: Check async job status
pub async fn reindex_status_handler(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let job_id = path.into_inner();

    let jobs = get_jobs_map();
    let jobs_lock = jobs.lock().unwrap();

    if let Some(job) = jobs_lock.get(&job_id) {
        Ok(HttpResponse::Ok().json(json!({
            "status": job.status,
            "job_id": job.job_id,
            "started_at": job.started_at,
            "completed_at": job.completed_at,
            "vectors_indexed": job.vectors_indexed,
            "mappings_indexed": job.mappings_indexed,
            "error": job.error,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::NotFound().json(json!({
            "status": "not_found",
            "message": format!("Job {} not found", job_id),
            "request_id": request_id
        })))
    }
}

/// Phase 15: Index info endpoint
pub async fn index_info_handler() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let in_ram = std::env::var("INDEX_IN_RAM")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);

    if let Some(retriever) = RETRIEVER.get() {
        let retriever = retriever.lock().unwrap();
        Ok(HttpResponse::Ok().json(json!({
            "index_in_ram": in_ram,
            "mode": if in_ram { "RAM (fast)" } else { "Disk (standard)" },
            "warning": if in_ram {
                json!("INDEX_IN_RAM enabled: High memory usage for large datasets. Recommended for <100 docs only.")
            } else {
                json!(null)
            },
            "total_documents": retriever.metrics.total_documents_indexed,
            "total_vectors": retriever.metrics.total_vectors,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

async fn search_documents_inner(query: web::Query<SearchQuery>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let start = std::time::Instant::now();
    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        let results = retriever.search(&query.q).unwrap_or_default();
        let elapsed = start.elapsed().as_millis() as u64;

        // Record tool execution
        crate::monitoring::record_tool_execution(
            "SemanticSearch",
            &query.q,
            true,
            &format!("{} results", results.len()),
            elapsed,
            1.0,
            Some("api/search"),
        );

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "results": results,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

pub async fn rerank(request: web::Json<RerankRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    if let Some(retriever) = RETRIEVER.get() {
        let retriever = retriever.lock().unwrap();
        let reranked = retriever.rerank_by_similarity(&request.query, &request.candidates);
        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "results": reranked,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

pub async fn summarize(request: web::Json<SummarizeRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    if let Some(retriever) = RETRIEVER.get() {
        let retriever = retriever.lock().unwrap();
        let summary = retriever.summarize_chunks(&request.query, &request.candidates);
        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "summary": summary,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

pub async fn save_vectors_handler() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        match retriever.force_save() {
            Ok(_) => Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "message": "Vectors saved successfully",
                "vector_count": retriever.vectors.len(),
                "request_id": request_id
            }))),
            Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save vectors: {}", e),
                "request_id": request_id
            }))),
        }
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

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
}

#[derive(serde::Deserialize)]
pub struct AgentRequest {
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default)]
    pub mode: ChatMode,
}

// Simple query variant for GET /agent/chat
#[derive(serde::Deserialize)]
pub struct AgentQueryParams {
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default)]
    pub mode: ChatMode,
}

fn default_top_k() -> usize {
    5
}
fn default_limit() -> usize {
    20
}

#[derive(serde::Deserialize)]
pub struct StoreRagRequest {
    pub agent_id: String,
    pub memory_type: String,
    pub content: String,
    pub timestamp: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct SearchRagRequest {
    pub agent_id: String,
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

#[derive(serde::Deserialize)]
pub struct RecallRagRequest {
    pub agent_id: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(serde::Deserialize)]
pub struct DeleteRagRequest {
    pub agent_id: String,
    pub ids: Vec<i64>,
}

#[derive(serde::Deserialize)]
pub struct ManualObservationRequest {
    pub entry_type: String,
    pub title: String,
    pub narrative: String,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub concepts: Vec<String>,
    #[serde(default)]
    pub files_read: Vec<String>,
    #[serde(default)]
    pub files_modified: Vec<String>,
    pub author: Option<String>,
    pub project: Option<String>,
}

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManualObservationOrder {
    Relevance,
    Newest,
    Oldest,
}

impl Default for ManualObservationOrder {
    fn default() -> Self {
        ManualObservationOrder::Relevance
    }
}

#[derive(serde::Deserialize)]
pub struct ManualObservationSearchRequest {
    pub query: String,
    pub entry_type: Option<String>,
    pub project: Option<String>,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
    #[serde(default)]
    pub order: ManualObservationOrder,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(serde::Deserialize)]
pub struct ManualObservationListQuery {
    pub entry_type: Option<String>,
    pub project: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(serde::Deserialize)]
pub struct ManualObservationTimelineRequest {
    pub anchor_id: Option<String>,
    pub query: Option<String>,
    #[serde(default = "default_limit")]
    pub depth_before: usize,
    #[serde(default = "default_limit")]
    pub depth_after: usize,
    pub entry_type: Option<String>,
    pub project: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct ManualObservationFetchRequest {
    pub ids: Vec<String>,
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

fn validate_memory_type(memory_type: &str) -> Result<(), Error> {
    if VALID_MEMORY_TYPES.contains(&memory_type) {
        Ok(())
    } else {
        Err(actix_web::error::ErrorBadRequest(format!(
            "Invalid memory_type '{}'. Valid types are: {}",
            memory_type,
            VALID_MEMORY_TYPES.join(", ")
        )))
    }
}

async fn list_memory_types() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().json(json!({
        "core": ["fact", "preference", "instruction", "context", "summary", "task"],
        "extended": ["conversation", "decision", "correction", "feedback", "persona", "note"],
        "all": VALID_MEMORY_TYPES,
        "request_id": generate_request_id()
    })))
}

async fn store_rag_memory(req: web::Json<StoreRagRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    validate_memory_type(&req.memory_type)?;
    let mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let ts = req
        .timestamp
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    mem.store_rag(&req.agent_id, &req.memory_type, &req.content, &ts)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "request_id": request_id
    })))
}

async fn search_rag_memory(req: web::Json<SearchRagRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let results: Vec<MemorySearchResult> = mem
        .search_rag(&req.agent_id, &req.query, req.top_k)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "results": results,
        "request_id": request_id
    })))
}

async fn recall_rag_memory(req: web::Json<RecallRagRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let items: Vec<MemoryItem> = mem
        .recall_rag(&req.agent_id, req.limit)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "items": items,
        "request_id": request_id
    })))
}

async fn delete_rag_memory(req: web::Json<DeleteRagRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let mut mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let deleted = mem
        .delete_rag_by_ids(&req.agent_id, &req.ids)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "deleted": deleted,
        "request_id": request_id
    })))
}

fn validate_manual_observation(req: &ManualObservationRequest) -> Result<(), Error> {
    if req.title.trim().is_empty() || req.title.len() > 200 {
        return Err(actix_web::error::ErrorBadRequest(
            "title must be 1-200 characters",
        ));
    }
    if req.entry_type.trim().is_empty() || req.entry_type.len() > 100 {
        return Err(actix_web::error::ErrorBadRequest(
            "entry_type must be 1-100 characters",
        ));
    }
    if req.narrative.trim().is_empty() || req.narrative.len() > 10_000 {
        return Err(actix_web::error::ErrorBadRequest(
            "narrative must be 1-10000 characters",
        ));
    }
    if req.facts.len() > 32 || req.concepts.len() > 32 {
        return Err(actix_web::error::ErrorBadRequest(
            "facts/concepts limit is 32 items",
        ));
    }
    if req.files_read.len() > 32 || req.files_modified.len() > 32 {
        return Err(actix_web::error::ErrorBadRequest(
            "files_read/files_modified limit is 32 items",
        ));
    }
    Ok(())
}

async fn create_manual_observation(
    req: web::Json<ManualObservationRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    validate_manual_observation(&req)?;
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    let start = std::time::Instant::now();
    let result = (|| {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let id = mem
            .create_manual_observation(
                &req.entry_type,
                &req.title,
                &req.narrative,
                &req.facts,
                &req.concepts,
                &req.files_read,
                &req.files_modified,
                req.author.as_deref(),
                req.project.as_deref(),
            )
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        Ok(HttpResponse::Ok().json(json!({
            "id": id,
            "request_id": request_id
        })))
    })();
    crate::monitoring::metrics::record_manual_observation(
        "create",
        result.is_ok(),
        start.elapsed().as_secs_f64() * 1000.0,
    );
    result
}

async fn list_manual_observations(
    query: web::Query<ManualObservationListQuery>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_manual_endpoint("list", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let results = mem
            .list_manual_observations(
                query.entry_type.as_deref(),
                query.project.as_deref(),
                query.limit,
            )
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        Ok(HttpResponse::Ok().json(json!({
            "observations": results,
            "request_id": request_id
        })))
    })
}

async fn get_manual_observation(
    path: web::Path<String>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_manual_endpoint("get", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        match mem
            .get_manual_observation(&path)
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
        {
            Some(obs) => Ok(HttpResponse::Ok().json(json!({
                "observation": obs,
                "request_id": request_id
            }))),
            None => Ok(HttpResponse::NotFound().json(json!({
                "error": "not_found",
                "request_id": request_id
            }))),
        }
    })
}

async fn update_manual_observation(
    path: web::Path<String>,
    req: web::Json<ManualObservationRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    validate_manual_observation(&req)?;
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_manual_endpoint("update", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let updated = mem
            .update_manual_observation(
                &path,
                &req.entry_type,
                &req.title,
                &req.narrative,
                &req.facts,
                &req.concepts,
                &req.files_read,
                &req.files_modified,
                req.author.as_deref(),
                req.project.as_deref(),
            )
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        if updated {
            Ok(HttpResponse::Ok().json(json!({
                "status": "updated",
                "request_id": request_id
            })))
        } else {
            Ok(HttpResponse::NotFound().json(json!({
                "error": "not_found",
                "request_id": request_id
            })))
        }
    })
}

async fn delete_manual_observation(
    path: web::Path<String>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_manual_endpoint("delete", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let deleted = mem
            .delete_manual_observation(&path)
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        if deleted {
            Ok(HttpResponse::Ok().json(json!({
                "status": "deleted",
                "request_id": request_id
            })))
        } else {
            Ok(HttpResponse::NotFound().json(json!({
                "error": "not_found",
                "request_id": request_id
            })))
        }
    })
}

async fn manual_observation_timeline(
    req: web::Json<ManualObservationTimelineRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_memory_search_layer("timeline", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let results = mem
            .timeline_manual_observations(
                req.anchor_id.as_deref(),
                req.query.as_deref(),
                req.entry_type.as_deref(),
                req.project.as_deref(),
                req.depth_before,
                req.depth_after,
            )
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        Ok(HttpResponse::Ok().json(json!({
            "timeline": results,
            "request_id": request_id
        })))
    })
}

async fn fetch_manual_observations(
    req: web::Json<ManualObservationFetchRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    if req.ids.is_empty() {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "empty_ids",
            "request_id": request_id
        })));
    }
    if req.ids.len() > 20 {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "too_many_ids",
            "request_id": request_id
        })));
    }
    observe_memory_search_layer("fetch", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let observations = mem
            .fetch_manual_observations(&req.ids)
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        Ok(HttpResponse::Ok().json(json!({
            "observations": observations,
            "request_id": request_id
        })))
    })
}

async fn search_manual_observations(
    req: web::Json<ManualObservationSearchRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_memory_search_layer("search", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let hits = mem
            .search_manual_observations(
                req.query.as_str(),
                req.entry_type.as_deref(),
                req.project.as_deref(),
                req.date_start.as_deref(),
                req.date_end.as_deref(),
                req.order,
                req.limit,
                req.offset,
            )
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        Ok(HttpResponse::Ok().json(json!({
            "results": hits,
            "offset": req.offset,
            "limit": req.limit,
            "request_id": request_id
        })))
    })
}

async fn get_manual_observation_metrics(_http_req: HttpRequest) -> Result<HttpResponse, Error> {
    // No admin auth required - this is read-only monitoring data
    let snapshot = metrics::manual_observation_metrics_snapshot();
    Ok(HttpResponse::Ok().json(json!({
        "metrics": snapshot,
        "request_id": generate_request_id()
    })))
}

/// GET /monitoring/memory/search/stats - 3-layer memory search metrics (SEARCH.md)
async fn get_memory_search_layer_stats(_http_req: HttpRequest) -> Result<HttpResponse, Error> {
    // No admin auth required - this is read-only monitoring data
    let layer_stats = metrics::memory_search_layer_stats();
    let tokens_saved = metrics::memory_search_tokens_saved_total();
    Ok(HttpResponse::Ok().json(json!({
        "layers": layer_stats,
        "tokens_saved_total": tokens_saved,
        "request_id": generate_request_id()
    })))
}

async fn get_recent_observations(
    query: web::Query<ManualObservationListQuery>,
) -> Result<HttpResponse, Error> {
    // No admin auth required - this is read-only monitoring data
    let mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let results = mem
        .list_manual_observations(
            query.entry_type.as_deref(),
            query.project.as_deref(),
            query.limit,
        )
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "observations": results,
        "request_id": generate_request_id()
    })))
}

#[derive(serde::Deserialize)]
struct RagMemoriesQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    agent_id: Option<String>,
}

async fn get_recent_rag_memories(
    query: web::Query<RagMemoriesQuery>,
) -> Result<HttpResponse, Error> {
    // No admin auth required - this is read-only monitoring data
    let mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let agent_id = query.agent_id.as_deref().unwrap_or("default");
    let items: Vec<MemoryItem> = mem
        .recall_rag(agent_id, query.limit)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "memories": items,
        "request_id": generate_request_id()
    })))
}

/// Chat command types
#[allow(dead_code)]
enum ChatCommand {
    // Existing goal/system helpers
    Goal(String),
    Goals,
    Status,
    Help,
    Models,
    Clear,
    // Knowledge management
    Forget(String),
    History,
    Sources,
    Learn(String),
    Note(String),
    // Goal & task management
    Subgoal(String),
    PauseGoal,
    ResumeGoal,
    AbandonGoal,
    Reflect,
    Why,
    // Context control
    Focus(String),
    Unfocus,
    Persona(String),
    Verbose,
    Brief,
    // Tools & execution
    RunTool(String),
    Chain(String, String),
    Retry,
    Undo,
    DryRun(String),
    // System commands
    Model(String),
    Temperature(String),
    Export,
    Import(Option<String>),
    Debug,
    Tokens,
}

fn extract_argument<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.strip_prefix(marker)
        .map(|rest| rest.trim())
        .filter(|s| !s.is_empty())
}

/// Parse chat commands from user input
fn parse_chat_command(query: &str) -> Option<ChatCommand> {
    let trimmed = query.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    if let Some(arg) = extract_argument(trimmed, "/goal ") {
        return Some(ChatCommand::Goal(arg.to_string()));
    }
    if trimmed == "/goals" {
        return Some(ChatCommand::Goals);
    }
    if trimmed == "/status" {
        return Some(ChatCommand::Status);
    }
    if trimmed == "/help" {
        return Some(ChatCommand::Help);
    }
    if trimmed == "/models" {
        return Some(ChatCommand::Models);
    }
    if trimmed == "/clear" {
        return Some(ChatCommand::Clear);
    }

    if let Some(arg) = extract_argument(trimmed, "/forget ") {
        return Some(ChatCommand::Forget(arg.to_string()));
    }
    if trimmed == "/history" {
        return Some(ChatCommand::History);
    }
    if trimmed == "/sources" {
        return Some(ChatCommand::Sources);
    }
    if let Some(arg) = extract_argument(trimmed, "/learn ") {
        return Some(ChatCommand::Learn(arg.to_string()));
    }
    if let Some(arg) = extract_argument(trimmed, "/note ") {
        return Some(ChatCommand::Note(arg.to_string()));
    }

    if let Some(arg) = extract_argument(trimmed, "/subgoal ") {
        return Some(ChatCommand::Subgoal(arg.to_string()));
    }
    if trimmed == "/pause" {
        return Some(ChatCommand::PauseGoal);
    }
    if trimmed == "/resume" {
        return Some(ChatCommand::ResumeGoal);
    }
    if trimmed == "/abandon" {
        return Some(ChatCommand::AbandonGoal);
    }
    if trimmed == "/reflect" {
        return Some(ChatCommand::Reflect);
    }
    if trimmed == "/why" {
        return Some(ChatCommand::Why);
    }

    if let Some(arg) = extract_argument(trimmed, "/focus ") {
        return Some(ChatCommand::Focus(arg.to_string()));
    }
    if trimmed == "/unfocus" {
        return Some(ChatCommand::Unfocus);
    }
    if let Some(arg) = extract_argument(trimmed, "/persona ") {
        return Some(ChatCommand::Persona(arg.to_string()));
    }
    if trimmed == "/verbose" {
        return Some(ChatCommand::Verbose);
    }
    if trimmed == "/brief" {
        return Some(ChatCommand::Brief);
    }

    if let Some(arg) = extract_argument(trimmed, "/run ") {
        return Some(ChatCommand::RunTool(arg.to_string()));
    }
    if let Some(arg) = extract_argument(trimmed, "/chain ") {
        let parts: Vec<&str> = arg.split("->").map(|p| p.trim()).collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some(ChatCommand::Chain(
                parts[0].to_string(),
                parts[1].to_string(),
            ));
        }
    }
    if trimmed == "/retry" {
        return Some(ChatCommand::Retry);
    }
    if trimmed == "/undo" {
        return Some(ChatCommand::Undo);
    }
    if let Some(arg) = extract_argument(trimmed, "/dry-run ") {
        return Some(ChatCommand::DryRun(arg.to_string()));
    }

    if let Some(arg) = extract_argument(trimmed, "/model ") {
        return Some(ChatCommand::Model(arg.to_string()));
    }
    if let Some(arg) = extract_argument(trimmed, "/temperature ") {
        return Some(ChatCommand::Temperature(arg.to_string()));
    }
    if trimmed == "/export" {
        return Some(ChatCommand::Export);
    }
    if trimmed == "/import" {
        return Some(ChatCommand::Import(None));
    }
    if trimmed == "/debug" {
        return Some(ChatCommand::Debug);
    }
    if trimmed == "/tokens" {
        return Some(ChatCommand::Tokens);
    }

    None
}

/// Create a goal via the agentic monitor routes
fn create_goal_from_command(goal_text: &str) -> Result<serde_json::Value, String> {
    use crate::api::agentic_monitor_routes::get_agent_db_connection;

    let conn = get_agent_db_connection().ok_or_else(|| "Database not available".to_string())?;

    let goal_id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    let agent_id = "default";

    conn.execute(
        "INSERT INTO goals (id, agent_id, goal, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![&goal_id, agent_id, goal_text, "active", now],
    )
    .map_err(|e| format!("Failed to create goal: {}", e))?;

    Ok(json!({
        "id": goal_id,
        "goal": goal_text,
        "status": "active",
        "agent_id": agent_id,
        "created_at": now
    }))
}

/// Get help text for chat commands
fn get_help_text() -> String {
    r#"Available commands:

/chain a -> b - Execute tool sequences (use $last placeholder)
/clear        - Clear chat history (frontend only)
/debug|/tokens - Inspect internals
/dry-run <query> - Plan without execution
/export|/import <json> - Export or import memories
/focus <topic>|/unfocus - Control attention
/forget <topic> - Forget matching memories
/goal <text>  - Create a new goal
/goals        - List active goals
/help         - Show this help message
/history      - Show recent agent episodes
/learn <url>  - Fetch & ingest a URL (preview)
/model <name> - Switch backend model (use 'default')
/models       - List available models
/note <text>  - Store a quick note
/pause|/resume|/abandon - Control current goal
/persona <name> - Swap agent persona
/reflect      - Generate a reflection summary
/retry|/undo  - Retry last query / undo change
/run <tool>   - Execute calculator/search/fetch
/sources      - Show last response sources
/status       - Show system health status
/subgoal <text> - Add task under current goal
/temperature <n> - Adjust creativity (use 'default')
/verbose|/brief - Change response verbosity
/why          - Explain the last reasoning steps

Examples:
  /goal Find all Rust error handling patterns
  /focus tracing metrics
  /run calculator 5+7"#
        .to_string()
}

/// Get active goals list
fn get_goals_list() -> Result<String, String> {
    use crate::api::agentic_monitor_routes::get_agent_db_connection;

    let conn = get_agent_db_connection().ok_or_else(|| "Database not available".to_string())?;

    let mut stmt = conn.prepare(
        "SELECT goal, status, created_at FROM goals WHERE status = 'active' ORDER BY created_at DESC LIMIT 10"
    ).map_err(|e| e.to_string())?;

    let goals: Vec<String> = stmt
        .query_map([], |row| {
            let goal: String = row.get(0)?;
            Ok(format!("• {}", goal))
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    if goals.is_empty() {
        Ok("No active goals. Create one with: /goal <your goal>".to_string())
    } else {
        Ok(format!(
            "Active Goals ({}):\n{}",
            goals.len(),
            goals.join("\n")
        ))
    }
}

/// Get system status
fn get_system_status() -> String {
    let health = if RETRIEVER.get().is_some() {
        "✓ Healthy"
    } else {
        "✗ Retriever not initialized"
    };
    format!(
        "System Status: {}\nBackend: Running\nTimestamp: {}",
        health,
        Utc::now().to_rfc3339()
    )
}

/// Get available models
fn get_models_list() -> String {
    // This would ideally query the actual models, but for now return a placeholder
    "Available models:\n• default (local embedding model)\n\nUse /config to change model settings."
        .to_string()
}

fn forget_topic(topic: &str) -> Result<String, String> {
    let mem = AgentMemory::new(path_resolver::agent_db_path_str()).map_err(|e| e.to_string())?;
    let removed = mem
        .forget_topic("default", topic)
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "Removed {} memories mentioning '{}'.",
        removed, topic
    ))
}

fn list_recent_history(limit: usize) -> Result<String, String> {
    use crate::api::agentic_monitor_routes::get_agent_db_connection;
    let conn = get_agent_db_connection().ok_or_else(|| "Database not available".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT query, response, created_at FROM episodes ORDER BY created_at DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit as i64], |row| {
            let ts: i64 = row.get(2)?;
            let timestamp = chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".into());
            Ok(format!(
                "• [{}] {}\n  ↳ {}",
                timestamp,
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?
            ))
        })
        .map_err(|e| e.to_string())?;
    let entries: Vec<String> = rows.filter_map(Result::ok).collect();
    if entries.is_empty() {
        Ok("No recorded history yet. Ask a question to get started.".to_string())
    } else {
        Ok(entries.join("\n"))
    }
}

fn last_sources_summary() -> String {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    if state.last_sources.is_empty() {
        "No sources captured yet. Run a search query first.".to_string()
    } else {
        let lines: Vec<String> = state
            .last_sources
            .iter()
            .enumerate()
            .map(|(idx, s)| format!("{}. {}", idx + 1, s))
            .collect();
        format!("Sources from last response:\n{}", lines.join("\n"))
    }
}

fn record_note(content: &str) -> Result<String, String> {
    let mem = AgentMemory::new(path_resolver::agent_db_path_str()).map_err(|e| e.to_string())?;
    let ts = chrono::Utc::now().to_rfc3339();
    mem.add_note("default", content, &ts)
        .map_err(|e| e.to_string())?;
    push_note_action(content.to_string());
    Ok("Note stored.".to_string())
}

fn add_subgoal(text: &str) -> Result<String, String> {
    let mem = AgentMemory::new(path_resolver::agent_db_path_str()).map_err(|e| e.to_string())?;
    if let Some((goal_id, goal_text)) = mem.latest_goal("default").map_err(|e| e.to_string())? {
        let task_id = mem
            .create_subgoal(&goal_id, text)
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "Added subgoal under '{}': {} (task {})",
            goal_text, text, task_id
        ))
    } else {
        Err("No active goal to attach a subgoal. Use /goal first.".to_string())
    }
}

fn update_goal_status_cmd(status: GoalStatus) -> Result<String, String> {
    let mem = AgentMemory::new(path_resolver::agent_db_path_str()).map_err(|e| e.to_string())?;
    if let Some((goal_id, goal_text)) = mem.latest_goal("default").map_err(|e| e.to_string())? {
        mem.update_goal_status(&goal_id, status.as_str())
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "Goal '{}' marked as {}.",
            goal_text,
            status.as_str()
        ))
    } else {
        Err("No goal found.".to_string())
    }
}

fn summarize_reflection() -> Result<String, String> {
    use crate::api::agentic_monitor_routes::get_agent_db_connection;
    let conn = get_agent_db_connection().ok_or_else(|| "Database not available".to_string())?;
    let one_day_ago = chrono::Utc::now().timestamp() - 24 * 3600;
    let mut stmt = conn
        .prepare(
            "SELECT COUNT(*), SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) FROM episodes WHERE created_at > ?1",
        )
        .map_err(|e| e.to_string())?;
    let (total, success): (i64, i64) = stmt
        .query_row([one_day_ago], |row| {
            Ok((row.get(0)?, row.get(1).unwrap_or(0)))
        })
        .map_err(|e| e.to_string())?;
    let rate = if total > 0 {
        (success as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    Ok(format!(
        "Last 24h episodes: {} (success {:.1}%)",
        total, rate
    ))
}

fn explain_last_reasoning() -> String {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    if state.last_steps.is_empty() {
        "No reasoning trace available yet.".to_string()
    } else {
        let details: Vec<String> = state
            .last_steps
            .iter()
            .map(|step| format!("- [{}] {}", step.kind, step.message))
            .collect();
        details.join("\n")
    }
}

fn apply_focus(topic: Option<String>) -> String {
    let previous = record_focus_change(topic.clone());
    match (previous, topic) {
        (Some(prev), Some(new_topic)) => {
            format!("Focus switched from '{}' to '{}'.", prev, new_topic)
        }
        (None, Some(new_topic)) => format!("Focus set to '{}'.", new_topic),
        (_, None) => "Focus cleared.".to_string(),
    }
}

fn apply_persona(persona: Option<String>) -> String {
    let previous = record_persona_change(persona.clone());
    match (previous, persona) {
        (Some(prev), Some(new_persona)) => {
            format!("Persona switched from '{}' to '{}'.", prev, new_persona)
        }
        (None, Some(new_persona)) => format!("Persona set to '{}'.", new_persona),
        (_, None) => "Persona reset to default.".to_string(),
    }
}

fn apply_verbosity(mode: Verbosity) -> String {
    let previous = record_verbosity_change(mode);
    format!(
        "Verbosity changed from {} to {}.",
        previous.label(),
        mode.label()
    )
}

fn apply_model(model: Option<String>) -> String {
    let previous = record_model_change(model.clone());
    match (previous, model) {
        (Some(prev), Some(new_model)) => {
            format!("Model switched from '{}' to '{}'.", prev, new_model)
        }
        (None, Some(new_model)) => format!("Model set to '{}'.", new_model),
        (_, None) => "Model reset to default.".to_string(),
    }
}

fn apply_temperature(temp: Option<f32>) -> String {
    let previous = record_temperature_change(temp);
    match (previous, temp) {
        (Some(prev), Some(new_temp)) => {
            format!("Temperature changed from {:.2} to {:.2}.", prev, new_temp)
        }
        (None, Some(new_temp)) => format!("Temperature set to {:.2}.", new_temp),
        (_, None) => "Temperature reset to default.".to_string(),
    }
}

async fn run_calculator_tool(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if let Some(inner) = trimmed
        .strip_prefix("length(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let len = inner.chars().count();
        return Ok(format!("length(...) = {}", len));
    }
    let tool = CalculatorTool::new();
    match tool.execute(trimmed).await {
        Ok(result) if result.success => Ok(result.result),
        Ok(result) => Err(result.result),
        Err(err) => Err(err),
    }
}

async fn run_web_search_tool(input: &str) -> Result<String, String> {
    let tool = WebSearchTool::new();
    let result = tool.execute(input).await.map_err(|e| e.to_string())?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

async fn run_translator_tool(input: &str) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("Provide text to translate, e.g. 'translate hello to spanish'.".to_string());
    }
    let tool = TranslatorTool::new();
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

async fn run_sentiment_tool(input: &str) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("Provide text to analyze, e.g. 'sentiment I love this product'.".to_string());
    }
    let tool = SentimentAnalyzerTool::new();
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

async fn run_entity_tool(input: &str) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err(
            "Provide text to extract entities from, e.g. 'entities Elon Musk founded SpaceX'."
                .to_string(),
        );
    }
    let tool = EntityExtractorTool::new();
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

async fn run_spell_checker_tool(input: &str) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("Provide text to spell check, e.g. 'spellcheck teh quikc fox'.".to_string());
    }
    let tool = SpellCheckerTool::new();
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

async fn run_scheduler_tool(input: &str) -> Result<String, String> {
    let tool = SchedulerTool::new();
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

async fn run_memory_tool(input: &str) -> Result<String, String> {
    let tool = MemoryTool::new(None);
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

fn normalize_pipe_separators(command: &str) -> String {
    command
        .split('|')
        .map(|segment| segment.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

async fn run_tool_command(command: &str) -> Result<String, String> {
    let command = normalize_pipe_separators(command);
    let trimmed = command.trim();
    if trimmed.starts_with("calculator") {
        let expr = trimmed.strip_prefix("calculator").unwrap_or("").trim();
        if expr.is_empty() {
            Err("Provide an expression after 'calculator'.".to_string())
        } else {
            run_calculator_tool(expr).await
        }
    } else if trimmed.starts_with("search") {
        let query = trimmed.strip_prefix("search").unwrap_or("").trim();
        if query.is_empty() {
            Err("Provide a query after 'search'.".to_string())
        } else {
            run_web_search_tool(query).await
        }
    } else if trimmed.starts_with("fetch") {
        let url = trimmed.strip_prefix("fetch").unwrap_or("").trim();
        if url.is_empty() {
            Err("Provide a URL after 'fetch'.".to_string())
        } else {
            preview_url_content(url).await
        }
    } else if trimmed.starts_with("translate") {
        let request = trimmed.strip_prefix("translate").unwrap_or("").trim();
        run_translator_tool(request).await
    } else if trimmed.starts_with("sentiment") {
        let request = trimmed.strip_prefix("sentiment").unwrap_or("").trim();
        run_sentiment_tool(request).await
    } else if trimmed.starts_with("entities") {
        let request = trimmed.strip_prefix("entities").unwrap_or("").trim();
        run_entity_tool(request).await
    } else if trimmed.starts_with("spell") {
        let request = trimmed.strip_prefix("spell").unwrap_or("").trim();
        run_spell_checker_tool(request).await
    } else if trimmed.starts_with("schedule") {
        let request = trimmed.strip_prefix("schedule").unwrap_or("").trim();
        run_scheduler_tool(request).await
    } else if trimmed.starts_with("memory") {
        let request = trimmed.strip_prefix("memory").unwrap_or("").trim();
        run_memory_tool(request).await
    } else {
        Err("Unknown tool. Use 'calculator', 'search', 'fetch', 'translate', 'sentiment', 'entities', 'spell', 'schedule', or 'memory'.".to_string())
    }
}

async fn run_chain_command(chain: (String, String)) -> Result<String, String> {
    let first = run_tool_command(&chain.0).await?;
    let second_input = if chain.1.trim().contains("$last") {
        chain.1.replace("$last", &first)
    } else {
        chain.1.clone()
    };
    let second = run_tool_command(&second_input).await?;
    Ok(format!("Step1:\n{}\n\nStep2:\n{}", first, second))
}

fn retry_last_query(default_top_k: usize) -> Result<AgentResponse, String> {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    if let Some(last_query) = &state.last_query {
        if let Some(retriever) = RETRIEVER.get() {
            let query_clone = last_query.clone();
            drop(state);
            let agent = Agent::new(
                "default",
                path_resolver::agent_db_path_str(),
                Arc::clone(retriever),
            );
            let response = agent.run(&query_clone, default_top_k);
            update_last_agent_run(query_clone, &response);
            Ok(response)
        } else {
            Err("Retriever not initialized".to_string())
        }
    } else {
        Err("No query to retry yet.".to_string())
    }
}

fn apply_undo() -> String {
    if let Some(action) = pop_undo_action() {
        match action {
            CommandAction::FocusSet(previous) => apply_focus(previous),
            CommandAction::PersonaSet(previous) => apply_persona(previous),
            CommandAction::VerbosityChanged(previous) => apply_verbosity(previous),
            CommandAction::ModelChanged(previous) => apply_model(previous),
            CommandAction::TemperatureChanged(previous) => apply_temperature(previous),
            CommandAction::NoteAdded(_) => "Last note removal not supported yet.".to_string(),
        }
    } else {
        "Nothing to undo.".to_string()
    }
}

fn render_dry_run_plan(plan: &str) -> String {
    store_dry_run_plan(plan.to_string());
    format!("Planned actions:\n{}", plan)
}

fn export_state() -> String {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    let payload = json!({
        "focus": state.focus_topic,
        "persona": state.persona,
        "verbosity": state.verbosity.label(),
        "model": state.preferred_model,
        "temperature": state.temperature,
        "last_query": state.last_query,
        "last_response": state.last_response,
        "dry_run_plan": state.dry_run_plan,
    });

    let export_root = env::var("AG_EXPORT_DIR").unwrap_or_else(|_| {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join(".local/share/ag/exports").display().to_string()
    });

    if let Err(err) = fs::create_dir_all(&export_root) {
        return format!(
            "Exported in-memory only (failed to write {}): {}",
            export_root, err
        );
    }

    let filename = format!("export-{}.json", chrono::Utc::now().format("%Y%m%dT%H%M%S"));
    let path = Path::new(&export_root).join(filename);
    match fs::write(&path, payload.to_string()) {
        Ok(_) => format!("Exported to {}", path.display()),
        Err(err) => format!(
            "Exported in-memory only (failed to write {}): {}",
            path.display(),
            err
        ),
    }
}

fn import_state(raw: &str) -> String {
    if raw.trim().is_empty() {
        return "Provide JSON payload after /import.".to_string();
    }
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value) => {
            if let Some(model) = value.get("model").and_then(|v| v.as_str()) {
                record_model_change(if model.eq_ignore_ascii_case("default") {
                    None
                } else {
                    Some(model.to_string())
                });
            }
            if let Some(temp) = value.get("temperature").and_then(|v| v.as_f64()) {
                record_temperature_change(Some(temp as f32));
            }
            if let Some(focus) = value.get("focus").and_then(|v| v.as_str()) {
                record_focus_change(Some(focus.to_string()));
            }
            if let Some(persona) = value.get("persona").and_then(|v| v.as_str()) {
                record_persona_change(Some(persona.to_string()));
            }
            if let Some(verbosity) = value.get("verbosity").and_then(|v| v.as_str()) {
                let mode = match verbosity.to_lowercase().as_str() {
                    "brief" => Verbosity::Brief,
                    "verbose" => Verbosity::Verbose,
                    _ => Verbosity::Normal,
                };
                record_verbosity_change(mode);
            }
            "Import applied.".to_string()
        }
        Err(err) => format!("✗ Invalid import: {}", err),
    }
}

fn debug_state_snapshot() -> String {
    let (focus, persona, verbosity, last_query) = snapshots_for_debug();
    format!(
        "Debug State:\n- Focus: {:?}\n- Persona: {:?}\n- Verbosity: {:?}\n- Last query: {:?}",
        focus,
        persona,
        verbosity.label(),
        last_query
    )
}

fn tokens_usage_snapshot() -> String {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    match state.last_token_usage {
        Some(tokens) => format!("Approximate token usage: {}", tokens),
        None => "No token usage recorded yet.".to_string(),
    }
}

async fn preview_url_content(url: &str) -> Result<String, String> {
    let tool = URLFetchTool::new();
    let query = format!("Fetch {}", url);
    let result = tool.execute(&query).await.map_err(|e| e.to_string())?;
    if result.success {
        Ok(format!("Learned from {}:\n{}", url, result.result))
    } else {
        Err(result.result)
    }
}

async fn run_agent(req: web::Json<AgentRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    // Check for chat commands
    if let Some(cmd) = parse_chat_command(&req.query) {
        let (answer, extra) = match cmd {
            ChatCommand::Goal(goal_text) => match create_goal_from_command(&goal_text) {
                Ok(goal) => (
                    format!("✓ Goal created: {}", goal_text),
                    Some(json!({ "goal": goal })),
                ),
                Err(e) => (format!("✗ Failed to create goal: {}", e), None),
            },
            ChatCommand::Goals => match get_goals_list() {
                Ok(list) => (list, None),
                Err(e) => (format!("✗ Failed to get goals: {}", e), None),
            },
            ChatCommand::Status => (get_system_status(), None),
            ChatCommand::Help => (get_help_text(), None),
            ChatCommand::Models => (get_models_list(), None),
            ChatCommand::Clear => (
                "Chat cleared. (This is handled by the frontend)".to_string(),
                None,
            ),
            ChatCommand::Forget(topic) => match forget_topic(&topic) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::History => match list_recent_history(5) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Sources => (last_sources_summary(), None),
            ChatCommand::Learn(url) => match preview_url_content(&url).await {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Note(text) => match record_note(&text) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Subgoal(text) => match add_subgoal(&text) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::PauseGoal => match update_goal_status_cmd(GoalStatus::Paused) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::ResumeGoal => match update_goal_status_cmd(GoalStatus::Active) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::AbandonGoal => match update_goal_status_cmd(GoalStatus::Abandoned) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Reflect => match summarize_reflection() {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Why => (explain_last_reasoning(), None),
            ChatCommand::Focus(topic) => (apply_focus(Some(topic)), None),
            ChatCommand::Unfocus => (apply_focus(None), None),
            ChatCommand::Persona(name) => {
                let persona_value =
                    if name.eq_ignore_ascii_case("default") || name.eq_ignore_ascii_case("reset") {
                        None
                    } else {
                        Some(name)
                    };
                (apply_persona(persona_value), None)
            }
            ChatCommand::Verbose => (apply_verbosity(Verbosity::Verbose), None),
            ChatCommand::Brief => (apply_verbosity(Verbosity::Brief), None),
            ChatCommand::RunTool(spec) => match run_tool_command(&spec).await {
                Ok(result) => (result, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Chain(first, second) => match run_chain_command((first, second)).await {
                Ok(result) => (result, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Retry => match retry_last_query(req.top_k) {
                Ok(agent_response) => (
                    agent_response.answer.clone(),
                    Some(json!({ "retry": agent_response })),
                ),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Undo => (apply_undo(), None),
            ChatCommand::DryRun(plan) => (render_dry_run_plan(&plan), None),
            ChatCommand::Model(name) => {
                let model_value = if name.eq_ignore_ascii_case("default") {
                    None
                } else {
                    Some(name)
                };
                (apply_model(model_value), None)
            }
            ChatCommand::Temperature(value) => {
                let parsed = value.parse::<f32>().ok();
                (apply_temperature(parsed), None)
            }
            ChatCommand::Export => (export_state(), None),
            ChatCommand::Import(payload) => {
                let body = payload.unwrap_or_else(|| "{}".to_string());
                (import_state(&body), None)
            }
            ChatCommand::Debug => (debug_state_snapshot(), None),
            ChatCommand::Tokens => (tokens_usage_snapshot(), None),
        };

        let mut response = json!({
            "response": {
                "answer": answer,
                "chunks_used": 0,
                "sources": []
            },
            "request_id": request_id
        });

        if let Some(extra_data) = extra {
            if let Some(obj) = response.as_object_mut() {
                for (k, v) in extra_data.as_object().unwrap() {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }

        return Ok(HttpResponse::Ok().json(response));
    }

    if let Some(retriever) = RETRIEVER.get() {
        // Convert ChatMode to AgentMode
        let agent_mode = match req.mode {
            ChatMode::Rag => crate::agent::AgentMode::Rag,
            ChatMode::Llm => crate::agent::AgentMode::Llm,
            ChatMode::Hybrid => crate::agent::AgentMode::Hybrid,
            ChatMode::Auto => crate::agent::AgentMode::Auto,
            ChatMode::RagStrict => crate::agent::AgentMode::RagStrict,
        };
        let query_clone = req.query.clone();
        let top_k = req.top_k;
        let retriever_clone = Arc::clone(retriever);

        // Get current chat settings
        let chat_settings = get_current_chat_settings();

        // Run agent in blocking thread pool to avoid blocking async runtime
        let resp = web::block(move || {
            let agent = Agent::new(
                "default",
                path_resolver::agent_db_path_str(),
                retriever_clone,
            )
            .with_settings(chat_settings);
            agent.run_with_mode(&query_clone, top_k, agent_mode)
        })
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Agent error: {}", e)))?;

        update_last_agent_run(req.query.clone(), &resp);
        Ok(HttpResponse::Ok().json(json!({
            "response": resp,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

// GET-based chat endpoint to avoid CORS preflight
async fn run_agent_get(query: web::Query<AgentQueryParams>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    // Check for chat commands
    if let Some(cmd) = parse_chat_command(&query.query) {
        let (answer, extra) = match cmd {
            ChatCommand::Goal(goal_text) => match create_goal_from_command(&goal_text) {
                Ok(goal) => (
                    format!("✓ Goal created: {}", goal_text),
                    Some(json!({ "goal": goal })),
                ),
                Err(e) => (format!("✗ Failed to create goal: {}", e), None),
            },
            ChatCommand::Goals => match get_goals_list() {
                Ok(list) => (list, None),
                Err(e) => (format!("✗ Failed to get goals: {}", e), None),
            },
            ChatCommand::Status => (get_system_status(), None),
            ChatCommand::Help => (get_help_text(), None),
            ChatCommand::Models => (get_models_list(), None),
            ChatCommand::Clear => (
                "Chat cleared. (This is handled by the frontend)".to_string(),
                None,
            ),
            ChatCommand::Forget(topic) => match forget_topic(&topic) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::History => match list_recent_history(5) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Sources => (last_sources_summary(), None),
            ChatCommand::Learn(url) => match preview_url_content(&url).await {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Note(text) => match record_note(&text) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Subgoal(text) => match add_subgoal(&text) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::PauseGoal => match update_goal_status_cmd(GoalStatus::Paused) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::ResumeGoal => match update_goal_status_cmd(GoalStatus::Active) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::AbandonGoal => match update_goal_status_cmd(GoalStatus::Abandoned) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Reflect => match summarize_reflection() {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Why => (explain_last_reasoning(), None),
            ChatCommand::Focus(topic) => (apply_focus(Some(topic)), None),
            ChatCommand::Unfocus => (apply_focus(None), None),
            ChatCommand::Persona(name) => {
                let persona_value =
                    if name.eq_ignore_ascii_case("default") || name.eq_ignore_ascii_case("reset") {
                        None
                    } else {
                        Some(name)
                    };
                (apply_persona(persona_value), None)
            }
            ChatCommand::Verbose => (apply_verbosity(Verbosity::Verbose), None),
            ChatCommand::Brief => (apply_verbosity(Verbosity::Brief), None),
            ChatCommand::RunTool(spec) => match run_tool_command(&spec).await {
                Ok(result) => (result, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Chain(first, second) => match run_chain_command((first, second)).await {
                Ok(result) => (result, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Retry => match retry_last_query(query.top_k) {
                Ok(agent_response) => (
                    agent_response.answer.clone(),
                    Some(json!({ "retry": agent_response })),
                ),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Undo => (apply_undo(), None),
            ChatCommand::DryRun(plan) => (render_dry_run_plan(&plan), None),
            ChatCommand::Model(name) => {
                let model_value = if name.eq_ignore_ascii_case("default") {
                    None
                } else {
                    Some(name)
                };
                (apply_model(model_value), None)
            }
            ChatCommand::Temperature(value) => {
                let parsed = value.parse::<f32>().ok();
                (apply_temperature(parsed), None)
            }
            ChatCommand::Export => (export_state(), None),
            ChatCommand::Import(payload) => {
                let body = payload.unwrap_or_else(|| "{}".to_string());
                (import_state(&body), None)
            }
            ChatCommand::Debug => (debug_state_snapshot(), None),
            ChatCommand::Tokens => (tokens_usage_snapshot(), None),
        };

        let mut response = json!({
            "response": {
                "answer": answer,
                "chunks_used": 0,
                "sources": []
            },
            "request_id": request_id
        });

        if let Some(extra_data) = extra {
            if let Some(obj) = response.as_object_mut() {
                for (k, v) in extra_data.as_object().unwrap() {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }

        return Ok(HttpResponse::Ok().json(response));
    }

    if let Some(retriever) = RETRIEVER.get() {
        // Convert ChatMode to AgentMode
        let agent_mode = match query.mode {
            ChatMode::Rag => crate::agent::AgentMode::Rag,
            ChatMode::Llm => crate::agent::AgentMode::Llm,
            ChatMode::Hybrid => crate::agent::AgentMode::Hybrid,
            ChatMode::Auto => crate::agent::AgentMode::Auto,
            ChatMode::RagStrict => crate::agent::AgentMode::RagStrict,
        };
        let query_str = query.query.clone();
        let top_k = query.top_k;
        let retriever_clone = Arc::clone(retriever);

        // Get current chat settings
        let chat_settings = get_current_chat_settings();

        // Run agent in blocking thread pool to avoid blocking async runtime
        let resp = web::block(move || {
            let agent = Agent::new(
                "default",
                path_resolver::agent_db_path_str(),
                retriever_clone,
            )
            .with_settings(chat_settings);
            agent.run_with_mode(&query_str, top_k, agent_mode)
        })
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Agent error: {}", e)))?;

        update_last_agent_run(query.query.clone(), &resp);
        Ok(HttpResponse::Ok().json(json!({
            "response": resp,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

// ============================================================================
// OPENAI STREAMING HANDLER
// ============================================================================

/// Stream response from OpenAI API
/// OpenAI uses Server-Sent Events with format:
/// data: {"choices":[{"delta":{"content":"text"}}]}
/// data: [DONE]
async fn stream_openai_response(
    client: reqwest::Client,
    api_key: &str,
    model: &str,
    body: serde_json::Value,
    chunks_count: usize,
    request_id: String,
) -> Result<HttpResponse, Error> {
    use actix_web::web::Bytes;
    use futures_util::stream::StreamExt;

    let url = "https://api.openai.com/v1/chat/completions";

    tracing::info!(
        model = %model,
        request_id = %request_id,
        "Streaming from OpenAI API"
    );

    match client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracing::error!("OpenAI API error: {} - {}", status, error_text);
                let error_response = serde_json::json!({
                    "type": "error",
                    "message": format!("OpenAI API error: {} - {}", status, error_text),
                    "request_id": request_id
                });
                return Ok(HttpResponse::Ok()
                    .content_type("text/event-stream")
                    .body(format!("data: {}\n\n", error_response)));
            }

            let stream = response.bytes_stream().map(move |chunk_result| {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let mut output = String::new();

                        for line in text.lines() {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }

                            // OpenAI SSE format: "data: {...}" or "data: [DONE]"
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    let event = serde_json::json!({
                                        "type": "done",
                                        "chunks_used": chunks_count
                                    });
                                    output.push_str(&format!("data: {}\n\n", event));
                                } else if let Ok(json) =
                                    serde_json::from_str::<serde_json::Value>(data)
                                {
                                    // Extract content from choices[0].delta.content
                                    if let Some(content) = json
                                        .get("choices")
                                        .and_then(|c| c.get(0))
                                        .and_then(|c| c.get("delta"))
                                        .and_then(|d| d.get("content"))
                                        .and_then(|c| c.as_str())
                                    {
                                        if !content.is_empty() {
                                            let event = serde_json::json!({
                                                "type": "token",
                                                "content": content
                                            });
                                            output.push_str(&format!("data: {}\n\n", event));
                                        }
                                    }
                                }
                            }
                        }

                        Ok::<Bytes, actix_web::error::Error>(Bytes::from(output))
                    }
                    Err(e) => {
                        let error_event = serde_json::json!({
                            "type": "error",
                            "message": format!("Stream error: {}", e)
                        });
                        Ok(Bytes::from(format!("data: {}\n\n", error_event)))
                    }
                }
            });

            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .insert_header(("Cache-Control", "no-cache"))
                .insert_header(("X-Accel-Buffering", "no"))
                .insert_header(("Access-Control-Allow-Origin", "*"))
                .streaming(stream))
        }
        Err(e) => {
            let error_response = serde_json::json!({
                "type": "error",
                "message": format!("Failed to connect to OpenAI: {}", e),
                "request_id": request_id
            });
            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .body(format!("data: {}\n\n", error_response)))
        }
    }
}

// ============================================================================
// ANTHROPIC STREAMING HANDLER
// ============================================================================

/// Stream response from Anthropic API
/// Anthropic uses Server-Sent Events with format:
/// event: content_block_delta
/// data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"content"}}
/// event: message_stop
/// data: {"type":"message_stop"}
#[allow(clippy::too_many_arguments)]
async fn stream_anthropic_response(
    client: reqwest::Client,
    api_key: &str,
    model: &str,
    payload: serde_json::Value,
    temperature: f32,
    max_tokens: usize,
    chunks_count: usize,
    request_id: String,
    use_caching: bool,
) -> Result<HttpResponse, Error> {
    use actix_web::web::Bytes;
    use futures_util::stream::StreamExt;

    let url = "https://api.anthropic.com/v1/messages";

    // Build the request body
    let system = payload
        .get("system")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let messages = payload
        .get("messages")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "stream": true,
        "messages": messages
    });

    // Add system if present
    if !system.is_null() {
        body["system"] = system;
    }

    tracing::info!(
        model = %model,
        request_id = %request_id,
        caching = use_caching,
        "Streaming from Anthropic API"
    );

    let mut request = client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json");

    // Add beta header for prompt caching if enabled
    if use_caching {
        request = request.header("anthropic-beta", "prompt-caching-2024-07-31");
    }

    match request.json(&body).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracing::error!("Anthropic API error: {} - {}", status, error_text);
                let error_response = serde_json::json!({
                    "type": "error",
                    "message": format!("Anthropic API error: {} - {}", status, error_text),
                    "request_id": request_id
                });
                return Ok(HttpResponse::Ok()
                    .content_type("text/event-stream")
                    .body(format!("data: {}\n\n", error_response)));
            }

            let stream = response.bytes_stream().map(move |chunk_result| {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let mut output = String::new();
                        let mut current_event_type = String::new();

                        for line in text.lines() {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }

                            // Anthropic SSE format: "event: type" followed by "data: {...}"
                            if let Some(event_type) = line.strip_prefix("event: ") {
                                current_event_type = event_type.to_string();
                            } else if let Some(data) = line.strip_prefix("data: ") {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                    match current_event_type.as_str() {
                                        "content_block_delta" => {
                                            // Extract text from delta.text
                                            if let Some(text_content) = json
                                                .get("delta")
                                                .and_then(|d| d.get("text"))
                                                .and_then(|t| t.as_str())
                                            {
                                                if !text_content.is_empty() {
                                                    let event = serde_json::json!({
                                                        "type": "token",
                                                        "content": text_content
                                                    });
                                                    output
                                                        .push_str(&format!("data: {}\n\n", event));
                                                }
                                            }
                                        }
                                        "message_stop" | "message_delta" => {
                                            // Check if this is the final message
                                            if json.get("type").and_then(|t| t.as_str())
                                                == Some("message_stop")
                                            {
                                                let event = serde_json::json!({
                                                    "type": "done",
                                                    "chunks_used": chunks_count
                                                });
                                                output.push_str(&format!("data: {}\n\n", event));
                                            }
                                        }
                                        "error" => {
                                            let error_msg = json
                                                .get("error")
                                                .and_then(|e| e.get("message"))
                                                .and_then(|m| m.as_str())
                                                .unwrap_or("Unknown error");
                                            let event = serde_json::json!({
                                                "type": "error",
                                                "message": error_msg
                                            });
                                            output.push_str(&format!("data: {}\n\n", event));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }

                        Ok::<Bytes, actix_web::error::Error>(Bytes::from(output))
                    }
                    Err(e) => {
                        let error_event = serde_json::json!({
                            "type": "error",
                            "message": format!("Stream error: {}", e)
                        });
                        Ok(Bytes::from(format!("data: {}\n\n", error_event)))
                    }
                }
            });

            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .insert_header(("Cache-Control", "no-cache"))
                .insert_header(("X-Accel-Buffering", "no"))
                .insert_header(("Access-Control-Allow-Origin", "*"))
                .streaming(stream))
        }
        Err(e) => {
            let error_response = serde_json::json!({
                "type": "error",
                "message": format!("Failed to connect to Anthropic: {}", e),
                "request_id": request_id
            });
            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .body(format!("data: {}\n\n", error_response)))
        }
    }
}

// Streaming agent endpoint using Server-Sent Events
async fn run_agent_stream(req: web::Json<AgentRequest>) -> Result<HttpResponse, Error> {
    use crate::memory::prompt_cache::CacheOptimizedPrompt;
    use actix_web::web::Bytes;
    use futures_util::stream::StreamExt;

    let request_id = generate_request_id();

    // For commands, redirect to non-streaming endpoint (commands don't benefit from streaming)
    if parse_chat_command(&req.query).is_some() {
        // Just call the regular endpoint for commands
        return run_agent(req).await;
    }

    // Get hardware config to determine backend type
    let hardware_config = crate::db::param_hardware::global_config();
    let backend_type = hardware_config.backend_type;
    let prompt_caching = get_prompt_caching_enabled();
    let thread_count = hardware_config.num_thread.max(1);

    // Get Ollama config (used for Ollama backend)
    let ollama_url =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let model = if !hardware_config.model.is_empty() {
        hardware_config.model.clone()
    } else {
        std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "phi:latest".to_string())
    };

    // Determine mode and build context
    let agent_mode = match req.mode {
        ChatMode::Rag => crate::agent::AgentMode::Rag,
        ChatMode::Llm => crate::agent::AgentMode::Llm,
        ChatMode::Hybrid => crate::agent::AgentMode::Hybrid,
        ChatMode::Auto => crate::agent::AgentMode::Auto,
        ChatMode::RagStrict => crate::agent::AgentMode::RagStrict,
    };

    // For RAG-only mode, use non-streaming (document search doesn't benefit from streaming)
    if matches!(
        agent_mode,
        crate::agent::AgentMode::Rag | crate::agent::AgentMode::RagStrict
    ) {
        if let Some(retriever) = RETRIEVER.get() {
            let query_clone = req.query.clone();
            let top_k = req.top_k;
            let retriever_clone = Arc::clone(retriever);
            let chat_settings = get_current_chat_settings();

            let resp = web::block(move || {
                let agent = Agent::new(
                    "default",
                    path_resolver::agent_db_path_str(),
                    retriever_clone,
                )
                .with_settings(chat_settings);
                agent.run_with_mode(&query_clone, top_k, crate::agent::AgentMode::Rag)
            })
            .await
            .map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Agent error: {}", e))
            })?;

            update_last_agent_run(req.query.clone(), &resp);
            let json_response = serde_json::json!({
                "type": "complete",
                "answer": resp.answer,
                "steps": resp.steps,
                "used_chunks": resp.used_chunks,
                "request_id": request_id
            });
            return Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .insert_header(("Cache-Control", "no-cache"))
                .body(format!("data: {}\n\n", json_response)));
        }
    }

    // For LLM and Hybrid modes, stream from Ollama
    let mut context = String::new();
    let mut used_chunks: Vec<String> = Vec::new();

    // For Hybrid/Auto mode, first get RAG context
    if matches!(
        agent_mode,
        crate::agent::AgentMode::Hybrid | crate::agent::AgentMode::Auto
    ) {
        let search_start = std::time::Instant::now();
        if let Some(retriever) = RETRIEVER.get() {
            if let Ok(mut r) = retriever.lock() {
                if let Ok(mut results) = r.hybrid_search(&req.query, None) {
                    let search_time = search_start.elapsed().as_millis() as u64;
                    if results.len() > req.top_k {
                        results.truncate(req.top_k);
                    }
                    let result_count = results.len();
                    if !results.is_empty() {
                        context = results.join("\n\n");
                        used_chunks = results;
                    }
                    // Record tool execution
                    crate::monitoring::record_tool_execution(
                        "SemanticSearch",
                        &req.query,
                        true,
                        &format!("{} chunks", result_count),
                        search_time,
                        1.0,
                        Some("api/chat/stream"),
                    );
                }
            }
        }
    }

    // Get chat settings for prompt building
    let chat_settings = get_current_chat_settings();
    let system_prompt = chat_settings.build_system_prompt();

    // Debug: log what's in the system prompt
    tracing::warn!(
        request_id = %request_id,
        system_prompt_len = system_prompt.len(),
        system_prompt_full = %system_prompt,
        memories_count = chat_settings.memories.len(),
        "DEBUG: Full system prompt being sent"
    );
    for (i, mem) in chat_settings.memories.iter().enumerate() {
        tracing::warn!(
            request_id = %request_id,
            memory_index = i,
            memory_type = %mem.memory_type,
            memory_content = %mem.content,
            "DEBUG: Memory item"
        );
    }

    // Build prompt with settings
    // Note: System prompt/instructions are sent via the 'system' field, not in the prompt
    // This prevents the LLM from echoing instructions back to the user
    let prompt = {
        let mut parts = Vec::new();

        // Add context if present, or fallback instruction for hybrid mode
        if !context.is_empty() {
            parts.push(format!(
                "Context (ignore if not relevant to the question):\n{}\n\nAnswer the question directly. If the context above is not relevant, use your own knowledge.",
                context
            ));
        } else if matches!(
            agent_mode,
            crate::agent::AgentMode::Hybrid | crate::agent::AgentMode::Auto
        ) {
            // Hybrid/Auto mode with no context: tell LLM to answer from its knowledge
            parts.push("Answer the question based on your knowledge.".to_string());
        }

        // Add question
        parts.push(format!("Question: {}", req.query));
        parts.push("Answer:".to_string());

        parts.join("\n\n")
    };

    // Get mode-specific config
    use crate::db::llm_settings::LlmConfig;
    let mut config = match agent_mode {
        crate::agent::AgentMode::Rag | crate::agent::AgentMode::RagStrict => {
            LlmConfig::documents_only()
        }
        crate::agent::AgentMode::Llm => LlmConfig::llm_only(),
        crate::agent::AgentMode::Hybrid | crate::agent::AgentMode::Auto => LlmConfig::combined(),
    };

    // Apply temperature override if set
    if let Some(temp) = chat_settings.temperature {
        config.temperature = temp;
    }

    // Use model override if set
    let final_model = chat_settings.model.unwrap_or(model);

    // Build cache-optimized prompt structure
    let cache_prompt = CacheOptimizedPrompt::new()
        .with_system_prompt(&system_prompt)
        .with_context(&context)
        .with_user_query(&req.query);

    // Create streaming request based on backend type and caching preference
    let client = reqwest::Client::new();
    let chunks_count = used_chunks.len();

    // Determine URL and request body based on backend and caching
    let (url, request_body) = match backend_type {
        crate::db::param_hardware::BackendType::Ollama => {
            if prompt_caching {
                // Use /api/chat for KV cache reuse
                let options = serde_json::json!({
                    "temperature": config.temperature,
                    "top_p": config.top_p,
                    "top_k": config.top_k,
                    "num_predict": config.max_tokens,
                    "repeat_penalty": config.repeat_penalty,
                    "num_thread": thread_count,
                    "num_ctx": hardware_config.num_ctx
                });
                let body =
                    cache_prompt.build_ollama_chat_request(&final_model, true, Some(options));
                (format!("{}/api/chat", ollama_url), body)
            } else {
                // Use /api/generate (no caching)
                let body = serde_json::json!({
                    "model": final_model,
                    "prompt": prompt,
                    "stream": true,
                    "options": {
                        "temperature": config.temperature,
                        "top_p": config.top_p,
                        "top_k": config.top_k,
                        "num_predict": config.max_tokens,
                        "repeat_penalty": config.repeat_penalty,
                        "num_thread": thread_count,
                        "num_ctx": hardware_config.num_ctx
                    },
                    "system": if system_prompt.is_empty() { serde_json::Value::Null } else { serde_json::json!(system_prompt) }
                });
                (format!("{}/api/generate", ollama_url), body)
            }
        }
        crate::db::param_hardware::BackendType::OpenAi => {
            // OpenAI API with automatic prefix caching
            let api_key = std::env::var("OPENAI_API_KEY")
                .unwrap_or_else(|_| crate::db::api_keys::global_config().openai_api_key.clone());

            if api_key.is_empty() {
                tracing::warn!("OpenAI API key not configured, falling back to Ollama");
                let fallback_body = serde_json::json!({
                    "model": final_model,
                    "prompt": prompt,
                    "stream": true,
                    "options": {
                        "temperature": config.temperature,
                        "top_p": config.top_p,
                        "top_k": config.top_k,
                        "num_predict": config.max_tokens,
                        "repeat_penalty": config.repeat_penalty,
                        "num_thread": thread_count
                    }
                });
                (format!("{}/api/generate", ollama_url), fallback_body)
            } else {
                let messages = if prompt_caching {
                    cache_prompt.build_openai_messages()
                } else {
                    vec![
                        serde_json::json!({"role": "system", "content": system_prompt}),
                        serde_json::json!({"role": "user", "content": format!("{}\n\nQuestion: {}", context, req.query)}),
                    ]
                };
                let body = serde_json::json!({
                    "model": final_model,
                    "messages": messages,
                    "stream": true,
                    "temperature": config.temperature,
                    "max_tokens": config.max_tokens
                });
                // Return special marker for OpenAI handling
                return stream_openai_response(
                    client,
                    &api_key,
                    &final_model,
                    body,
                    chunks_count,
                    request_id,
                )
                .await;
            }
        }
        crate::db::param_hardware::BackendType::Anthropic => {
            // Anthropic API with explicit cache_control
            let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_else(|_| {
                crate::db::api_keys::global_config()
                    .anthropic_api_key
                    .clone()
            });

            if api_key.is_empty() {
                tracing::warn!("Anthropic API key not configured, falling back to Ollama");
                let fallback_body = serde_json::json!({
                    "model": final_model,
                    "prompt": prompt,
                    "stream": true,
                    "options": {
                        "temperature": config.temperature,
                        "top_p": config.top_p,
                        "top_k": config.top_k,
                        "num_predict": config.max_tokens,
                        "repeat_penalty": config.repeat_penalty,
                        "num_thread": thread_count
                    }
                });
                (format!("{}/api/generate", ollama_url), fallback_body)
            } else {
                let anthropic_payload = if prompt_caching {
                    cache_prompt.build_anthropic_messages()
                } else {
                    serde_json::json!({
                        "system": system_prompt,
                        "messages": [{
                            "role": "user",
                            "content": format!("{}\n\nQuestion: {}", context, req.query)
                        }]
                    })
                };
                // Return special marker for Anthropic handling
                return stream_anthropic_response(
                    client,
                    &api_key,
                    &final_model,
                    anthropic_payload,
                    config.temperature,
                    config.max_tokens,
                    chunks_count,
                    request_id,
                    prompt_caching,
                )
                .await;
            }
        }
        crate::db::param_hardware::BackendType::LlamaCpp => {
            // llama-server: OpenAI-compatible streaming
            let llama_url = hardware_config.llama_server_url.clone();
            let mut messages = Vec::new();
            if !system_prompt.is_empty() {
                messages.push(serde_json::json!({"role": "system", "content": system_prompt}));
            }
            messages.push(serde_json::json!({"role": "user", "content": prompt}));
            let body = serde_json::json!({
                "model": final_model,
                "messages": messages,
                "stream": true,
                "temperature": config.temperature,
                "max_tokens": config.max_tokens
            });
            (format!("{}/v1/chat/completions", llama_url), body)
        }
        _ => {
            // Default to Ollama /api/generate for other backends
            let body = serde_json::json!({
                "model": final_model,
                "prompt": prompt,
                "stream": true,
                "options": {
                    "temperature": config.temperature,
                    "top_p": config.top_p,
                    "top_k": config.top_k,
                    "num_predict": config.max_tokens,
                    "repeat_penalty": config.repeat_penalty,
                    "num_thread": thread_count
                },
                "system": if system_prompt.is_empty() { serde_json::Value::Null } else { serde_json::json!(system_prompt) }
            });
            (format!("{}/api/generate", ollama_url), body)
        }
    };

    tracing::warn!(
        backend = ?backend_type,
        caching = prompt_caching,
        url = %url,
        "DEBUG Sending LLM request"
    );

    match client.post(&url).json(&request_body).send().await {
        Ok(response) => {
            let stream = response.bytes_stream().map(move |chunk_result| {
                match chunk_result {
                    Ok(bytes) => {
                        // Parse Ollama's streaming response (newline-delimited JSON)
                        // Handles both /api/generate ("response" field) and /api/chat ("message.content" field)
                        let text = String::from_utf8_lossy(&bytes);
                        let mut output = String::new();

                        for line in text.lines() {
                            if line.is_empty() {
                                continue;
                            }
                            // Handle OpenAI SSE format: "data: {...}" or "data: [DONE]"
                            let json_str = if line.starts_with("data: ") {
                                let payload = &line[6..];
                                if payload == "[DONE]" {
                                    let event = serde_json::json!({
                                        "type": "done",
                                        "chunks_used": chunks_count
                                    });
                                    output.push_str(&format!("data: {}\n\n", event));
                                    continue;
                                }
                                payload
                            } else {
                                line
                            };
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                                // Try OpenAI streaming format (choices[0].delta.content)
                                let response_text = json
                                    .get("choices")
                                    .and_then(|c| c.as_array())
                                    .and_then(|c| c.first())
                                    .and_then(|c| c.get("delta"))
                                    .and_then(|d| d.get("content"))
                                    .and_then(|v| v.as_str())
                                    // Try /api/generate format ("response" field)
                                    .or_else(|| {
                                        json.get("response").and_then(|v| v.as_str())
                                    })
                                    // Try /api/chat format ("message.content" field)
                                    .or_else(|| {
                                        json.get("message")
                                            .and_then(|m| m.get("content"))
                                            .and_then(|c| c.as_str())
                                    });

                                if let Some(text) = response_text {
                                    if !text.is_empty() {
                                        let event = serde_json::json!({
                                            "type": "token",
                                            "content": text
                                        });
                                        output.push_str(&format!("data: {}\n\n", event));
                                    }
                                }
                                if json.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
                                    let event = serde_json::json!({
                                        "type": "done",
                                        "chunks_used": chunks_count
                                    });
                                    output.push_str(&format!("data: {}\n\n", event));
                                }
                            }
                        }

                        Ok::<Bytes, actix_web::error::Error>(Bytes::from(output))
                    }
                    Err(e) => {
                        let error_event = serde_json::json!({
                            "type": "error",
                            "message": format!("Stream error: {}", e)
                        });
                        Ok(Bytes::from(format!("data: {}\n\n", error_event)))
                    }
                }
            });

            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .insert_header(("Cache-Control", "no-cache"))
                .insert_header(("X-Accel-Buffering", "no"))
                .insert_header(("Access-Control-Allow-Origin", "*"))
                .streaming(stream))
        }
        Err(e) => {
            let error_response = serde_json::json!({
                "type": "error",
                "message": format!("Failed to connect to Ollama: {}", e),
                "request_id": request_id
            });
            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .body(format!("data: {}\n\n", error_response)))
        }
    }
}

fn latest_log_file(log_dir: &Path) -> Option<PathBuf> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    if let Ok(entries) = fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !file_name.starts_with(LOG_FILE_PREFIX) {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let replace = newest
                .as_ref()
                .map(|(ts, _)| modified > *ts)
                .unwrap_or(true);
            if replace {
                newest = Some((modified, path));
            }
        }
    }
    newest.map(|(_, path)| path)
}

fn read_recent_lines(path: &Path, limit: usize) -> std::io::Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut buffer = VecDeque::with_capacity(limit);
    for line in reader.lines() {
        let line = line?;
        if buffer.len() == limit {
            buffer.pop_front();
        }
        buffer.push_back(line);
    }
    Ok(buffer.into_iter().collect())
}

fn parse_log_line(line: &str) -> LogEntry {
    let parsed = serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|value| match value {
            Value::Object(_) => Some(value),
            _ => None,
        });
    if let Some(value) = parsed {
        let timestamp = value
            .get("timestamp")
            .or_else(|| value.get("ts"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let level = value
            .get("level")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let target = value
            .get("target")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let fields = value.get("fields").cloned();
        let message = fields
            .as_ref()
            .and_then(|f| f.get("message"))
            .or_else(|| value.get("message"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        LogEntry {
            timestamp,
            level,
            target,
            message,
            raw: line.to_string(),
            fields,
        }
    } else {
        LogEntry {
            timestamp: None,
            level: None,
            target: None,
            message: None,
            raw: line.to_string(),
            fields: None,
        }
    }
}

pub mod agentic_monitor_routes;
pub mod graph_routes;
pub mod sys_routes;
pub mod tool_routes;

fn require_admin(req: &HttpRequest, config: &ApiConfig) -> Result<(), Error> {
    if let Some(expected) = &config.admin_api_token {
        if expected.is_empty() {
            return Ok(());
        }
        if let Some(header) = req.headers().get(AUTHORIZATION) {
            if let Ok(value) = header.to_str() {
                if value == expected || value.trim_start_matches("Bearer ") == expected {
                    return Ok(());
                }
            }
        }
        Err(actix_web::error::ErrorUnauthorized(
            "Missing or invalid admin API token",
        ))
    } else {
        Err(actix_web::error::ErrorUnauthorized(
            "ADMIN_API_TOKEN not configured",
        ))
    }
}

fn observe_manual_endpoint<F>(endpoint: &'static str, f: F) -> Result<HttpResponse, Error>
where
    F: FnOnce() -> Result<HttpResponse, Error>,
{
    let start = Instant::now();
    let span = info_span!("manual_observation", endpoint);
    let _guard = span.enter();
    let result = f();
    metrics::record_manual_observation(
        endpoint,
        result.is_ok(),
        start.elapsed().as_secs_f64() * 1000.0,
    );
    result
}

/// Helper for 3-layer memory search metrics (SEARCH.md)
/// layer: "search" | "timeline" | "fetch"
fn observe_memory_search_layer<F>(layer: &'static str, f: F) -> Result<HttpResponse, Error>
where
    F: FnOnce() -> Result<HttpResponse, Error>,
{
    let start = Instant::now();
    let span = info_span!("memory_search_layer", layer);
    let _guard = span.enter();
    let result = f();
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    // Record both the general manual observation metric and the layer-specific metric
    metrics::record_manual_observation(layer, result.is_ok(), duration_ms);
    metrics::record_memory_search_layer(layer, result.is_ok(), duration_ms);

    result
}

pub fn start_api_server(
    config: &ApiConfig,
) -> impl std::future::Future<Output = std::io::Result<()>> {
    // Snapshot needed config values to satisfy 'static factory closure
    let bind_addr = config.bind_addr();
    let trust_proxy = config.trust_proxy;
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
    let api_config = config.clone();
    let mut http_server = HttpServer::new(move || {
        let api_config = api_config.clone();
        // Shared RateLimiter across workers (middleware-only enforcement)
        let rl_cfg = crate::security::rate_limiter::RateLimiterConfig {
            enabled: rate_limit_enabled,
            qps: rate_limit_qps.max(0.0),
            burst: rate_limit_burst,
            max_ips: rate_limit_lru_capacity,
        };
        let rl = std::sync::Arc::new(crate::security::rate_limiter::RateLimiter::new(rl_cfg));
        let opts = RateLimitOptions {
            trust_proxy,
            search_qps: search_qps.max(0.0),
            search_burst,
            upload_qps: upload_qps.max(0.0),
            upload_burst,
            rules: vec![
                RouteRule {
                    pattern: "/reindex".into(),
                    match_kind: MatchKind::Exact,
                    qps: 0.5,
                    burst: 2.0,
                    label: Some("admin-reindex".into()),
                },
                RouteRule {
                    pattern: "/upload".into(),
                    match_kind: MatchKind::Prefix,
                    qps: upload_qps.max(0.0),
                    burst: upload_burst.max(0.0),
                    label: Some("upload".into()),
                },
            ],
            exempt_prefixes: vec![
                "/".into(),
                "/health".into(),
                "/ready".into(),
                "/metrics".into(),
                "/monitoring".into(),
            ],
        }
        .with_env_overrides();
        let rate_limit_state_data = web::Data::new(RateLimitSharedState {
            limiter: rl.clone(),
            opts: opts.clone(),
        });

        // Log effective rate limit options for visibility
        info!(
            trust_proxy = opts.trust_proxy,
            search_qps = opts.search_qps,
            search_burst = opts.search_burst,
            upload_qps = opts.upload_qps,
            upload_burst = opts.upload_burst,
            rules = %serde_json::to_string(&opts.rules).unwrap_or_default(),
            exempt_prefixes = %serde_json::to_string(&opts.exempt_prefixes).unwrap_or_default(),
            "Rate limit options initialized"
        );
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST", "DELETE"])
            .allowed_headers(vec![
                actix_web::http::header::CONTENT_TYPE,
                actix_web::http::header::AUTHORIZATION,
            ])
            .max_age(3600);

        App::new()
            .app_data(web::Data::new(api_config.clone()))
            .app_data(rate_limit_state_data.clone())
            .wrap(cors)
            .wrap(crate::trace_middleware::TraceMiddleware::new())
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
                    .route("/ui/requests", web::get().to(get_ui_requests)) // ← Self-contained UI metrics for Requests
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
                    .route("/ollama", web::get().to(get_ollama_status)),
            )
            // ============================================================================
            // ROOT & CORE ROUTES
            // ============================================================================
            .route("/", web::get().to(root_handler))
            .route("/upload", web::post().to(upload_document_inner))
            .route("/documents", web::get().to(list_documents))
            .route("/documents/{filename}", web::delete().to(delete_document))
            .route("/config/chunk_size", web::get().to(get_chunk_config))
            .route("/config/chunk_size", web::post().to(commit_chunk_config))
            .route("/config/embedding", web::get().to(get_embedding_config))
            .route("/config/embedding", web::post().to(set_embedding_config))
            .route("/config/llm", web::get().to(get_llm_config))
            .route("/config/llm", web::post().to(commit_llm_config))
            .route("/config/prompt_caching", web::get().to(get_prompt_caching))
            .route("/config/prompt_caching", web::post().to(set_prompt_caching))
            .route("/config/hardware", web::get().to(get_hardware_config))
            .route("/config/hardware", web::post().to(commit_hardware_config))
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
            .route("/reindex", web::post().to(reindex_handler))
            .route("/reindex/async", web::post().to(reindex_async_handler))
            .route(
                "/reindex/status/{job_id}",
                web::get().to(reindex_status_handler),
            )
            .route("/index/info", web::get().to(index_info_handler))
            .route("/search", web::get().to(search_documents_inner))
            .route("/rerank", web::post().to(rerank))
            .route("/summarize", web::post().to(summarize))
            .route("/save_vectors", web::post().to(save_vectors_handler))
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
                "/monitor/inference_gateway",
                web::get().to(get_inference_gateway_stats),
            )
            .route("/monitor/logs/recent", web::get().to(get_recent_logs))
            // ============================================================================
            // RAG MEMORY ROUTES
            // ============================================================================
            .route("/memory/types", web::get().to(list_memory_types))
            .route("/memory/store_rag", web::post().to(store_rag_memory))
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
        http_server = http_server.workers(1);
    }
    http_server
        .client_request_timeout(std::time::Duration::from_secs(30))
        .bind(bind_addr.clone())
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", bind_addr, e))
        .run()
}

/// GET /monitoring/ollama
/// Returns Ollama service status fetched directly from the Ollama API
async fn get_ollama_status() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    // Check version
    let version_resp = client
        .get("http://localhost:11434/api/version")
        .send()
        .await;

    let available = version_resp
        .as_ref()
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    let version = if let Ok(resp) = version_resp {
        resp.json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v["version"].as_str().map(|s| s.to_string()))
    } else {
        None
    };

    // Get loaded/available models
    let tags_resp = client.get("http://localhost:11434/api/tags").send().await;

    let (loaded_model, model_count) = if let Ok(resp) = tags_resp {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            let models = json["models"].as_array();
            let count = models.map(|m| m.len()).unwrap_or(0);
            let first = models
                .and_then(|m| m.first())
                .and_then(|m| m["name"].as_str())
                .map(|s| s.to_string());
            (first, count)
        } else {
            (None, 0)
        }
    } else {
        (None, 0)
    };

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "request_id": request_id,
        "available": available,
        "version": version,
        "loaded_model": loaded_model,
        "model_count": model_count
    })))
}

/// GET /monitoring/docker/inspect?name=<container>
async fn get_container_inspect(
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let name = match query.get("name") {
        Some(n) => n.clone(),
        None => return Ok(HttpResponse::BadRequest().json(json!({"error": "name is required"}))),
    };

    // docker inspect
    let inspect_out = tokio::process::Command::new("docker")
        .args(["inspect", "--format", "{{json .State}}", &name])
        .env("DOCKER_HOST", "unix:///var/run/docker.sock")
        .output()
        .await;

    let (restart_count, exit_code, started_at, finished_at) = if let Ok(out) = inspect_out {
        let text = String::from_utf8_lossy(&out.stdout);
        let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        (
            json["RestartCount"].as_u64().unwrap_or(0),
            json["ExitCode"].as_i64().unwrap_or(0),
            json["StartedAt"].as_str().unwrap_or("").to_string(),
            json["FinishedAt"].as_str().unwrap_or("").to_string(),
        )
    } else {
        (0, 0, String::new(), String::new())
    };

    // docker logs --tail 20
    let logs_out = tokio::process::Command::new("docker")
        .args(["logs", "--tail", "20", "--timestamps", &name])
        .env("DOCKER_HOST", "unix:///var/run/docker.sock")
        .output()
        .await;

    let logs = if let Ok(out) = logs_out {
        // docker logs writes to stderr by default
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        if stderr.is_empty() {
            stdout
        } else {
            stderr
        }
    } else {
        "Failed to fetch logs".to_string()
    };

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "request_id": request_id,
        "name": name,
        "restart_count": restart_count,
        "exit_code": exit_code,
        "started_at": started_at,
        "finished_at": finished_at,
        "logs": logs
    })))
}