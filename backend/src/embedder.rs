// src/embedder.rs - ONNX-only embedding support
// Uses ONNX Runtime for fast, optimized embeddings

use lru::LruCache;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::env;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task;
use tracing::{debug, info, info_span, warn, Instrument};

static UPLOAD_BLOCKING_RT: std::sync::OnceLock<tokio::runtime::Handle> = std::sync::OnceLock::new();

pub fn upload_pool_ready() -> bool {
    UPLOAD_BLOCKING_RT.get().is_some()
}

pub fn init_upload_blocking_pool(max_threads: usize) {
    UPLOAD_BLOCKING_RT.get_or_init(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(max_threads)
            .thread_name("upload-onnx")
            .build()
            .expect("Failed to build upload ONNX blocking pool");
        let handle = rt.handle().clone();
        Box::leak(Box::new(rt));
        handle
    });
}

/// Embedding vector (defaults to 384-dimensional to match BGE-small)
pub type EmbeddingVector = Vec<f32>;

const DEFAULT_EMBEDDING_DIM: usize = 384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingProvider {
    /// ONNX Runtime - the only supported provider (2-10x faster)
    Onnx,
}

/// Supported ONNX embedding models.
///
/// Set `EMBEDDING_MODEL` in `.env` to select a model. The ONNX graph file
/// must be placed at `ONNX_MODEL_PATH` (default: `models/embedding_model.onnx`)
/// and a `tokenizer.json` file must exist in the same directory. Both files
/// can be downloaded from the model's HuggingFace page.
#[derive(Debug, Clone, Copy)]
pub enum EmbeddingModelConfig {
    /// BAAI/bge-small-en-v1.5 — 384 dims, 33 MB. Default. Good balance of
    /// speed and quality for English text.
    BgeSmallEnV15,
    /// BAAI/bge-small-en-v1.5 (INT8 quantized) — 384 dims, ~8 MB. ~30% faster
    /// than the full model with minor quality trade-off.
    BgeSmallEnV15Q,
    /// sentence-transformers/all-MiniLM-L6-v2 — 384 dims, 22 MB. Excellent
    /// general-purpose model; slightly faster than BGE-small.
    AllMiniLML6V2,
    /// BAAI/bge-base-en-v1.5 — 768 dims, 109 MB. Meaningfully better retrieval
    /// quality than the small variants; requires re-indexing when switching.
    BgeBaseEnV15,
    /// intfloat/e5-small-v2 — 384 dims, 33 MB. Instruction-following model;
    /// prefix queries with "query: " and passages with "passage: " for best results.
    E5SmallV2,
}

impl EmbeddingModelConfig {
    pub fn from_env() -> Self {
        match env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "bge-small-en-v1.5".to_string())
            .to_lowercase()
            .as_str()
        {
            "bge-small-en-v1.5q" => EmbeddingModelConfig::BgeSmallEnV15Q,
            "all-minilm-l6-v2" => EmbeddingModelConfig::AllMiniLML6V2,
            "bge-base-en-v1.5" => EmbeddingModelConfig::BgeBaseEnV15,
            "e5-small-v2" => EmbeddingModelConfig::E5SmallV2,
            _ => EmbeddingModelConfig::BgeSmallEnV15,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            EmbeddingModelConfig::BgeSmallEnV15 => "bge-small-en-v1.5",
            EmbeddingModelConfig::BgeSmallEnV15Q => "bge-small-en-v1.5q",
            EmbeddingModelConfig::AllMiniLML6V2 => "all-minilm-l6-v2",
            EmbeddingModelConfig::BgeBaseEnV15 => "bge-base-en-v1.5",
            EmbeddingModelConfig::E5SmallV2 => "e5-small-v2",
        }
    }

    pub fn dimension(&self) -> usize {
        match self {
            EmbeddingModelConfig::BgeBaseEnV15 => 768,
            _ => DEFAULT_EMBEDDING_DIM,
        }
    }

    /// HuggingFace model ID — used in log messages and error hints.
    pub fn huggingface_id(&self) -> &'static str {
        match self {
            EmbeddingModelConfig::BgeSmallEnV15 | EmbeddingModelConfig::BgeSmallEnV15Q => {
                "BAAI/bge-small-en-v1.5"
            }
            EmbeddingModelConfig::AllMiniLML6V2 => "sentence-transformers/all-MiniLM-L6-v2",
            EmbeddingModelConfig::BgeBaseEnV15 => "BAAI/bge-base-en-v1.5",
            EmbeddingModelConfig::E5SmallV2 => "intfloat/e5-small-v2",
        }
    }
}

