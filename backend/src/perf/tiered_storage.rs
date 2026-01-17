//! Tiered Storage
//! 
//! Manages data across multiple storage tiers:
//! - Hot: In-memory, fastest access
//! - Warm: SSD/fast disk, good performance
//! - Cold: HDD/archive, cost-effective
//! 
//! # Benefits
//! - Optimizes cost vs performance
//! - Automatic data migration based on access patterns
//! - Transparent access across tiers

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

/// Storage tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageTier {
    /// In-memory, fastest
    Hot,
    /// SSD/fast disk
    Warm,
    /// HDD/archive
    Cold,
}

impl StorageTier {
    pub fn latency_estimate(&self) -> Duration {
        match self {
            Self::Hot => Duration::from_micros(1),
            Self::Warm => Duration::from_millis(1),
            Self::Cold => Duration::from_millis(10),
        }
    }

    pub fn cost_per_gb(&self) -> f64 {
        match self {
            Self::Hot => 10.0,   // RAM is expensive
            Self::Warm => 0.5,   // SSD
            Self::Cold => 0.02,  // HDD/S3
        }
    }
}

/// Item metadata for tiering decisions
#[derive(Debug, Clone)]
pub struct ItemMetadata {
    pub key: String,
    pub size_bytes: usize,
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub access_count: u64,
    pub current_tier: StorageTier,
}

impl ItemMetadata {
    pub fn new(key: String, size_bytes: usize) -> Self {
        let now = Instant::now();
        Self {
            key,
            size_bytes,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            current_tier: StorageTier::Hot,
        }
    }

    pub fn record_access(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }

    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    pub fn idle_time(&self) -> Duration {
        self.last_accessed.elapsed()
    }
}

/// Tiering policy
#[derive(Debug, Clone)]
pub struct TieringPolicy {
    /// Move to warm after this idle time
    pub hot_to_warm_idle: Duration,
    /// Move to cold after this idle time
    pub warm_to_cold_idle: Duration,
    /// Promote to hot if accessed this many times
    pub promote_access_threshold: u64,
    /// Maximum size for hot tier (bytes)
    pub hot_tier_max_bytes: usize,
    /// Maximum size for warm tier (bytes)
    pub warm_tier_max_bytes: usize,
}

impl Default for TieringPolicy {
    fn default() -> Self {
        Self {
            hot_to_warm_idle: Duration::from_secs(300),      // 5 minutes
            warm_to_cold_idle: Duration::from_secs(3600),    // 1 hour
            promote_access_threshold: 10,
            hot_tier_max_bytes: 1024 * 1024 * 1024,          // 1 GB
            warm_tier_max_bytes: 10 * 1024 * 1024 * 1024,    // 10 GB
        }
    }
}

/// Tiered storage manager
pub struct TieredStorage<T> {
    /// Hot tier (in-memory)
    hot: DashMap<String, T>,
    /// Warm tier paths
    warm_path: PathBuf,
    /// Cold tier paths
    cold_path: PathBuf,
    /// Item metadata
    metadata: DashMap<String, ItemMetadata>,
    /// Policy
    policy: TieringPolicy,
    /// Statistics
    stats: TierStats,
}

/// Tier statistics
#[derive(Debug, Default)]
pub struct TierStats {
    pub hot_items: AtomicU64,
    pub warm_items: AtomicU64,
    pub cold_items: AtomicU64,
    pub hot_bytes: AtomicU64,
    pub warm_bytes: AtomicU64,
    pub cold_bytes: AtomicU64,
    pub promotions: AtomicU64,
    pub demotions: AtomicU64,
}

impl<T: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static> TieredStorage<T> {
    pub fn new(warm_path: PathBuf, cold_path: PathBuf, policy: TieringPolicy) -> Self {
        // Create directories if they don't exist
        let _ = std::fs::create_dir_all(&warm_path);
        let _ = std::fs::create_dir_all(&cold_path);

        Self {
            hot: DashMap::new(),
            warm_path,
            cold_path,
            metadata: DashMap::new(),
            policy,
            stats: TierStats::default(),
        }
    }

    /// Get an item, promoting if necessary
    pub fn get(&self, key: &str) -> Option<T> {
        // Check hot tier first
        if let Some(item) = self.hot.get(key) {
            if let Some(mut meta) = self.metadata.get_mut(key) {
                meta.record_access();
            }
            return Some(item.clone());
        }

        // Check warm tier
        if let Some(item) = self.load_from_warm(key) {
            // Consider promotion
            if let Some(meta) = self.metadata.get(key) {
                if meta.access_count >= self.policy.promote_access_threshold {
                    self.promote_to_hot(key, item.clone());
                }
            }
            return Some(item);
        }

        // Check cold tier
        if let Some(item) = self.load_from_cold(key) {
            return Some(item);
        }

        None
    }

    /// Put an item (always starts in hot tier)
    pub fn put(&self, key: String, item: T, size_bytes: usize) {
        let meta = ItemMetadata::new(key.clone(), size_bytes);
        self.metadata.insert(key.clone(), meta);
        self.hot.insert(key, item);
        self.stats.hot_items.fetch_add(1, Ordering::Relaxed);
        self.stats.hot_bytes.fetch_add(size_bytes as u64, Ordering::Relaxed);
    }

    /// Remove an item from all tiers
    pub fn remove(&self, key: &str) -> bool {
        let removed = self.hot.remove(key).is_some()
            || self.remove_from_warm(key)
            || self.remove_from_cold(key);
        
        self.metadata.remove(key);
        removed
    }

