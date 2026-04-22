//! HNSW (Hierarchical Navigable Small World) Index
//!
//! Provides O(log n) approximate nearest neighbor search instead of O(n) linear scan.
//! This is a game-changer for large vector collections.
//!
//! # Performance
//! - Build time: O(n log n)
//! - Search time: O(log n)
//! - Memory: ~1.5x the vector data
//! - Recall: >95% at default settings

use crate::config::DistanceMetric;
use instant_distance::{Builder, HnswMap, Search};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

/// HNSW Index for fast approximate nearest neighbor search
pub struct HnswIndex {
    map: Option<HnswMap<Point, String>>,
    doc_vectors: HashMap<String, Vec<f32>>,
    dimension: usize,
    dirty: bool,
    pub metric: DistanceMetric,
    pub ef_construction: usize,
    pub ef_search: usize,
}

/// Point wrapper — carries the distance metric so instant-distance dispatches correctly.
#[derive(Clone)]
struct Point {
    data: Vec<f32>,
    metric: DistanceMetric,
}

impl instant_distance::Point for Point {
    fn distance(&self, other: &Self) -> f32 {
        match self.metric {
            DistanceMetric::Cosine => {
                let dot: f32 = self.data.iter().zip(other.data.iter()).map(|(a, b)| a * b).sum();
                let na: f32 = self.data.iter().map(|x| x * x).sum::<f32>().sqrt();
                let nb: f32 = other.data.iter().map(|x| x * x).sum::<f32>().sqrt();
                if na == 0.0 || nb == 0.0 { 1.0 } else { 1.0 - (dot / (na * nb)) }
            }
            DistanceMetric::DotProduct => {
                let dot: f32 = self.data.iter().zip(other.data.iter()).map(|(a, b)| a * b).sum();
                1.0 - dot // distance = 1 - similarity
            }
            DistanceMetric::Euclidean => {
                self.data.iter().zip(other.data.iter())
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum::<f32>()
                    .sqrt()
            }
        }
    }
}

impl HnswIndex {
    pub fn new(dimension: usize) -> Self {
        Self::with_params(dimension, DistanceMetric::Cosine, 100, 100)
    }

    pub fn with_params(
        dimension: usize,
        metric: DistanceMetric,
        ef_construction: usize,
        ef_search: usize,
    ) -> Self {
        Self {
            map: None,
            doc_vectors: HashMap::new(),
            dimension,
            dirty: false,
            metric,
            ef_construction,
            ef_search,
        }
    }

    pub fn from_vectors(vectors: &[(String, Vec<f32>)]) -> Self {
        let dimension = vectors.first().map(|(_, v)| v.len()).unwrap_or(384);
        let mut index = Self::new(dimension);
        for (doc_id, vector) in vectors {
            index.add(doc_id.clone(), vector.clone());
        }
        index.build();
        index
    }

    /// Add a vector to the index
    pub fn add(&mut self, doc_id: String, vector: Vec<f32>) {
        self.doc_vectors.insert(doc_id, vector);
        self.dirty = true;
    }