/// Configuration for the embedding service
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub batch_size: usize,
    pub cache_size: usize,
    pub provider: EmbeddingProvider,
    pub model: EmbeddingModelConfig,
}

impl EmbeddingConfig {
    pub fn from_env() -> Self {
        let batch_size = env::var("EMBEDDING_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(32);

        let cache_size = env::var("EMBEDDING_CACHE_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000);

        let model = EmbeddingModelConfig::from_env();

        Self {
            batch_size,
            cache_size,
            provider: EmbeddingProvider::Onnx,
            model,
        }
    }
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            cache_size: 10_000,
            provider: EmbeddingProvider::Onnx,
            model: EmbeddingModelConfig::BgeSmallEnV15,
        }
    }
}

type EmbeddingCache = LruCache<String, EmbeddingVector>;

enum EmbeddingBackend {
    /// ONNX Runtime - the only backend
    Onnx {
        inner: Mutex<crate::perf::onnx_embedder::OnnxEmbedder>,
    },
}

struct EmbeddingRuntime {
    backend: EmbeddingBackend,
    dim: usize,
    batch_size: AtomicUsize,
}

impl EmbeddingRuntime {
    fn new(config: &EmbeddingConfig) -> Self {
        eprintln!("[EMBEDDER] Starting ONNX embedding runtime initialization...");
        info!(
            model = %config.model.as_str(),
            hf_id = %config.model.huggingface_id(),
            dims = config.model.dimension(),
            "Initializing ONNX embedding runtime"
        );

        let onnx_model_path = env::var("ONNX_MODEL_PATH")
            .unwrap_or_else(|_| "models/embedding_model.onnx".to_string());

        eprintln!("[EMBEDDER] ONNX model path: {}", onnx_model_path);
        eprintln!(
            "[EMBEDDER] Model exists: {}",
            std::path::Path::new(&onnx_model_path).exists()
        );

        let onnx_config = crate::perf::onnx_embedder::OnnxConfig {
            model_path: onnx_model_path.clone(),
            embedding_dim: config.model.dimension(),
            ..Default::default()
        };

        eprintln!("[EMBEDDER] Creating OnnxEmbedder...");
        match crate::perf::onnx_embedder::OnnxEmbedder::new(onnx_config) {
            Ok(embedder) => {
                info!(
                    model_path = %onnx_model_path,
                    model = %config.model.as_str(),
                    dims = config.model.dimension(),
                    "ONNX embedder ready"
                );
                Self {
                    backend: EmbeddingBackend::Onnx {
                        inner: Mutex::new(embedder),
                    },
                    dim: config.model.dimension(),
                    batch_size: AtomicUsize::new(config.batch_size),
                }
            }
            Err(err) => {
                panic!(
                    "Failed to initialize ONNX embedder: {}. Make sure ONNX model exists at {}",
                    err, onnx_model_path
                );
            }
        }
    }

    fn embed_batch_owned(&self, texts: Vec<String>) -> Vec<EmbeddingVector> {
        let batch_size = texts.len();
        if batch_size == 0 {
            return Vec::new();
        }

        let start = std::time::Instant::now();
        crate::monitoring::metrics::observe_embedding_batch_size(batch_size);

        let result = match &self.backend {
            EmbeddingBackend::Onnx { inner } => {
                let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
                let mut guard = inner.lock();
                match guard.embed(&refs) {
                    Ok(vectors) => vectors,
                    Err(err) => {
                        warn!("ONNX batch failed: {err}");
                        // Return zero vectors on error
                        texts.iter().map(|_| vec![0.0; self.dim]).collect()
                    }
                }
            }
        };

        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
        crate::monitoring::metrics::observe_embedding_latency_ms(duration_ms);
        crate::monitoring::metrics::record_embedding_generated(batch_size as u64);

        debug!(
            batch_size = batch_size,
            duration_ms = duration_ms,
            "Embedding batch completed"
        );

        result
    }

    fn embed_owned(&self, text: String) -> EmbeddingVector {
        self.embed_batch_owned(vec![text])
            .into_iter()
            .next()
            .unwrap_or_else(|| vec![0.0; self.dim])
    }
}

fn global_runtime() -> &'static Arc<EmbeddingRuntime> {
    static GLOBAL: OnceCell<Arc<EmbeddingRuntime>> = OnceCell::new();
    GLOBAL.get_or_init(|| {
        let cfg = EmbeddingConfig::from_env();
        let rt = Arc::new(EmbeddingRuntime::new(&cfg));
        crate::monitoring::onnx_metrics::register_model(
            cfg.model.as_str(),
            cfg.model.dimension(),
            cfg.batch_size,
        );
        rt
    })
}

