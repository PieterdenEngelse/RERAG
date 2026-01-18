//! Semantic Query Cache
//! 
//! Caches search results for semantically similar queries.
//! Unlike exact-match caching, this returns cached results
//! when a new query is similar enough to a cached query.
//! 
//! # How it works
//! 1. Compute embedding for incoming query
//! 2. Compare to cached query embeddings
//! 3. If similarity > threshold, return cached results
//! 4. Otherwise, execute search and cache results
//! 
//! # Benefits
//! - Handles paraphrased queries
//! - Reduces embedding + search latency
//! - Configurable similarity threshold

use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use super::cache_aligned::CacheAligned;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Cached query entry
#[derive(Debug, Clone)]
pub struct CachedQuery {
    /// Original query text
    pub query: String,
    /// Query embedding
    pub embedding: Vec<f32>,
    /// Cached search results
    pub results: Vec<CachedResult>,
    /// When this entry was created
    pub created_at: Instant,
    /// Number of times this cache entry was hit
    pub hit_count: u64,
}

/// Cached search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResult {
    pub doc_id: String,
    pub score: f32,
    pub content: Option<String>,
}

/// Semantic cache configuration
#[derive(Debug, Clone)]
pub struct SemanticCacheConfig {
    /// Minimum similarity to consider a cache hit (0.0 to 1.0)
    pub similarity_threshold: f32,
    /// Maximum number of cached queries
    pub max_entries: usize,
    /// Time-to-live for cache entries
    pub ttl: Duration,
    /// Whether to update hit counts
    pub track_hits: bool,
}

impl Default for SemanticCacheConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.92,
            max_entries: 1000,
            ttl: Duration::from_secs(3600), // 1 hour
            track_hits: true,
        }
    }
}

/// Semantic query cache
/// 
/// Stats counters are cache-line aligned to prevent false sharing
/// when concurrent lookups update hits/misses counters.
pub struct SemanticCache {
    /// Cached queries indexed by hash
    cache: DashMap<u64, CachedQuery>,
    /// All embeddings for similarity search
    embeddings: DashMap<u64, Vec<f32>>,
    /// Configuration
    config: SemanticCacheConfig,
    /// Statistics - cache-line aligned
    hits: CacheAligned<AtomicU64>,
    misses: CacheAligned<AtomicU64>,
    semantic_hits: CacheAligned<AtomicU64>,
}

