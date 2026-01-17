//! SIMD-Accelerated Vector Operations
//! 
//! Provides 4-8x faster vector math using SIMD instructions.
//! Falls back to scalar operations on unsupported platforms.

use wide::f32x8;

/// SIMD-accelerated cosine similarity
/// 
/// Processes 8 floats at a time using AVX2/SSE instructions.
/// Falls back to scalar for remaining elements.
/// 
/// # Performance
/// - ~4-8x faster than scalar for vectors >= 32 dimensions
/// - Optimal for 384-dim embeddings (BGE-small)
pub fn cosine_similarity_simd(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let len = a.len();
    let chunks = len / 8;

    let mut dot_sum = f32x8::ZERO;
    let mut a_sq_sum = f32x8::ZERO;
    let mut b_sq_sum = f32x8::ZERO;

    // Process 8 elements at a time
    for i in 0..chunks {
        let offset = i * 8;
        let a_chunk = f32x8::from(&a[offset..offset + 8]);
        let b_chunk = f32x8::from(&b[offset..offset + 8]);

        dot_sum += a_chunk * b_chunk;
        a_sq_sum += a_chunk * a_chunk;
        b_sq_sum += b_chunk * b_chunk;
    }

    // Horizontal sum of SIMD vectors
    let mut dot: f32 = dot_sum.reduce_add();
    let mut a_sq: f32 = a_sq_sum.reduce_add();
    let mut b_sq: f32 = b_sq_sum.reduce_add();

    // Handle remainder with scalar ops
    let start = chunks * 8;
    for i in start..len {
        dot += a[i] * b[i];
        a_sq += a[i] * a[i];
        b_sq += b[i] * b[i];
    }

    let norm_a = a_sq.sqrt();
    let norm_b = b_sq.sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// SIMD-accelerated dot product
pub fn dot_product_simd(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let len = a.len();
    let chunks = len / 8;

    let mut sum = f32x8::ZERO;

    for i in 0..chunks {
        let offset = i * 8;
        let a_chunk = f32x8::from(&a[offset..offset + 8]);
        let b_chunk = f32x8::from(&b[offset..offset + 8]);
        sum += a_chunk * b_chunk;
    }

    let mut result = sum.reduce_add();

    // Handle remainder
    let start = chunks * 8;
    for i in start..len {
        result += a[i] * b[i];
    }

    result
}

/// SIMD-accelerated vector normalization (in-place)
pub fn normalize_simd(v: &mut [f32]) {
    let len = v.len();
    let chunks = len / 8;

    // Calculate magnitude
    let mut sq_sum = f32x8::ZERO;
    for i in 0..chunks {
        let offset = i * 8;
        let chunk = f32x8::from(&v[offset..offset + 8]);
        sq_sum += chunk * chunk;
    }

    let mut magnitude_sq = sq_sum.reduce_add();
    let start = chunks * 8;
    for i in start..len {
        magnitude_sq += v[i] * v[i];
    }

    let magnitude = magnitude_sq.sqrt();
    if magnitude == 0.0 {
        return;
    }

    let inv_mag = 1.0 / magnitude;
    let inv_mag_simd = f32x8::splat(inv_mag);

    // Normalize
    for i in 0..chunks {
        let offset = i * 8;
        let chunk = f32x8::from(&v[offset..offset + 8]);
        let normalized = chunk * inv_mag_simd;
        // Write back
        for j in 0..8 {
            v[offset + j] = normalized.as_array_ref()[j];
        }
    }

    for i in start..len {
        v[i] *= inv_mag;
    }
}

/// SIMD-accelerated Euclidean distance
pub fn euclidean_distance_simd(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return f32::MAX;
    }

    let len = a.len();
    let chunks = len / 8;

    let mut sum = f32x8::ZERO;

    for i in 0..chunks {
        let offset = i * 8;
        let a_chunk = f32x8::from(&a[offset..offset + 8]);
        let b_chunk = f32x8::from(&b[offset..offset + 8]);
        let diff = a_chunk - b_chunk;
        sum += diff * diff;
    }

    let mut result = sum.reduce_add();

    let start = chunks * 8;
    for i in start..len {
        let diff = a[i] - b[i];
        result += diff * diff;
    }

    result.sqrt()
}

/// Batch cosine similarity - compute similarity of query against multiple vectors
pub fn batch_cosine_similarity_simd(query: &[f32], vectors: &[Vec<f32>]) -> Vec<f32> {
    vectors
        .iter()
        .map(|v| cosine_similarity_simd(query, v))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_simd() {
        let a = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert!((cosine_similarity_simd(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert!(cosine_similarity_simd(&a, &c).abs() < 1e-6);
    }

    #[test]
    fn test_dot_product_simd() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        assert!((dot_product_simd(&a, &b) - 36.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_simd() {
        let mut v = vec![3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        normalize_simd(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_384_dim_vectors() {
        // Test with realistic embedding dimensions
        let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
        let b: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01 + 0.001).collect();
        
        let sim = cosine_similarity_simd(&a, &b);
        assert!(sim > 0.99); // Should be very similar
    }
}