/// Thread-safe async embedding service with caching
pub struct EmbeddingService {
    config: EmbeddingConfig,
    cache: Arc<RwLock<EmbeddingCache>>,
    runtime: Arc<EmbeddingRuntime>,
}

impl EmbeddingService {
    /// Create a new embedding service
    pub fn new(config: EmbeddingConfig) -> Self {
        let cache_size = NonZeroUsize::new(config.cache_size).expect("cache_size must be > 0");
        let cache = LruCache::new(cache_size);

        info!(
            batch_size = config.batch_size,
            cache_size = config.cache_size,
            "EmbeddingService initialized"
        );

        Self {
            config,
            cache: Arc::new(RwLock::new(cache)),
            runtime: global_runtime().clone(),
        }
    }

    /// Embed a single text, with cache lookup
    pub async fn embed_text(&self, text: &str) -> EmbeddingVector {
        let span = info_span!("embed_text", text_len = text.len());
        async move {
            let key = format!("{:x}", seahash::hash(text.as_bytes()));

            {
                let mut cache = self.cache.write().await;
                if let Some(embedding) = cache.get(&key) {
                    debug!(cache_key = %key, "Embedding cache hit");
                    crate::monitoring::metrics::record_embedding_cache_hit();
                    crate::monitoring::onnx_metrics::record_cache_hit();
                    return embedding.clone();
                }
            }

            debug!(text_len = text.len(), "Generating embedding (cache miss)");
            crate::monitoring::metrics::record_embedding_cache_miss();
            crate::monitoring::onnx_metrics::record_cache_miss();

            let start = std::time::Instant::now();
            let runtime = self.runtime.clone();
            let owned = text.to_owned();
            let dim = self.runtime.dim;
            let embedding = match task::spawn_blocking(move || runtime.embed_owned(owned)).await {
                Ok(vec) => vec,
                Err(err) => {
                    warn!("spawn_blocking join error: {err}; returning zero vector");
                    vec![0.0; dim]
                }
            };
            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
            crate::monitoring::onnx_metrics::record_single_embed(duration_ms);
            crate::monitoring::metrics::observe_embedding_latency_ms(duration_ms);
            debug!(duration_ms = duration_ms, "Single embedding generated");

            {
                let mut cache = self.cache.write().await;
                cache.put(key.clone(), embedding.clone());
            }

            embedding
        }
        .instrument(span)
        .await
    }

    /// Embed multiple texts in a single ONNX call via one spawn_blocking round-trip.
    pub async fn embed_batch(&self, texts: &[&str]) -> Vec<EmbeddingVector> {
        let span = info_span!(
            "embed_batch",
            total_texts = texts.len(),
            batch_size = self.config.batch_size
        );
        async move {
            if texts.is_empty() {
                return Vec::new();
            }

            info!(
                total_texts = texts.len(),
                batch_size = self.config.batch_size,
                "Starting batch embedding"
            );

            crate::monitoring::onnx_metrics::record_batch(texts.len());
            crate::monitoring::metrics::observe_embedding_batch_size(texts.len());
            let start = std::time::Instant::now();

            let owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
            let runtime = self.runtime.clone();

            let embedding_future = if let Some(handle) = UPLOAD_BLOCKING_RT.get() {
                handle.spawn_blocking(move || runtime.embed_batch_owned(owned))
            } else {
                tokio::task::spawn_blocking(move || runtime.embed_batch_owned(owned))
            };
            let results = embedding_future.await.unwrap_or_else(|e| {
                warn!("embed_batch join error: {e}");
                vec![]
            });

            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
            info!(
                total_embeddings = results.len(),
                duration_ms = duration_ms,
                "Batch embedding completed"
            );
            results
        }
        .instrument(span)
        .await
    }

    /// Embed multiple texts with indices (preserves order) via one spawn_blocking round-trip.
    pub async fn embed_batch_indexed(
        &self,
        texts: &[(usize, &str)],
    ) -> Vec<(usize, EmbeddingVector)> {
        if texts.is_empty() {
            return Vec::new();
        }

        info!(
            total_texts = texts.len(),
            batch_size = self.config.batch_size,
            "Starting indexed batch embedding"
        );

        let indices: Vec<usize> = texts.iter().map(|(idx, _)| *idx).collect();
        let owned: Vec<String> = texts.iter().map(|(_, s)| s.to_string()).collect();
        let runtime = self.runtime.clone();

        let embedding_future = if let Some(handle) = UPLOAD_BLOCKING_RT.get() {
            handle.spawn_blocking(move || runtime.embed_batch_owned(owned))
        } else {
            tokio::task::spawn_blocking(move || runtime.embed_batch_owned(owned))
        };
        let vectors = embedding_future.await.unwrap_or_else(|e| {
            warn!("embed_batch_indexed join error: {e}");
            vec![]
        });

        let results: Vec<(usize, EmbeddingVector)> = indices.into_iter().zip(vectors).collect();

        info!(
            total_embeddings = results.len(),
            "Indexed batch embedding completed"
        );
        results
    }

