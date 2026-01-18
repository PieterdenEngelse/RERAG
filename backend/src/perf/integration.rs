//! Performance Integration Module
//! 
//! Wires all performance optimizations into the main application.
//! This module provides high-level APIs that combine multiple optimizations.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::{
    hybrid_search::{HybridSearcher, HybridSearchConfig},
    semantic_cache::{SemanticCache, SemanticCacheConfig, CachedResult},
    reranking::{Reranker, RerankConfig, RerankCandidate},
    request_coalescing::{RequestCoalescer, Singleflight},
    hnsw::HnswIndex,
    bloom::VectorBloomFilter,
};

/// Integrated search engine with all optimizations
pub struct OptimizedSearchEngine {
    /// Semantic query cache
    pub semantic_cache: SemanticCache,
    /// Hybrid searcher (BM25 + vector)
    pub hybrid_searcher: HybridSearcher,
    /// Re-ranker for result diversity
    pub reranker: Reranker,
    /// Request coalescer for deduplication
    pub request_coalescer: RequestCoalescer<String, Vec<SearchResult>>,
    /// Singleflight for embedding requests
    pub embedding_singleflight: Singleflight<String, Vec<f32>>,
    /// HNSW index for fast ANN search
    pub hnsw_index: Option<HnswIndex>,
    /// Bloom filter for document existence
    pub bloom_filter: VectorBloomFilter,
}

/// Search result
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub doc_id: String,
    pub score: f32,
    pub content: String,
}