    /// Remove a vector from the index
    pub fn remove(&mut self, doc_id: &str) -> bool {
        if self.doc_vectors.remove(doc_id).is_some() {
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// Build or rebuild the HNSW graph
    pub fn build(&mut self) {
        if self.doc_vectors.is_empty() {
            self.map = None;
            self.dirty = false;
            return;
        }

        info!(
            vectors = self.doc_vectors.len(),
            dimension = self.dimension,
            "Building HNSW index"
        );

        let metric = self.metric;
        let points: Vec<Point> = self
            .doc_vectors
            .values()
            .map(|vec| Point { data: vec.clone(), metric })
            .collect();

        let values: Vec<String> = self.doc_vectors.keys().cloned().collect();

        let hnsw = Builder::default()
            .ef_construction(self.ef_construction)
            .ef_search(self.ef_search)
            .build(points, values);
        self.map = Some(hnsw);
        self.dirty = false;

        info!(vectors = self.doc_vectors.len(), "HNSW index built");
    }

    /// Ensure index is up to date
    pub fn ensure_built(&mut self) {
        if self.dirty {
            self.build();
        }
    }

    /// Search for k nearest neighbors
    ///
    /// Returns (doc_id, similarity_score) pairs sorted by similarity (highest first)
    pub fn search(&mut self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        self.ensure_built();

        let map = match &self.map {
            Some(m) => m,
            None => return Vec::new(),
        };

        let query_point = Point { data: query.to_vec(), metric: self.metric };
        let mut search = Search::default();

        let results: Vec<(String, f32)> = map
            .search(&query_point, &mut search)
            .take(k)
            .map(|item| {
                let doc_id = item.value.clone();
                let distance = item.distance;
                let similarity = 1.0 - distance; // Convert distance back to similarity
                (doc_id, similarity)
            })
            .collect();

        debug!(
            query_dim = query.len(),
            k = k,
            results = results.len(),
            "HNSW search completed"
        );

        results
    }

    /// Get vector by document ID
    pub fn get(&self, doc_id: &str) -> Option<&Vec<f32>> {
        self.doc_vectors.get(doc_id)
    }

    /// Number of vectors in the index
    pub fn len(&self) -> usize {
        self.doc_vectors.len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.doc_vectors.is_empty()
    }

    /// Check if index needs rebuilding
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Get all document IDs
    pub fn doc_ids(&self) -> Vec<&String> {
        self.doc_vectors.keys().collect()
    }

    /// Clear the index
    pub fn clear(&mut self) {
        self.doc_vectors.clear();
        self.map = None;
        self.dirty = false;
    }

    /// Memory usage estimate in bytes
    pub fn memory_bytes(&self) -> usize {
        let vector_bytes = self
            .doc_vectors
            .values()
            .map(|v| v.len() * 4)
            .sum::<usize>();

        // HNSW graph adds ~50% overhead
        (vector_bytes as f64 * 1.5) as usize
    }
}

/// Serializable HNSW index state for persistence
#[derive(Serialize, Deserialize)]
pub struct HnswIndexState {
    pub doc_vectors: Vec<(String, Vec<f32>)>,
    pub dimension: usize,
}

impl From<&HnswIndex> for HnswIndexState {
    fn from(index: &HnswIndex) -> Self {
        Self {
            doc_vectors: index
                .doc_vectors
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            dimension: index.dimension,
        }
    }
}

impl From<HnswIndexState> for HnswIndex {
    fn from(state: HnswIndexState) -> Self {
        let mut index = HnswIndex::new(state.dimension);
        for (doc_id, vector) in state.doc_vectors {
            index.add(doc_id, vector);
        }
        index.build();
        index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
        (0..dim)
            .map(|i| ((i as u64 + seed) % 100) as f32 / 100.0)
            .collect()
    }

    #[test]
    fn test_hnsw_basic() {
        let mut index = HnswIndex::new(384);

        // Add some vectors
        for i in 0..100 {
            let vec = random_vector(384, i);
            index.add(format!("doc_{}", i), vec);
        }

        index.build();
        assert_eq!(index.len(), 100);
        assert!(!index.is_dirty());
    }

    #[test]
    fn test_hnsw_search() {
        let mut index = HnswIndex::new(8);

        // Add vectors with known patterns
        index.add(
            "a".to_string(),
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        );
        index.add(
            "b".to_string(),
            vec![0.9, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        );
        index.add(
            "c".to_string(),
            vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        );
        index.add(
            "d".to_string(),
            vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        );

        index.build();

        // Search for vector similar to "a"
        let query = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let results = index.search(&query, 2);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "a"); // Exact match should be first
        assert_eq!(results[1].0, "b"); // Similar vector should be second
    }

    #[test]
    fn test_hnsw_persistence() {
        let mut index = HnswIndex::new(8);
        index.add("test".to_string(), vec![1.0; 8]);
        index.build();

        // Convert to state
        let state = HnswIndexState::from(&index);

        // Restore from state
        let restored = HnswIndex::from(state);

        assert_eq!(restored.len(), 1);
        assert!(restored.get("test").is_some());
    }
}
