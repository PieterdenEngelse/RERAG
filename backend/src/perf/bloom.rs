//! Bloom Filter for Fast Negative Lookups
//! 
//! Bloom filters provide O(1) probabilistic membership testing.
//! They can definitively say "not in set" but may have false positives.
//! 
//! # Use Cases
//! - Skip expensive vector search if document definitely doesn't exist
//! - Fast cache miss detection
//! - Duplicate detection during indexing
//! 
//! # Performance
//! - Check: O(1)
//! - Insert: O(1)
//! - Memory: ~10 bits per element for 1% false positive rate

use bloomfilter::Bloom;

/// Bloom filter for vector document IDs
pub struct VectorBloomFilter {
    filter: Bloom<String>,
    count: usize,
    capacity: usize,
    false_positive_rate: f64,
}

impl VectorBloomFilter {
    /// Create a new bloom filter
    /// 
    /// # Arguments
    /// * `capacity` - Expected number of elements
    /// * `false_positive_rate` - Acceptable false positive rate (e.g., 0.01 for 1%)
    pub fn new(capacity: usize, false_positive_rate: f64) -> Self {
        let filter = Bloom::new_for_fp_rate(capacity, false_positive_rate);
        Self {
            filter,
            count: 0,
            capacity,
            false_positive_rate,
        }
    }

    /// Create with default settings (10,000 capacity, 1% FP rate)
    pub fn with_defaults() -> Self {
        Self::new(10_000, 0.01)
    }

    /// Insert a document ID
    pub fn insert(&mut self, doc_id: &str) {
        self.filter.set(&doc_id.to_string());
        self.count += 1;
    }

    /// Check if a document ID might exist
    /// 
    /// Returns:
    /// - `false`: Definitely not in the set
    /// - `true`: Might be in the set (could be false positive)
    pub fn might_contain(&self, doc_id: &str) -> bool {
        self.filter.check(&doc_id.to_string())
    }

    /// Check if definitely not in set (inverse of might_contain)
    pub fn definitely_not_contains(&self, doc_id: &str) -> bool {
        !self.might_contain(doc_id)
    }

    /// Number of elements inserted
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// False positive rate
    pub fn false_positive_rate(&self) -> f64 {
        self.false_positive_rate
    }

    /// Memory usage in bytes (approximate)
    pub fn memory_bytes(&self) -> usize {
        // Bloom filter uses about 10 bits per element for 1% FP rate
        (self.capacity * 10) / 8
    }

    /// Clear the filter
    pub fn clear(&mut self) {
        self.filter.clear();
        self.count = 0;
    }

    /// Check if filter is getting full (might increase FP rate)
    pub fn is_saturated(&self) -> bool {
        self.count > self.capacity
    }
}

/// Bloom filter with multiple hash functions for different key types
pub struct MultiBloomFilter {
    doc_ids: VectorBloomFilter,
    content_hashes: VectorBloomFilter,
}

impl MultiBloomFilter {
    pub fn new(capacity: usize) -> Self {
        Self {
            doc_ids: VectorBloomFilter::new(capacity, 0.01),
            content_hashes: VectorBloomFilter::new(capacity, 0.01),
        }
    }

    /// Insert document with both ID and content hash
    pub fn insert(&mut self, doc_id: &str, content_hash: &str) {
        self.doc_ids.insert(doc_id);
        self.content_hashes.insert(content_hash);
    }

    /// Check if document ID might exist
    pub fn might_contain_id(&self, doc_id: &str) -> bool {
        self.doc_ids.might_contain(doc_id)
    }

    /// Check if content hash might exist (for deduplication)
    pub fn might_contain_content(&self, content_hash: &str) -> bool {
        self.content_hashes.might_contain(content_hash)
    }

    /// Check if content is definitely new
    pub fn is_new_content(&self, content_hash: &str) -> bool {
        !self.might_contain_content(content_hash)
    }
}

/// Counting bloom filter that supports deletion
pub struct CountingBloomFilter {
    counts: Vec<u8>,
    num_hashes: usize,
    size: usize,
}

impl CountingBloomFilter {
    pub fn new(capacity: usize, false_positive_rate: f64) -> Self {
        // Calculate optimal size and number of hashes
        let size = Self::optimal_size(capacity, false_positive_rate);
        let num_hashes = Self::optimal_hashes(size, capacity);
        
        Self {
            counts: vec![0; size],
            num_hashes,
            size,
        }
    }

    fn optimal_size(capacity: usize, fp_rate: f64) -> usize {
        let ln2_sq = std::f64::consts::LN_2.powi(2);
        (-(capacity as f64 * fp_rate.ln()) / ln2_sq).ceil() as usize
    }

    fn optimal_hashes(size: usize, capacity: usize) -> usize {
        ((size as f64 / capacity as f64) * std::f64::consts::LN_2).ceil() as usize
    }

    fn hash_indices(&self, item: &str) -> Vec<usize> {
        let mut indices = Vec::with_capacity(self.num_hashes);
        let hash1 = seahash::hash(item.as_bytes());
        let hash2 = seahash::hash(&hash1.to_le_bytes());
        
        for i in 0..self.num_hashes {
            let combined = hash1.wrapping_add((i as u64).wrapping_mul(hash2));
            indices.push((combined as usize) % self.size);
        }
        
        indices
    }

    pub fn insert(&mut self, item: &str) {
        for idx in self.hash_indices(item) {
            self.counts[idx] = self.counts[idx].saturating_add(1);
        }
    }

    pub fn remove(&mut self, item: &str) {
        for idx in self.hash_indices(item) {
            self.counts[idx] = self.counts[idx].saturating_sub(1);
        }
    }

    pub fn might_contain(&self, item: &str) -> bool {
        self.hash_indices(item).iter().all(|&idx| self.counts[idx] > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_basic() {
        let mut filter = VectorBloomFilter::new(1000, 0.01);
        
        filter.insert("doc_1");
        filter.insert("doc_2");
        filter.insert("doc_3");
        
        assert!(filter.might_contain("doc_1"));
        assert!(filter.might_contain("doc_2"));
        assert!(filter.might_contain("doc_3"));
        
        // This should definitely not be in the filter
        // (with very high probability)
        let mut false_positives = 0;
        for i in 100..200 {
            if filter.might_contain(&format!("nonexistent_{}", i)) {
                false_positives += 1;
            }
        }
        
        // Should have very few false positives
        assert!(false_positives < 5, "Too many false positives: {}", false_positives);
    }

    #[test]
    fn test_counting_bloom_filter() {
        let mut filter = CountingBloomFilter::new(1000, 0.01);
        
        filter.insert("item_1");
        assert!(filter.might_contain("item_1"));
        
        filter.remove("item_1");
        assert!(!filter.might_contain("item_1"));
    }

    #[test]
    fn test_multi_bloom_filter() {
        let mut filter = MultiBloomFilter::new(1000);
        
        filter.insert("doc_1", "hash_abc123");
        
        assert!(filter.might_contain_id("doc_1"));
        assert!(filter.might_contain_content("hash_abc123"));
        assert!(!filter.might_contain_id("doc_2"));
        assert!(filter.is_new_content("hash_xyz789"));
    }
}
