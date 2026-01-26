//! Hybrid Search: BM25 + Vector Search
//!
//! Combines keyword-based BM25 search with semantic vector search
//! for significantly better retrieval quality.
//!
//! # Algorithm
//! Uses Reciprocal Rank Fusion (RRF) to combine results:
//! `score = sum(1 / (k + rank_i))` where k=60 is standard
//!
//! # Benefits
//! - Better recall than either method alone
//! - Handles both exact matches and semantic similarity
//! - Robust to vocabulary mismatch

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Reciprocal Rank Fusion constant (standard value)
const RRF_K: f32 = 60.0;

/// Search result from a single source
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub doc_id: String,
    pub score: f32,
    pub source: SearchSource,
}

/// Source of search result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchSource {
    BM25,
    Vector,
    Hybrid,
}

/// Combined hybrid search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchResult {
    pub doc_id: String,
    pub hybrid_score: f32,
    pub bm25_score: Option<f32>,
    pub vector_score: Option<f32>,
    pub bm25_rank: Option<usize>,
    pub vector_rank: Option<usize>,
}

/// Hybrid search configuration
#[derive(Debug, Clone)]
pub struct HybridSearchConfig {
    /// Weight for BM25 results (0.0 to 1.0)
    pub bm25_weight: f32,
    /// Weight for vector results (0.0 to 1.0)
    pub vector_weight: f32,
    /// RRF constant (default 60)
    pub rrf_k: f32,
    /// Minimum score threshold
    pub min_score: f32,
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            bm25_weight: 0.5,
            vector_weight: 0.5,
            rrf_k: RRF_K,
            min_score: 0.0,
        }
    }
}

impl HybridSearchConfig {
    /// Favor keyword search
    pub fn keyword_heavy() -> Self {
        Self {
            bm25_weight: 0.7,
            vector_weight: 0.3,
            ..Default::default()
        }
    }

    /// Favor semantic search
    pub fn semantic_heavy() -> Self {
        Self {
            bm25_weight: 0.3,
            vector_weight: 0.7,
            ..Default::default()
        }
    }
}

/// Reciprocal Rank Fusion for combining search results
pub fn reciprocal_rank_fusion(
    bm25_results: &[(String, f32)],
    vector_results: &[(String, f32)],
    config: &HybridSearchConfig,
) -> Vec<HybridSearchResult> {
    let mut scores: HashMap<String, HybridSearchResult> = HashMap::new();

    // Process BM25 results
    for (rank, (doc_id, score)) in bm25_results.iter().enumerate() {
        let rrf_score = config.bm25_weight / (config.rrf_k + rank as f32 + 1.0);

        scores
            .entry(doc_id.clone())
            .or_insert_with(|| HybridSearchResult {
                doc_id: doc_id.clone(),
                hybrid_score: 0.0,
                bm25_score: None,
                vector_score: None,
                bm25_rank: None,
                vector_rank: None,
            });

        let entry = scores.get_mut(doc_id).unwrap();
        entry.hybrid_score += rrf_score;
        entry.bm25_score = Some(*score);
        entry.bm25_rank = Some(rank + 1);
    }

    // Process vector results
    for (rank, (doc_id, score)) in vector_results.iter().enumerate() {
        let rrf_score = config.vector_weight / (config.rrf_k + rank as f32 + 1.0);

        scores
            .entry(doc_id.clone())
            .or_insert_with(|| HybridSearchResult {
                doc_id: doc_id.clone(),
                hybrid_score: 0.0,
                bm25_score: None,
                vector_score: None,
                bm25_rank: None,
                vector_rank: None,
            });

        let entry = scores.get_mut(doc_id).unwrap();
        entry.hybrid_score += rrf_score;
        entry.vector_score = Some(*score);
        entry.vector_rank = Some(rank + 1);
    }

    // Sort by hybrid score
    let mut results: Vec<HybridSearchResult> = scores
        .into_values()
        .filter(|r| r.hybrid_score >= config.min_score)
        .collect();

    results.sort_by(|a, b| b.hybrid_score.partial_cmp(&a.hybrid_score).unwrap());
    results
}

