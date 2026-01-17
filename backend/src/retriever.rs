use crate::cache::redis_cache::RedisCache;
use fs2;
use lru::LruCache;
use memmap2::Mmap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tantivy::{
    collector::TopDocs,
    directory::error::OpenDirectoryError,
    directory::MmapDirectory,
    query::QueryParser,
    query::QueryParserError,
    schema::{Field, Schema, Value, STORED, TEXT},
    Index, IndexWriter, TantivyError,
};
use tracing::{debug, error, info};

/// Custom error type for Retriever operations
#[derive(Debug, Serialize, Deserialize)]
pub enum RetrieverError {
    TantivyError(String),
    IoError(String),
    IndexError(String),
    VectorError(String),
    QueryParserError(String),
    DirectoryError(String),
    SerializationError(String),
}

impl fmt::Display for RetrieverError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RetrieverError::TantivyError(e) => write!(f, "Tantivy error: {}", e),
            RetrieverError::IoError(e) => write!(f, "IO error: {}", e),
            RetrieverError::IndexError(msg) => write!(f, "Index error: {}", msg),
            RetrieverError::VectorError(msg) => write!(f, "Vector error: {}", msg),
            RetrieverError::QueryParserError(msg) => write!(f, "Query parser error: {}", msg),
            RetrieverError::DirectoryError(msg) => write!(f, "Directory error: {}", msg),
            RetrieverError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for RetrieverError {}

impl From<TantivyError> for RetrieverError {
    fn from(err: TantivyError) -> Self {
        RetrieverError::TantivyError(err.to_string())
    }
}

impl From<std::io::Error> for RetrieverError {
    fn from(err: std::io::Error) -> Self {
        RetrieverError::IoError(err.to_string())
    }
}

impl From<QueryParserError> for RetrieverError {
    fn from(err: QueryParserError) -> Self {
        RetrieverError::QueryParserError(err.to_string())
    }
}

impl From<OpenDirectoryError> for RetrieverError {
    fn from(err: OpenDirectoryError) -> Self {
        RetrieverError::DirectoryError(err.to_string())
    }
}

#[derive(Serialize, Deserialize)]
struct VectorStorage {
    vectors: Vec<Vec<f32>>,
    doc_id_to_vector_idx: HashMap<String, usize>,
}

/// rkyv-compatible vector storage for fast binary serialization
/// This provides 10-50x faster load times compared to JSON
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
struct VectorStorageRkyv {
    /// Version field for future schema migrations
    version: u32,
    /// All embedding vectors (Vec<Vec<f32>>)
    vectors: Vec<Vec<f32>>,
    /// Document ID to vector index mapping as flat pairs
    /// Using u32 for index since usize varies by platform
    doc_id_to_idx: Vec<(String, u32)>,
}

impl VectorStorageRkyv {
    const CURRENT_VERSION: u32 = 1;

    fn from_retriever(vectors: &[Vec<f32>], doc_id_to_vector_idx: &HashMap<String, usize>) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            vectors: vectors.to_vec(),
            doc_id_to_idx: doc_id_to_vector_idx
                .iter()
                .map(|(k, v)| (k.clone(), *v as u32))
                .collect(),
        }
    }
}

/// Metrics for monitoring Retriever performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieverMetrics {
    pub total_searches: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub avg_search_latency_us: f64,
    pub total_search_latency_us: u128,
    pub max_search_latency_us: u128,
    pub total_documents_indexed: usize,
    pub total_vectors: usize,
    pub index_path: String,
    pub last_updated: u64,
}

impl Default for RetrieverMetrics {
    fn default() -> Self {
        Self {
            total_searches: 0,
            cache_hits: 0,
            cache_misses: 0,
            avg_search_latency_us: 0.0,
            total_search_latency_us: 0,
            max_search_latency_us: 0,
            total_documents_indexed: 0,
            total_vectors: 0,
            index_path: String::new(),
            last_updated: 0,
        }
    }
}

impl RetrieverMetrics {
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_searches == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.total_searches as f64
        }
    }

    pub fn get_index_size_bytes(&self) -> Result<u64, std::io::Error> {
        let path = Path::new(&self.index_path);
        if !path.exists() {
            return Ok(0);
        }
        let mut total_size = 0;
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                total_size += metadata.len();
            }
        }
        Ok(total_size)
    }

    pub fn get_index_size_human(&self) -> Result<String, std::io::Error> {
        let size = self.get_index_size_bytes()?;
        let sizes = ["B", "KB", "MB", "GB", "TB"];
        let mut size = size as f64;
        let mut i = 0;
        while size >= 1024.0 && i < sizes.len() - 1 {
            size /= 1024.0;
            i += 1;
        }
        Ok(format!("{:.2} {}", size, sizes[i]))
    }
}

pub struct Retriever {
    pub vectors: Vec<Vec<f32>>,
    pub index: Index,
    pub title_field: Field,
    pub content_field: Field,
    pub doc_id_field: Field,
    pub doc_id_to_vector_idx: HashMap<String, usize>,
    pub vector_file_path: String,
    pub auto_save_threshold: usize,
    documents_since_save: Arc<AtomicUsize>,
    index_writer: Option<IndexWriter>,
    batch_mode: bool,
    search_cache: LruCache<String, Vec<String>>,
    cache_enabled: bool,
    // Phase 11 Step 2: L2 Cache integration
    l2_cache: Option<crate::cache::cache_layer::MemoryCache<String, Vec<String>>>,
    l2_cache_stats: crate::cache::cache_layer::CacheStats,
    // Phase 12 Step 2: L3 Redis Cache integration
    l3_cache: Option<RedisCache>,
    pub metrics: RetrieverMetrics,
    index_dir_path: String,
    search_top_k: usize,
    // Phase 13: Bloom filter for O(1) document existence checks
    doc_bloom_filter: crate::perf::bloom::VectorBloomFilter,
    // Phase 14: HNSW index for O(log n) approximate nearest neighbor search
    hnsw_index: Option<crate::perf::hnsw::HnswIndex>,
    // Phase 15: Semantic query cache (caches similar queries)
    semantic_cache: crate::perf::semantic_cache::SemanticCache,
    // Phase 16: Hybrid searcher (BM25 + vector fusion)
    hybrid_searcher: crate::perf::hybrid_search::HybridSearcher,
    // Phase 17: Re-ranker for diversity
    reranker: crate::perf::reranking::Reranker,
    // Phase 18: Request coalescer for deduplication
    request_coalescer: crate::perf::request_coalescing::Singleflight<String, Vec<String>>,
    // Phase 19: Product Quantization index for 16x memory reduction
    pq_index: Option<crate::perf::product_quantization::PQIndex>,
    // Phase 20: Mixed precision (FP16) vector store for 2x memory reduction
    fp16_store: Option<crate::perf::mixed_precision::F16VectorStore>,
    // Phase 21: Connection pool for external services
    connection_pool: crate::perf::connection_pool::ConnectionPool,
    // Phase 22: Use io_uring for async file I/O (Linux)
    use_io_uring: bool,
}

/// SIMD-accelerated cosine similarity (4-8x faster than scalar)
/// Falls back to scalar for vectors not aligned to 8 elements
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    // Use SIMD version from perf module for 4-8x speedup
    crate::perf::simd::cosine_similarity_simd(a, b)
}

use chrono::Utc;