    /// Embed a query (same as embed_text, but semantically different)
    pub async fn embed_query(&self, query: &str) -> EmbeddingVector {
        self.embed_text(query).await
    }

    /// Get embedding dimension (matches the loaded model's output size).
    pub fn dimension(&self) -> usize {
        self.runtime.dim
    }

    // ========================================================================
    // Cache Persistence (rkyv-based)
    // ========================================================================

    /// Save embedding cache to disk using rkyv binary format
    pub async fn save_cache(&self, path: &std::path::Path) -> Result<(), String> {
        let cache = self.cache.read().await;

        // Convert LruCache to Vec for serialization
        let entries: Vec<(String, Vec<f32>)> =
            cache.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entries)
            .map_err(|e| format!("rkyv serialize error: {}", e))?;

        std::fs::write(path, &bytes).map_err(|e| format!("IO error: {}", e))?;

        info!(
            path = ?path,
            entries = entries.len(),
            bytes = bytes.len(),
            "Embedding cache saved"
        );
        Ok(())
    }

    /// Load embedding cache from disk using rkyv binary format
    pub async fn load_cache(&self, path: &std::path::Path) -> Result<usize, String> {
        if !path.exists() {
            return Ok(0);
        }

        let bytes = std::fs::read(path).map_err(|e| format!("IO error: {}", e))?;

        let archived =
            rkyv::access::<rkyv::Archived<Vec<(String, Vec<f32>)>>, rkyv::rancor::Error>(&bytes)
                .map_err(|e| format!("rkyv access error: {}", e))?;

        let mut cache = self.cache.write().await;
        let mut loaded = 0;

        for entry in archived.iter() {
            let key = entry.0.to_string();
            let value: Vec<f32> = entry.1.iter().map(|f| f.to_native()).collect();
            cache.put(key, value);
            loaded += 1;
        }

        info!(
            path = ?path,
            entries = loaded,
            bytes = bytes.len(),
            "Embedding cache loaded"
        );
        Ok(loaded)
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        (cache.len(), cache.cap().get())
    }
}

/// Similarity functions for embeddings
pub mod similarity {
    use super::EmbeddingVector;

