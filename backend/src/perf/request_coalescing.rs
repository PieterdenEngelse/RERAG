//! Request Coalescing
//! 
//! Batches similar requests together to reduce overhead.
//! Multiple requests for the same or similar data are combined
//! into a single backend request.
//! 
//! # Use Cases
//! - Multiple users searching for the same query
//! - Batch embedding requests
//! - Deduplicating concurrent requests
//! 
//! # Benefits
//! - Reduced backend load
//! - Lower latency for duplicate requests
//! - Better resource utilization

use dashmap::DashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, oneshot, Mutex};
use tracing::debug;

/// Request coalescer for deduplicating concurrent requests
#[allow(dead_code)]
pub struct RequestCoalescer<K, V> {
    /// Pending requests
    pending: DashMap<K, Arc<PendingRequest<V>>>,
    /// Configuration
    config: CoalescerConfig,
    /// Statistics
    stats: CoalescerStats,
}

/// Pending request state
#[allow(dead_code)]
struct PendingRequest<V> {
    /// Sender for broadcasting result
    sender: broadcast::Sender<Arc<V>>,
    /// When this request started
    started_at: Instant,
}

/// Coalescer configuration
#[derive(Debug, Clone)]
pub struct CoalescerConfig {
    /// Maximum time to wait for coalescing
    pub max_wait: Duration,
    /// Maximum number of pending requests
    pub max_pending: usize,
}

impl Default for CoalescerConfig {
    fn default() -> Self {
        Self {
            max_wait: Duration::from_millis(50),
            max_pending: 1000,
        }
    }
}

use super::cache_aligned::CacheAligned;
use std::sync::atomic::AtomicU64;

/// Coalescer statistics
/// 
/// Counters are cache-line aligned to prevent false sharing
/// when concurrent requests update different counters.
#[derive(Debug)]
pub struct CoalescerStats {
    pub total_requests: CacheAligned<AtomicU64>,
    pub coalesced_requests: CacheAligned<AtomicU64>,
    pub executed_requests: CacheAligned<AtomicU64>,
}

impl Default for CoalescerStats {
    fn default() -> Self {
        Self {
            total_requests: CacheAligned::new(AtomicU64::new(0)),
            coalesced_requests: CacheAligned::new(AtomicU64::new(0)),
            executed_requests: CacheAligned::new(AtomicU64::new(0)),
        }
    }
}

impl<K, V> RequestCoalescer<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(config: CoalescerConfig) -> Self {
        Self {
            pending: DashMap::new(),
            config,
            stats: CoalescerStats::default(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(CoalescerConfig::default())
    }

    /// Execute a request with coalescing
    /// 
    /// If another request with the same key is in progress,
    /// wait for its result instead of executing again.
    pub async fn execute<F, Fut>(&self, key: K, f: F) -> Result<Arc<V>, CoalesceError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V>,
    {
        use std::sync::atomic::Ordering;
        
        self.stats.total_requests.fetch_add(1, Ordering::Relaxed);

        // Check if request is already pending
        if let Some(pending) = self.pending.get(&key) {
            let mut receiver = pending.sender.subscribe();
            drop(pending); // Release lock
            
            self.stats.coalesced_requests.fetch_add(1, Ordering::Relaxed);
            debug!("Request coalesced");
            
            match receiver.recv().await {
                Ok(result) => return Ok(result),
                Err(_) => return Err(CoalesceError::ChannelClosed),
            }
        }

        // Create new pending request
        let (sender, _) = broadcast::channel(1);
        let pending = Arc::new(PendingRequest {
            sender: sender.clone(),
            started_at: Instant::now(),
        });
        
        self.pending.insert(key.clone(), pending);
        self.stats.executed_requests.fetch_add(1, Ordering::Relaxed);

        // Execute the request
        let result = f().await;
        let result = Arc::new(result);

        // Broadcast result to waiting requests
        let _ = sender.send(result.clone());

        // Remove from pending
        self.pending.remove(&key);

        Ok(result)
    }

    /// Get statistics
    pub fn stats(&self) -> (u64, u64, u64) {
        use std::sync::atomic::Ordering;
        (
            self.stats.total_requests.load(Ordering::Relaxed),
            self.stats.coalesced_requests.load(Ordering::Relaxed),
            self.stats.executed_requests.load(Ordering::Relaxed),
        )
    }

    /// Get coalescing ratio
    pub fn coalesce_ratio(&self) -> f64 {
        use std::sync::atomic::Ordering;
        let total = self.stats.total_requests.load(Ordering::Relaxed);
        let coalesced = self.stats.coalesced_requests.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            coalesced as f64 / total as f64
        }
    }

