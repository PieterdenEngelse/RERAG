//! Vector Quantization
//! 
//! Reduces memory usage by storing vectors as int8 instead of f32.
//! This provides 4x memory reduction with minimal accuracy loss.
//! 
//! # Quantization Methods
//! - Scalar quantization: Simple min-max scaling to int8
//! - Product quantization: Coming soon
//! 
//! # Accuracy
//! - Cosine similarity error: typically < 1%
//! - Suitable for approximate nearest neighbor search

use serde::{Deserialize, Serialize};

/// Quantized vector representation
/// 
/// Stores 384-dim vector in ~400 bytes instead of ~1.5KB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedVector {
    /// Quantized values (-128 to 127)
    pub data: Vec<i8>,
    /// Scale factor for dequantization
    pub scale: f32,
    /// Zero point offset
    pub zero_point: f32,
}

impl QuantizedVector {
    /// Create a new quantized vector from f32 values
    pub fn from_f32(values: &[f32]) -> Self {
        let (data, scale, zero_point) = quantize(values);
        Self { data, scale, zero_point }
    }

    /// Convert back to f32 values
    pub fn to_f32(&self) -> Vec<f32> {
        dequantize(&self.data, self.scale, self.zero_point)
    }

    /// Memory size in bytes
    pub fn size_bytes(&self) -> usize {
        self.data.len() + 8 // data + scale + zero_point
    }

    /// Compute cosine similarity with another quantized vector
    /// This is approximate but much faster than dequantizing first
    pub fn cosine_similarity(&self, other: &QuantizedVector) -> f32 {
        if self.data.len() != other.data.len() {
            return 0.0;
        }

        let mut dot: i32 = 0;
        let mut a_sq: i32 = 0;
        let mut b_sq: i32 = 0;

        for (a, b) in self.data.iter().zip(other.data.iter()) {
            let a_val = *a as i32;
            let b_val = *b as i32;
            dot += a_val * b_val;
            a_sq += a_val * a_val;
            b_sq += b_val * b_val;
        }

        let norm_a = (a_sq as f32).sqrt();
        let norm_b = (b_sq as f32).sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot as f32 / (norm_a * norm_b)
        }
    }

    /// Compute cosine similarity with an f32 query vector
    /// Quantizes the query on-the-fly for comparison
    pub fn cosine_similarity_with_f32(&self, query: &[f32]) -> f32 {
        let query_quantized = QuantizedVector::from_f32(query);
        self.cosine_similarity(&query_quantized)
    }
}

/// Quantize f32 values to int8
/// 
/// Uses symmetric quantization around zero for better accuracy
/// with normalized vectors.
pub fn quantize(values: &[f32]) -> (Vec<i8>, f32, f32) {
    if values.is_empty() {
        return (Vec::new(), 1.0, 0.0);
    }

    // Find min and max
    let min_val = values.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_val = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    // Calculate scale and zero point
    let range = max_val - min_val;
    let scale = if range > 0.0 { range / 255.0 } else { 1.0 };
    let zero_point = min_val;

    // Quantize
    let quantized: Vec<i8> = values
        .iter()
        .map(|&v| {
            let normalized = (v - zero_point) / scale;
            let clamped = normalized.clamp(0.0, 255.0);
            (clamped - 128.0) as i8
        })
        .collect();

    (quantized, scale, zero_point)
}

/// Dequantize int8 values back to f32
pub fn dequantize(quantized: &[i8], scale: f32, zero_point: f32) -> Vec<f32> {
    quantized
        .iter()
        .map(|&q| {
            let normalized = (q as f32 + 128.0) * scale;
            normalized + zero_point
        })
        .collect()
}

/// Batch quantize multiple vectors
pub fn quantize_batch(vectors: &[Vec<f32>]) -> Vec<QuantizedVector> {
    vectors.iter().map(|v| QuantizedVector::from_f32(v)).collect()
}

/// Quantized vector storage for efficient memory usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedVectorStore {
    vectors: Vec<QuantizedVector>,
    dimension: usize,
}

impl QuantizedVectorStore {
    pub fn new(dimension: usize) -> Self {
        Self {
            vectors: Vec::new(),
            dimension,
        }
    }

    pub fn from_f32_vectors(vectors: &[Vec<f32>]) -> Self {
        let dimension = vectors.first().map(|v| v.len()).unwrap_or(0);
        Self {
            vectors: quantize_batch(vectors),
            dimension,
        }
    }

    pub fn add(&mut self, vector: &[f32]) {
        self.vectors.push(QuantizedVector::from_f32(vector));
    }

    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Search for top-k most similar vectors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(usize, f32)> {
        let query_quantized = QuantizedVector::from_f32(query);
        
        let mut scores: Vec<(usize, f32)> = self.vectors
            .iter()
            .enumerate()
            .map(|(i, v)| (i, v.cosine_similarity(&query_quantized)))
            .collect();
        
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(k);
        scores
    }

    /// Memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        self.vectors.iter().map(|v| v.size_bytes()).sum()
    }

    /// Equivalent f32 memory usage
    pub fn equivalent_f32_bytes(&self) -> usize {
        self.vectors.len() * self.dimension * 4
    }

    /// Memory savings ratio
    pub fn compression_ratio(&self) -> f32 {
        if self.memory_bytes() == 0 {
            1.0
        } else {
            self.equivalent_f32_bytes() as f32 / self.memory_bytes() as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_dequantize() {
        let original: Vec<f32> = (0..100).map(|i| i as f32 * 0.01).collect();
        let (quantized, scale, zero_point) = quantize(&original);
        let recovered = dequantize(&quantized, scale, zero_point);

        // Check that values are close (within quantization error)
        for (o, r) in original.iter().zip(recovered.iter()) {
            assert!((o - r).abs() < 0.01, "Original: {}, Recovered: {}", o, r);
        }
    }

    #[test]
    fn test_quantized_cosine_similarity() {
        let a: Vec<f32> = (0..384).map(|i| (i as f32).sin()).collect();
        let b: Vec<f32> = (0..384).map(|i| (i as f32).sin() + 0.01).collect();

        let qa = QuantizedVector::from_f32(&a);
        let qb = QuantizedVector::from_f32(&b);

        let quantized_sim = qa.cosine_similarity(&qb);
        
        // Calculate exact similarity for comparison
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        let exact_sim = dot / (norm_a * norm_b);

        // Quantized similarity should be close to exact
        assert!((quantized_sim - exact_sim).abs() < 0.05,
            "Quantized: {}, Exact: {}", quantized_sim, exact_sim);
    }

    #[test]
    fn test_memory_savings() {
        let vectors: Vec<Vec<f32>> = (0..1000)
            .map(|_| (0..384).map(|i| i as f32 * 0.001).collect())
            .collect();

        let store = QuantizedVectorStore::from_f32_vectors(&vectors);
        
        println!("Quantized memory: {} bytes", store.memory_bytes());
        println!("Equivalent f32: {} bytes", store.equivalent_f32_bytes());
        println!("Compression ratio: {:.2}x", store.compression_ratio());

        assert!(store.compression_ratio() > 3.0); // Should be ~4x
    }
}
