use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use lru::LruCache;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::env;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task;
use tracing::{debug, info, info_span, warn, Instrument};

/// Embedding vector (defaults to 384-dimensional to match BGE-small)
pub type EmbeddingVector = Vec<f32>;

const DEFAULT_EMBEDDING_DIM: usize = 384;

fn hash_embedding(text: &str) -> EmbeddingVector {
    let hash = seahash::hash(text.as_bytes());
    let mut vec = vec![0.0; DEFAULT_EMBEDDING_DIM];
    vec[0] = (hash & 0xFFFF) as f32;
    vec
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingProvider {
    FastEmbed,
    Hash,
}

impl EmbeddingProvider {
    fn from_str(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "hash" => EmbeddingProvider::Hash,
            _ => EmbeddingProvider::FastEmbed,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            EmbeddingProvider::FastEmbed => "fastembed",
            EmbeddingProvider::Hash => "hash",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum EmbeddingModelConfig {
    BgeSmallEnV15,
    BgeSmallEnV15Q,
}

impl EmbeddingModelConfig {
    fn from_env() -> Self {
        match env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "bge-small-en-v1.5".to_string())
            .to_lowercase()
            .as_str()
        {
            "bge-small-en-v1.5q" => EmbeddingModelConfig::BgeSmallEnV15Q,
            "bge-small-en-v1.5" | _ => EmbeddingModelConfig::BgeSmallEnV15,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            EmbeddingModelConfig::BgeSmallEnV15 => "bge-small-en-v1.5",
            EmbeddingModelConfig::BgeSmallEnV15Q => "bge-small-en-v1.5q",
        }
    }

    fn dimension(&self) -> usize {
        DEFAULT_EMBEDDING_DIM
    }

    fn to_fastembed(&self) -> EmbeddingModel {
        match self {
            EmbeddingModelConfig::BgeSmallEnV15 => EmbeddingModel::BGESmallENV15,
            EmbeddingModelConfig::BgeSmallEnV15Q => EmbeddingModel::BGESmallENV15Q,
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

fn default_provider() -> EmbeddingProvider {
    if cfg!(test) {
        EmbeddingProvider::Hash
    } else {
        EmbeddingProvider::FastEmbed
    }
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

        let provider = env::var("EMBEDDING_PROVIDER")
            .map(|v| EmbeddingProvider::from_str(&v))
            .unwrap_or_else(|_| default_provider());

        let model = EmbeddingModelConfig::from_env();

        Self {
            batch_size,
            cache_size,
            provider,
            model,
        }
    }
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            cache_size: 10_000,
            provider: EmbeddingProvider::Hash,
            model: EmbeddingModelConfig::BgeSmallEnV15,
        }
    }
}

type EmbeddingCache = LruCache<String, EmbeddingVector>;

enum EmbeddingBackend {
    FastEmbed { inner: Mutex<TextEmbedding> },
    Hash,
}

struct EmbeddingRuntime {
    backend: EmbeddingBackend,
    dim: usize,
}

impl EmbeddingRuntime {
    fn new(config: &EmbeddingConfig) -> Self {
        info!(
            provider = %config.provider.as_str(),
            model = %config.model.as_str(),
            "Initializing embedding runtime"
        );

        if matches!(config.provider, EmbeddingProvider::FastEmbed) {
            match TextEmbedding::try_new(InitOptions::new(config.model.to_fastembed())) {
                Ok(model) => {
                    info!("fastembed model ready");
                    return Self {
                        backend: EmbeddingBackend::FastEmbed {
                            inner: Mutex::new(model),
                        },
                        dim: config.model.dimension(),
                    };
                }
                Err(err) => {
                    warn!(
                        error = %err,
                        fallback = "hash",
                        "Failed to initialize fastembed runtime; falling back"
                    );
                }
            }
        }

        Self {
            backend: EmbeddingBackend::Hash,
            dim: DEFAULT_EMBEDDING_DIM,
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
            EmbeddingBackend::FastEmbed { inner, .. } => {
                let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
                let mut guard = inner.lock();
                match guard.embed(refs, None) {
                    Ok(vectors) => vectors,
                    Err(err) => {
                        warn!("fastembed batch failed: {err}; using hash fallback");
                        texts.iter().map(|t| hash_embedding(t)).collect()
                    }
                }
            }
            EmbeddingBackend::Hash => texts.iter().map(|t| hash_embedding(t)).collect(),
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
        Arc::new(EmbeddingRuntime::new(&cfg))
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
            "Initializing EmbeddingService"
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
                    return embedding.clone();
                }
            }

            debug!(text_len = text.len(), "Generating embedding (cache miss)");
            crate::monitoring::metrics::record_embedding_cache_miss();

            let start = std::time::Instant::now();
            let runtime = self.runtime.clone();
            let owned = text.to_owned();
            let embedding = match task::spawn_blocking(move || runtime.embed_owned(owned)).await {
                Ok(vec) => vec,
                Err(err) => {
                    warn!("spawn_blocking join error: {err}; using fallback");
                    hash_embedding(text)
                }
            };
            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
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

    /// Embed multiple texts in batches (efficient for bulk operations)
    pub async fn embed_batch(&self, texts: &[&str]) -> Vec<EmbeddingVector> {
        let span = info_span!(
            "embed_batch",
            total_texts = texts.len(),
            batch_size = self.config.batch_size
        );
        async move {
            info!(
                total_texts = texts.len(),
                batch_size = self.config.batch_size,
                "Starting batch embedding"
            );

            let start = std::time::Instant::now();
            let mut results = Vec::new();

            for batch in texts.chunks(self.config.batch_size) {
                for text in batch {
                    results.push(self.embed_text(text).await);
                }
                // Yield to tokio runtime to avoid blocking
                task::yield_now().await;
            }

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

    /// Embed multiple texts with indices (preserves order)
    pub async fn embed_indexed_batch(
        &self,
        texts: &[(usize, &str)],
    ) -> Vec<(usize, EmbeddingVector)> {
        info!(
            total_texts = texts.len(),
            batch_size = self.config.batch_size,
            "Starting indexed batch embedding"
        );

        let mut results = Vec::new();

        for batch in texts.chunks(self.config.batch_size) {
            for (idx, text) in batch {
                let embedding = self.embed_text(text).await;
                results.push((*idx, embedding));
            }
            task::yield_now().await;
        }

        info!(
            total_embeddings = results.len(),
            "Indexed batch embedding completed"
        );
        results
    }

    /// Clear the embedding cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        info!("Embedding cache cleared");
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        CacheStats {
            len: cache.len(),
            cap: cache.cap().get(),
        }
    }

    /// Embed a query string (for semantic search)
    pub async fn embed_query(&self, query: &str) -> EmbeddingVector {
        debug!(query = %query, "Generating query embedding");
        self.embed_text(query).await
    }
}

/// Statistics for the embedding cache
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub len: usize,
    pub cap: usize,
}

/// Similarity search helper functions
pub mod similarity {
    use super::EmbeddingVector;

    /// Cosine similarity between two vectors
    pub fn cosine_similarity(a: &EmbeddingVector, b: &EmbeddingVector) -> f32 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if mag_a == 0.0 || mag_b == 0.0 {
            0.0
        } else {
            dot / (mag_a * mag_b)
        }
    }

    /// Euclidean distance between two vectors
    pub fn euclidean_distance(a: &EmbeddingVector, b: &EmbeddingVector) -> f32 {
        if a.is_empty() || b.is_empty() {
            return f32::INFINITY;
        }

        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// Find top-k most similar embeddings
    pub fn top_k_similar(
        query_embedding: &EmbeddingVector,
        candidates: &[(usize, &EmbeddingVector)],
        k: usize,
    ) -> Vec<(usize, f32)> {
        let mut scores: Vec<_> = candidates
            .iter()
            .map(|(idx, emb)| (*idx, cosine_similarity(query_embedding, emb)))
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(k);
        scores
    }
}

/// Convenience helper for synchronous contexts (chunking/indexing)
pub fn embed(text: &str) -> EmbeddingVector {
    global_runtime().embed_owned(text.to_owned())
}

/// Batch helper for synchronous indexers
pub fn embed_batch(texts: &[String]) -> Vec<EmbeddingVector> {
    if texts.is_empty() {
        return Vec::new();
    }
    let runtime = global_runtime();
    runtime.embed_batch_owned(texts.iter().map(|s| s.to_string()).collect())
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

    #[tokio::test]
    async fn test_embedding_service_creation() {
        let config = EmbeddingConfig::default();
        let service = EmbeddingService::new(config);

        let stats = service.cache_stats().await;
        assert_eq!(stats.len, 0);
    }

    #[tokio::test]
    async fn test_embed_text() {
        let service = EmbeddingService::new(EmbeddingConfig::default());

        let embedding = service.embed_text("test query").await;
        assert_eq!(embedding.len(), DEFAULT_EMBEDDING_DIM);
    }

    #[tokio::test]
    async fn test_embedding_cache_hit() {
        let service = EmbeddingService::new(EmbeddingConfig::default());

        // First call
        let result1 = service.embed_text("test").await;

        // Second call (should hit cache)
        let result2 = service.embed_text("test").await;

        assert_eq!(result1, result2);

        let stats = service.cache_stats().await;
        assert_eq!(stats.len, 1);
    }

    #[tokio::test]
    async fn test_batch_embedding() {
        let service = EmbeddingService::new(EmbeddingConfig {
            batch_size: 2,
            ..Default::default()
        });

        let texts = vec!["text 1", "text 2", "text 3"];
        let results = service.embed_batch(&texts).await;

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|v| v.len() == DEFAULT_EMBEDDING_DIM));
    }

    #[tokio::test]
    async fn test_indexed_batch() {
        let service = EmbeddingService::new(EmbeddingConfig::default());

        let texts = vec![(0usize, "text 1"), (1usize, "text 2"), (2usize, "text 3")];
        let results = service.embed_indexed_batch(&texts).await;

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, 0);
        assert_eq!(results[1].0, 1);
        assert_eq!(results[2].0, 2);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let service = EmbeddingService::new(EmbeddingConfig::default());

        let _ = service.embed_text("test").await;

        let stats = service.cache_stats().await;
        assert!(stats.len > 0);

        service.clear_cache().await;

        let stats = service.cache_stats().await;
        assert_eq!(stats.len, 0);
    }

    #[test]
    fn test_cosine_similarity() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let v3 = vec![0.0, 1.0, 0.0];

        assert!((similarity::cosine_similarity(&v1, &v2) - 1.0).abs() < 0.001);
        assert!((similarity::cosine_similarity(&v1, &v3)).abs() < 0.001);
    }

    #[test]
    fn test_top_k_similar() {
        let query = vec![1.0, 0.0];
        let v1 = vec![1.0, 0.0];
        let v2 = vec![0.0, 1.0];
        let v3 = vec![0.9, 0.1];

        let candidates = vec![(0, &v1), (1, &v2), (2, &v3)];

        let results = similarity::top_k_similar(&query, &candidates, 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0); // Most similar
    }

    #[tokio::test]
    async fn test_embed_query() {
        let service = EmbeddingService::new(EmbeddingConfig::default());
        let embedding = service.embed_query("test query").await;
        assert_eq!(embedding.len(), DEFAULT_EMBEDDING_DIM);
    }
}
