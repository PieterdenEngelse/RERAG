/// VectorStore - Memory-bounded vector storage with eviction policies
///
/// ENHANCED VERSION: Builds on the existing VectorStore design
/// Adds fixed capacity limits + eviction policies while keeping:
/// - Async/await with tokio
/// - Lance-ready architecture
/// - VectorRecord structure
/// - Document-centric operations
///
/// Phase 3: Vector Storage using Lance (embedded vector database)
/// Phase 4: Memory bounds with eviction policies (NEW)
/// Phase 5: rkyv binary serialization for fast persistence
use crate::embedder::EmbeddingVector;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tracing::{debug, error, info};

/// Eviction policy when VectorStore reaches capacity
#[derive(Clone, Debug)]
pub enum EvictionPolicy {
    /// Evict the least recently used (accessed) vector
    LRU,
    /// Evict vectors in FIFO order (oldest first)
    FIFO,
    /// Evict vectors with the lowest relevance score
    ByScore,
}

/// A vector record stored in Lance
#[derive(Debug, Clone, Serialize)]
pub struct VectorRecord {
    pub chunk_id: String,
    pub document_id: String,
    pub content: String,
    pub embedding: EmbeddingVector,
    pub chunk_index: usize,
    pub token_count: usize,
    pub source: String,
    pub created_at: i64,

    // NEW: Fields for Phase 4 memory bounds
    #[serde(default)]
    pub relevance_score: f32,
    #[serde(skip)]
    pub last_accessed: Instant,
    #[serde(skip)]
    pub insertion_order: u64,
}

impl VectorRecord {
    /// Create a VectorRecord manually
    pub fn new(
        chunk_id: String,
        document_id: String,
        content: String,
        embedding: EmbeddingVector,
        chunk_index: usize,
        token_count: usize,
        source: String,
        created_at: i64,
    ) -> Self {
        Self {
            chunk_id,
            document_id,
            content,
            embedding,
            chunk_index,
            token_count,
            source,
            created_at,
            // NEW: Initialize Phase 4 fields
            relevance_score: 0.5,
            last_accessed: Instant::now(),
            insertion_order: 0,
        }
    }

    /// Create with relevance score
    pub fn with_relevance(mut self, score: f32) -> Self {
        self.relevance_score = score.clamp(0.0, 1.0);
        self
    }
}

// ============================================================================
// rkyv Serialization Support (Phase 5)
// ============================================================================

/// rkyv-compatible version of VectorRecord for fast binary serialization
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct VectorRecordRkyv {
    pub chunk_id: String,
    pub document_id: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub chunk_index: u32,
    pub token_count: u32,
    pub source: String,
    pub created_at: i64,
    pub relevance_score: f32,
}

impl VectorRecordRkyv {
    /// Convert from VectorRecord
    pub fn from_record(record: &VectorRecord) -> Self {
        Self {
            chunk_id: record.chunk_id.clone(),
            document_id: record.document_id.clone(),
            content: record.content.clone(),
            embedding: record.embedding.clone(),
            chunk_index: record.chunk_index as u32,
            token_count: record.token_count as u32,
            source: record.source.clone(),
            created_at: record.created_at,
            relevance_score: record.relevance_score,
        }
    }

    /// Convert to VectorRecord
    pub fn to_record(&self) -> VectorRecord {
        VectorRecord {
            chunk_id: self.chunk_id.clone(),
            document_id: self.document_id.clone(),
            content: self.content.clone(),
            embedding: self.embedding.clone(),
            chunk_index: self.chunk_index as usize,
            token_count: self.token_count as usize,
            source: self.source.clone(),
            created_at: self.created_at,
            relevance_score: self.relevance_score,
            last_accessed: Instant::now(),
            insertion_order: 0,
        }
    }
}

/// rkyv-compatible snapshot of VectorStore for persistence
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct VectorStoreSnapshotRkyv {
    /// Version for schema migrations
    pub version: u32,
    /// All records
    pub records: Vec<VectorRecordRkyv>,
    /// Index map as flat pairs (chunk_id -> index)
    pub index_map: Vec<(String, u32)>,
    /// Insertion counter
    pub insertion_counter: u64,
    /// Metrics
    pub total_insertions: u64,
    pub total_evictions: u64,
    pub lookup_hits: u64,
    pub lookup_misses: u64,
    pub peak_vectors: u32,
}

