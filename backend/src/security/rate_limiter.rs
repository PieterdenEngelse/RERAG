use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Runtime-mutable search/upload QPS and burst thresholds.
/// All fields are atomics so they can be updated without locking.
#[derive(Debug)]
pub struct RuntimeThresholds {
    search_qps: AtomicU64,
    search_burst: AtomicU64,
    upload_qps: AtomicU64,
    upload_burst: AtomicU64,
}

impl RuntimeThresholds {
    pub fn new(
        search_qps: f64,
        search_burst: f64,
        upload_qps: f64,
        upload_burst: f64,
    ) -> Arc<Self> {
        Arc::new(Self {
            search_qps: AtomicU64::new(search_qps.to_bits()),
            search_burst: AtomicU64::new(search_burst.to_bits()),
            upload_qps: AtomicU64::new(upload_qps.to_bits()),
            upload_burst: AtomicU64::new(upload_burst.to_bits()),
        })
    }
    pub fn get_search_qps(&self) -> f64 {
        f64::from_bits(self.search_qps.load(Ordering::Relaxed))
    }
    pub fn get_search_burst(&self) -> f64 {
        f64::from_bits(self.search_burst.load(Ordering::Relaxed))
    }
    pub fn get_upload_qps(&self) -> f64 {
        f64::from_bits(self.upload_qps.load(Ordering::Relaxed))
    }
    pub fn get_upload_burst(&self) -> f64 {
        f64::from_bits(self.upload_burst.load(Ordering::Relaxed))
    }
    pub fn set(&self, search_qps: f64, search_burst: f64, upload_qps: f64, upload_burst: f64) {
        self.search_qps
            .store(search_qps.to_bits(), Ordering::Relaxed);
        self.search_burst
            .store(search_burst.to_bits(), Ordering::Relaxed);
        self.upload_qps
            .store(upload_qps.to_bits(), Ordering::Relaxed);
        self.upload_burst
            .store(upload_burst.to_bits(), Ordering::Relaxed);
    }
    pub fn snapshot(&self) -> ThresholdsSnapshot {
        ThresholdsSnapshot {
            search_qps: self.get_search_qps(),
            search_burst: self.get_search_burst(),
            upload_qps: self.get_upload_qps(),
            upload_burst: self.get_upload_burst(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdsSnapshot {
    pub search_qps: f64,
    pub search_burst: f64,
    pub upload_qps: f64,
    pub upload_burst: f64,
}

#[derive(Clone, Debug)]
pub struct RateLimiterConfig {
    pub enabled: bool,
    pub qps: f64,
    pub burst: f64,
    pub max_ips: usize,
}

#[derive(Clone, Debug)]
struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

/// Per-key token-bucket rate limiter with LRU-bounded state
pub struct RateLimiter {
    config: RateLimiterConfig,
    enabled: AtomicBool,
    buckets: Mutex<LruCache<String, Bucket>>,
    pub thresholds: Arc<RuntimeThresholds>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RateLimiterState {
    pub enabled: bool,
    pub active_keys: usize,
    pub capacity: usize,
}

impl RateLimiter {
    pub fn new(config: RateLimiterConfig, thresholds: Arc<RuntimeThresholds>) -> Self {
        let cap = NonZeroUsize::new(config.max_ips.max(1)).unwrap();
        let enabled = config.enabled;
        Self {
            config,
            enabled: AtomicBool::new(enabled),
            buckets: Mutex::new(LruCache::new(cap)),
            thresholds,
        }
    }

    pub fn snapshot(&self) -> RateLimiterState {
        let guard = self.buckets.lock().expect("rate limiter mutex poisoned");
        RateLimiterState {
            enabled: self.enabled.load(Ordering::Relaxed),
            active_keys: guard.len(),
            capacity: self.config.max_ips,
        }
    }

    /// Toggle the rate limiter on/off and return the new state
    pub fn set_enabled(&self, enabled: bool) -> bool {
        self.enabled.store(enabled, Ordering::Relaxed);
        enabled
    }

    /// Check if rate limiter is currently enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn key_for(ip: IpAddr) -> String {
        ip.to_string()
    }

    /// Compatibility API used by middleware: override QPS per call; burst from config
    pub fn check_rate_limit(&self, ip: IpAddr, qps_override: f64) -> bool {
        if !self.is_enabled() {
            return true;
        }
        let qps = if qps_override > 0.0 {
            qps_override
        } else {
            self.config.qps
        };
        let burst = self.config.burst.max(0.0);
        let key = Self::key_for(ip);
        let (allow, _retry) = self.check_key(&key, qps, burst);
        allow
    }

    /// General check for an arbitrary key; returns (allow, retry_after_secs)
    pub fn check_key(&self, key: &str, qps: f64, burst: f64) -> (bool, u64) {
        if !self.is_enabled() {
            return (true, 0);
        }
        if burst <= 0.0 {
            return (false, 1);
        }

        let now = Instant::now();
        let mut map = self.buckets.lock().expect("rate limiter mutex poisoned");

        // Test-only optional discrete refill mode controlled via env var
        // RATE_LIMIT_DISCRETE_REFILL=true enables discrete refill using interval RATE_LIMIT_REFILL_INTERVAL_MS (default 1000ms)
        let discrete = std::env::var("RATE_LIMIT_DISCRETE_REFILL")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);
        let interval_ms: u64 = std::env::var("RATE_LIMIT_REFILL_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);

        if let Some(bucket) = map.get_mut(key) {
            if qps <= 0.0 {
                // No refill when qps<=0; allow only while tokens remain
                if bucket.tokens >= 1.0 {
                    bucket.tokens -= 1.0;
                    return (true, 0);
                } else {
                    return (false, 1);
                }
            }
            // Refill tokens
            if discrete {
                // Add whole tokens per complete intervals elapsed
                let elapsed = now.duration_since(bucket.last_refill);
                let intervals = (elapsed.as_millis() as u64) / interval_ms.max(1);
                if intervals > 0 {
                    let add = (intervals as f64) * qps;
                    bucket.tokens = (bucket.tokens + add).min(burst);
                    // advance last_refill by full intervals
                    let advance = Duration::from_millis(intervals * interval_ms);
                    bucket.last_refill += advance;
                }
            } else {
                let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
                bucket.tokens = (bucket.tokens + elapsed * qps).min(burst);
                bucket.last_refill = now;
            }

            if bucket.tokens >= 1.0 {
                bucket.tokens -= 1.0;
                (true, 0)
            } else {
                let needed = 1.0 - bucket.tokens;
                let secs = (needed / qps).ceil() as u64;
                (false, secs.max(1))
            }
        } else {
            // New bucket: allow immediately if burst allows at least one token
            let allow = burst >= 1.0;
            let tokens = if allow { (burst - 1.0).max(0.0) } else { 0.0 };
            map.put(
                key.to_string(),
                Bucket {
                    tokens,
                    last_refill: now,
                },
            );
            if allow {
                (true, 0)
            } else {
                (false, 1)
            }
        }
    }

    /// Compute Retry-After for a key at current state and qps
    pub fn compute_retry_after(&self, key: &str, qps: f64) -> u64 {
        if !self.config.enabled {
            return 0;
        }
        if qps <= 0.0 {
            return 1;
        }

        let now = Instant::now();
        let mut map = self.buckets.lock().expect("rate limiter mutex poisoned");

        // Mirror discrete refill logic for retry-after calculation
        let discrete = std::env::var("RATE_LIMIT_DISCRETE_REFILL")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);
        let interval_ms: u64 = std::env::var("RATE_LIMIT_REFILL_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);

        if let Some(bucket) = map.get_mut(key) {
            if discrete {
                let elapsed = now.duration_since(bucket.last_refill);
                let intervals = (elapsed.as_millis() as u64) / interval_ms.max(1);
                if intervals > 0 {
                    let add = (intervals as f64) * qps;
                    bucket.tokens = (bucket.tokens + add).min(self.config.burst.max(0.0));
                    let advance = Duration::from_millis(intervals * interval_ms);
                    bucket.last_refill += advance;
                }
            } else {
                let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
                bucket.tokens = (bucket.tokens + elapsed * qps).min(self.config.burst.max(0.0));
                bucket.last_refill = now;
            }

            if bucket.tokens >= 1.0 {
                0
            } else if discrete {
                // compute number of full intervals to reach >=1 token
                let needed = 1.0 - bucket.tokens;
                let intervals_needed = (needed / qps).ceil().max(1.0) as u64;
                intervals_needed * interval_ms / 1000 // seconds
            } else {
                let needed = 1.0 - bucket.tokens;
                (needed / qps).ceil() as u64
            }
        } else {
            1
        }
    }
}
