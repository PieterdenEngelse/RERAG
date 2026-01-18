//! Product Quantization (PQ)
//! 
//! Provides 16x memory reduction compared to f32 vectors by:
//! 1. Splitting vectors into subvectors
//! 2. Clustering each subspace with k-means
//! 3. Storing only centroid IDs (1 byte each)
//! 
//! # Memory Usage
//! - Original: 384 dims × 4 bytes = 1536 bytes
//! - PQ (48 subvectors): 48 bytes
//! - Compression: 32x
//! 
//! # Accuracy
//! - Approximate distance computation
//! - Typically 95-99% recall at 10-NN

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Number of centroids per subspace (256 = 1 byte per code)
const NUM_CENTROIDS: usize = 256;

/// Product Quantization codebook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PQCodebook {
    /// Number of subvectors
    pub num_subvectors: usize,
    /// Dimension of each subvector
    pub subvector_dim: usize,
    /// Centroids for each subspace: [num_subvectors][NUM_CENTROIDS][subvector_dim]
    pub centroids: Vec<Vec<Vec<f32>>>,
}

impl PQCodebook {
    /// Create a new codebook by training on sample vectors
    pub fn train(vectors: &[Vec<f32>], num_subvectors: usize, iterations: usize) -> Self {
        if vectors.is_empty() {
            return Self {
                num_subvectors,
                subvector_dim: 0,
                centroids: Vec::new(),
            };
        }

        let dim = vectors[0].len();
        let subvector_dim = dim / num_subvectors;
        
        assert!(dim % num_subvectors == 0, 
            "Vector dimension {} must be divisible by num_subvectors {}", dim, num_subvectors);

        // Train centroids for each subspace using k-means
        let centroids: Vec<Vec<Vec<f32>>> = (0..num_subvectors)
            .into_par_iter()
            .map(|m| {
                let start = m * subvector_dim;
                let end = start + subvector_dim;
                
                // Extract subvectors for this subspace
                let subvectors: Vec<Vec<f32>> = vectors
                    .iter()
                    .map(|v| v[start..end].to_vec())
                    .collect();
                
                // Run k-means
                kmeans(&subvectors, NUM_CENTROIDS, iterations)
            })
            .collect();

        Self {
            num_subvectors,
            subvector_dim,
            centroids,
        }
    }

    /// Encode a vector to PQ codes
    pub fn encode(&self, vector: &[f32]) -> Vec<u8> {
        let mut codes = Vec::with_capacity(self.num_subvectors);
        
        for m in 0..self.num_subvectors {
            let start = m * self.subvector_dim;
            let end = start + self.subvector_dim;
            let subvector = &vector[start..end];
            
            // Find nearest centroid
            let mut min_dist = f32::MAX;
            let mut min_idx = 0u8;
            
            for (idx, centroid) in self.centroids[m].iter().enumerate() {
                let dist = euclidean_distance_sq(subvector, centroid);
                if dist < min_dist {
                    min_dist = dist;
                    min_idx = idx as u8;
                }
            }
            
            codes.push(min_idx);
        }
        
        codes
    }

    /// Decode PQ codes back to approximate vector
    pub fn decode(&self, codes: &[u8]) -> Vec<f32> {
        let mut vector = Vec::with_capacity(self.num_subvectors * self.subvector_dim);
        
        for (m, &code) in codes.iter().enumerate() {
            vector.extend_from_slice(&self.centroids[m][code as usize]);
        }
        
        vector
    }

    /// Precompute distance table for a query (for fast search)
    pub fn compute_distance_table(&self, query: &[f32]) -> Vec<Vec<f32>> {
        (0..self.num_subvectors)
            .map(|m| {
                let start = m * self.subvector_dim;
                let end = start + self.subvector_dim;
                let query_sub = &query[start..end];
                
                self.centroids[m]
                    .iter()
                    .map(|centroid| euclidean_distance_sq(query_sub, centroid))
                    .collect()
            })
            .collect()
    }

    /// Compute distance using precomputed table (very fast)
    pub fn asymmetric_distance(&self, table: &[Vec<f32>], codes: &[u8]) -> f32 {
        codes.iter()
            .enumerate()
            .map(|(m, &code)| table[m][code as usize])
            .sum::<f32>()
            .sqrt()
    }
}

/// Product-quantized vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PQVector {
    pub codes: Vec<u8>,
}

impl PQVector {
    pub fn new(codes: Vec<u8>) -> Self {
        Self { codes }
    }

    pub fn memory_bytes(&self) -> usize {
        self.codes.len()
    }
}

/// Product Quantization index for fast approximate search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PQIndex {
    codebook: PQCodebook,
    vectors: Vec<PQVector>,
    doc_ids: Vec<String>,
}

impl PQIndex {
    /// Create a new PQ index
    pub fn new(codebook: PQCodebook) -> Self {
        Self {
            codebook,
            vectors: Vec::new(),
            doc_ids: Vec::new(),
        }
    }

    /// Build index from vectors
    pub fn build(vectors: &[(String, Vec<f32>)], num_subvectors: usize) -> Self {
        let vecs: Vec<Vec<f32>> = vectors.iter().map(|(_, v)| v.clone()).collect();
        let codebook = PQCodebook::train(&vecs, num_subvectors, 20);
        
        let mut index = Self::new(codebook);
        for (doc_id, vector) in vectors {
            index.add(doc_id.clone(), vector);
        }
        
        index
    }