/// Linear combination of scores (alternative to RRF)
pub fn linear_combination(
    bm25_results: &[(String, f32)],
    vector_results: &[(String, f32)],
    bm25_weight: f32,
    vector_weight: f32,
) -> Vec<HybridSearchResult> {
    let mut scores: HashMap<String, HybridSearchResult> = HashMap::new();

    // Normalize BM25 scores
    let bm25_max = bm25_results.iter().map(|(_, s)| *s).fold(0.0f32, f32::max);
    let bm25_norm = if bm25_max > 0.0 { bm25_max } else { 1.0 };

    // Normalize vector scores (already 0-1 for cosine similarity)
    let vector_max = vector_results
        .iter()
        .map(|(_, s)| *s)
        .fold(0.0f32, f32::max);
    let vector_norm = if vector_max > 0.0 { vector_max } else { 1.0 };

    for (rank, (doc_id, score)) in bm25_results.iter().enumerate() {
        let normalized = score / bm25_norm;
        scores
            .entry(doc_id.clone())
            .or_insert_with(|| HybridSearchResult {
                doc_id: doc_id.clone(),
                hybrid_score: 0.0,
                bm25_score: None,
                vector_score: None,
                bm25_rank: None,
                vector_rank: None,
            });

        let entry = scores.get_mut(doc_id).unwrap();
        entry.hybrid_score += normalized * bm25_weight;
        entry.bm25_score = Some(*score);
        entry.bm25_rank = Some(rank + 1);
    }

    for (rank, (doc_id, score)) in vector_results.iter().enumerate() {
        let normalized = score / vector_norm;
        scores
            .entry(doc_id.clone())
            .or_insert_with(|| HybridSearchResult {
                doc_id: doc_id.clone(),
                hybrid_score: 0.0,
                bm25_score: None,
                vector_score: None,
                bm25_rank: None,
                vector_rank: None,
            });

        let entry = scores.get_mut(doc_id).unwrap();
        entry.hybrid_score += normalized * vector_weight;
        entry.vector_score = Some(*score);
        entry.vector_rank = Some(rank + 1);
    }

    let mut results: Vec<HybridSearchResult> = scores.into_values().collect();
    results.sort_by(|a, b| b.hybrid_score.partial_cmp(&a.hybrid_score).unwrap());
    results
}

/// Hybrid searcher that combines BM25 and vector search
pub struct HybridSearcher {
    config: HybridSearchConfig,
}

impl HybridSearcher {
    pub fn new(config: HybridSearchConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(HybridSearchConfig::default())
    }

    /// Combine BM25 and vector results using RRF
    pub fn search(
        &self,
        bm25_results: &[(String, f32)],
        vector_results: &[(String, f32)],
        top_k: usize,
    ) -> Vec<HybridSearchResult> {
        let mut results = reciprocal_rank_fusion(bm25_results, vector_results, &self.config);
        results.truncate(top_k);
        results
    }

    /// Update configuration
    pub fn set_weights(&mut self, bm25_weight: f32, vector_weight: f32) {
        self.config.bm25_weight = bm25_weight;
        self.config.vector_weight = vector_weight;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_basic() {
        let bm25 = vec![
            ("doc1".to_string(), 10.0),
            ("doc2".to_string(), 8.0),
            ("doc3".to_string(), 5.0),
        ];

        let vector = vec![
            ("doc2".to_string(), 0.95),
            ("doc1".to_string(), 0.90),
            ("doc4".to_string(), 0.85),
        ];

        let config = HybridSearchConfig::default();
        let results = reciprocal_rank_fusion(&bm25, &vector, &config);

        // doc1 and doc2 should be top (appear in both)
        assert!(results.len() >= 2);

        // Both doc1 and doc2 should have both scores
        let doc1 = results.iter().find(|r| r.doc_id == "doc1").unwrap();
        assert!(doc1.bm25_score.is_some());
        assert!(doc1.vector_score.is_some());
    }

    #[test]
    fn test_hybrid_searcher() {
        let searcher = HybridSearcher::with_defaults();

        let bm25 = vec![("a".to_string(), 1.0), ("b".to_string(), 0.5)];
        let vector = vec![("b".to_string(), 0.9), ("c".to_string(), 0.8)];

        let results = searcher.search(&bm25, &vector, 10);

        // "b" appears in both, should rank high
        assert!(!results.is_empty());
    }

    #[test]
    fn test_linear_combination() {
        let bm25 = vec![("doc1".to_string(), 10.0)];
        let vector = vec![("doc1".to_string(), 0.9)];

        let results = linear_combination(&bm25, &vector, 0.5, 0.5);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, "doc1");
        // Score should be 0.5 * 1.0 + 0.5 * 1.0 = 1.0 (both normalized to 1.0)
        assert!((results[0].hybrid_score - 1.0).abs() < 0.01);
    }
}
