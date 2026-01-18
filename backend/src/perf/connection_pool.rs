//! Connection Pooling Utilities
//! 
//! Provides connection pool management for:
//! - Redis connections
//! - Database connections
//! - HTTP client connections
//! 
//! # Benefits
//! - Reduced connection overhead
//! - Better resource utilization
//! - Connection reuse

use std::sync::atomic::{AtomicUsize, Ordering};
use super::cache_aligned::CacheAligned;
use std::time::{Duration, Instant};
use tokio::sync::{Semaphore, SemaphorePermit};
use tracing::{debug, warn};

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Minimum number of connections to maintain
    pub min_connections: usize,
    /// Maximum number of connections allowed
    pub max_connections: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Idle timeout before closing a connection
    pub idle_timeout: Duration,
    /// Maximum lifetime of a connection
    pub max_lifetime: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 2,
            max_connections: 10,
            connection_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300),
            max_lifetime: Duration::from_secs(3600),
            health_check_interval: Duration::from_secs(30),
        }
    }
}

impl PoolConfig {
    /// High-throughput configuration
    pub fn high_throughput() -> Self {
        Self {
            min_connections: 5,
            max_connections: 50,
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(7200),
            health_check_interval: Duration::from_secs(60),
        }
    }

    /// Low-latency configuration
    pub fn low_latency() -> Self {
        Self {
            min_connections: 10,
            max_connections: 20,
            connection_timeout: Duration::from_secs(2),
            idle_timeout: Duration::from_secs(120),
            max_lifetime: Duration::from_secs(1800),
            health_check_interval: Duration::from_secs(15),
        }
    }
}

/// Connection pool statistics
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    pub active_connections: usize,
    pub idle_connections: usize,
    pub total_connections: usize,
    pub waiting_requests: usize,
    pub total_acquired: u64,
    pub total_released: u64,
    pub total_timeouts: u64,
    pub total_errors: u64,
}

/// Generic connection pool using semaphore for limiting
/// 
/// Stats counters are cache-line aligned to prevent false sharing
/// when multiple threads acquire/release connections concurrently.
pub struct ConnectionPool {
    config: PoolConfig,
    semaphore: Semaphore,
    // Stats - cache-line aligned to prevent false sharing
    active: CacheAligned<AtomicUsize>,
    acquired: CacheAligned<AtomicUsize>,
    released: CacheAligned<AtomicUsize>,
    timeouts: CacheAligned<AtomicUsize>,
    errors: CacheAligned<AtomicUsize>,
}

impl ConnectionPool {
    pub fn new(config: PoolConfig) -> Self {
        let semaphore = Semaphore::new(config.max_connections);
        Self {
            config,
            semaphore,
            active: CacheAligned::new(AtomicUsize::new(0)),
            acquired: CacheAligned::new(AtomicUsize::new(0)),
            released: CacheAligned::new(AtomicUsize::new(0)),
            timeouts: CacheAligned::new(AtomicUsize::new(0)),
            errors: CacheAligned::new(AtomicUsize::new(0)),
        }
    }

    /// Acquire a connection permit
    pub async fn acquire(&self) -> Result<PoolGuard<'_>, PoolError> {
        let start = Instant::now();
        
        match tokio::time::timeout(
            self.config.connection_timeout,
            self.semaphore.acquire(),
        ).await {
            Ok(Ok(permit)) => {
                self.active.fetch_add(1, Ordering::Relaxed);
                self.acquired.fetch_add(1, Ordering::Relaxed);
                debug!(
                    elapsed_ms = start.elapsed().as_millis(),
                    active = self.active.load(Ordering::Relaxed),
                    "Connection acquired"
                );
                Ok(PoolGuard {
                    pool: self,
                    _permit: permit,
                })
            }
            Ok(Err(_)) => {
                self.errors.fetch_add(1, Ordering::Relaxed);
                Err(PoolError::Closed)
            }
            Err(_) => {
                self.timeouts.fetch_add(1, Ordering::Relaxed);
                warn!(
                    timeout_ms = self.config.connection_timeout.as_millis(),
                    "Connection acquisition timed out"
                );
                Err(PoolError::Timeout)
            }
        }
    }

    /// Try to acquire without waiting
    pub fn try_acquire(&self) -> Result<PoolGuard<'_>, PoolError> {
        match self.semaphore.try_acquire() {
            Ok(permit) => {
                self.active.fetch_add(1, Ordering::Relaxed);
                self.acquired.fetch_add(1, Ordering::Relaxed);
                Ok(PoolGuard {
                    pool: self,
                    _permit: permit,
                })
            }
            Err(_) => Err(PoolError::NoAvailable),
        }
    }

    /// Release a connection (called automatically by PoolGuard)
    fn release(&self) {
        self.active.fetch_sub(1, Ordering::Relaxed);
        self.released.fetch_add(1, Ordering::Relaxed);
        debug!(
            active = self.active.load(Ordering::Relaxed),
            "Connection released"
        );
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        let active = self.active.load(Ordering::Relaxed);
        PoolStats {
            active_connections: active,
            idle_connections: self.config.max_connections - active,
            total_connections: self.config.max_connections,
            waiting_requests: 0, // Would need additional tracking
            total_acquired: self.acquired.load(Ordering::Relaxed) as u64,
            total_released: self.released.load(Ordering::Relaxed) as u64,
            total_timeouts: self.timeouts.load(Ordering::Relaxed) as u64,
            total_errors: self.errors.load(Ordering::Relaxed) as u64,
        }
    }

    /// Get configuration
    pub fn config(&self) -> &PoolConfig {
        &self.config
    }
}

