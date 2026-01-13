// src/inference_gateway.rs
// Concurrency gateway for inference operations (embeddings, LLM calls)
//
// Provides semaphore-based concurrency control to prevent resource exhaustion
// when multiple requests try to run inference simultaneously.

use once_cell::sync::OnceCell;
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{debug, info, warn};

/// Configuration for the inference gateway
#[derive(Debug, Clone)]
pub struct InferenceGatewayConfig {
    /// Max concurrent embedding operations
    pub max_concurrent_embeddings: usize,
    /// Max concurrent LLM inference operations
    pub max_concurrent_llm: usize,
    /// Timeout for acquiring a permit (milliseconds, 0 = no timeout)
    pub acquire_timeout_ms: u64,
}

impl Default for InferenceGatewayConfig {
    fn default() -> Self {
        Self {
            max_concurrent_embeddings: 4,
            max_concurrent_llm: 2,
            acquire_timeout_ms: 30_000, // 30 seconds
        }
    }
}

impl InferenceGatewayConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let max_concurrent_embeddings = env::var("INFERENCE_MAX_CONCURRENT_EMBEDDINGS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4);

        let max_concurrent_llm = env::var("INFERENCE_MAX_CONCURRENT_LLM")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        let acquire_timeout_ms = env::var("INFERENCE_ACQUIRE_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30_000);

        Self {
            max_concurrent_embeddings: max_concurrent_embeddings.max(1),
            max_concurrent_llm: max_concurrent_llm.max(1),
            acquire_timeout_ms,
        }
    }
}

/// Statistics for the inference gateway
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct GatewayStats {
    pub embedding_permits_total: usize,
    pub embedding_permits_available: usize,
    pub embedding_acquired_total: u64,
    pub embedding_rejected_total: u64,
    pub embedding_wait_total_ms: u64,
    pub llm_permits_total: usize,
    pub llm_permits_available: usize,
    pub llm_acquired_total: u64,
    pub llm_rejected_total: u64,
    pub llm_wait_total_ms: u64,
}

/// Inference gateway for controlling concurrent inference operations
pub struct InferenceGateway {
    config: InferenceGatewayConfig,
    embedding_semaphore: Arc<Semaphore>,
    llm_semaphore: Arc<Semaphore>,
    // Metrics
    embedding_acquired: AtomicU64,
    embedding_rejected: AtomicU64,
    embedding_wait_ms: AtomicU64,
    llm_acquired: AtomicU64,
    llm_rejected: AtomicU64,
    llm_wait_ms: AtomicU64,
}

impl InferenceGateway {
    /// Create a new inference gateway with the given configuration
    pub fn new(config: InferenceGatewayConfig) -> Self {
        info!(
            max_concurrent_embeddings = config.max_concurrent_embeddings,
            max_concurrent_llm = config.max_concurrent_llm,
            acquire_timeout_ms = config.acquire_timeout_ms,
            "Initializing inference gateway"
        );

        Self {
            embedding_semaphore: Arc::new(Semaphore::new(config.max_concurrent_embeddings)),
            llm_semaphore: Arc::new(Semaphore::new(config.max_concurrent_llm)),
            config,
            embedding_acquired: AtomicU64::new(0),
            embedding_rejected: AtomicU64::new(0),
            embedding_wait_ms: AtomicU64::new(0),
            llm_acquired: AtomicU64::new(0),
            llm_rejected: AtomicU64::new(0),
            llm_wait_ms: AtomicU64::new(0),
        }
    }