impl OptimizedSearchEngine {
    /// Create a new optimized search engine with default settings
    pub fn new() -> Self {
        Self {
            semantic_cache: SemanticCache::new(SemanticCacheConfig {
                similarity_threshold: 0.92,
                max_entries: 1000,
                ttl: std::time::Duration::from_secs(3600),
                track_hits: true,
            }),
            hybrid_searcher: HybridSearcher::new(HybridSearchConfig::default()),
            reranker: Reranker::new(RerankConfig::default()),
            request_coalescer: RequestCoalescer::with_defaults(),
            embedding_singleflight: Singleflight::new(),
            hnsw_index: None,
            bloom_filter: VectorBloomFilter::new(100_000, 0.01),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        cache_config: SemanticCacheConfig,
        hybrid_config: HybridSearchConfig,
        rerank_config: RerankConfig,
    ) -> Self {
        Self {
            semantic_cache: SemanticCache::new(cache_config),
            hybrid_searcher: HybridSearcher::new(hybrid_config),
            reranker: Reranker::new(rerank_config),
            request_coalescer: RequestCoalescer::with_defaults(),
            embedding_singleflight: Singleflight::new(),
            hnsw_index: None,
            bloom_filter: VectorBloomFilter::new(100_000, 0.01),
        }
    }

    /// Initialize HNSW index from vectors
    pub fn init_hnsw(&mut self, vectors: &[(String, Vec<f32>)]) {
        info!("Initializing HNSW index with {} vectors", vectors.len());
        let mut index = HnswIndex::new(vectors.first().map(|(_, v)| v.len()).unwrap_or(384));
        for (doc_id, vector) in vectors {
            index.add(doc_id.clone(), vector.clone());
            self.bloom_filter.insert(doc_id);
        }
        index.build();
        self.hnsw_index = Some(index);
        info!("HNSW index initialized");
    }

    /// Search with all optimizations applied
    pub async fn search(
        &mut self,
        query: &str,
        query_embedding: &[f32],
        bm25_results: &[(String, f32)],
        top_k: usize,
    ) -> Vec<SearchResult> {
        // 1. Check semantic cache first
        if let Some(cached) = self.semantic_cache.get(query, query_embedding) {
            debug!("Semantic cache hit for query");
            return cached.into_iter().map(|c| SearchResult {
                doc_id: c.doc_id,
                score: c.score,
                content: c.content.unwrap_or_default(),
            }).collect();
        }

        // 2. Get vector results (use HNSW if available)
        let vector_results: Vec<(String, f32)> = if let Some(ref mut hnsw) = self.hnsw_index {
            hnsw.search(query_embedding, top_k * 2)
        } else {
            Vec::new()
        };

        // 3. Hybrid search (combine BM25 + vector)
        let hybrid_results = self.hybrid_searcher.search(bm25_results, &vector_results, top_k * 2);

        // 4. Convert to rerank candidates
        let candidates: Vec<RerankCandidate> = hybrid_results.iter().map(|r| {
            RerankCandidate {
                doc_id: r.doc_id.clone(),
                content: String::new(), // Would be filled from retriever
                initial_score: r.hybrid_score,
                embedding: None,
                metadata: Default::default(),
            }
        }).collect();

        // 5. Re-rank for diversity
        let reranked = self.reranker.rerank(query, Some(query_embedding), candidates, top_k);

        // 6. Convert to search results
        let results: Vec<SearchResult> = reranked.into_iter().map(|r| SearchResult {
            doc_id: r.doc_id,
            score: r.final_score,
            content: String::new(),
        }).collect();

        // 7. Cache results
        let cached_results: Vec<CachedResult> = results.iter().map(|r| CachedResult {
            doc_id: r.doc_id.clone(),
            score: r.score,
            content: Some(r.content.clone()),
        }).collect();
        self.semantic_cache.put(query, query_embedding.to_vec(), cached_results);

        results
    }

    /// Check if document might exist (O(1) bloom filter check)
    pub fn might_contain(&self, doc_id: &str) -> bool {
        self.bloom_filter.might_contain(doc_id)
    }

    /// Add document to index
    pub fn add_document(&mut self, doc_id: String, embedding: Vec<f32>) {
        self.bloom_filter.insert(&doc_id);
        if let Some(ref mut hnsw) = self.hnsw_index {
            hnsw.add(doc_id, embedding);
        }
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> super::semantic_cache::CacheStats {
        self.semantic_cache.stats()
    }

    /// Clear caches
    pub fn clear_caches(&self) {
        self.semantic_cache.clear();
    }
}

impl Default for OptimizedSearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Global optimized search engine instance
static SEARCH_ENGINE: std::sync::OnceLock<Arc<RwLock<OptimizedSearchEngine>>> = std::sync::OnceLock::new();

/// Initialize the global search engine
pub fn init_search_engine() -> Arc<RwLock<OptimizedSearchEngine>> {
    SEARCH_ENGINE.get_or_init(|| {
        info!("Initializing optimized search engine");
        Arc::new(RwLock::new(OptimizedSearchEngine::new()))
    }).clone()
}

/// Get the global search engine
pub fn get_search_engine() -> Option<Arc<RwLock<OptimizedSearchEngine>>> {
    SEARCH_ENGINE.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimized_search_engine() {
        let mut engine = OptimizedSearchEngine::new();
        
        // Add some documents
        let vectors = vec![
            ("doc1".to_string(), vec![1.0; 384]),
            ("doc2".to_string(), vec![0.5; 384]),
        ];
        engine.init_hnsw(&vectors);
        
        assert!(engine.might_contain("doc1"));
        assert!(engine.might_contain("doc2"));
    }

    #[tokio::test]
    async fn test_search_with_cache() {
        let mut engine = OptimizedSearchEngine::new();
        
        let query = "test query";
        let embedding = vec![0.5; 384];
        let bm25 = vec![("doc1".to_string(), 1.0)];
        
        // First search - cache miss
        let first = engine.search(query, &embedding, &bm25, 10).await;
        assert!(!first.is_empty());
        
        // Second search - should hit cache and return same docs
        let second = engine.search(query, &embedding, &bm25, 10).await;
        assert_eq!(first, second);
        
        let stats = engine.cache_stats();
        assert!(stats.hits >= 1);
    }
}