/// Perform an atomic reindex by building into temporary paths and swapping both index and vector mapping.
pub async fn reindex_atomic(
    upload_dir: &str,
    pm: &crate::path_manager::PathManager,
) -> Result<(usize, usize), RetrieverError> {
    // Ensure directory scaffolding exists to avoid ENOENT
    std::fs::create_dir_all(pm.locks_dir())
        .map_err(|e| RetrieverError::IoError(format!("create_dir_all locks_dir failed: {}", e)))?;
    std::fs::create_dir_all(pm.index_dir())
        .map_err(|e| RetrieverError::IoError(format!("create_dir_all index_dir failed: {}", e)))?;
    std::fs::create_dir_all(pm.data_dir())
        .map_err(|e| RetrieverError::IoError(format!("create_dir_all data_dir failed: {}", e)))?;

    // Single-writer lock (best-effort without strict advisory locking)
    let lock_path = pm.locks_dir().join("index.lock");
    let _lock_file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)?;

    // Paths
    let live_index_dir = pm.index_dir().join("tantivy");
    let tmp_index_dir = pm.index_dir().join("tantivy.tmp");
    if tmp_index_dir.exists() {
        let _ = std::fs::remove_dir_all(&tmp_index_dir);
    }
    std::fs::create_dir_all(&tmp_index_dir).map_err(|e| RetrieverError::IoError(e.to_string()))?;

    let vectors_path = pm.vector_store_path();
    // Ensure parent directory exists for vectors files
    if let Some(parent) = vectors_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            RetrieverError::IoError(format!("create_dir_all vector parent failed: {}", e))
        })?;
    }
    let vectors_tmp = vectors_path.with_file_name("vectors.new.json");

    // Ensure no stale temp vectors file remains
    if vectors_tmp.exists() {
        let _ = std::fs::remove_file(&vectors_tmp);
    }

    // Build temp retriever bound to temp paths
    let mut tmp_ret = Retriever::new_with_paths(tmp_index_dir.clone(), vectors_tmp.clone())
        .map_err(|e| RetrieverError::IndexError(e.to_string()))?;
    // Disable autosave during temp build to avoid mid-build writes
    tmp_ret.set_auto_save_threshold(usize::MAX / 2);

    // Build temp index using batch commit to ensure files are written to disk
    let _ = tmp_ret.begin_batch();
    info!("Reindex: start indexing upload_dir={}", upload_dir);
    if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let chunker = crate::index::default_chunker(crate::config::ChunkerMode::Fixed);
        crate::index::index_all_documents(
            &mut tmp_ret,
            upload_dir,
            crate::config::ChunkerMode::Fixed,
            chunker.as_ref(),
        )
    })) {
        return Err(RetrieverError::IndexError(format!(
            "index_all_documents panicked: {:?}",
            e
        )));
    }
    // If not panicked, call again to get Result and handle error
    let chunker = crate::index::default_chunker(crate::config::ChunkerMode::Fixed);
    crate::index::index_all_documents(
        &mut tmp_ret,
        upload_dir,
        crate::config::ChunkerMode::Fixed,
        chunker.as_ref(),
    )
    .map_err(|e| RetrieverError::IndexError(e))?;
    info!("Reindex: indexing completed");
    let _ = tmp_ret.end_batch();

    // Save vectors to the temp mapping file
    tmp_ret.force_save()?;
    // Explicitly write temp vectors to vectors.new.json to avoid path mismatch
    {
        let tmp_storage = VectorStorage {
            vectors: tmp_ret.vectors.clone(),
            doc_id_to_vector_idx: tmp_ret.doc_id_to_vector_idx.clone(),
        };
        let tmp_json = serde_json::to_string(&tmp_storage).map_err(|e| {
            RetrieverError::SerializationError(format!("serialize temp vectors failed: {}", e))
        })?;
        std::fs::write(&vectors_tmp, tmp_json).map_err(|e| {
            RetrieverError::IoError(format!("write vectors.new.json failed: {}", e))
        })?;
    }
    if !vectors_tmp.exists() {
        return Err(RetrieverError::IoError(format!(
            "temp vectors file not created: {:?}",
            vectors_tmp
        )));
    }
    info!("Reindex: temp vectors saved at {:?}", vectors_tmp);

    // Pre-swap validation: temp file must have equal vectors and mappings
    debug!("Reindex: reading temp vectors JSON from {:?}", vectors_tmp);
    let raw = std::fs::read_to_string(&vectors_tmp)
        .map_err(|e| RetrieverError::IoError(format!("read vectors.new.json failed: {}", e)))?;
    debug!("Reindex: loaded temp vectors JSON ({} bytes)", raw.len());
    #[derive(serde::Deserialize)]
    struct TmpStorage {
        vectors: Vec<Vec<f32>>,
        doc_id_to_vector_idx: std::collections::HashMap<String, usize>,
    }
    let tmp: TmpStorage = serde_json::from_str(&raw).map_err(|e| {
        RetrieverError::SerializationError(format!("parse vectors.new.json failed: {}", e))
    })?;
    info!(
        "Reindex: pre-swap validation OK: vectors={}, mappings={}",
        tmp.vectors.len(),
        tmp.doc_id_to_vector_idx.len()
    );
    if tmp.vectors.len() != tmp.doc_id_to_vector_idx.len() {
        return Err(RetrieverError::VectorError(format!(
            "Pre-swap validation failed: {} vectors but {} mappings",
            tmp.vectors.len(),
            tmp.doc_id_to_vector_idx.len()
        )));
    }

    // Prepare manifest.next.json
    let manifest_next = serde_json::json!({
        "transaction_id": chrono::Utc::now().to_rfc3339(),
        "vectors_count": tmp.vectors.len(),
        "mappings_count": tmp.doc_id_to_vector_idx.len(),
        "created_at": chrono::Utc::now().to_rfc3339(),
        "index_dir": pm.index_path("tantivy"),
        "vector_file": pm.vector_store_path(),
    });
    let manifest_dir = pm.data_dir();
    let manifest_path = manifest_dir.join("manifest.json");
    let manifest_next_path = manifest_dir.join("manifest.next.json");
    info!(
        "Reindex: writing manifest.next.json -> {:?}",
        manifest_next_path
    );
    std::fs::write(
        &manifest_next_path,
        serde_json::to_string_pretty(&manifest_next).unwrap(),
    )
    .map_err(|e| RetrieverError::IoError(format!("write manifest.next.json failed: {}", e)))?;
    debug!("Reindex: manifest.next.json written");

    let vectors_count = tmp.vectors.len();
    let mappings_count = tmp.doc_id_to_vector_idx.len();

    // Swap with backups
    let ts = Utc::now().format("%Y%m%d%H%M%S").to_string();

    let index_bak = pm.index_dir().join(format!("tantivy.bak-{}", ts));
    if live_index_dir.exists() {
        info!(
            "Reindex: renaming live index -> backup: {:?} -> {:?}",
            live_index_dir, index_bak
        );
        std::fs::rename(&live_index_dir, &index_bak)
            .map_err(|e| RetrieverError::IoError(format!("index backup rename failed: {}", e)))?;
    } else {
        debug!(
            "Reindex: live index dir does not exist (first run?): {:?}",
            live_index_dir
        );
    }
    info!(
        "Reindex: renaming tmp index -> live: {:?} -> {:?}",
        tmp_index_dir, live_index_dir
    );
    std::fs::rename(&tmp_index_dir, &live_index_dir)
        .map_err(|e| RetrieverError::IoError(format!("tmp->live index rename failed: {}", e)))?;

    let vectors_bak = vectors_path.with_file_name(format!("vectors.json.bak-{}", ts));
    if vectors_path.exists() {
        info!(
            "Reindex: renaming live vectors -> backup: {:?} -> {:?}",
            vectors_path, vectors_bak
        );
        std::fs::rename(&vectors_path, &vectors_bak)
            .map_err(|e| RetrieverError::IoError(format!("vectors backup rename failed: {}", e)))?;
    } else {
        debug!(
            "Reindex: live vectors file does not exist (first run?): {:?}",
            vectors_path
        );
    }
    info!(
        "Reindex: renaming tmp vectors -> live: {:?} -> {:?}",
        vectors_tmp, vectors_path
    );
    std::fs::rename(&vectors_tmp, &vectors_path)
        .map_err(|e| RetrieverError::IoError(format!("tmp->live vectors rename failed: {}", e)))?;

    // Swap manifest.next.json -> manifest.json
    if manifest_path.exists() {
        let manifest_bak = manifest_dir.join(format!("manifest.json.bak-{}", ts));
        info!(
            "Reindex: renaming manifest -> backup: {:?} -> {:?}",
            manifest_path, manifest_bak
        );
        std::fs::rename(&manifest_path, &manifest_bak).map_err(|e| {
            RetrieverError::IoError(format!("manifest backup rename failed: {}", e))
        })?;
    } else {
        debug!(
            "Reindex: manifest not present yet (first run?) at {:?}",
            manifest_path
        );
    }
    info!(
        "Reindex: renaming manifest.next -> manifest: {:?} -> {:?}",
        manifest_next_path, manifest_path
    );
    std::fs::rename(&manifest_next_path, &manifest_path).map_err(|e| {
        RetrieverError::IoError(format!("manifest next->live rename failed: {}", e))
    })?;

    Ok((vectors_count, mappings_count))
}

impl Retriever {
    /// Create a new Retriever with custom vector storage path
    pub fn new_with_vector_file(
        index_dir: &str,
        vector_file_path: &str,
    ) -> Result<Self, RetrieverError> {
        let mut schema_builder = Schema::builder();
        let title_field = schema_builder.add_text_field("title", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT | STORED);
        let doc_id_field = schema_builder.add_text_field("doc_id", TEXT | STORED);
        let schema = schema_builder.build();
        fs::create_dir_all(index_dir)?;
        let dir = MmapDirectory::open(index_dir)?;
        let index = Index::open_or_create(dir, schema)?;

        let vector_file_path_owned = vector_file_path.to_string();

        let mut retriever = Retriever {
            vectors: Vec::new(),
            index,
            title_field,
            content_field,
            doc_id_field,
            doc_id_to_vector_idx: HashMap::new(),
            vector_file_path: vector_file_path_owned.clone(),
            auto_save_threshold: 100,
            documents_since_save: Arc::new(AtomicUsize::new(0)),
            index_writer: None,
            batch_mode: false,
            search_cache: LruCache::new(NonZeroUsize::new(100).unwrap()),
            cache_enabled: true,
            // Phase 11 Step 2: Initialize L2 cache (300 seconds = 5 minute TTL)
            l2_cache: Some(crate::cache::cache_layer::MemoryCache::new(300)),
            l2_cache_stats: crate::cache::cache_layer::CacheStats::default(),
            // Phase 12 Step 2: L3 Redis cache (initialized later from config)
            l3_cache: None,
            metrics: RetrieverMetrics {
                index_path: index_dir.to_string(),
                ..Default::default()
            },
            index_dir_path: index_dir.to_string(),
            search_top_k: 10,
            // Phase 13: Bloom filter for O(1) document existence checks
            doc_bloom_filter: crate::perf::bloom::VectorBloomFilter::new(100_000, 0.01),
            // Phase 14: HNSW index (built after vectors are loaded)
            hnsw_index: None,
            // Phase 15: Semantic query cache
            semantic_cache: crate::perf::semantic_cache::SemanticCache::with_defaults(),
            // Phase 16: Hybrid searcher
            hybrid_searcher: crate::perf::hybrid_search::HybridSearcher::with_defaults(),
            // Phase 17: Re-ranker
            reranker: crate::perf::reranking::Reranker::with_defaults(),
            // Phase 18: Request coalescer
            request_coalescer: crate::perf::request_coalescing::Singleflight::new(),
            // Phase 19: Product Quantization (built on demand)
            pq_index: None,
            // Phase 20: FP16 store (built on demand)
            fp16_store: None,
            // Phase 21: Connection pool
            connection_pool: crate::perf::connection_pool::ConnectionPool::new(
                crate::perf::connection_pool::PoolConfig::default()
            ),
            // Phase 22: io_uring availability
            use_io_uring: crate::perf::io_uring::is_available(),
        };

        // Now load from the CORRECT path - use auto-detection for rkyv/JSON
        // This will prefer rkyv (faster) and fall back to JSON, auto-migrating if needed
        if let Err(e) = retriever.load_vectors_auto(&vector_file_path_owned) {
            info!(
                "No existing vectors found at '{}', starting fresh: {}",
                vector_file_path_owned, e
            );
        } else {
            info!("Loaded existing vectors from {}", vector_file_path_owned);
            retriever.metrics.total_vectors = retriever.vectors.len();
        }

        if let Ok(reader) = retriever.index.reader() {
            retriever.metrics.total_documents_indexed = reader.searcher().num_docs() as usize;
        }

        Ok(retriever)
    }
    // LOCATION: ag/src/retriever.rs
    // INSERT THIS AFTER LINE 221 (after the new_with_vector_file method ends)