    /// Acquire a permit for embedding operations
    /// Returns None if timeout expires or semaphore is closed
    pub async fn acquire_embedding_permit(&self) -> Option<OwnedSemaphorePermit> {
        let start = std::time::Instant::now();

        let result = if self.config.acquire_timeout_ms == 0 {
            // No timeout - wait indefinitely
            self.embedding_semaphore.clone().acquire_owned().await.ok()
        } else {
            // With timeout
            tokio::time::timeout(
                std::time::Duration::from_millis(self.config.acquire_timeout_ms),
                self.embedding_semaphore.clone().acquire_owned(),
            )
            .await
            .ok()
            .and_then(|r| r.ok())
        };

        let wait_ms = start.elapsed().as_millis() as u64;
        self.embedding_wait_ms.fetch_add(wait_ms, Ordering::Relaxed);

        if result.is_some() {
            self.embedding_acquired.fetch_add(1, Ordering::Relaxed);
            debug!(wait_ms = wait_ms, "Acquired embedding permit");
            crate::monitoring::metrics::record_inference_permit_acquired("embedding");
        } else {
            self.embedding_rejected.fetch_add(1, Ordering::Relaxed);
            warn!(
                wait_ms = wait_ms,
                timeout_ms = self.config.acquire_timeout_ms,
                "Failed to acquire embedding permit (timeout)"
            );
            crate::monitoring::metrics::record_inference_permit_rejected("embedding");
        }

        result
    }

    /// Acquire a permit for LLM inference operations
    /// Returns None if timeout expires or semaphore is closed
    pub async fn acquire_llm_permit(&self) -> Option<OwnedSemaphorePermit> {
        let start = std::time::Instant::now();

        let result = if self.config.acquire_timeout_ms == 0 {
            self.llm_semaphore.clone().acquire_owned().await.ok()
        } else {
            tokio::time::timeout(
                std::time::Duration::from_millis(self.config.acquire_timeout_ms),
                self.llm_semaphore.clone().acquire_owned(),
            )
            .await
            .ok()
            .and_then(|r| r.ok())
        };

        let wait_ms = start.elapsed().as_millis() as u64;
        self.llm_wait_ms.fetch_add(wait_ms, Ordering::Relaxed);

        if result.is_some() {
            self.llm_acquired.fetch_add(1, Ordering::Relaxed);
            debug!(wait_ms = wait_ms, "Acquired LLM permit");
            crate::monitoring::metrics::record_inference_permit_acquired("llm");
        } else {
            self.llm_rejected.fetch_add(1, Ordering::Relaxed);
            warn!(
                wait_ms = wait_ms,
                timeout_ms = self.config.acquire_timeout_ms,
                "Failed to acquire LLM permit (timeout)"
            );
            crate::monitoring::metrics::record_inference_permit_rejected("llm");
        }

        result
    }

    /// Try to acquire an embedding permit without waiting
    pub fn try_acquire_embedding_permit(&self) -> Option<OwnedSemaphorePermit> {
        match self.embedding_semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                self.embedding_acquired.fetch_add(1, Ordering::Relaxed);
                crate::monitoring::metrics::record_inference_permit_acquired("embedding");
                Some(permit)
            }
            Err(_) => {
                self.embedding_rejected.fetch_add(1, Ordering::Relaxed);
                crate::monitoring::metrics::record_inference_permit_rejected("embedding");
                None
            }
        }
    }

    /// Try to acquire an LLM permit without waiting
    pub fn try_acquire_llm_permit(&self) -> Option<OwnedSemaphorePermit> {
        match self.llm_semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                self.llm_acquired.fetch_add(1, Ordering::Relaxed);
                crate::monitoring::metrics::record_inference_permit_acquired("llm");
                Some(permit)
            }
            Err(_) => {
                self.llm_rejected.fetch_add(1, Ordering::Relaxed);
                crate::monitoring::metrics::record_inference_permit_rejected("llm");
                None
            }
        }
    }

    /// Get current gateway statistics
    pub fn stats(&self) -> GatewayStats {
        GatewayStats {
            embedding_permits_total: self.config.max_concurrent_embeddings,
            embedding_permits_available: self.embedding_semaphore.available_permits(),
            embedding_acquired_total: self.embedding_acquired.load(Ordering::Relaxed),
            embedding_rejected_total: self.embedding_rejected.load(Ordering::Relaxed),
            embedding_wait_total_ms: self.embedding_wait_ms.load(Ordering::Relaxed),
            llm_permits_total: self.config.max_concurrent_llm,
            llm_permits_available: self.llm_semaphore.available_permits(),
            llm_acquired_total: self.llm_acquired.load(Ordering::Relaxed),
            llm_rejected_total: self.llm_rejected.load(Ordering::Relaxed),
            llm_wait_total_ms: self.llm_wait_ms.load(Ordering::Relaxed),
        }
    }

    /// Get number of available embedding permits
    pub fn available_embedding_permits(&self) -> usize {
        self.embedding_semaphore.available_permits()
    }

    /// Get number of available LLM permits
    pub fn available_llm_permits(&self) -> usize {
        self.llm_semaphore.available_permits()
    }
}

