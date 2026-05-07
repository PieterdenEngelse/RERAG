//! Re-ranking Module
//!
//! Provides re-ranking of search results using more sophisticated
//! scoring methods after initial retrieval.
//!
//! # Strategies
//! 1. Cross-encoder simulation (query-document interaction)
//! 2. BM25 re-scoring
//! 3. Diversity re-ranking (MMR)
//! 4. Recency boosting
//! 5. Popularity boosting

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Search result to be re-ranked
#[derive(Debug, Clone)]
pub struct RerankCandidate {
    pub doc_id: String,
    pub content: String,
    pub initial_score: f32,
    pub embedding: Option<Vec<f32>>,
    pub metadata: CandidateMetadata,
}

/// Metadata for re-ranking decisions
#[derive(Debug, Clone, Default)]
pub struct CandidateMetadata {
    pub created_at: Option<i64>,
    pub view_count: Option<u64>,
    pub word_count: Option<usize>,
}

/// Re-ranked result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResult {
    pub doc_id: String,
    pub final_score: f32,
    pub initial_score: f32,
    pub rerank_factors: RerankFactors,
}

/// Factors that contributed to re-ranking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RerankFactors {
    pub relevance_boost: f32,
    pub diversity_penalty: f32,
    pub recency_boost: f32,
    pub popularity_boost: f32,
    pub length_factor: f32,
}

/// Re-ranking configuration
#[derive(Debug, Clone)]
pub struct RerankConfig {
    /// Weight for initial retrieval score
    pub initial_score_weight: f32,
    /// Weight for query-document relevance
    pub relevance_weight: f32,
    /// Lambda for MMR diversity (0 = pure relevance, 1 = pure diversity)
    pub diversity_lambda: f32,
    /// Boost factor for recent documents
    pub recency_boost: f32,
    /// Decay rate for recency (documents older than this many days get no boost)
    pub recency_decay_days: f32,
    /// Boost factor for popular documents
    pub popularity_boost: f32,
    /// Preferred document length (in words)
    pub preferred_length: usize,
    /// Length penalty factor
    pub length_penalty: f32,
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            initial_score_weight: 0.5,
            relevance_weight: 0.3,
            diversity_lambda: 0.3,
            recency_boost: 0.1,
            recency_decay_days: 30.0,
            popularity_boost: 0.05,
            preferred_length: 500,
            length_penalty: 0.05,
        }
    }
}

/// Re-ranker for search results
pub struct Reranker {
    config: RerankConfig,
}

impl Reranker {
    pub fn new(config: RerankConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(RerankConfig::default())
    }

    /// Re-rank candidates using all factors
    pub fn rerank(
        &self,
        query: &str,
        query_embedding: Option<&[f32]>,
        candidates: Vec<RerankCandidate>,
        top_k: usize,
    ) -> Vec<RerankResult> {
        if candidates.is_empty() {
            return Vec::new();
        }

        // Calculate relevance scores
        let scored: Vec<(RerankCandidate, RerankFactors)> = candidates
            .into_iter()
            .map(|c| {
                let factors = self.calculate_factors(query, query_embedding, &c);
                (c, factors)
            })
            .collect();

        // Apply MMR for diversity

        self.apply_mmr(scored, top_k)
    }

    /// Calculate re-ranking factors for a candidate
    #[allow(clippy::field_reassign_with_default)]
    fn calculate_factors(
        &self,
        query: &str,
        _query_embedding: Option<&[f32]>,
        candidate: &RerankCandidate,
    ) -> RerankFactors {
        let mut factors = RerankFactors::default();

        // Relevance boost (simple term overlap)
        factors.relevance_boost = self.calculate_relevance(query, &candidate.content);

        // Recency boost
        if let Some(created_at) = candidate.metadata.created_at {
            factors.recency_boost = self.calculate_recency_boost(created_at);
        }

        // Popularity boost
        if let Some(views) = candidate.metadata.view_count {
            factors.popularity_boost = self.calculate_popularity_boost(views);
        }

        // Length factor
        if let Some(word_count) = candidate.metadata.word_count {
            factors.length_factor = self.calculate_length_factor(word_count);
        }

        factors
    }

    /// Calculate relevance based on term overlap
    fn calculate_relevance(&self, query: &str, content: &str) -> f32 {
        let query_lower = query.to_lowercase();
        let query_terms: HashSet<&str> = query_lower.split_whitespace().collect();

        let content_lower = content.to_lowercase();
        let content_terms: HashSet<&str> = content_lower.split_whitespace().collect();

        if query_terms.is_empty() {
            return 0.0;
        }

        let overlap = query_terms.intersection(&content_terms).count();
        overlap as f32 / query_terms.len() as f32
    }