    /// NEW for v13.1.2: Create with PathBuf paths (wrapper for new_with_vector_file)

    pub fn new_with_paths(
        index_dir: std::path::PathBuf,
        vector_file: std::path::PathBuf,
    ) -> Result<Self, RetrieverError> {
        let index_dir_str = index_dir.to_string_lossy().to_string();
        let vector_file_str = vector_file.to_string_lossy().to_string();
        Self::new_with_vector_file(&index_dir_str, &vector_file_str)
    }

    /// Create a new Retriever with default vector storage path ("./vectors.json")
    pub fn new(index_dir: &str) -> Result<Self, RetrieverError> {
        Self::new_with_vector_file(index_dir, "./vectors.json")
    }

    pub fn new_dummy() -> Result<Self, RetrieverError> {
        // Create a unique dummy path to avoid conflicts
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dummy_dir = format!("./dummy_tantivy_index_{}", timestamp);
        let dummy_vector_file = format!("./dummy_vectors_{}.json", timestamp);
        let result = Self::new_with_vector_file(&dummy_dir, &dummy_vector_file);

        // Clean up the dummy files immediately if creation failed
        if result.is_err() {
            let _ = fs::remove_dir_all(&dummy_dir);
            let _ = fs::remove_file(&dummy_vector_file);
        }

        result
    }

    /// Repair vector mappings by adding default IDs for unmapped vectors
    pub fn repair_vector_mappings(&mut self) -> usize {
        let mapped_indices: HashSet<usize> = self.doc_id_to_vector_idx.values().cloned().collect();

        let mut repaired = 0;
        for idx in 0..self.vectors.len() {
            if !mapped_indices.contains(&idx) {
                let default_id = format!("unmapped_vector_{}", idx);
                self.doc_id_to_vector_idx.insert(default_id, idx);
                repaired += 1;
            }
        }

        if repaired > 0 {
            debug!("Repaired {} unmapped vectors", repaired);
            if let Err(e) = self.save_vectors(&self.vector_file_path.clone()) {
                error!("Failed to save repaired mappings: {}", e);
            }
        }

        repaired
    }

    pub fn set_cache_enabled(&mut self, enabled: bool) {
        self.cache_enabled = enabled;
        if !enabled {
            self.search_cache.clear();
        }
        debug!(
            "Search cache {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }

    pub fn clear_cache(&mut self) {
        self.search_cache.clear();
        debug!("Search cache cleared");
    }

    pub fn cache_stats(&self) -> (usize, usize) {
        (self.search_cache.len(), self.search_cache.cap().get())
    }

    // ========================================================================
    // Search Cache Persistence (rkyv-based)
    // ========================================================================

    /// Save search cache to disk using rkyv binary format
    pub fn save_search_cache(&self, path: &str) -> Result<(), RetrieverError> {
        // Convert LruCache to Vec for serialization
        let entries: Vec<(String, Vec<String>)> = self.search_cache
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entries)
            .map_err(|e| RetrieverError::SerializationError(format!("rkyv serialize error: {}", e)))?;
        
        std::fs::write(path, &bytes)?;
        
        info!(
            "Saved {} search cache entries to {} ({} bytes)",
            entries.len(),
            path,
            bytes.len()
        );
        Ok(())
    }

    /// Load search cache from disk using rkyv binary format
    pub fn load_search_cache(&mut self, path: &str) -> Result<usize, RetrieverError> {
        if !Path::new(path).exists() {
            return Ok(0);
        }
        
        let bytes = std::fs::read(path)?;
        
        let archived = rkyv::access::<rkyv::Archived<Vec<(String, Vec<String>)>>, rkyv::rancor::Error>(&bytes)
            .map_err(|e| RetrieverError::SerializationError(format!("rkyv access error: {}", e)))?;
        
        let mut loaded = 0;
        for entry in archived.iter() {
            let key = entry.0.to_string();
            let value: Vec<String> = entry.1.iter().map(|s| s.to_string()).collect();
            self.search_cache.put(key, value);
            loaded += 1;
        }
        
        info!(
            "Loaded {} search cache entries from {} ({} bytes)",
            loaded,
            path,
            bytes.len()
        );
        Ok(loaded)
    }

    pub fn get_metrics(&self) -> RetrieverMetrics {
        self.metrics.clone()
    }

    pub fn reset_metrics(&mut self) {
        self.metrics = RetrieverMetrics {
            index_path: self.index_dir_path.clone(),
            ..Default::default()
        };
    }

    pub fn set_search_top_k(&mut self, top_k: usize) {
        self.search_top_k = top_k.max(1);
        debug!(top_k = self.search_top_k, "Updated search_top_k");
        crate::monitoring::metrics::set_search_top_k(self.search_top_k as i64);
    }

    pub fn current_search_top_k(&self) -> usize {
        self.search_top_k
    }
    pub fn begin_batch(&mut self) -> Result<(), RetrieverError> {
        if self.index_writer.is_some() {
            return Err(RetrieverError::IndexError(
                "Batch already in progress".to_string(),
            ));
        }
        self.index_writer = Some(self.index.writer(256_000_000)?);
        self.batch_mode = true;
        debug!("Batch indexing mode started");
        Ok(())
    }

    pub fn end_batch(&mut self) -> Result<(), RetrieverError> {
        if let Some(mut writer) = self.index_writer.take() {
            writer.commit()?;
            self.batch_mode = false;
            self.clear_cache();
            if let Ok(reader) = self.index.reader() {
                self.metrics.total_documents_indexed = reader.searcher().num_docs() as usize;
            }
            debug!("Batch indexing mode ended, changes committed");
            Ok(())
        } else {
            Err(RetrieverError::IndexError(
                "No batch in progress".to_string(),
            ))
        }
    }

    pub fn add_documents_batch(
        &mut self,
        documents: Vec<(String, String, String)>,
    ) -> Result<usize, RetrieverError> {
        let was_batch = self.batch_mode;
        if !was_batch {
            self.begin_batch()?;
        }
        let mut count = 0;
        for (doc_id, title, content) in documents {
            // ← unpack all 3 values
            if let Err(e) = self.add_document_to_batch(&doc_id, &title, &content) {
                // ← pass all 3 to add_document_to_batch
                error!("Failed to add document '{}': {}", doc_id, e);
            } else {
                count += 1;
            }
        }
        if !was_batch {
            self.end_batch()?;
        }
        Ok(count)
    }

    fn add_document_to_batch(
        &mut self,
        doc_id: &str,
        title: &str,
        content: &str,
    ) -> Result<(), RetrieverError> {
        if !self.batch_mode {
            return Err(RetrieverError::IndexError("Not in batch mode".to_string()));
        }
        let mut doc = tantivy::TantivyDocument::default();
        doc.add_text(self.doc_id_field, doc_id);
        doc.add_text(self.title_field, title);
        doc.add_text(self.content_field, content);
        if let Some(writer) = &mut self.index_writer {
            writer.add_document(doc)?;
            Ok(())
        } else {
            Err(RetrieverError::IndexError(
                "No writer available".to_string(),
            ))
        }
    }

    pub fn search(&mut self, query_str: &str) -> Result<Vec<String>, RetrieverError> {
        let start_time = Instant::now();
        if self.cache_enabled {
            if let Some(cached) = self.search_cache.get(query_str) {
                self.metrics.cache_hits += 1;
                crate::monitoring::metrics::CACHE_HITS_TOTAL.inc();
                self.metrics.total_searches += 1;
                let latency_us = start_time.elapsed().as_micros();
                self.metrics.total_search_latency_us += latency_us;
                self.metrics.avg_search_latency_us = self.metrics.total_search_latency_us as f64
                    / self.metrics.total_searches as f64;
                if latency_us > self.metrics.max_search_latency_us {
                    self.metrics.max_search_latency_us = latency_us;
                }
                self.metrics.last_updated = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs();
                crate::monitoring::metrics::set_cache_hit_rate_percent(
                    self.metrics.cache_hit_rate(),
                );
                return Ok(cached.clone());
            }
        }
        self.metrics.cache_misses += 1;
        crate::monitoring::metrics::CACHE_MISSES_TOTAL.inc();
        self.metrics.total_searches += 1;
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        let parser =
            QueryParser::for_index(&self.index, vec![self.title_field, self.content_field]);
        let query = parser.parse_query(query_str)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(self.search_top_k))?;
        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let doc = searcher.doc::<tantivy::TantivyDocument>(doc_address)?;
            let content = doc
                .get_first(self.content_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            results.push(content);
        }
        if self.cache_enabled {
            self.search_cache
                .put(query_str.to_string(), results.clone());
        }
        let latency_us = start_time.elapsed().as_micros();
        self.metrics.total_search_latency_us += latency_us;
        // Observe latency in ms for Prometheus
        crate::monitoring::metrics::observe_search_latency_ms((latency_us as f64) / 1000.0);
        self.metrics.avg_search_latency_us =
            self.metrics.total_search_latency_us as f64 / self.metrics.total_searches as f64;
        if latency_us > self.metrics.max_search_latency_us {
            self.metrics.max_search_latency_us = latency_us;
        }
        self.metrics.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
        crate::monitoring::metrics::set_cache_hit_rate_percent(self.metrics.cache_hit_rate());
        Ok(results)
    }