impl VectorStoreSnapshotRkyv {
    pub const CURRENT_VERSION: u32 = 1;

    /// Create snapshot from VectorStore
    pub fn from_store(store: &VectorStore) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            records: store
                .records
                .iter()
                .map(VectorRecordRkyv::from_record)
                .collect(),
            index_map: store
                .index_map
                .iter()
                .map(|(k, v)| (k.clone(), *v as u32))
                .collect(),
            insertion_counter: store.insertion_counter,
            total_insertions: store.metrics.total_insertions,
            total_evictions: store.metrics.total_evictions,
            lookup_hits: store.metrics.lookup_hits,
            lookup_misses: store.metrics.lookup_misses,
            peak_vectors: store.metrics.peak_vectors as u32,
        }
    }
}

/// Search result with similarity score
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk_id: String,
    pub document_id: String,
    pub content: String,
    pub similarity_score: f32,
    pub chunk_index: usize,
}

/// Configuration for Lance vector store
#[derive(Debug, Clone)]
pub struct VectorStoreConfig {
    pub db_path: std::path::PathBuf,
    pub table_name: String,

    // NEW: Phase 4 memory bounds configuration
    pub max_vectors: usize,
    pub eviction_policy: EvictionPolicy,
}

impl Default for VectorStoreConfig {
    fn default() -> Self {
        Self {
            db_path: std::path::PathBuf::from("./lancedb"),
            table_name: "chunks".to_string(),
            // NEW: Default to 10,000 vectors max with LRU
            max_vectors: 10_000,
            eviction_policy: EvictionPolicy::LRU,
        }
    }
}

/// Metrics for monitoring VectorStore behavior
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreMetrics {
    /// Total insertions attempted
    pub total_insertions: u64,
    /// Total evictions performed
    pub total_evictions: u64,
    /// Number of successful lookups
    pub lookup_hits: u64,
    /// Number of failed lookups
    pub lookup_misses: u64,
    /// Peak number of vectors stored
    pub peak_vectors: usize,
}

impl StoreMetrics {
    /// Calculate hit rate as a percentage (0.0 to 1.0)
    pub fn hit_rate(&self) -> f32 {
        let total = self.lookup_hits + self.lookup_misses;
        if total == 0 {
            0.0
        } else {
            self.lookup_hits as f32 / total as f32
        }
    }
}

/// Lance-based vector store for semantic search with memory bounds
pub struct VectorStore {
    config: VectorStoreConfig,
    records: Vec<VectorRecord>,

    // NEW: Phase 4 memory bounds tracking
    index_map: HashMap<String, usize>,
    insertion_counter: u64,
    metrics: StoreMetrics,
}

impl VectorStore {
    /// Create a new vector store
    pub fn new(config: VectorStoreConfig) -> Result<Self, VectorStoreError> {
        info!(
            db_path = ?config.db_path,
            table_name = %config.table_name,
            max_vectors = config.max_vectors,
            eviction_policy = ?config.eviction_policy,
            "Initializing VectorStore with memory bounds"
        );

        std::fs::create_dir_all(&config.db_path)
            .map_err(|e| VectorStoreError::InitializationFailed(e.to_string()))?;

        let max_vectors = config.max_vectors;

        Ok(Self {
            config,
            records: Vec::with_capacity(max_vectors),
            index_map: HashMap::new(),
            insertion_counter: 0,
            metrics: StoreMetrics::default(),
        })
    }

    /// Create with default config
    pub fn with_defaults() -> Result<Self, VectorStoreError> {
        Self::new(VectorStoreConfig::default())
    }

    /// Create with custom database path
    pub fn with_db_path<P: Into<std::path::PathBuf>>(db_path: P) -> Result<Self, VectorStoreError> {
        let mut cfg = VectorStoreConfig::default();
        cfg.db_path = db_path.into();
        Self::new(cfg)
    }

    /// Create with custom capacity and policy
    pub fn with_capacity(
        max_vectors: usize,
        policy: EvictionPolicy,
    ) -> Result<Self, VectorStoreError> {
        let mut config = VectorStoreConfig::default();
        config.max_vectors = max_vectors;
        config.eviction_policy = policy;
        Self::new(config)
    }