    /// Add a vector to the index
    pub fn add(&mut self, doc_id: String, vector: &[f32]) {
        let codes = self.codebook.encode(vector);
        self.vectors.push(PQVector::new(codes));
        self.doc_ids.push(doc_id);
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        // Precompute distance table
        let table = self.codebook.compute_distance_table(query);
        
        // Compute distances to all vectors
        let mut distances: Vec<(usize, f32)> = self.vectors
            .par_iter()
            .enumerate()
            .map(|(i, pq)| {
                let dist = self.codebook.asymmetric_distance(&table, &pq.codes);
                (i, dist)
            })
            .collect();
        
        // Sort by distance
        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        
        // Return top-k
        distances.into_iter()
            .take(k)
            .map(|(i, dist)| (self.doc_ids[i].clone(), 1.0 / (1.0 + dist))) // Convert to similarity
            .collect()
    }

    /// Number of vectors in index
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        let codebook_size = self.codebook.centroids.iter()
            .map(|c| c.iter().map(|v| v.len() * 4).sum::<usize>())
            .sum::<usize>();
        let vectors_size: usize = self.vectors.iter().map(|v| v.memory_bytes()).sum();
        codebook_size + vectors_size
    }

    /// Equivalent f32 memory usage
    pub fn equivalent_f32_bytes(&self) -> usize {
        self.vectors.len() * self.codebook.num_subvectors * self.codebook.subvector_dim * 4
    }

    /// Compression ratio
    pub fn compression_ratio(&self) -> f32 {
        if self.memory_bytes() == 0 {
            1.0
        } else {
            self.equivalent_f32_bytes() as f32 / self.memory_bytes() as f32
        }
    }
}

/// Simple k-means clustering
fn kmeans(vectors: &[Vec<f32>], k: usize, iterations: usize) -> Vec<Vec<f32>> {
    if vectors.is_empty() {
        return Vec::new();
    }

    let dim = vectors[0].len();
    let k = k.min(vectors.len());
    
    // Initialize centroids randomly (use first k vectors)
    let mut centroids: Vec<Vec<f32>> = vectors.iter()
        .take(k)
        .cloned()
        .collect();
    
    // Pad with zeros if not enough vectors
    while centroids.len() < k {
        centroids.push(vec![0.0; dim]);
    }

    for _ in 0..iterations {
        // Assign vectors to nearest centroid
        let mut assignments: Vec<Vec<usize>> = vec![Vec::new(); k];
        
        for (i, vector) in vectors.iter().enumerate() {
            let mut min_dist = f32::MAX;
            let mut min_idx = 0;
            
            for (j, centroid) in centroids.iter().enumerate() {
                let dist = euclidean_distance_sq(vector, centroid);
                if dist < min_dist {
                    min_dist = dist;
                    min_idx = j;
                }
            }
            
            assignments[min_idx].push(i);
        }

        // Update centroids
        for (j, assigned) in assignments.iter().enumerate() {
            if assigned.is_empty() {
                continue;
            }
            
            let mut new_centroid = vec![0.0; dim];
            for &i in assigned {
                for (d, val) in vectors[i].iter().enumerate() {
                    new_centroid[d] += val;
                }
            }
            
            let count = assigned.len() as f32;
            for val in &mut new_centroid {
                *val /= count;
            }
            
            centroids[j] = new_centroid;
        }
    }

    centroids
}

/// Squared Euclidean distance
fn euclidean_distance_sq(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
        (0..dim).map(|i| ((i as u64 + seed) % 100) as f32 / 100.0).collect()
    }

    #[test]
    fn test_pq_encode_decode() {
        let vectors: Vec<Vec<f32>> = (0..100)
            .map(|i| random_vector(384, i))
            .collect();
        
        let codebook = PQCodebook::train(&vectors, 48, 5);
        
        let original = &vectors[0];
        let codes = codebook.encode(original);
        let decoded = codebook.decode(&codes);
        
        assert_eq!(codes.len(), 48);
        assert_eq!(decoded.len(), 384);
        
        // Decoded should be somewhat close to original
        let error: f32 = original.iter()
            .zip(decoded.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>() / original.len() as f32;
        
        println!("Average reconstruction error: {}", error);
        assert!(error < 0.5); // Reasonable error
    }

    #[test]
    fn test_pq_index_search() {
        let vectors: Vec<(String, Vec<f32>)> = (0..100)
            .map(|i| (format!("doc_{}", i), random_vector(384, i)))
            .collect();
        
        let index = PQIndex::build(&vectors, 48);
        
        let query = random_vector(384, 0); // Same as doc_0
        let results = index.search(&query, 5);
        
        assert!(!results.is_empty());
        // doc_0 should be in top results (exact match)
        assert!(results.iter().any(|(id, _)| id == "doc_0"));
        
        println!("Compression ratio: {:.1}x", index.compression_ratio());
    }

    #[test]
    fn test_pq_memory_savings() {
        let vectors: Vec<(String, Vec<f32>)> = (0..1000)
            .map(|i| (format!("doc_{}", i), random_vector(384, i)))
            .collect();
        
        let index = PQIndex::build(&vectors, 48);
        
        println!("PQ memory: {} bytes", index.memory_bytes());
        println!("Equivalent f32: {} bytes", index.equivalent_f32_bytes());
        let compression = index.compression_ratio();
        println!("Compression: {:.1}x", compression);
        
        // Should achieve significant compression (>=3x vs baseline)
        assert!(compression > 3.0, "Expected >3x compression, got {:.2}x", compression);
    }
}