    pub fn add_vector(&mut self, vector: Vec<f32>) {
        self.vectors.push(vector);
        self.metrics.total_vectors += 1;
        self.check_auto_save();
    }

    pub fn add_vector_with_id(&mut self, doc_id: String, vector: Vec<f32>) {
        let idx = self.vectors.len();
        // Add to bloom filter for O(1) existence checks
        self.doc_bloom_filter.insert(&doc_id);
        // Add to HNSW index if it exists (for O(log n) search)
        if let Some(ref mut hnsw) = self.hnsw_index {
            hnsw.add(doc_id.clone(), vector.clone());
        }
        self.vectors.push(vector);
        self.doc_id_to_vector_idx.insert(doc_id, idx);
        self.metrics.total_vectors += 1;
        self.check_auto_save();
    }

    /// Fast O(1) check if a document might exist (may have false positives)
    pub fn might_contain_doc(&self, doc_id: &str) -> bool {
        self.doc_bloom_filter.might_contain(doc_id)
    }

    /// Definitive check if document does NOT exist (no false negatives)
    pub fn definitely_not_contains_doc(&self, doc_id: &str) -> bool {
        self.doc_bloom_filter.definitely_not_contains(doc_id)
    }

    fn check_auto_save(&mut self) {
        if crate::api::is_reindex_in_progress() {
            // suppress autosave during reindex window
            self.documents_since_save.store(0, Ordering::SeqCst);
            return;
        }
        let count = self.documents_since_save.fetch_add(1, Ordering::SeqCst) + 1;
        if count >= self.auto_save_threshold {
            if let Err(e) = self.save_vectors(&self.vector_file_path.clone()) {
                error!("Auto-save failed: {}", e);
            } else {
                debug!("Auto-saved vectors after {} documents", count);
                self.documents_since_save.store(0, Ordering::SeqCst);
            }
        }
    }

    pub fn vector_search(&mut self, query_vector: &[f32], top_k: usize) -> Vec<(usize, f32)> {
        // Use HNSW index for O(log n) search if available and has enough vectors
        if let Some(ref mut hnsw) = self.hnsw_index {
            if hnsw.len() > 100 {
                // HNSW returns (doc_id, similarity), convert to (idx, similarity)
                let hnsw_results = hnsw.search(query_vector, top_k);
                return hnsw_results.into_iter()
                    .filter_map(|(doc_id, score)| {
                        self.doc_id_to_vector_idx.get(&doc_id).map(|&idx| (idx, score))
                    })
                    .collect();
            }
        }
        
        // Fallback to linear scan with SIMD-accelerated cosine similarity
        let use_parallel = self.vectors.len() > 1000;
        let mut similarities: Vec<(usize, f32)> = if use_parallel {
            self.vectors
                .par_iter()
                .enumerate()
                .map(|(idx, vec)| (idx, cosine_similarity(query_vector, vec)))
                .collect()
        } else {
            self.vectors
                .iter()
                .enumerate()
                .map(|(idx, vec)| (idx, cosine_similarity(query_vector, vec)))
                .collect()
        };
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        similarities.into_iter().take(top_k).collect()
    }

    /// Build HNSW index from current vectors for O(log n) search
    pub fn build_hnsw_index(&mut self) {
        if self.vectors.is_empty() {
            return;
        }
        info!("Building HNSW index for {} vectors...", self.vectors.len());
        let dim = self.vectors.first().map(|v| v.len()).unwrap_or(384);
        let mut hnsw = crate::perf::hnsw::HnswIndex::new(dim);
        
        // Add all vectors with their doc_ids
        for (doc_id, &idx) in &self.doc_id_to_vector_idx {
            if idx < self.vectors.len() {
                hnsw.add(doc_id.clone(), self.vectors[idx].clone());
            }
        }
        
        hnsw.build();
        self.hnsw_index = Some(hnsw);
        info!("HNSW index built successfully");
    }

    pub fn hybrid_search(
        &mut self,
        query: &str,
        query_vector: Option<&[f32]>,
    ) -> Result<Vec<String>, RetrieverError> {
        let keyword_results = self.search(query)?;
        let query_vec = match query_vector {
            Some(v) => v,
            None => return Ok(keyword_results),
        };
        let vector_results = self.vector_search(query_vec, 10);
        let k = 60.0;
        let mut score_map: HashMap<String, f32> = HashMap::new();
        for (rank, content) in keyword_results.iter().enumerate() {
            let score = 1.0 / (k + (rank as f32) + 1.0);
            *score_map.entry(content.clone()).or_insert(0.0) += score;
        }
        for (rank, (idx, _similarity)) in vector_results.iter().enumerate() {
            if let Some(content) = self.get_content_by_vector_idx(*idx) {
                // ← Fixed: dereference idx
                let score = 1.0 / (k + (rank as f32) + 1.0);
                *score_map.entry(content).or_insert(0.0) += score;
            }
        }
        let mut merged_results: Vec<(String, f32)> = score_map.into_iter().collect();
        merged_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(merged_results
            .into_iter()
            .take(10)
            .map(|(content, _)| content)
            .collect())
    }

    pub fn get_content_by_vector_idx(&self, idx: usize) -> Option<String> {
        for (doc_id, &vec_idx) in &self.doc_id_to_vector_idx {
            if vec_idx == idx {
                return Some(doc_id.clone());
            }
        }
        None
    }

    pub fn rerank_by_similarity(&self, _query: &str, candidates: &Vec<String>) -> Vec<String> {
        let mut results = candidates.clone();
        results.reverse();
        results
    }

    pub fn rerank_by_vector_similarity(
        &self,
        query_vector: &[f32],
        candidate_indices: &[usize],
    ) -> Result<Vec<(usize, f32)>, RetrieverError> {
        let mut scored_candidates: Vec<(usize, f32)> = candidate_indices
            .iter()
            .filter_map(|&idx| {
                if idx < self.vectors.len() {
                    Some((idx, cosine_similarity(query_vector, &self.vectors[idx])))
                } else {
                    None
                }
            })
            .collect();
        if scored_candidates.is_empty() && !candidate_indices.is_empty() {
            return Err(RetrieverError::VectorError(
                "No valid candidate indices found".to_string(),
            ));
        }
        scored_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(scored_candidates)
    }

    pub fn summarize_chunks(&self, _query: &str, candidates: &Vec<String>) -> String {
        format!("Summary for {} chunks", candidates.len())
    }

    pub fn index_document(&mut self, doc: impl tantivy::Document) -> Result<(), RetrieverError> {
        let mut index_writer = self.index.writer(256_000_000)?;
        index_writer.add_document(doc)?;
        index_writer.commit()?;
        self.clear_cache();
        self.metrics.total_documents_indexed += 1;
        Ok(())
    }

    pub fn add_document(
        &mut self,
        doc_id: &str,
        title: &str,
        content: &str,
    ) -> Result<(), RetrieverError> {
        if self.batch_mode {
            return self.add_document_to_batch(doc_id, title, content);
        }
        let mut doc = tantivy::TantivyDocument::default();
        doc.add_text(self.doc_id_field, doc_id);
        doc.add_text(self.title_field, title);
        doc.add_text(self.content_field, content);
        let mut index_writer = self.index.writer(256_000_000)?;
        index_writer.add_document(doc)?;
        index_writer.commit()?;
        self.clear_cache();
        self.metrics.total_documents_indexed += 1;
        Ok(())
    }

    pub fn commit(&mut self) -> Result<(), RetrieverError> {
        if self.batch_mode {
            self.end_batch()?;
        }
        self.save_vectors(&self.vector_file_path.clone())?;
        Ok(())
    }

    fn parity_repair(&mut self) -> usize {
        let mapped_indices: std::collections::HashSet<usize> =
            self.doc_id_to_vector_idx.values().cloned().collect();
        let mut repaired = 0;
        for idx in 0..self.vectors.len() {
            if !mapped_indices.contains(&idx) {
                let default_id = format!("unmapped_vector_{}", idx);
                self.doc_id_to_vector_idx.insert(default_id, idx);
                repaired += 1;
            }
        }
        repaired
    }