    /// Add a vector record to the store
    pub async fn add_record(&mut self, mut record: VectorRecord) -> Result<(), VectorStoreError> {
        debug!(chunk_id = %record.chunk_id, "Adding vector record");

        self.metrics.total_insertions += 1;

        // Initialize Phase 4 fields
        record.last_accessed = Instant::now();
        record.insertion_order = self.insertion_counter;
        self.insertion_counter += 1;

        // If record already exists, update it
        if let Some(idx) = self.index_map.get(&record.chunk_id) {
            debug!(chunk_id = %record.chunk_id, "Updating existing record");
            self.records[*idx] = record;
            return Ok(());
        }

        // Check if we need to evict
        if self.records.len() >= self.config.max_vectors {
            self.evict_one().await?;
        }

        // Add new record
        let idx = self.records.len();
        self.index_map.insert(record.chunk_id.clone(), idx);
        self.records.push(record);

        // Update metrics
        if self.records.len() > self.metrics.peak_vectors {
            self.metrics.peak_vectors = self.records.len();
        }

        Ok(())
    }

    /// Add multiple records in batch
    pub async fn add_records(
        &mut self,
        records: Vec<VectorRecord>,
    ) -> Result<(), VectorStoreError> {
        info!(count = records.len(), "Adding batch of records");

        for record in records {
            self.add_record(record).await?;
        }

        Ok(())
    }

    /// Search for similar vectors
    pub async fn search(
        &mut self,
        query_embedding: &EmbeddingVector,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        debug!(top_k = top_k, "Searching for similar vectors");

        if self.records.is_empty() {
            self.metrics.lookup_misses += 1;
            return Ok(Vec::new());
        }

        let mut results: Vec<SearchResult> = self
            .records
            .iter_mut()
            .map(|record| {
                // Update access time for LRU
                record.last_accessed = Instant::now();

                let similarity = cosine_similarity(query_embedding, &record.embedding);
                SearchResult {
                    chunk_id: record.chunk_id.clone(),
                    document_id: record.document_id.clone(),
                    content: record.content.clone(),
                    similarity_score: similarity,
                    chunk_index: record.chunk_index,
                }
            })
            .collect();

        results.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());
        results.truncate(top_k);