/// RAII guard for connection pool
pub struct PoolGuard<'a> {
    pool: &'a ConnectionPool,
    _permit: SemaphorePermit<'a>,
}

impl<'a> Drop for PoolGuard<'a> {
    fn drop(&mut self) {
        self.pool.release();
    }
}

/// Pool error types
#[derive(Debug)]
pub enum PoolError {
    Timeout,
    NoAvailable,
    Closed,
    ConnectionError(String),
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => write!(f, "Connection acquisition timed out"),
            Self::NoAvailable => write!(f, "No connections available"),
            Self::Closed => write!(f, "Pool is closed"),
            Self::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
        }
    }
}

impl std::error::Error for PoolError {}

/// Rate limiter for connection pools
pub struct PoolRateLimiter {
    requests_per_second: f64,
    last_request: std::sync::Mutex<Instant>,
}

impl PoolRateLimiter {
    pub fn new(requests_per_second: f64) -> Self {
        Self {
            requests_per_second,
            last_request: std::sync::Mutex::new(Instant::now()),
        }
    }

    /// Wait if necessary to respect rate limit
    pub async fn wait(&self) {
        let min_interval = Duration::from_secs_f64(1.0 / self.requests_per_second);
        
        let elapsed = {
            let last = self.last_request.lock().unwrap();
            last.elapsed()
        };

        if elapsed < min_interval {
            tokio::time::sleep(min_interval - elapsed).await;
        }

        *self.last_request.lock().unwrap() = Instant::now();
    }
}

/// Connection health checker
pub struct HealthChecker {
    interval: Duration,
    last_check: std::sync::Mutex<Instant>,
    healthy: std::sync::atomic::AtomicBool,
}

impl HealthChecker {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_check: std::sync::Mutex::new(Instant::now()),
            healthy: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// Check if health check is needed
    pub fn needs_check(&self) -> bool {
        let last = self.last_check.lock().unwrap();
        last.elapsed() >= self.interval
    }

    /// Mark as healthy
    pub fn mark_healthy(&self) {
        self.healthy.store(true, Ordering::Relaxed);
        *self.last_check.lock().unwrap() = Instant::now();
    }

    /// Mark as unhealthy
    pub fn mark_unhealthy(&self) {
        self.healthy.store(false, Ordering::Relaxed);
    }

    /// Check if healthy
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_pool() {
        let config = PoolConfig {
            max_connections: 2,
            ..Default::default()
        };
        let pool = ConnectionPool::new(config);

        // Acquire two connections
        let _guard1 = pool.acquire().await.unwrap();
        let _guard2 = pool.acquire().await.unwrap();

        // Third should fail immediately with try_acquire
        assert!(pool.try_acquire().is_err());

        let stats = pool.stats();
        assert_eq!(stats.active_connections, 2);
    }

    #[tokio::test]
    async fn test_pool_release() {
        let config = PoolConfig {
            max_connections: 1,
            ..Default::default()
        };
        let pool = ConnectionPool::new(config);

        {
            let _guard = pool.acquire().await.unwrap();
            assert_eq!(pool.stats().active_connections, 1);
        }

        // Guard dropped, connection released
        assert_eq!(pool.stats().active_connections, 0);

        // Can acquire again
        let _guard = pool.acquire().await.unwrap();
        assert_eq!(pool.stats().active_connections, 1);
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = PoolRateLimiter::new(10.0); // 10 requests per second
        
        let start = Instant::now();
        for _ in 0..3 {
            limiter.wait().await;
        }
        let elapsed = start.elapsed();
        
        // Should take at least 200ms for 3 requests at 10/s
        assert!(elapsed >= Duration::from_millis(180));
    }
}