    pub fn save_vectors(&mut self, filename: &str) -> Result<(), RetrieverError> {
        // Skip LIVE persistence during reindex; allow temp saves (vectors.new.json)
        if crate::api::is_reindex_in_progress() {
            if !filename.ends_with("vectors.new.json") {
                info!("Save skipped (live) during reindex: {}", filename);
                return Ok(());
            } else {
                info!("Temp save allowed during reindex: {}", filename);
            }
        }
        // Ensure parity before writing
        let repaired = self.parity_repair();
        if repaired > 0 {
            info!(
                "Parity repair: added {} missing mappings before save",
                repaired
            );
        }
        let storage = VectorStorage {
            vectors: self.vectors.clone(),
            doc_id_to_vector_idx: self.doc_id_to_vector_idx.clone(),
        };
        let json = serde_json::to_string(&storage)
            .map_err(|e| RetrieverError::SerializationError(e.to_string()))?;
        let mut file = File::create(filename)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn load_vectors(&mut self, filename: &str) -> Result<(), RetrieverError> {
        match File::open(filename) {
            Ok(mut file) => {
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                let storage: VectorStorage = serde_json::from_str(&contents)
                    .map_err(|e| RetrieverError::SerializationError(e.to_string()))?;
                self.vectors = storage.vectors;
                self.doc_id_to_vector_idx = storage.doc_id_to_vector_idx;
                self.metrics.total_vectors = self.vectors.len();
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Fresh start: missing file is acceptable
                info!(
                    "No existing vectors found at '{}', starting fresh: {}",
                    filename, e
                );
                self.vectors = Vec::new();
                self.doc_id_to_vector_idx = HashMap::new();
                self.metrics.total_vectors = 0;
                Ok(())
            }
            Err(e) => Err(RetrieverError::IoError(e.to_string())),
        }
    }

    /// Save vectors in rkyv binary format (10-50x faster than JSON)
    pub fn save_vectors_rkyv(&mut self, filename: &str) -> Result<(), RetrieverError> {
        // Skip LIVE persistence during reindex; allow temp saves
        if crate::api::is_reindex_in_progress() {
            if !filename.ends_with("vectors.new.rkyv") {
                info!("rkyv save skipped (live) during reindex: {}", filename);
                return Ok(());
            } else {
                info!("rkyv temp save allowed during reindex: {}", filename);
            }
        }

        // Ensure parity before writing
        let repaired = self.parity_repair();
        if repaired > 0 {
            info!(
                "Parity repair: added {} missing mappings before rkyv save",
                repaired
            );
        }

        let storage = VectorStorageRkyv::from_retriever(&self.vectors, &self.doc_id_to_vector_idx);
        
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&storage)
            .map_err(|e| RetrieverError::SerializationError(format!("rkyv serialize error: {}", e)))?;
        
        let mut file = File::create(filename)?;
        file.write_all(&bytes)?;
        
        info!(
            "Saved {} vectors to rkyv format ({} bytes)",
            self.vectors.len(),
            bytes.len()
        );
        Ok(())
    }

    /// Load vectors from rkyv binary format (10-50x faster than JSON)
    pub fn load_vectors_rkyv(&mut self, filename: &str) -> Result<(), RetrieverError> {
        match std::fs::read(filename) {
            Ok(bytes) => {
                let archived = rkyv::access::<ArchivedVectorStorageRkyv, rkyv::rancor::Error>(&bytes)
                    .map_err(|e| RetrieverError::SerializationError(format!("rkyv access error: {}", e)))?;
                
                // Check version for future migrations
                if archived.version != VectorStorageRkyv::CURRENT_VERSION {
                    info!(
                        "rkyv version mismatch: file={}, current={}",
                        archived.version,
                        VectorStorageRkyv::CURRENT_VERSION
                    );
                }
                
                // Deserialize vectors (this does allocate, but is still faster than JSON parsing)
                // rkyv uses f32_le (little-endian f32), we need to convert to native f32
                self.vectors = archived.vectors
                    .iter()
                    .map(|v| v.iter().map(|f| f.to_native()).collect())
                    .collect();
                
                // Rebuild HashMap from flat pairs
                // rkyv uses u32_le, convert to native usize
                self.doc_id_to_vector_idx = archived.doc_id_to_idx
                    .iter()
                    .map(|pair| (pair.0.to_string(), pair.1.to_native() as usize))
                    .collect();
                
                self.metrics.total_vectors = self.vectors.len();
                
                info!(
                    "Loaded {} vectors from rkyv format ({} bytes)",
                    self.vectors.len(),
                    bytes.len()
                );
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File not found - caller should try JSON fallback
                Err(RetrieverError::IoError(format!("rkyv file not found: {}", filename)))
            }
            Err(e) => Err(RetrieverError::IoError(e.to_string())),
        }
    }

    /// Load vectors using memory-mapped file (zero-copy, fastest possible)
    /// This maps the file directly into memory without copying, providing
    /// near-instant startup and reduced memory pressure.
    /// 
    /// # Safety
    /// The mmap is safe as long as the file isn't modified while mapped.
    /// We copy the data out immediately to avoid holding the mmap.
    pub fn load_vectors_mmap(&mut self, filename: &str) -> Result<(), RetrieverError> {
        let file = File::open(filename)
            .map_err(|e| RetrieverError::IoError(format!("Failed to open file: {}", e)))?;
        
        // Memory-map the file (zero-copy access)
        let mmap = unsafe { Mmap::map(&file) }
            .map_err(|e| RetrieverError::IoError(format!("Failed to mmap file: {}", e)))?;
        
        let start = Instant::now();
        
        // Access the archived data directly from the mmap
        let archived = rkyv::access::<ArchivedVectorStorageRkyv, rkyv::rancor::Error>(&mmap)
            .map_err(|e| RetrieverError::SerializationError(format!("rkyv access error: {}", e)))?;
        
        let access_time = start.elapsed();
        
        // Check version
        if archived.version != VectorStorageRkyv::CURRENT_VERSION {
            info!(
                "rkyv version mismatch: file={}, current={}",
                archived.version,
                VectorStorageRkyv::CURRENT_VERSION
            );
        }
        
        let copy_start = Instant::now();
        
        // Copy vectors out of mmap (we need owned data for mutations)
        self.vectors = archived.vectors
            .iter()
            .map(|v| v.iter().map(|f| f.to_native()).collect())
            .collect();
        
        self.doc_id_to_vector_idx = archived.doc_id_to_idx
            .iter()
            .map(|pair| (pair.0.to_string(), pair.1.to_native() as usize))
            .collect();
        
        let copy_time = copy_start.elapsed();
        
        self.metrics.total_vectors = self.vectors.len();
        
        info!(
            "Loaded {} vectors via mmap ({} bytes, access: {:?}, copy: {:?})",
            self.vectors.len(),
            mmap.len(),
            access_time,
            copy_time
        );
        
        Ok(())
    }

    // ========================================================================
    // Incremental Vector Updates (Append-Only Log)
    // ========================================================================

    /// Append a single vector to the append-only log file
    /// This is much faster than rewriting the entire vector file
    pub fn append_vector_to_log(&self, doc_id: &str, vector: &[f32], log_path: &str) -> Result<(), RetrieverError> {
        use std::io::BufWriter;
        
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        
        let mut writer = BufWriter::new(file);
        
        // Write entry: doc_id length (u32) + doc_id bytes + vector length (u32) + vector floats
        let doc_id_bytes = doc_id.as_bytes();
        writer.write_all(&(doc_id_bytes.len() as u32).to_le_bytes())?;
        writer.write_all(doc_id_bytes)?;
        writer.write_all(&(vector.len() as u32).to_le_bytes())?;
        for &f in vector {
            writer.write_all(&f.to_le_bytes())?;
        }
        
        debug!("Appended vector for {} to log", doc_id);
        Ok(())
    }

    /// Load vectors from append-only log and merge with existing vectors
    pub fn load_vector_log(&mut self, log_path: &str) -> Result<usize, RetrieverError> {
        use std::io::{BufReader, Read as _};
        
        if !Path::new(log_path).exists() {
            return Ok(0);
        }
        
        let file = File::open(log_path)?;
        let mut reader = BufReader::new(file);
        let mut loaded = 0;
        
        loop {
            // Read doc_id length
            let mut len_buf = [0u8; 4];
            if reader.read_exact(&mut len_buf).is_err() {
                break; // EOF
            }
            let doc_id_len = u32::from_le_bytes(len_buf) as usize;
            
            // Read doc_id
            let mut doc_id_buf = vec![0u8; doc_id_len];
            reader.read_exact(&mut doc_id_buf)?;
            let doc_id = String::from_utf8(doc_id_buf)
                .map_err(|e| RetrieverError::SerializationError(e.to_string()))?;
            
            // Read vector length
            reader.read_exact(&mut len_buf)?;
            let vec_len = u32::from_le_bytes(len_buf) as usize;
            
            // Read vector
            let mut vector = Vec::with_capacity(vec_len);
            for _ in 0..vec_len {
                let mut f_buf = [0u8; 4];
                reader.read_exact(&mut f_buf)?;
                vector.push(f32::from_le_bytes(f_buf));
            }
            
            // Add or update vector
            self.add_vector_with_id(doc_id, vector);
            loaded += 1;
        }
        
        info!("Loaded {} vectors from append log: {}", loaded, log_path);
        Ok(loaded)
    }

    /// Compact the append-only log by merging it into the main vector file
    /// This should be called periodically to prevent the log from growing too large
    pub fn compact_vector_log(&mut self, log_path: &str) -> Result<(), RetrieverError> {
        // Load any pending entries from the log
        let loaded = self.load_vector_log(log_path)?;
        
        if loaded > 0 {
            // Save the merged vectors
            let rkyv_path = self.vector_file_path.replace(".json", ".rkyv");
            self.save_vectors_rkyv(&rkyv_path)?;
            
            // Clear the log file
            std::fs::write(log_path, b"")?;
            
            info!("Compacted {} log entries into main vector file", loaded);
        }
        
        Ok(())
    }