// Global inference gateway instance
static GLOBAL_GATEWAY: OnceCell<Arc<InferenceGateway>> = OnceCell::new();

/// Get or initialize the global inference gateway
pub fn global_gateway() -> &'static Arc<InferenceGateway> {
    GLOBAL_GATEWAY.get_or_init(|| {
        let config = InferenceGatewayConfig::from_env();
        Arc::new(InferenceGateway::new(config))
    })
}

/// Convenience function to acquire an embedding permit from the global gateway
pub async fn acquire_embedding_permit() -> Option<OwnedSemaphorePermit> {
    global_gateway().acquire_embedding_permit().await
}

/// Convenience function to acquire an LLM permit from the global gateway
pub async fn acquire_llm_permit() -> Option<OwnedSemaphorePermit> {
    global_gateway().acquire_llm_permit().await
}

/// Get stats from the global gateway
pub fn gateway_stats() -> GatewayStats {
    global_gateway().stats()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = InferenceGatewayConfig::default();
        assert_eq!(config.max_concurrent_embeddings, 4);
        assert_eq!(config.max_concurrent_llm, 2);
        assert_eq!(config.acquire_timeout_ms, 30_000);
    }

    #[tokio::test]
    async fn test_acquire_embedding_permit() {
        let gateway = InferenceGateway::new(InferenceGatewayConfig {
            max_concurrent_embeddings: 2,
            max_concurrent_llm: 1,
            acquire_timeout_ms: 1000,
        });

        // Should be able to acquire 2 permits
        let p1 = gateway.acquire_embedding_permit().await;
        assert!(p1.is_some());

        let p2 = gateway.acquire_embedding_permit().await;
        assert!(p2.is_some());

        // Third should timeout (we only have 2 permits)
        let start = std::time::Instant::now();
        let p3 = gateway.acquire_embedding_permit().await;
        assert!(p3.is_none());
        assert!(start.elapsed().as_millis() >= 900); // Should have waited ~1000ms

        // Drop one permit
        drop(p1);

        // Now we should be able to acquire again
        let p4 = gateway.acquire_embedding_permit().await;
        assert!(p4.is_some());
    }

    #[tokio::test]
    async fn test_try_acquire() {
        let gateway = InferenceGateway::new(InferenceGatewayConfig {
            max_concurrent_embeddings: 1,
            max_concurrent_llm: 1,
            acquire_timeout_ms: 0,
        });

        let p1 = gateway.try_acquire_embedding_permit();
        assert!(p1.is_some());

        // Second try should fail immediately
        let p2 = gateway.try_acquire_embedding_permit();
        assert!(p2.is_none());

        drop(p1);

        let p3 = gateway.try_acquire_embedding_permit();
        assert!(p3.is_some());
    }

    #[tokio::test]
    async fn test_stats() {
        let gateway = InferenceGateway::new(InferenceGatewayConfig {
            max_concurrent_embeddings: 2,
            max_concurrent_llm: 1,
            acquire_timeout_ms: 100,
        });

        let stats = gateway.stats();
        assert_eq!(stats.embedding_permits_total, 2);
        assert_eq!(stats.embedding_permits_available, 2);
        assert_eq!(stats.embedding_acquired_total, 0);

        let _p = gateway.acquire_embedding_permit().await;
        let stats = gateway.stats();
        assert_eq!(stats.embedding_permits_available, 1);
        assert_eq!(stats.embedding_acquired_total, 1);
    }
}