    /// Cosine similarity between two vectors
    pub fn cosine(a: &EmbeddingVector, b: &EmbeddingVector) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Euclidean distance between two vectors
    pub fn euclidean_distance(a: &EmbeddingVector, b: &EmbeddingVector) -> f32 {
        if a.len() != b.len() {
            return f32::MAX;
        }

        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// Dot product between two vectors
    pub fn dot_product(a: &EmbeddingVector, b: &EmbeddingVector) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// Normalize a vector to unit length
    pub fn normalize(v: &mut EmbeddingVector) {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
    }
}

/// Convenience helper for synchronous contexts (chunking/indexing)
pub fn embed(text: &str) -> EmbeddingVector {
    let start = std::time::Instant::now();
    crate::monitoring::onnx_metrics::record_cache_miss();
    let result = global_runtime().embed_owned(text.to_owned());
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    crate::monitoring::onnx_metrics::record_single_embed(duration_ms);
    crate::monitoring::metrics::observe_embedding_latency_ms(duration_ms);
    result
}

/// Batch helper for synchronous indexers.
/// Processes chunks in sub-batches of `EMBEDDING_BATCH_SIZE` (default 32) to bound
/// ONNX attention memory: a single pass of N×512 tokens allocates O(N×heads×512²) RAM,
/// which OOMs for N in the hundreds.
pub fn embed_batch(texts: &[String]) -> Vec<EmbeddingVector> {
    if texts.is_empty() {
        return Vec::new();
    }
    crate::monitoring::metrics::observe_embedding_batch_size(texts.len());
    let start = std::time::Instant::now();
    let runtime = global_runtime();
    let batch_size = runtime.batch_size.load(Ordering::Relaxed).max(1);
    let mut result = Vec::with_capacity(texts.len());
    match &runtime.backend {
        EmbeddingBackend::Onnx { inner } => {
            for chunk in texts.chunks(batch_size) {
                let refs: Vec<&str> = chunk.iter().map(String::as_str).collect();
                let mut guard = inner.lock();
                match guard.embed(&refs) {
                    Ok(mut vectors) => result.append(&mut vectors),
                    Err(err) => {
                        warn!("ONNX batch embed failed: {err}");
                        result.extend(chunk.iter().map(|_| vec![0.0; runtime.dim]));
                    }
                }
            }
        }
    }
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    crate::monitoring::metrics::observe_embedding_latency_ms(duration_ms);
    result
}
/// Live-update the embedding batch size without restarting.
pub fn set_embedding_batch_size(size: usize) {
    global_runtime()
        .batch_size
        .store(size.max(1), Ordering::Relaxed);
}

/// Read the current live embedding batch size.
pub fn get_embedding_batch_size() -> usize {
    global_runtime().batch_size.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_basic() {
        let vec = embed("hello world");
        assert_eq!(vec.len(), DEFAULT_EMBEDDING_DIM);
        assert!(vec.iter().any(|&x| x != 0.0));
    }

    #[test]
    fn test_embed_deterministic() {
        let vec1 = embed("test query");
        let vec2 = embed("test query");
        assert_eq!(vec1, vec2);
    }

    #[tokio::test]
    async fn test_embedding_service() {
        let service = EmbeddingService::new(EmbeddingConfig::default());

        let embedding = service.embed_text("test query").await;
        assert_eq!(embedding.len(), DEFAULT_EMBEDDING_DIM);
    }

    /// Integration test for FastEmbed - verifies neural embeddings work.
    /// Run with: cargo test --lib test_fastembed_integration -- --ignored --nocapture
    ///
    /// This test is ignored by default because:
    /// - It downloads the model on first run (~100MB)
    /// - It requires ONNX runtime
    /// - It's slower than hash-based tests
    #[tokio::test]
    #[ignore]
    #[cfg(not(target_os = "windows"))]
    async fn test_fastembed_integration() {
        use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

        println!("\n=== FASTEMBED INTEGRATION TEST ===");
        println!("Attempting to initialize FastEmbed directly...");

        // Try to initialize FastEmbed directly to see the error
        match TextEmbedding::try_new(InitOptions::new(EmbeddingModel::BGESmallENV15)) {
            Ok(mut model) => {
                println!("FastEmbed initialized successfully!");
                let texts = vec!["Hello world, this is a test of neural embeddings."];
                match model.embed(texts, None) {
                    Ok(embeddings) => {
                        let embedding = &embeddings[0];
                        let non_zero_count = embedding.iter().filter(|&&x| x != 0.0).count();
                        let first_10: Vec<f32> = embedding.iter().take(10).copied().collect();

                        println!("Dimension: {}", embedding.len());
                        println!("Non-zero values: {} / {}", non_zero_count, embedding.len());
                        println!("First 10 values: {:?}", first_10);
                        println!("=================================\n");

                        assert!(non_zero_count > 300,
                            "Expected dense neural embeddings (>300 non-zero out of 384), got {} non-zero values.", 
                            non_zero_count);
                    }
                    Err(e) => {
                        panic!("FastEmbed embed() failed: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("FastEmbed initialization FAILED: {}", e);
                println!("=================================\n");
                panic!("FastEmbed failed to initialize: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_embedding_cache() {
        let service = EmbeddingService::new(EmbeddingConfig::default());

        // First call - cache miss
        let embedding1 = service.embed_text("cached query").await;

        // Second call - should be cache hit
        let embedding2 = service.embed_text("cached query").await;

        assert_eq!(embedding1, embedding2);
    }

    #[tokio::test]
    async fn test_batch_embedding() {
        let service = EmbeddingService::new(EmbeddingConfig::default());
        let texts = vec!["text1", "text2", "text3"];

        let results = service.embed_batch(&texts).await;

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|v| v.len() == DEFAULT_EMBEDDING_DIM));
    }

    #[tokio::test]
    async fn test_embed_query() {
        let service = EmbeddingService::new(EmbeddingConfig::default());
        let embedding = service.embed_query("test query").await;
        assert_eq!(embedding.len(), DEFAULT_EMBEDDING_DIM);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((similarity::cosine(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((similarity::cosine(&a, &c)).abs() < 0.001);
    }

    #[test]
    fn test_normalize() {
        let mut v = vec![3.0, 4.0];
        similarity::normalize(&mut v);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }
}