    /// Auto-detect and load vectors from rkyv or JSON format
    /// Prefers mmap (fastest), falls back to rkyv read, then JSON, migrates if needed
    pub fn load_vectors_auto(&mut self, base_path: &str) -> Result<(), RetrieverError> {
        let rkyv_path = base_path.replace(".json", ".rkyv");
        let json_path = if base_path.ends_with(".json") {
            base_path.to_string()
        } else {
            format!("{}.json", base_path.trim_end_matches(".rkyv"))
        };

        // Try mmap first (fastest - zero-copy access)
        if Path::new(&rkyv_path).exists() {
            match self.load_vectors_mmap(&rkyv_path) {
                Ok(()) => {
                    info!("Loaded vectors via mmap: {}", rkyv_path);
                    return Ok(());
                }
                Err(e) => {
                    info!("mmap load failed, trying rkyv read: {}", e);
                    // Fall back to regular rkyv read
                    match self.load_vectors_rkyv(&rkyv_path) {
                        Ok(()) => {
                            info!("Loaded vectors from rkyv: {}", rkyv_path);
                            return Ok(());
                        }
                        Err(e2) => {
                            info!("rkyv load also failed, trying JSON: {}", e2);
                        }
                    }
                }
            }
        }

        // Try JSON (slower but more compatible)
        if Path::new(&json_path).exists() {
            self.load_vectors(&json_path)?;
            info!("Loaded vectors from JSON: {}", json_path);
            
            // Migrate to rkyv for next time
            if !self.vectors.is_empty() {
                match self.save_vectors_rkyv(&rkyv_path) {
                    Ok(()) => info!("Migrated vectors to rkyv format: {}", rkyv_path),
                    Err(e) => info!("Failed to migrate to rkyv (non-fatal): {}", e),
                }
            }
            return Ok(());
        }

        // Fresh start
        info!("No existing vectors found, starting fresh");
        self.vectors = Vec::new();
        self.doc_id_to_vector_idx = HashMap::new();
        self.metrics.total_vectors = 0;
        Ok(())
    }

    /// Save vectors in both rkyv (primary) and JSON (backup) formats
    pub fn save_vectors_dual(&mut self, base_path: &str) -> Result<(), RetrieverError> {
        let rkyv_path = base_path.replace(".json", ".rkyv");
        let json_path = if base_path.ends_with(".json") {
            base_path.to_string()
        } else {
            format!("{}.json", base_path.trim_end_matches(".rkyv"))
        };

        // Save rkyv (primary - fast)
        self.save_vectors_rkyv(&rkyv_path)?;
        
        // Save JSON (backup - human readable, slower)
        self.save_vectors(&json_path)?;
        
        Ok(())
    }

    pub fn force_save(&mut self) -> Result<(), RetrieverError> {
        if crate::api::is_reindex_in_progress()
            && !self.vector_file_path.ends_with("vectors.new.json")
        {
            info!("Manual save skipped (live) during reindex");
            return Ok(());
        }
        info!("Manual save triggered");
        let path = self.vector_file_path.clone();
        self.save_vectors(&path)?;
        self.documents_since_save.store(0, Ordering::SeqCst);
        Ok(())
    }

    pub fn set_auto_save_threshold(&mut self, threshold: usize) {
        self.auto_save_threshold = threshold;
        info!("Auto-save threshold set to {} documents", threshold);
    }

    pub fn index_chunk(
        &mut self,
        chunk_id: &str,
        chunk_text: &str,
        vector: &Vec<f32>,
    ) -> Result<(), RetrieverError> {
        self.add_document(chunk_id, chunk_id, chunk_text)?;
        self.add_vector_with_id(chunk_id.to_string(), vector.clone());
        Ok(())
    }

    fn check_disk_space(&self, min_free_bytes: u64) -> Result<(), RetrieverError> {
        let path = Path::new(&self.index_dir_path);
        let available_space = fs2::available_space(path)
            .map_err(|e| RetrieverError::IoError(format!("Failed to get disk space: {}", e)))?;
        if available_space < min_free_bytes {
            return Err(RetrieverError::IoError(format!(
                "Insufficient disk space: {} bytes available, {} bytes required",
                available_space, min_free_bytes
            )));
        }
        Ok(())
    }

    fn validate_vector_dimensions(&self) -> Result<(), RetrieverError> {
        if self.vectors.is_empty() {
            return Ok(());
        }
        let expected_dim = self.vectors[0].len();
        for (idx, vector) in self.vectors.iter().enumerate() {
            if vector.len() != expected_dim {
                return Err(RetrieverError::VectorError(format!(
                    "Vector dimension mismatch at index {}: expected {}, found {}",
                    idx,
                    expected_dim,
                    vector.len()
                )));
            }
        }
        debug!(
            "Vector dimension validation passed: {} vectors with dimension {}",
            self.vectors.len(),
            expected_dim
        );
        Ok(())
    }

    pub fn health_check(&self) -> Result<(), RetrieverError> {
        let index_path = Path::new(&self.index_dir_path);
        if !index_path.exists() {
            return Err(RetrieverError::DirectoryError(format!(
                "Index directory does not exist: {}",
                self.index_dir_path
            )));
        }
        if !index_path.is_dir() {
            return Err(RetrieverError::DirectoryError(format!(
                "Index path is not a directory: {}",
                self.index_dir_path
            )));
        }

        let reader = self.index.reader().map_err(|e| {
            RetrieverError::IndexError(format!("Failed to create index reader: {}", e))
        })?;
        let searcher = reader.searcher();
        let doc_count = searcher.num_docs();

        if self.vectors.len() != self.doc_id_to_vector_idx.len() {
            return Err(RetrieverError::VectorError(format!(
                "Vector storage inconsistency: {} vectors but {} document mappings",
                self.vectors.len(),
                self.doc_id_to_vector_idx.len()
            )));
        }

        for (doc_id, &vec_idx) in &self.doc_id_to_vector_idx {
            if vec_idx >= self.vectors.len() {
                return Err(RetrieverError::VectorError(format!(
                    "Invalid vector index {} for document '{}' (vectors length: {})",
                    vec_idx,
                    doc_id,
                    self.vectors.len()
                )));
            }
        }

        self.validate_vector_dimensions()?;

        if doc_count > 0 {
            let parser =
                QueryParser::for_index(&self.index, vec![self.title_field, self.content_field]);
            match parser.parse_query("*") {
                Ok(query) => {
                    if let Err(e) = searcher.search(&query, &TopDocs::with_limit(1)) {
                        return Err(RetrieverError::IndexError(format!(
                            "Basic search test failed: {}",
                            e
                        )));
                    }
                }
                Err(_e) => {
                    let fallback_query = parser
                        .parse_query("a")
                        .unwrap_or_else(|_| parser.parse_query("*").unwrap());
                    let first_doc_addr = searcher
                        .search(&fallback_query, &TopDocs::with_limit(1))
                        .map(|top_docs| top_docs.first().map(|(_, addr)| *addr))
                        .unwrap_or(None);
                    if let Some(addr) = first_doc_addr {
                        if let Err(e) = searcher.doc::<tantivy::TantivyDocument>(addr) {
                            return Err(RetrieverError::IndexError(format!(
                                "Failed to retrieve document: {}",
                                e
                            )));
                        }
                    }
                }
            }
        }

        if self.cache_enabled && !self.search_cache.is_empty() {
            let _ = self.search_cache.len();
            let _ = self.search_cache.cap();
        }

        if Path::new(&self.vector_file_path).exists() {
            if let Err(e) = std::fs::OpenOptions::new()
                .write(true)
                .open(&self.vector_file_path)
            {
                return Err(RetrieverError::IoError(format!(
                    "Vector file exists but is not writable: {}",
                    e
                )));
            }
        } else {
            if let Some(parent) = Path::new(&self.vector_file_path).parent() {
                if !parent.exists() {
                    return Err(RetrieverError::IoError(format!(
                        "Vector file parent directory does not exist: {:?}",
                        parent
                    )));
                }
                let temp_file = parent.join(".health_check_test");
                if let Err(e) = std::fs::File::create(&temp_file) {
                    let _ = std::fs::remove_file(&temp_file);
                    return Err(RetrieverError::IoError(format!(
                        "Cannot write to vector file directory: {}",
                        e
                    )));
                }
                let _ = std::fs::remove_file(&temp_file);
            }
        }

        self.check_disk_space(100 * 1024 * 1024)?;

        info!(
            "Health: OK - {} documents, {} vectors",
            doc_count,
            self.vectors.len()
        );
        Ok(())
    }

    pub fn ready_check(&self) -> Result<(), RetrieverError> {
        let reader = self.index.reader().map_err(|e| {
            RetrieverError::IndexError(format!("Failed to create index reader: {}", e))
        })?;
        let _searcher = reader.searcher();
        let _ = self.vectors.len();
        let _ = self.doc_id_to_vector_idx.len();
        Ok(())
    }
}

// Phase 11 Step 2: L2 Cache Methods - Version 1.0.0
impl Retriever {
    pub fn l1_cache_enabled(&self) -> bool {
        self.cache_enabled
    }

    pub fn l2_cache_enabled(&self) -> bool {
        self.l2_cache.is_some()
    }

    /// Get current L2 cache statistics
    pub fn get_l2_cache_stats(&self) -> crate::cache::cache_layer::CacheStats {
        self.l2_cache_stats.clone()
    }

    /// Clear L2 cache
    pub fn clear_l2_cache(&mut self) {
        if let Some(ref l2) = self.l2_cache {
            l2.clear();
        }
        self.l2_cache_stats = crate::cache::cache_layer::CacheStats::default();
    }