    /// Calculate recency boost
    fn calculate_recency_boost(&self, created_at: i64) -> f32 {
        let now = chrono::Utc::now().timestamp();
        let age_days = (now - created_at) as f32 / 86400.0;

        if age_days <= 0.0 {
            self.config.recency_boost
        } else if age_days >= self.config.recency_decay_days {
            0.0
        } else {
            self.config.recency_boost * (1.0 - age_days / self.config.recency_decay_days)
        }
    }

    /// Calculate popularity boost
    fn calculate_popularity_boost(&self, views: u64) -> f32 {
        // Logarithmic scaling
        let log_views = (views as f32 + 1.0).ln();
        (log_views / 10.0).min(1.0) * self.config.popularity_boost
    }

    /// Calculate length factor
    fn calculate_length_factor(&self, word_count: usize) -> f32 {
        let diff = (word_count as f32 - self.config.preferred_length as f32).abs();
        let penalty = (diff / self.config.preferred_length as f32) * self.config.length_penalty;
        -penalty.min(self.config.length_penalty)
    }

    /// Apply Maximal Marginal Relevance for diversity
    fn apply_mmr(
        &self,
        mut candidates: Vec<(RerankCandidate, RerankFactors)>,
        top_k: usize,
    ) -> Vec<RerankResult> {
        let mut results = Vec::with_capacity(top_k);
        let mut selected_embeddings: Vec<Vec<f32>> = Vec::new();

        while results.len() < top_k && !candidates.is_empty() {
            let mut best_idx = 0;
            let mut best_mmr = f32::NEG_INFINITY;

            for (i, (candidate, factors)) in candidates.iter().enumerate() {
                // Calculate base score
                let base_score = candidate.initial_score * self.config.initial_score_weight
                    + factors.relevance_boost * self.config.relevance_weight
                    + factors.recency_boost
                    + factors.popularity_boost
                    + factors.length_factor;

                // Calculate diversity penalty
                let diversity_penalty = if let Some(ref emb) = candidate.embedding {
                    self.max_similarity_to_selected(emb, &selected_embeddings)
                } else {
                    0.0
                };

                // MMR score
                let mmr = (1.0 - self.config.diversity_lambda) * base_score
                    - self.config.diversity_lambda * diversity_penalty;

                if mmr > best_mmr {
                    best_mmr = mmr;
                    best_idx = i;
                }
            }

            let (candidate, mut factors) = candidates.remove(best_idx);

            // Store embedding for diversity calculation
            if let Some(ref emb) = candidate.embedding {
                factors.diversity_penalty =
                    self.max_similarity_to_selected(emb, &selected_embeddings);
                selected_embeddings.push(emb.clone());
            }

            let final_score = candidate.initial_score * self.config.initial_score_weight
                + factors.relevance_boost * self.config.relevance_weight
                + factors.recency_boost
                + factors.popularity_boost
                + factors.length_factor
                - factors.diversity_penalty * self.config.diversity_lambda;

            results.push(RerankResult {
                doc_id: candidate.doc_id,
                final_score,
                initial_score: candidate.initial_score,
                rerank_factors: factors,
            });
        }

        results
    }

    /// Calculate maximum similarity to already selected documents
    fn max_similarity_to_selected(&self, embedding: &[f32], selected: &[Vec<f32>]) -> f32 {
        selected
            .iter()
            .map(|s| cosine_similarity(embedding, s))
            .fold(0.0f32, f32::max)
    }
}

/// Cosine similarity
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

    #[test]
    fn test_reranker_basic() {
        let reranker = Reranker::with_defaults();

        let candidates = vec![
            RerankCandidate {
                doc_id: "doc1".to_string(),
                content: "rust programming language".to_string(),
                initial_score: 0.9,
                embedding: None,
                metadata: CandidateMetadata::default(),
            },
            RerankCandidate {
                doc_id: "doc2".to_string(),
                content: "python programming".to_string(),
                initial_score: 0.8,
                embedding: None,
                metadata: CandidateMetadata::default(),
            },
        ];

        let results = reranker.rerank("rust programming", None, candidates, 10);

        assert_eq!(results.len(), 2);
        // doc1 should rank higher (better term overlap)
        assert_eq!(results[0].doc_id, "doc1");
    }

    #[test]
    fn test_relevance_calculation() {
        let reranker = Reranker::with_defaults();

        let relevance = reranker
            .calculate_relevance("rust programming", "rust is a systems programming language");

        assert!(relevance > 0.5); // Both terms present
    }

    #[test]
    fn test_recency_boost() {
        let reranker = Reranker::with_defaults();

        let now = chrono::Utc::now().timestamp();

        // Recent document
        let recent_boost = reranker.calculate_recency_boost(now - 86400); // 1 day ago

        // Old document
        let old_boost = reranker.calculate_recency_boost(now - 86400 * 60); // 60 days ago

        assert!(recent_boost > old_boost);
    }
}