impl SemanticCache {
    pub fn new(config: SemanticCacheConfig) -> Self {
        Self {
            cache: DashMap::new(),
            embeddings: DashMap::new(),
            config,
            hits: CacheAligned::new(AtomicU64::new(0)),
            misses: CacheAligned::new(AtomicU64::new(0)),
            semantic_hits: CacheAligned::new(AtomicU64::new(0)),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(SemanticCacheConfig::default())
    }

    /// Try to get cached results for a query
    /// 
    /// Returns Some if:
    /// 1. Exact query match exists, or
    /// 2. Semantically similar query exists (similarity > threshold)
    pub fn get(&self, query: &str, query_embedding: &[f32]) -> Option<Vec<CachedResult>> {
        let query_hash = self.hash_query(query);
        
        // Check for exact match first
        if let Some(mut entry) = self.cache.get_mut(&query_hash) {
            if entry.created_at.elapsed() < self.config.ttl {
                self.hits.fetch_add(1, Ordering::Relaxed);
                if self.config.track_hits {
                    entry.hit_count += 1;
                }
                return Some(entry.results.clone());
            } else {
                // Expired, remove it
                drop(entry);
                self.cache.remove(&query_hash);
                self.embeddings.remove(&query_hash);
            }
        }

        // Check for semantic match
        if let Some((similar_hash, similarity)) = self.find_similar(query_embedding) {
            if similarity >= self.config.similarity_threshold {
                if let Some(mut entry) = self.cache.get_mut(&similar_hash) {
                    if entry.created_at.elapsed() < self.config.ttl {
                        self.semantic_hits.fetch_add(1, Ordering::Relaxed);
                        self.hits.fetch_add(1, Ordering::Relaxed);
                        if self.config.track_hits {
                            entry.hit_count += 1;
                        }
                        return Some(entry.results.clone());
                    }
                }
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Store results for a query
    pub fn put(&self, query: &str, embedding: Vec<f32>, results: Vec<CachedResult>) {
        // Evict if at capacity
        if self.cache.len() >= self.config.max_entries {
            self.evict_oldest();
        }

        let query_hash = self.hash_query(query);
        
        let entry = CachedQuery {
            query: query.to_string(),
            embedding: embedding.clone(),
            results,
            created_at: Instant::now(),
            hit_count: 0,
        };

        self.cache.insert(query_hash, entry);
        self.embeddings.insert(query_hash, embedding);
    }

    /// Find the most similar cached query
    fn find_similar(&self, query_embedding: &[f32]) -> Option<(u64, f32)> {
        let mut best_hash = 0u64;
        let mut best_similarity = 0.0f32;

        for entry in self.embeddings.iter() {
            let similarity = cosine_similarity(query_embedding, entry.value());
            if similarity > best_similarity {
                best_similarity = similarity;
                best_hash = *entry.key();
            }
        }

        if best_similarity > 0.0 {
            Some((best_hash, best_similarity))
        } else {
            None
        }
    }

    /// Evict the oldest entry
    fn evict_oldest(&self) {
        let mut oldest_hash = 0u64;
        let mut oldest_time = Instant::now();

        for entry in self.cache.iter() {
            if entry.created_at < oldest_time {
                oldest_time = entry.created_at;
                oldest_hash = *entry.key();
            }
        }

        if oldest_hash != 0 {
            self.cache.remove(&oldest_hash);
            self.embeddings.remove(&oldest_hash);
        }
    }

    /// Hash a query string
    fn hash_query(&self, query: &str) -> u64 {
        seahash::hash(query.as_bytes())
    }

    /// Clear the cache
    pub fn clear(&self) {
        self.cache.clear();
        self.embeddings.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.len(),
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            semantic_hits: self.semantic_hits.load(Ordering::Relaxed),
        }
    }

    /// Get hit rate
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// Remove expired entries
    pub fn cleanup_expired(&self) {
        let expired: Vec<u64> = self.cache
            .iter()
            .filter(|e| e.created_at.elapsed() >= self.config.ttl)
            .map(|e| *e.key())
            .collect();

        for hash in expired {
            self.cache.remove(&hash);
            self.embeddings.remove(&hash);
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub semantic_hits: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    pub fn semantic_hit_rate(&self) -> f64 {
        if self.hits == 0 {
            0.0
        } else {
            self.semantic_hits as f64 / self.hits as f64
        }
    }
}

/// Cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_embedding(seed: u64) -> Vec<f32> {
        (0..384).map(|i| ((i as u64 + seed) % 100) as f32 / 100.0).collect()
    }

    #[test]
    fn test_exact_match() {
        let cache = SemanticCache::with_defaults();
        
        let query = "test query";
        let embedding = random_embedding(42);
        let results = vec![CachedResult {
            doc_id: "doc1".to_string(),
            score: 0.9,
            content: None,
        }];
        
        cache.put(query, embedding.clone(), results.clone());
        
        let cached = cache.get(query, &embedding);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);
    }

    #[test]
    fn test_semantic_match() {
        let mut config = SemanticCacheConfig::default();
        config.similarity_threshold = 0.9;
        let cache = SemanticCache::new(config);
        
        let embedding1 = random_embedding(42);
        let results = vec![CachedResult {
            doc_id: "doc1".to_string(),
            score: 0.9,
            content: None,
        }];
        
        cache.put("original query", embedding1.clone(), results);
        
        // Slightly different embedding (should still match)
        let mut embedding2 = embedding1.clone();
        embedding2[0] += 0.01;
        
        let cached = cache.get("different query", &embedding2);
        assert!(cached.is_some());
        
        let stats = cache.stats();
        assert!(stats.semantic_hits > 0 || stats.hits > 0);
    }

    #[test]
    fn test_cache_stats() {
        let cache = SemanticCache::with_defaults();
        
        let embedding = random_embedding(1);
        
        // Miss
        cache.get("query1", &embedding);
        
        // Put and hit
        cache.put("query2", embedding.clone(), vec![]);
        cache.get("query2", &embedding);
        
        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 1);
    }
}