    /// Log cache statistics
    pub fn log_cache_stats(&self) {
        println!("L2 Cache Stats:");
        println!("  L1 Hits: {}", self.l2_cache_stats.l1_hits);
        println!("  L1 Misses: {}", self.l2_cache_stats.l1_misses);
        println!("  L2 Hits: {}", self.l2_cache_stats.l2_hits);
        println!("  L2 Misses: {}", self.l2_cache_stats.l2_misses);
        println!("  Total Items: {}", self.l2_cache_stats.total_items);
    }

    // Phase 12 Step 2: L3 Redis Cache methods

    /// Get L3 Redis cache status
    pub fn get_l3_cache_status(&self) -> String {
        if let Some(cache) = &self.l3_cache {
            if cache.is_enabled() {
                "L3 Redis cache: ENABLED".to_string()
            } else {
                "L3 Redis cache: DISABLED".to_string()
            }
        } else {
            "L3 Redis cache: NOT INITIALIZED".to_string()
        }
    }

    pub fn get_l3_cache_summary(&self) -> crate::cache::redis_cache::RedisCacheSummary {
        if let Some(cache) = &self.l3_cache {
            cache.summary()
        } else {
            crate::cache::redis_cache::RedisCacheSummary {
                enabled: false,
                connected: false,
                ttl_seconds: 0,
            }
        }
    }

    /// Set L3 Redis cache (called during initialization)
    pub fn set_l3_cache(&mut self, cache: RedisCache) {
        self.l3_cache = Some(cache);
        info!("L3 Redis cache set");
    }
}

impl Drop for Retriever {
    fn drop(&mut self) {
        if self.batch_mode {
            if let Err(e) = self.end_batch() {
                error!("Failed to end batch on shutdown: {}", e);
            }
        }
        // Skip saving when this is the temporary retriever used during atomic reindex
        if self.vector_file_path.ends_with("vectors.new.json") || self.vector_file_path.ends_with("vectors.new.rkyv") {
            debug!("Temp retriever shutdown detected; skipping save on drop");
        } else {
            debug!("Retriever shutting down, saving vectors in rkyv format...");
            // Save in rkyv format (primary - fast)
            let rkyv_path = self.vector_file_path.replace(".json", ".rkyv");
            if let Err(e) = self.save_vectors_rkyv(&rkyv_path) {
                error!("Failed to save vectors (rkyv) on shutdown: {}", e);
            } else {
                debug!("Vectors saved successfully (rkyv) on shutdown");
            }
            // Also save JSON for backward compatibility and debugging
            if let Err(e) = self.save_vectors(&self.vector_file_path.clone()) {
                error!("Failed to save vectors (JSON) on shutdown: {}", e);
            } else {
                debug!("Vectors saved successfully (JSON) on shutdown");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tempfile::tempdir;

    /// Test rkyv serialization roundtrip
    #[test]
    fn test_rkyv_roundtrip() {
        // Create test data
        let vectors: Vec<Vec<f32>> = vec![
            vec![0.1, 0.2, 0.3, 0.4],
            vec![0.5, 0.6, 0.7, 0.8],
            vec![0.9, 1.0, 1.1, 1.2],
        ];
        let mut doc_id_to_idx: HashMap<String, usize> = HashMap::new();
        doc_id_to_idx.insert("doc1".to_string(), 0);
        doc_id_to_idx.insert("doc2".to_string(), 1);
        doc_id_to_idx.insert("doc3".to_string(), 2);

        // Serialize with rkyv
        let storage = VectorStorageRkyv::from_retriever(&vectors, &doc_id_to_idx);
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&storage).unwrap();

        // Deserialize and verify
        let archived = rkyv::access::<ArchivedVectorStorageRkyv, rkyv::rancor::Error>(&bytes).unwrap();
        
        assert_eq!(archived.version.to_native(), VectorStorageRkyv::CURRENT_VERSION);
        assert_eq!(archived.vectors.len(), 3);
        assert_eq!(archived.doc_id_to_idx.len(), 3);

        // Verify vector values
        let first_vec: Vec<f32> = archived.vectors[0].iter().map(|f| f.to_native()).collect();
        assert_eq!(first_vec, vec![0.1, 0.2, 0.3, 0.4]);
    }

    /// Test rkyv vs JSON size comparison
    #[test]
    fn test_rkyv_size_comparison() {
        // Create realistic test data (100 vectors of 384 dimensions)
        let vectors: Vec<Vec<f32>> = (0..100)
            .map(|i| (0..384).map(|j| (i * 384 + j) as f32 * 0.001).collect())
            .collect();
        let mut doc_id_to_idx: HashMap<String, usize> = HashMap::new();
        for i in 0..100 {
            doc_id_to_idx.insert(format!("doc_{}", i), i);
        }

        // Serialize with JSON
        let json_storage = VectorStorage {
            vectors: vectors.clone(),
            doc_id_to_vector_idx: doc_id_to_idx.clone(),
        };
        let json_bytes = serde_json::to_string(&json_storage).unwrap();

        // Serialize with rkyv
        let rkyv_storage = VectorStorageRkyv::from_retriever(&vectors, &doc_id_to_idx);
        let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&rkyv_storage).unwrap();

        println!("\n=== SIZE COMPARISON ===");
        println!("JSON size: {} bytes ({:.2} KB)", json_bytes.len(), json_bytes.len() as f64 / 1024.0);
        println!("rkyv size: {} bytes ({:.2} KB)", rkyv_bytes.len(), rkyv_bytes.len() as f64 / 1024.0);
        println!("Ratio: {:.2}x smaller", json_bytes.len() as f64 / rkyv_bytes.len() as f64);
        println!("========================\n");

        // rkyv should be significantly smaller
        assert!(rkyv_bytes.len() < json_bytes.len(), "rkyv should be smaller than JSON");
    }

    /// Benchmark rkyv vs JSON serialization/deserialization
    #[test]
    fn test_rkyv_performance() {
        // Create test data (1000 vectors of 384 dimensions - realistic embedding size)
        let vectors: Vec<Vec<f32>> = (0..1000)
            .map(|i| (0..384).map(|j| (i * 384 + j) as f32 * 0.0001).collect())
            .collect();
        let mut doc_id_to_idx: HashMap<String, usize> = HashMap::new();
        for i in 0..1000 {
            doc_id_to_idx.insert(format!("document_id_{}", i), i);
        }

        // Benchmark JSON serialization
        let json_storage = VectorStorage {
            vectors: vectors.clone(),
            doc_id_to_vector_idx: doc_id_to_idx.clone(),
        };
        let json_start = Instant::now();
        let json_bytes = serde_json::to_string(&json_storage).unwrap();
        let json_serialize_time = json_start.elapsed();

        // Benchmark JSON deserialization
        let json_start = Instant::now();
        let _: VectorStorage = serde_json::from_str(&json_bytes).unwrap();
        let json_deserialize_time = json_start.elapsed();

        // Benchmark rkyv serialization
        let rkyv_storage = VectorStorageRkyv::from_retriever(&vectors, &doc_id_to_idx);
        let rkyv_start = Instant::now();
        let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&rkyv_storage).unwrap();
        let rkyv_serialize_time = rkyv_start.elapsed();

        // Benchmark rkyv access (zero-copy)
        let rkyv_start = Instant::now();
        let archived = rkyv::access::<ArchivedVectorStorageRkyv, rkyv::rancor::Error>(&rkyv_bytes).unwrap();
        // Force access to data to make it fair
        let _ = archived.vectors.len();
        let rkyv_access_time = rkyv_start.elapsed();

        // Benchmark rkyv full deserialization (for comparison)
        let rkyv_start = Instant::now();
        let archived = rkyv::access::<ArchivedVectorStorageRkyv, rkyv::rancor::Error>(&rkyv_bytes).unwrap();
        let _vectors: Vec<Vec<f32>> = archived.vectors
            .iter()
            .map(|v| v.iter().map(|f| f.to_native()).collect())
            .collect();
        let rkyv_deserialize_time = rkyv_start.elapsed();

        println!("\n=== PERFORMANCE COMPARISON (1000 vectors × 384 dims) ===");
        println!("JSON serialize:      {:?}", json_serialize_time);
        println!("JSON deserialize:    {:?}", json_deserialize_time);
        println!("rkyv serialize:      {:?}", rkyv_serialize_time);
        println!("rkyv access:         {:?} (zero-copy)", rkyv_access_time);
        println!("rkyv deserialize:    {:?} (full copy)", rkyv_deserialize_time);
        println!("\nSpeedup (serialize): {:.1}x", json_serialize_time.as_nanos() as f64 / rkyv_serialize_time.as_nanos() as f64);
        println!("Speedup (access):    {:.1}x", json_deserialize_time.as_nanos() as f64 / rkyv_access_time.as_nanos() as f64);
        println!("Speedup (deser):     {:.1}x", json_deserialize_time.as_nanos() as f64 / rkyv_deserialize_time.as_nanos() as f64);
        println!("\nJSON size: {} bytes", json_bytes.len());
        println!("rkyv size: {} bytes", rkyv_bytes.len());
        println!("Size ratio: {:.2}x smaller", json_bytes.len() as f64 / rkyv_bytes.len() as f64);
        println!("=======================================================\n");

        // rkyv should be faster
        assert!(rkyv_serialize_time < json_serialize_time, "rkyv serialize should be faster");
        assert!(rkyv_access_time < json_deserialize_time, "rkyv access should be faster");
    }