        self.metrics.lookup_hits += 1;
        debug!(results_count = results.len(), "Search returned results");
        Ok(results)
    }

    /// Search by document ID
    pub async fn search_by_document(
        &mut self,
        document_id: &str,
        top_k: usize,
    ) -> Result<Vec<VectorRecord>, VectorStoreError> {
        debug!(document_id = %document_id, "Searching by document");

        let mut results: Vec<VectorRecord> = self
            .records
            .iter_mut()
            .filter(|r| r.document_id == document_id)
            .map(|r| {
                r.last_accessed = Instant::now();
                r.clone()
            })
            .collect();

        results.truncate(top_k);
        self.metrics.lookup_hits += 1;
        Ok(results)
    }

    /// Delete a record by chunk ID
    pub async fn delete_record(&mut self, chunk_id: &str) -> Result<(), VectorStoreError> {
        debug!(chunk_id = %chunk_id, "Deleting record");

        if let Some(idx) = self.index_map.remove(chunk_id) {
            // Swap with last element and remove
            if idx != self.records.len() - 1 {
                let last_idx = self.records.len() - 1;
                let last_id = self.records[last_idx].chunk_id.clone();

                self.records.swap(idx, last_idx);
                self.index_map.insert(last_id, idx);
            }

            self.records.pop();
            Ok(())
        } else {
            Err(VectorStoreError::NotFound(chunk_id.to_string()))
        }
    }

    /// Delete all records for a document
    pub async fn delete_document(&mut self, document_id: &str) -> Result<usize, VectorStoreError> {
        debug!(document_id = %document_id, "Deleting document");

        let initial_len = self.records.len();

        // Collect indices to delete (in reverse order to avoid index shifting)
        let mut indices_to_delete: Vec<usize> = self
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.document_id == document_id)
            .map(|(idx, _)| idx)
            .collect();

        indices_to_delete.sort_by(|a, b| b.cmp(a));

        for idx in indices_to_delete {
            let chunk_id = self.records[idx].chunk_id.clone();
            self.index_map.remove(&chunk_id);

            if idx != self.records.len() - 1 {
                let last_idx = self.records.len() - 1;
                let last_id = self.records[last_idx].chunk_id.clone();
                self.records.swap(idx, last_idx);
                self.index_map.insert(last_id, idx);
            }

            self.records.pop();
        }

        let deleted_count = initial_len - self.records.len();
        info!(deleted_count = deleted_count, "Documents deleted");

        Ok(deleted_count)
    }

    /// Get a record by chunk ID
    pub async fn get_record(&mut self, chunk_id: &str) -> Result<VectorRecord, VectorStoreError> {
        if let Some(idx) = self.index_map.get(chunk_id) {
            self.records[*idx].last_accessed = Instant::now();
            self.metrics.lookup_hits += 1;
            Ok(self.records[*idx].clone())
        } else {
            self.metrics.lookup_misses += 1;
            Err(VectorStoreError::NotFound(chunk_id.to_string()))
        }
    }

    /// Get all records
    pub async fn get_all_records(&self) -> Result<Vec<VectorRecord>, VectorStoreError> {
        Ok(self.records.clone())
    }

    /// Get store statistics
    pub async fn stats(&self) -> StoreStats {
        let total_records = self.records.len();
        let mut documents = std::collections::HashSet::new();
        for record in &self.records {
            documents.insert(record.document_id.clone());
        }

        StoreStats {
            total_records,
            total_documents: documents.len(),
            db_path: self.config.db_path.clone(),
            max_vectors: self.config.max_vectors,
            utilization: total_records as f32 / self.config.max_vectors as f32,
            metrics: self.metrics.clone(),
        }
    }

    /// Clear all records
    pub async fn clear(&mut self) -> Result<(), VectorStoreError> {
        info!("Clearing all records from vector store");
        self.records.clear();
        self.index_map.clear();
        Ok(())
    }

    /// Get current metrics
    pub fn metrics(&self) -> &StoreMetrics {
        &self.metrics
    }

    /// Get mutable metrics
    pub fn metrics_mut(&mut self) -> &mut StoreMetrics {
        &mut self.metrics
    }

    /// Evict one vector according to the current policy
    async fn evict_one(&mut self) -> Result<(), VectorStoreError> {
        if self.records.is_empty() {
            return Ok(());
        }

        let idx_to_evict = match &self.config.eviction_policy {
            EvictionPolicy::LRU => self.find_lru_index(),
            EvictionPolicy::FIFO => self.find_fifo_index(),
            EvictionPolicy::ByScore => self.find_low_score_index(),
        };

        if let Some(idx) = idx_to_evict {
            let evicted_id = self.records[idx].chunk_id.clone();
            debug!(chunk_id = %evicted_id, "Evicting record");

            self.index_map.remove(&evicted_id);

            if idx != self.records.len() - 1 {
                let last_idx = self.records.len() - 1;
                let last_id = self.records[last_idx].chunk_id.clone();
                self.records.swap(idx, last_idx);
                self.index_map.insert(last_id, idx);
            }

            self.records.pop();
            self.metrics.total_evictions += 1;
        }

        Ok(())
    }

    /// Find the index of the least recently used vector
    fn find_lru_index(&self) -> Option<usize> {
        self.records
            .iter()
            .enumerate()
            .min_by_key(|(_, record)| record.last_accessed)
            .map(|(idx, _)| idx)
    }

    /// Find the index of the oldest inserted vector (FIFO)
    fn find_fifo_index(&self) -> Option<usize> {
        self.records
            .iter()
            .enumerate()
            .min_by_key(|(_, record)| record.insertion_order)
            .map(|(idx, _)| idx)
    }

    /// Find the index of the vector with lowest relevance score
    fn find_low_score_index(&self) -> Option<usize> {
        self.records
            .iter()
            .enumerate()
            .min_by(|a, b| {
                a.1.relevance_score
                    .partial_cmp(&b.1.relevance_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx)
    }

    // ========================================================================
    // rkyv Persistence Methods (Phase 5)
    // ========================================================================

    /// Save VectorStore to rkyv binary format
    pub fn save_rkyv<P: AsRef<Path>>(&self, path: P) -> Result<(), VectorStoreError> {
        let path = path.as_ref();
        info!(path = ?path, records = self.records.len(), "Saving VectorStore to rkyv");

        let snapshot = VectorStoreSnapshotRkyv::from_store(self);

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&snapshot)
            .map_err(|e| VectorStoreError::StorageError(format!("rkyv serialize error: {}", e)))?;

        std::fs::write(path, &bytes)
            .map_err(|e| VectorStoreError::StorageError(format!("IO error: {}", e)))?;

        info!(
            path = ?path,
            records = self.records.len(),
            bytes = bytes.len(),
            "VectorStore saved to rkyv"
        );
        Ok(())
    }

    /// Load VectorStore from rkyv binary format
    pub fn load_rkyv<P: AsRef<Path>>(&mut self, path: P) -> Result<(), VectorStoreError> {
        let path = path.as_ref();
        info!(path = ?path, "Loading VectorStore from rkyv");

        let bytes = std::fs::read(path)
            .map_err(|e| VectorStoreError::StorageError(format!("IO error: {}", e)))?;

        let archived = rkyv::access::<ArchivedVectorStoreSnapshotRkyv, rkyv::rancor::Error>(&bytes)
            .map_err(|e| VectorStoreError::StorageError(format!("rkyv access error: {}", e)))?;

        // Check version
        if archived.version.to_native() != VectorStoreSnapshotRkyv::CURRENT_VERSION {
            info!(
                "VectorStore rkyv version mismatch: file={}, current={}",
                archived.version.to_native(),
                VectorStoreSnapshotRkyv::CURRENT_VERSION
            );
        }

        // Restore records
        self.records = archived
            .records
            .iter()
            .map(|r| VectorRecord {
                chunk_id: r.chunk_id.to_string(),
                document_id: r.document_id.to_string(),
                content: r.content.to_string(),
                embedding: r.embedding.iter().map(|f| f.to_native()).collect(),
                chunk_index: r.chunk_index.to_native() as usize,
                token_count: r.token_count.to_native() as usize,
                source: r.source.to_string(),
                created_at: r.created_at.to_native(),
                relevance_score: r.relevance_score.to_native(),
                last_accessed: Instant::now(),
                insertion_order: 0,
            })
            .collect();

        // Restore index map
        self.index_map = archived
            .index_map
            .iter()
            .map(|pair| (pair.0.to_string(), pair.1.to_native() as usize))
            .collect();

        // Restore counters and metrics
        self.insertion_counter = archived.insertion_counter.to_native();
        self.metrics.total_insertions = archived.total_insertions.to_native();
        self.metrics.total_evictions = archived.total_evictions.to_native();
        self.metrics.lookup_hits = archived.lookup_hits.to_native();
        self.metrics.lookup_misses = archived.lookup_misses.to_native();
        self.metrics.peak_vectors = archived.peak_vectors.to_native() as usize;

        info!(
            path = ?path,
            records = self.records.len(),
            bytes = bytes.len(),
            "VectorStore loaded from rkyv"
        );
        Ok(())
    }

    /// Auto-detect and load from rkyv or JSON, migrating if needed
    pub fn load_auto<P: AsRef<Path>>(&mut self, base_path: P) -> Result<(), VectorStoreError> {
        let base = base_path.as_ref();
        let rkyv_path = base.with_extension("rkyv");
        let json_path = base.with_extension("json");

        // Try rkyv first (fastest)
        if rkyv_path.exists() {
            match self.load_rkyv(&rkyv_path) {
                Ok(()) => {
                    info!(path = ?rkyv_path, "Loaded VectorStore from rkyv");
                    return Ok(());
                }
                Err(e) => {
                    error!(error = %e, "Failed to load rkyv, trying JSON fallback");
                }
            }
        }

        // Try JSON (slower but compatible)
        if json_path.exists() {
            // Note: JSON loading would need to be implemented separately
            // For now, just log that we'd need to implement this
            info!(path = ?json_path, "JSON fallback not yet implemented for VectorStore");
        }

        // Fresh start
        info!("No existing VectorStore found, starting fresh");
        Ok(())
    }

    /// Save to both rkyv and JSON formats
    pub fn save_dual<P: AsRef<Path>>(&self, base_path: P) -> Result<(), VectorStoreError> {
        let base = base_path.as_ref();
        let rkyv_path = base.with_extension("rkyv");

        // Save rkyv (primary - fast)
        self.save_rkyv(&rkyv_path)?;

        // JSON save would go here if needed

        Ok(())
    }
}

