//! Batch Processing Utilities
//!
//! Provides efficient batch processing for embeddings and vector operations.
//! Optimizes throughput by processing multiple items together.

use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Optimal batch size for different operations
pub mod batch_sizes {
    /// Optimal batch size for CPU embedding generation
    pub const EMBEDDING_CPU: usize = 32;

    /// Optimal batch size for GPU embedding generation
    pub const EMBEDDING_GPU: usize = 128;

    /// Optimal batch size for vector similarity calculations
    pub const SIMILARITY: usize = 1000;

    /// Optimal batch size for indexing operations
    pub const INDEXING: usize = 100;

    /// Optimal batch size for database operations
    pub const DATABASE: usize = 500;
}

/// Batch processor for parallel operations
pub struct BatchProcessor {
    batch_size: usize,
    processed: AtomicUsize,
}

impl BatchProcessor {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size,
            processed: AtomicUsize::new(0),
        }
    }

    /// Process items in batches with a function
    pub fn process<T, R, F>(&self, items: &[T], f: F) -> Vec<R>
    where
        T: Sync,
        R: Send,
        F: Fn(&T) -> R + Sync,
    {
        items
            .par_chunks(self.batch_size)
            .flat_map(|chunk| {
                let results: Vec<R> = chunk.iter().map(&f).collect();
                self.processed.fetch_add(chunk.len(), Ordering::Relaxed);
                results
            })
            .collect()
    }

    /// Process items in batches with index
    pub fn process_indexed<T, R, F>(&self, items: &[T], f: F) -> Vec<(usize, R)>
    where
        T: Sync,
        R: Send,
        F: Fn(usize, &T) -> R + Sync,
    {
        items
            .par_iter()
            .enumerate()
            .map(|(i, item)| {
                self.processed.fetch_add(1, Ordering::Relaxed);
                (i, f(i, item))
            })
            .collect()
    }

    /// Get number of processed items
    pub fn processed_count(&self) -> usize {
        self.processed.load(Ordering::Relaxed)
    }

    /// Reset processed counter
    pub fn reset(&self) {
        self.processed.store(0, Ordering::Relaxed);
    }
}

/// Chunked iterator for memory-efficient batch processing
pub struct ChunkedIterator<I> {
    iter: I,
    chunk_size: usize,
}

impl<I> ChunkedIterator<I> {
    pub fn new(iter: I, chunk_size: usize) -> Self {
        Self { iter, chunk_size }
    }
}

impl<I: Iterator> Iterator for ChunkedIterator<I> {
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        let chunk: Vec<_> = self.iter.by_ref().take(self.chunk_size).collect();
        if chunk.is_empty() {
            None
        } else {
            Some(chunk)
        }
    }
}

/// Extension trait for chunked iteration
pub trait ChunkedExt: Iterator + Sized {
    fn chunked(self, size: usize) -> ChunkedIterator<Self> {
        ChunkedIterator::new(self, size)
    }
}

impl<I: Iterator> ChunkedExt for I {}

/// Batch embedding request
#[derive(Debug, Clone)]
pub struct BatchEmbeddingRequest {
    pub texts: Vec<String>,
    pub batch_size: usize,
}

impl BatchEmbeddingRequest {
    pub fn new(texts: Vec<String>) -> Self {
        Self {
            texts,
            batch_size: batch_sizes::EMBEDDING_CPU,
        }
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub fn chunks(&self) -> impl Iterator<Item = &[String]> {
        self.texts.chunks(self.batch_size)
    }

    pub fn num_batches(&self) -> usize {
        self.texts.len().div_ceil(self.batch_size)
    }
}

/// Progress tracker for batch operations
pub struct BatchProgress {
    total: usize,
    completed: AtomicUsize,
    callback: Option<Box<dyn Fn(usize, usize) + Send + Sync>>,
}

impl BatchProgress {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            completed: AtomicUsize::new(0),
            callback: None,
        }
    }

    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(usize, usize) + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(callback));
        self
    }

    pub fn increment(&self, count: usize) {
        let completed = self.completed.fetch_add(count, Ordering::Relaxed) + count;
        if let Some(ref callback) = self.callback {
            callback(completed, self.total);
        }
    }

    pub fn completed(&self) -> usize {
        self.completed.load(Ordering::Relaxed)
    }

    pub fn total(&self) -> usize {
        self.total
    }

    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            1.0
        } else {
            self.completed() as f64 / self.total as f64
        }
    }

    pub fn is_complete(&self) -> bool {
        self.completed() >= self.total
    }
}

/// Parallel batch processor with progress tracking
pub fn process_with_progress<T, R, F>(
    items: Vec<T>,
    batch_size: usize,
    f: F,
    progress: &BatchProgress,
) -> Vec<R>
where
    T: Send + Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    items
        .par_chunks(batch_size)
        .flat_map(|chunk| {
            let results: Vec<R> = chunk.iter().map(&f).collect();
            progress.increment(chunk.len());
            results
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_processor() {
        let processor = BatchProcessor::new(10);
        let items: Vec<i32> = (0..100).collect();

        let results: Vec<i32> = processor.process(&items, |x| x * 2);

        assert_eq!(results.len(), 100);
        assert_eq!(results[0], 0);
        assert_eq!(results[50], 100);
        assert_eq!(processor.processed_count(), 100);
    }

    #[test]
    fn test_chunked_iterator() {
        let items: Vec<i32> = (0..25).collect();
        let chunks: Vec<Vec<i32>> = items.into_iter().chunked(10).collect();

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 10);
        assert_eq!(chunks[1].len(), 10);
        assert_eq!(chunks[2].len(), 5);
    }

    #[test]
    fn test_batch_progress() {
        let progress = BatchProgress::new(100);

        progress.increment(25);
        assert_eq!(progress.completed(), 25);
        assert!((progress.progress() - 0.25).abs() < 0.001);

        progress.increment(75);
        assert!(progress.is_complete());
    }

    #[test]
    fn test_batch_embedding_request() {
        let texts: Vec<String> = (0..100).map(|i| format!("text {}", i)).collect();
        let request = BatchEmbeddingRequest::new(texts).with_batch_size(32);

        assert_eq!(request.num_batches(), 4); // 100 / 32 = 3.125, rounded up = 4
    }
}
