//! Mixed Precision Support
//!
//! Provides FP16 (half precision) support for:
//! - 2x memory reduction for vectors
//! - Faster SIMD operations on supported hardware
//! - Reduced memory bandwidth
//!
//! # Accuracy
//! - FP16 range: ±65504 with ~3 decimal digits precision
//! - Suitable for normalized embeddings (-1 to 1)
//! - May need scaling for larger values

use half::f16;
use serde::{Deserialize, Serialize};

/// Half-precision vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct F16Vector {
    data: Vec<u16>, // Store as u16 for serialization
}

impl F16Vector {
    /// Create from f32 vector
    pub fn from_f32(values: &[f32]) -> Self {
        let data = values.iter().map(|&v| f16::from_f32(v).to_bits()).collect();
        Self { data }
    }

    /// Convert to f32 vector
    pub fn to_f32(&self) -> Vec<f32> {
        self.data
            .iter()
            .map(|&bits| f16::from_bits(bits).to_f32())
            .collect()
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Memory size in bytes
    pub fn memory_bytes(&self) -> usize {
        self.data.len() * 2
    }

    /// Equivalent f32 memory size
    pub fn equivalent_f32_bytes(&self) -> usize {
        self.data.len() * 4
    }

    /// Compute dot product with another F16Vector
    pub fn dot(&self, other: &F16Vector) -> f32 {
        if self.len() != other.len() {
            return 0.0;
        }

        self.data
            .iter()
            .zip(other.data.iter())
            .map(|(&a, &b)| {
                let a = f16::from_bits(a).to_f32();
                let b = f16::from_bits(b).to_f32();
                a * b
            })
            .sum()
    }

    /// Compute cosine similarity
    pub fn cosine_similarity(&self, other: &F16Vector) -> f32 {
        if self.len() != other.len() || self.is_empty() {
            return 0.0;
        }

        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for (&a_bits, &b_bits) in self.data.iter().zip(other.data.iter()) {
            let a = f16::from_bits(a_bits).to_f32();
            let b = f16::from_bits(b_bits).to_f32();
            dot += a * b;
            norm_a += a * a;
            norm_b += b * b;
        }

        let norm_a = norm_a.sqrt();
        let norm_b = norm_b.sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }

    /// Compute cosine similarity with f32 query
    pub fn cosine_similarity_f32(&self, query: &[f32]) -> f32 {
        if self.len() != query.len() || self.is_empty() {
            return 0.0;
        }

        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for (&a_bits, &b) in self.data.iter().zip(query.iter()) {
            let a = f16::from_bits(a_bits).to_f32();
            dot += a * b;
            norm_a += a * a;
            norm_b += b * b;
        }

        let norm_a = norm_a.sqrt();
        let norm_b = norm_b.sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }
}

/// Mixed precision vector store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct F16VectorStore {
    vectors: Vec<F16Vector>,
    doc_ids: Vec<String>,
    dimension: usize,
}

impl F16VectorStore {
    pub fn new(dimension: usize) -> Self {
        Self {
            vectors: Vec::new(),
            doc_ids: Vec::new(),
            dimension,
        }
    }

    /// Build from f32 vectors
    pub fn from_f32_vectors(vectors: &[(String, Vec<f32>)]) -> Self {
        let dimension = vectors.first().map(|(_, v)| v.len()).unwrap_or(0);
        let mut store = Self::new(dimension);

        for (doc_id, vector) in vectors {
            store.add(doc_id.clone(), vector);
        }

        store
    }

    /// Add a vector
    pub fn add(&mut self, doc_id: String, vector: &[f32]) {
        self.vectors.push(F16Vector::from_f32(vector));
        self.doc_ids.push(doc_id);
    }

    /// Search for similar vectors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let mut scores: Vec<(usize, f32)> = self
            .vectors
            .iter()
            .enumerate()
            .map(|(i, v)| (i, v.cosine_similarity_f32(query)))
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        scores
            .into_iter()
            .take(k)
            .map(|(i, score)| (self.doc_ids[i].clone(), score))
            .collect()
    }

    /// Get vector by index
    pub fn get(&self, index: usize) -> Option<Vec<f32>> {
        self.vectors.get(index).map(|v| v.to_f32())
    }

    /// Get vector by doc_id
    pub fn get_by_id(&self, doc_id: &str) -> Option<Vec<f32>> {
        self.doc_ids
            .iter()
            .position(|id| id == doc_id)
            .and_then(|i| self.get(i))
    }

    /// Number of vectors
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        self.vectors.iter().map(|v| v.memory_bytes()).sum()
    }

    /// Equivalent f32 memory usage
    pub fn equivalent_f32_bytes(&self) -> usize {
        self.vectors.iter().map(|v| v.equivalent_f32_bytes()).sum()
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

/// Convert f32 slice to f16 bytes (for storage)
pub fn f32_to_f16_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 2);
    for &v in values {
        let f16_val = f16::from_f32(v);
        bytes.extend_from_slice(&f16_val.to_le_bytes());
    }
    bytes
}

/// Convert f16 bytes back to f32
pub fn f16_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|chunk| {
            let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
            f16::from_bits(bits).to_f32()
        })
        .collect()
}

/// Check if a value can be safely represented in f16
pub fn is_f16_safe(value: f32) -> bool {
    let abs = value.abs();
    abs <= 65504.0 && (abs == 0.0 || abs >= 6.1e-5)
}

/// Clamp value to f16 range
pub fn clamp_to_f16_range(value: f32) -> f32 {
    value.clamp(-65504.0, 65504.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f16_vector_roundtrip() {
        let original: Vec<f32> = (0..100).map(|i| i as f32 * 0.01).collect();
        let f16_vec = F16Vector::from_f32(&original);
        let recovered = f16_vec.to_f32();

        // Check values are close (f16 has limited precision)
        for (o, r) in original.iter().zip(recovered.iter()) {
            assert!((o - r).abs() < 0.01, "Original: {}, Recovered: {}", o, r);
        }
    }

    #[test]
    fn test_f16_cosine_similarity() {
        let a = F16Vector::from_f32(&[1.0, 0.0, 0.0]);
        let b = F16Vector::from_f32(&[1.0, 0.0, 0.0]);
        let c = F16Vector::from_f32(&[0.0, 1.0, 0.0]);

        assert!((a.cosine_similarity(&b) - 1.0).abs() < 0.01);
        assert!(a.cosine_similarity(&c).abs() < 0.01);
    }

    #[test]
    fn test_f16_store() {
        let vectors: Vec<(String, Vec<f32>)> = (0..100)
            .map(|i| {
                let vec: Vec<f32> = (0..384).map(|j| (i * 384 + j) as f32 * 0.0001).collect();
                (format!("doc_{}", i), vec)
            })
            .collect();

        let store = F16VectorStore::from_f32_vectors(&vectors);

        println!("F16 memory: {} bytes", store.memory_bytes());
        println!("F32 equivalent: {} bytes", store.equivalent_f32_bytes());
        println!("Compression: {:.1}x", store.compression_ratio());

        assert!((store.compression_ratio() - 2.0).abs() < 0.1);
    }

    #[test]
    fn test_f16_search() {
        let vectors: Vec<(String, Vec<f32>)> = vec![
            ("a".to_string(), vec![1.0, 0.0, 0.0]),
            ("b".to_string(), vec![0.9, 0.1, 0.0]),
            ("c".to_string(), vec![0.0, 1.0, 0.0]),
        ];

        let store = F16VectorStore::from_f32_vectors(&vectors);
        let results = store.search(&[1.0, 0.0, 0.0], 2);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "a");
        assert_eq!(results[1].0, "b");
    }
}