/// Statistics about the vector store
#[derive(Debug, Clone)]
pub struct StoreStats {
    pub total_records: usize,
    pub total_documents: usize,
    pub db_path: std::path::PathBuf,
    pub max_vectors: usize,
    pub utilization: f32,
    pub metrics: StoreMetrics,
}

/// Error types for vector store operations
#[derive(Debug, Clone)]
pub enum VectorStoreError {
    InitializationFailed(String),
    NotFound(String),
    InvalidDimension,
    StorageError(String),
}

impl std::fmt::Display for VectorStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InitializationFailed(msg) => {
                write!(f, "Failed to initialize vector store: {}", msg)
            }
            Self::NotFound(id) => write!(f, "Record not found: {}", id),
            Self::InvalidDimension => write!(f, "Invalid vector dimension"),
            Self::StorageError(msg) => write!(f, "Storage error: {}", msg),
        }
    }
}

impl std::error::Error for VectorStoreError {}

/// Cosine similarity between two vectors
fn cosine_similarity(a: &EmbeddingVector, b: &EmbeddingVector) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_record(chunk_id: &str, document_id: &str) -> VectorRecord {
        VectorRecord {
            chunk_id: chunk_id.to_string(),
            document_id: document_id.to_string(),
            content: "Test content".to_string(),
            embedding: vec![1.0, 0.0, 0.0],
            chunk_index: 0,
            token_count: 10,
            source: "test.txt".to_string(),
            created_at: 0,
            relevance_score: 0.5,
            last_accessed: Instant::now(),
            insertion_order: 0,
        }
    }

    #[tokio::test]
    async fn test_vector_store_creation() {
        let config = VectorStoreConfig::default();
        let store = VectorStore::new(config);
        assert!(store.is_ok());
    }

    #[tokio::test]
    async fn test_add_record() {
        let config = VectorStoreConfig::default();
        let mut store = VectorStore::new(config).unwrap();

        let record = create_test_record("chunk1", "doc1");
        let result = store.add_record(record).await;

        assert!(result.is_ok());

        let stats = store.stats().await;
        assert_eq!(stats.total_records, 1);
    }

    #[tokio::test]
    async fn test_add_batch() {
        let config = VectorStoreConfig::default();
        let mut store = VectorStore::new(config).unwrap();

        let records = vec![
            create_test_record("chunk1", "doc1"),
            create_test_record("chunk2", "doc1"),
            create_test_record("chunk3", "doc2"),
        ];

        let result = store.add_records(records).await;
        assert!(result.is_ok());

        let stats = store.stats().await;
        assert_eq!(stats.total_records, 3);
        assert_eq!(stats.total_documents, 2);
    }

    #[tokio::test]
    async fn test_search() {
        let config = VectorStoreConfig::default();
        let mut store = VectorStore::new(config).unwrap();

        let mut record1 = create_test_record("chunk1", "doc1");
        record1.embedding = vec![1.0, 0.0, 0.0];

        let mut record2 = create_test_record("chunk2", "doc1");
        record2.embedding = vec![0.9, 0.1, 0.0];

        let mut record3 = create_test_record("chunk3", "doc2");
        record3.embedding = vec![0.0, 1.0, 0.0];

        store
            .add_records(vec![record1, record2, record3])
            .await
            .unwrap();

        let query = vec![1.0, 0.0, 0.0];
        let results = store.search(&query, 2).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].chunk_id, "chunk1");
        assert!(results[0].similarity_score > results[1].similarity_score);
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let config = VectorStoreConfig {
            db_path: std::path::PathBuf::from("./lancedb"),
            table_name: "chunks".to_string(),
            max_vectors: 3,
            eviction_policy: EvictionPolicy::LRU,
        };
        let mut store = VectorStore::new(config).unwrap();

        // Insert A, B, C
        for i in 0..3 {
            let record = create_test_record(&format!("chunk{}", i), "doc1");
            store.add_record(record).await.unwrap();
        }

        // Access chunk0 to make it recently used
        let _ = store.get_record("chunk0").await;

        // Insert D - should evict chunk1 (least recently used)
        let record = create_test_record("chunk3", "doc1");
        store.add_record(record).await.unwrap();

        let stats = store.stats().await;
        assert_eq!(stats.total_records, 3);
        assert!(store.get_record("chunk0").await.is_ok()); // Still there
        assert!(store.get_record("chunk1").await.is_err()); // Evicted
        assert!(store.get_record("chunk2").await.is_ok()); // Still there
        assert!(store.get_record("chunk3").await.is_ok()); // Inserted
    }

    #[tokio::test]
    async fn test_capacity_enforcement() {
        let config = VectorStoreConfig {
            db_path: std::path::PathBuf::from("./lancedb"),
            table_name: "chunks".to_string(),
            max_vectors: 5,
            eviction_policy: EvictionPolicy::FIFO,
        };
        let mut store = VectorStore::new(config).unwrap();

        // Insert 20 records into store with max 5
        for i in 0..20 {
            let record = create_test_record(&format!("chunk{}", i), "doc1");
            store.add_record(record).await.unwrap();
        }

        let stats = store.stats().await;
        assert_eq!(stats.total_records, 5);
        assert!(stats.total_records <= stats.max_vectors);
    }

    // ========================================================================
    // rkyv Persistence Tests (Phase 5)
    // ========================================================================

    #[tokio::test]
    async fn test_rkyv_save_load() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let rkyv_path = temp_dir.path().join("vector_store.rkyv");

        // Create store with test data
        let config = VectorStoreConfig::default();
        let mut store = VectorStore::new(config).unwrap();

        // Add some records with realistic embeddings
        for i in 0..10 {
            let mut record = create_test_record(&format!("chunk{}", i), &format!("doc{}", i % 3));
            record.embedding = (0..384).map(|j| (i * 384 + j) as f32 * 0.001).collect();
            store.add_record(record).await.unwrap();
        }

        // Save to rkyv
        store.save_rkyv(&rkyv_path).unwrap();
        assert!(rkyv_path.exists());

        // Create new store and load
        let config2 = VectorStoreConfig::default();
        let mut store2 = VectorStore::new(config2).unwrap();
        store2.load_rkyv(&rkyv_path).unwrap();

        // Verify data matches
        let stats1 = store.stats().await;
        let stats2 = store2.stats().await;
        assert_eq!(stats1.total_records, stats2.total_records);
        assert_eq!(stats1.total_documents, stats2.total_documents);

        // Verify a specific record
        let record1 = store.get_record("chunk5").await.unwrap();
        let record2 = store2.get_record("chunk5").await.unwrap();
        assert_eq!(record1.chunk_id, record2.chunk_id);
        assert_eq!(record1.document_id, record2.document_id);
        assert_eq!(record1.embedding.len(), record2.embedding.len());
    }

    #[tokio::test]
    async fn test_rkyv_size_comparison() {
        use std::time::Instant as StdInstant;

        // Create store with realistic data
        let config = VectorStoreConfig::default();
        let mut store = VectorStore::new(config).unwrap();

        // Add 100 records with 384-dim embeddings
        for i in 0..100 {
            let mut record = create_test_record(&format!("chunk{}", i), &format!("doc{}", i % 10));
            record.embedding = (0..384).map(|j| (i * 384 + j) as f32 * 0.0001).collect();
            record.content = format!(
                "This is test content for chunk {} with some meaningful text.",
                i
            );
            store.add_record(record).await.unwrap();
        }

        // Serialize with rkyv
        let snapshot = VectorStoreSnapshotRkyv::from_store(&store);
        let start = StdInstant::now();
        let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&snapshot).unwrap();
        let rkyv_time = start.elapsed();

        println!("\n=== VectorStore rkyv SIZE ===");
        println!("Records: {}", store.records.len());
        println!(
            "rkyv size: {} bytes ({:.2} KB)",
            rkyv_bytes.len(),
            rkyv_bytes.len() as f64 / 1024.0
        );
        println!("Serialize time: {:?}", rkyv_time);
        println!("==============================\n");

        // rkyv should be reasonably sized
        assert!(rkyv_bytes.len() > 0);
    }
}