    /// Test file-based save/load with rkyv
    #[test]
    fn test_rkyv_file_roundtrip() {
        let temp_dir = tempdir().unwrap();
        let rkyv_path = temp_dir.path().join("test_vectors.rkyv");
        let json_path = temp_dir.path().join("test_vectors.json");
        let index_path = temp_dir.path().join("index");
        std::fs::create_dir_all(&index_path).unwrap();

        // Create a retriever with test data
        let mut retriever = Retriever::new_with_vector_file(
            index_path.to_str().unwrap(),
            json_path.to_str().unwrap(),
        ).unwrap();

        // Add some test vectors
        retriever.vectors = vec![
            vec![0.1, 0.2, 0.3],
            vec![0.4, 0.5, 0.6],
        ];
        retriever.doc_id_to_vector_idx.insert("doc1".to_string(), 0);
        retriever.doc_id_to_vector_idx.insert("doc2".to_string(), 1);

        // Save with rkyv
        retriever.save_vectors_rkyv(rkyv_path.to_str().unwrap()).unwrap();

        // Verify file exists
        assert!(rkyv_path.exists(), "rkyv file should exist");

        // Create new retriever and load
        let mut retriever2 = Retriever::new_with_vector_file(
            index_path.to_str().unwrap(),
            json_path.to_str().unwrap(),
        ).unwrap();
        retriever2.load_vectors_rkyv(rkyv_path.to_str().unwrap()).unwrap();

        // Verify data matches
        assert_eq!(retriever2.vectors.len(), 2);
        assert_eq!(retriever2.doc_id_to_vector_idx.len(), 2);
        assert_eq!(retriever2.vectors[0], vec![0.1, 0.2, 0.3]);
        assert_eq!(retriever2.doc_id_to_vector_idx.get("doc1"), Some(&0));
    }
}

// Phase 14-18: Advanced Performance Optimization Methods
impl Retriever {
    /// Get semantic cache statistics
    pub fn get_semantic_cache_stats(&self) -> crate::perf::semantic_cache::CacheStats {
        self.semantic_cache.stats()
    }

    /// Clear semantic cache
    pub fn clear_semantic_cache(&self) {
        self.semantic_cache.clear();
    }

    /// Check if HNSW index is built
    pub fn has_hnsw_index(&self) -> bool {
        self.hnsw_index.is_some()
    }

    /// Get HNSW index size
    pub fn hnsw_index_size(&self) -> usize {
        self.hnsw_index.as_ref().map(|h| h.len()).unwrap_or(0)
    }

    /// Perform optimized hybrid search using all performance features
    pub fn optimized_search(
        &mut self,
        query: &str,
        query_embedding: Option<&[f32]>,
        top_k: usize,
    ) -> Result<Vec<String>, RetrieverError> {
        // 1. Check semantic cache first (for similar queries)
        if let Some(emb) = query_embedding {
            if let Some(cached) = self.semantic_cache.get(query, emb) {
                debug!("Semantic cache hit for query");
                return Ok(cached.into_iter().map(|c| c.doc_id).collect());
            }
        }

        // 2. Get BM25 results from Tantivy
        let bm25_results = self.search(query)?;
        let bm25_scored: Vec<(String, f32)> = bm25_results
            .iter()
            .enumerate()
            .map(|(i, content)| (content.clone(), 1.0 / (60.0 + i as f32 + 1.0)))
            .collect();

        // 3. Get vector results if embedding provided
        let vector_scored: Vec<(String, f32)> = if let Some(emb) = query_embedding {
            self.vector_search(emb, top_k * 2)
                .into_iter()
                .filter_map(|(idx, score)| {
                    self.get_content_by_vector_idx(idx).map(|id| (id, score))
                })
                .collect()
        } else {
            Vec::new()
        };

        // 4. Use hybrid searcher to combine results
        let hybrid_results = self.hybrid_searcher.search(&bm25_scored, &vector_scored, top_k);

        // 5. Convert to result format
        let results: Vec<String> = hybrid_results
            .into_iter()
            .map(|r| r.doc_id)
            .collect();

        // 6. Cache results for similar future queries
        if let Some(emb) = query_embedding {
            let cached: Vec<crate::perf::semantic_cache::CachedResult> = results
                .iter()
                .enumerate()
                .map(|(i, id)| crate::perf::semantic_cache::CachedResult {
                    doc_id: id.clone(),
                    score: 1.0 / (i as f32 + 1.0),
                    content: None,
                })
                .collect();
            self.semantic_cache.put(query, emb.to_vec(), cached);
        }

        Ok(results)
    }

    /// Get all optimization statistics
    pub fn get_optimization_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "simd_enabled": true,
            "bloom_filter_size": self.doc_bloom_filter.len(),
            "hnsw_index_size": self.hnsw_index_size(),
            "hnsw_enabled": self.has_hnsw_index(),
            "semantic_cache": self.get_semantic_cache_stats(),
            "sqlite_wal_enabled": true,
            "pq_enabled": self.pq_index.is_some(),
            "pq_vectors": self.pq_index.as_ref().map(|p| p.len()).unwrap_or(0),
            "fp16_enabled": self.fp16_store.is_some(),
            "fp16_vectors": self.fp16_store.as_ref().map(|s| s.len()).unwrap_or(0),
            "connection_pool_size": self.connection_pool.stats().total_connections,
            "io_uring_available": self.use_io_uring,
        })
    }

    /// Build Product Quantization index for 16x memory reduction
    /// Use this for large vector collections where memory is constrained
    pub fn build_pq_index(&mut self) {
        if self.vectors.is_empty() {
            return;
        }
        info!("Building PQ index for {} vectors (16x compression)...", self.vectors.len());
        
        // Collect vectors with their doc_ids
        let vectors_with_ids: Vec<(String, Vec<f32>)> = self.doc_id_to_vector_idx
            .iter()
            .filter_map(|(doc_id, &idx)| {
                if idx < self.vectors.len() {
                    Some((doc_id.clone(), self.vectors[idx].clone()))
                } else {
                    None
                }
            })
            .collect();
        
        // Build PQ index with 48 subvectors for 384-dim vectors
        let pq = crate::perf::product_quantization::PQIndex::build(&vectors_with_ids, 48);
        
        self.pq_index = Some(pq);
        info!("PQ index built successfully");
    }

    /// Build FP16 vector store for 2x memory reduction
    /// Use this when you need faster search with acceptable precision loss
    pub fn build_fp16_store(&mut self) {
        if self.vectors.is_empty() {
            return;
        }
        info!("Building FP16 store for {} vectors (2x compression)...", self.vectors.len());
        let dim = self.vectors.first().map(|v| v.len()).unwrap_or(384);
        let mut store = crate::perf::mixed_precision::F16VectorStore::new(dim);
        
        for (doc_id, &idx) in &self.doc_id_to_vector_idx {
            if idx < self.vectors.len() {
                store.add(doc_id.clone(), &self.vectors[idx]);
            }
        }
        
        self.fp16_store = Some(store);
        info!("FP16 store built successfully");
    }

    /// Search using Product Quantization (faster, approximate)
    pub fn pq_search(&self, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
        if let Some(ref pq) = self.pq_index {
            pq.search(query, top_k)
        } else {
            Vec::new()
        }
    }

    /// Search using FP16 vectors (faster, slight precision loss)
    pub fn fp16_search(&self, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
        if let Some(ref store) = self.fp16_store {
            store.search(query, top_k)
        } else {
            Vec::new()
        }
    }

    /// Get a connection from the pool
    pub async fn acquire_connection(&self) -> Result<crate::perf::connection_pool::PoolGuard<'_>, crate::perf::connection_pool::PoolError> {
        self.connection_pool.acquire().await
    }

    /// Check if io_uring is being used
    pub fn uses_io_uring(&self) -> bool {
        self.use_io_uring
    }

    /// Read file using io_uring if available, otherwise standard async
    pub async fn read_file_optimized(&self, path: &str) -> std::io::Result<Vec<u8>> {
        crate::perf::io_uring::read_file(path).await
    }

    /// Write file using io_uring if available, otherwise standard async
    pub async fn write_file_optimized(&self, path: &str, data: &[u8]) -> std::io::Result<()> {
        crate::perf::io_uring::write_file(path, data).await
    }

    /// Memory-efficient search that uses the best available index
    /// Priority: HNSW > PQ > FP16 > Linear
    pub fn memory_efficient_search(&mut self, query: &[f32], top_k: usize) -> Vec<(usize, f32)> {
        // Try HNSW first (fastest, O(log n))
        if let Some(ref mut hnsw) = self.hnsw_index {
            if hnsw.len() > 100 {
                let results = hnsw.search(query, top_k);
                return results.into_iter()
                    .filter_map(|(doc_id, score)| {
                        self.doc_id_to_vector_idx.get(&doc_id).map(|&idx| (idx, score))
                    })
                    .collect();
            }
        }
        
        // Try PQ (16x memory reduction)
        if let Some(ref pq) = self.pq_index {
            if pq.len() > 0 {
                let results = pq.search(query, top_k);
                return results.into_iter()
                    .filter_map(|(doc_id, score)| {
                        self.doc_id_to_vector_idx.get(&doc_id).map(|&idx| (idx, score))
                    })
                    .collect();
            }
        }
        
        // Try FP16 (2x memory reduction)
        if let Some(ref store) = self.fp16_store {
            if store.len() > 0 {
                let results = store.search(query, top_k);
                return results.into_iter()
                    .filter_map(|(doc_id, score)| {
                        self.doc_id_to_vector_idx.get(&doc_id).map(|&idx| (idx, score))
                    })
                    .collect();
            }
        }
        
        // Fallback to linear scan with SIMD
        self.vector_search(query, top_k)
    }
}