    /// Number of pending requests
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

/// Coalescing error
#[derive(Debug)]
pub enum CoalesceError {
    ChannelClosed,
    Timeout,
}

impl std::fmt::Display for CoalesceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChannelClosed => write!(f, "Channel closed"),
            Self::Timeout => write!(f, "Request timed out"),
        }
    }
}

impl std::error::Error for CoalesceError {}

/// Batch request coalescer
/// 
/// Collects multiple requests and executes them as a batch
#[allow(dead_code)]
pub struct BatchCoalescer<K, V> {
    /// Pending items
    pending: Arc<Mutex<Vec<(K, oneshot::Sender<V>)>>>,
    /// Configuration
    config: BatchConfig,
    /// Batch processor
    processor: Arc<dyn Fn(Vec<K>) -> Vec<V> + Send + Sync>,
}

/// Batch configuration
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Maximum wait time before processing
    pub max_wait: Duration,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 32,
            max_wait: Duration::from_millis(10),
        }
    }
}

/// Singleflight pattern - ensures only one execution per key
pub struct Singleflight<K, V> {
    calls: DashMap<K, Arc<tokio::sync::Mutex<Option<V>>>>,
}

impl<K, V> Singleflight<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new() -> Self {
        Self {
            calls: DashMap::new(),
        }
    }

    /// Execute function, ensuring only one execution per key
    pub async fn do_once<F, Fut>(&self, key: K, f: F) -> V
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V>,
    {
        // Check if already computed
        if let Some(entry) = self.calls.get(&key) {
            let guard = entry.lock().await;
            if let Some(ref value) = *guard {
                return value.clone();
            }
        }

        // Create entry
        let entry = self.calls
            .entry(key.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(None)));
        
        let mut guard = entry.lock().await;
        
        // Double-check after acquiring lock
        if let Some(ref value) = *guard {
            return value.clone();
        }

        // Execute
        let result = f().await;
        *guard = Some(result.clone());
        
        result
    }

    /// Clear cached result for a key
    pub fn forget(&self, key: &K) {
        self.calls.remove(key);
    }

    /// Clear all cached results
    pub fn clear(&self) {
        self.calls.clear();
    }
}

impl<K, V> Default for Singleflight<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_request_coalescing() {
        let coalescer: Arc<RequestCoalescer<String, i32>> = Arc::new(RequestCoalescer::with_defaults());
        let counter = Arc::new(AtomicUsize::new(0));

        // Spawn multiple requests for the same key
        let mut handles = vec![];
        for _ in 0..10 {
            let coalescer = Arc::clone(&coalescer);
            let counter = Arc::clone(&counter);
            handles.push(tokio::spawn(async move {
                coalescer.execute("key".to_string(), || async {
                    counter.fetch_add(1, Ordering::Relaxed);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    42
                }).await
            }));
        }

        // Wait for all
        for handle in handles {
            let result = handle.await.unwrap().unwrap();
            assert_eq!(*result, 42);
        }

        // Should have executed only once (or a few times due to timing)
        let executions = counter.load(Ordering::Relaxed);
        println!("Executions: {}", executions);
        assert!(executions < 10);
    }

    #[tokio::test]
    async fn test_singleflight() {
        let sf: Singleflight<String, i32> = Singleflight::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let counter1 = counter.clone();
        let result1 = sf.do_once("key".to_string(), || async move {
            counter1.fetch_add(1, Ordering::Relaxed);
            42
        }).await;

        let counter2 = counter.clone();
        let result2 = sf.do_once("key".to_string(), || async move {
            counter2.fetch_add(1, Ordering::Relaxed);
            99
        }).await;

        assert_eq!(result1, 42);
        assert_eq!(result2, 42); // Should return cached value
        assert_eq!(counter.load(Ordering::Relaxed), 1); // Only executed once
    }
}