    /// Run tiering maintenance
    pub fn maintain(&self) {
        let mut to_demote_warm: Vec<String> = Vec::new();
        let mut to_demote_cold: Vec<String> = Vec::new();

        for entry in self.metadata.iter() {
            let meta = entry.value();
            let idle = meta.idle_time();

            match meta.current_tier {
                StorageTier::Hot => {
                    if idle >= self.policy.hot_to_warm_idle {
                        to_demote_warm.push(meta.key.clone());
                    }
                }
                StorageTier::Warm => {
                    if idle >= self.policy.warm_to_cold_idle {
                        to_demote_cold.push(meta.key.clone());
                    }
                }
                StorageTier::Cold => {}
            }
        }

        // Demote items
        for key in to_demote_warm {
            self.demote_to_warm(&key);
        }
        for key in to_demote_cold {
            self.demote_to_cold(&key);
        }
    }

    fn promote_to_hot(&self, key: &str, item: T) {
        self.hot.insert(key.to_string(), item);
        self.remove_from_warm(key);
        
        if let Some(mut meta) = self.metadata.get_mut(key) {
            meta.current_tier = StorageTier::Hot;
        }
        
        self.stats.promotions.fetch_add(1, Ordering::Relaxed);
    }

    fn demote_to_warm(&self, key: &str) {
        if let Some((_, item)) = self.hot.remove(key) {
            self.save_to_warm(key, &item);
            
            if let Some(mut meta) = self.metadata.get_mut(key) {
                meta.current_tier = StorageTier::Warm;
            }
            
            self.stats.demotions.fetch_add(1, Ordering::Relaxed);
            self.stats.hot_items.fetch_sub(1, Ordering::Relaxed);
            self.stats.warm_items.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn demote_to_cold(&self, key: &str) {
        if let Some(item) = self.load_from_warm(key) {
            self.remove_from_warm(key);
            self.save_to_cold(key, &item);
            
            if let Some(mut meta) = self.metadata.get_mut(key) {
                meta.current_tier = StorageTier::Cold;
            }
            
            self.stats.demotions.fetch_add(1, Ordering::Relaxed);
            self.stats.warm_items.fetch_sub(1, Ordering::Relaxed);
            self.stats.cold_items.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn warm_path_for(&self, key: &str) -> PathBuf {
        self.warm_path.join(format!("{}.bin", seahash::hash(key.as_bytes())))
    }

    fn cold_path_for(&self, key: &str) -> PathBuf {
        self.cold_path.join(format!("{}.bin", seahash::hash(key.as_bytes())))
    }

    fn save_to_warm(&self, key: &str, item: &T) {
        let path = self.warm_path_for(key);
        if let Ok(json) = serde_json::to_vec(item) {
            let _ = std::fs::write(path, &json);
        }
    }

    fn load_from_warm(&self, key: &str) -> Option<T> {
        let path = self.warm_path_for(key);
        std::fs::read(path).ok().and_then(|bytes| {
            serde_json::from_slice(&bytes).ok()
        })
    }

    fn remove_from_warm(&self, key: &str) -> bool {
        let path = self.warm_path_for(key);
        std::fs::remove_file(path).is_ok()
    }

    fn save_to_cold(&self, key: &str, item: &T) {
        let path = self.cold_path_for(key);
        if let Ok(json) = serde_json::to_vec(item) {
            // Compress for cold storage
            let compressed = lz4_flex::compress_prepend_size(&json);
            let _ = std::fs::write(path, &compressed);
        }
    }

    fn load_from_cold(&self, key: &str) -> Option<T> {
        let path = self.cold_path_for(key);
        std::fs::read(path).ok().and_then(|compressed| {
            lz4_flex::decompress_size_prepended(&compressed).ok()
        }).and_then(|bytes| {
            serde_json::from_slice(&bytes).ok()
        })
    }

    fn remove_from_cold(&self, key: &str) -> bool {
        let path = self.cold_path_for(key);
        std::fs::remove_file(path).is_ok()
    }

    /// Get statistics
    pub fn stats(&self) -> TierStatsSnapshot {
        TierStatsSnapshot {
            hot_items: self.stats.hot_items.load(Ordering::Relaxed),
            warm_items: self.stats.warm_items.load(Ordering::Relaxed),
            cold_items: self.stats.cold_items.load(Ordering::Relaxed),
            hot_bytes: self.stats.hot_bytes.load(Ordering::Relaxed),
            warm_bytes: self.stats.warm_bytes.load(Ordering::Relaxed),
            cold_bytes: self.stats.cold_bytes.load(Ordering::Relaxed),
            promotions: self.stats.promotions.load(Ordering::Relaxed),
            demotions: self.stats.demotions.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of tier statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierStatsSnapshot {
    pub hot_items: u64,
    pub warm_items: u64,
    pub cold_items: u64,
    pub hot_bytes: u64,
    pub warm_bytes: u64,
    pub cold_bytes: u64,
    pub promotions: u64,
    pub demotions: u64,
}

impl TierStatsSnapshot {
    pub fn total_items(&self) -> u64 {
        self.hot_items + self.warm_items + self.cold_items
    }

    pub fn total_bytes(&self) -> u64 {
        self.hot_bytes + self.warm_bytes + self.cold_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_tier() {
        assert!(StorageTier::Hot.latency_estimate() < StorageTier::Warm.latency_estimate());
        assert!(StorageTier::Warm.latency_estimate() < StorageTier::Cold.latency_estimate());
    }

    #[test]
    fn test_item_metadata() {
        let mut meta = ItemMetadata::new("test".to_string(), 100);
        assert_eq!(meta.access_count, 0);
        
        meta.record_access();
        assert_eq!(meta.access_count, 1);
    }

    #[test]
    fn test_tiering_policy() {
        let policy = TieringPolicy::default();
        assert!(policy.hot_to_warm_idle < policy.warm_to_cold_idle);
    }
}
