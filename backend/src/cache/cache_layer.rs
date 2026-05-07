//! Phase 11: Caching Layer
//!
//! Minimal, testable multi-layer cache implementation
//! L1: In-process LRU (already in retriever)
//! L2: SQLite-backed persistent cache

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Cache statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheStats {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub total_items: usize,
}

impl CacheStats {
    pub fn total_hits(&self) -> u64 {
        self.l1_hits + self.l2_hits
    }

    pub fn total_misses(&self) -> u64 {
        self.l1_misses + self.l2_misses
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.total_hits() + self.total_misses();
        if total == 0 {
            0.0
        } else {
            self.total_hits() as f64 / total as f64
        }
    }
}

/// Cache entry with timestamp for TTL checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    pub value: T,
    pub created_at: SystemTime,
    pub hit_count: u64,
}

impl<T> CacheEntry<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            created_at: SystemTime::now(),
            hit_count: 0,
        }
    }

    pub fn is_expired(&self, ttl: Duration) -> bool {
        match self.created_at.elapsed() {
            Ok(elapsed) => elapsed > ttl,
            Err(_) => false,
        }
    }
}

/// Simple in-memory cache with TTL support (for testing)
#[derive(Debug, Clone)]
pub struct MemoryCache<K: Clone, V: Clone> {
    entries: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<K, CacheEntry<V>>>>,
    ttl: Duration,
}

impl<K: Clone + std::hash::Hash + Eq, V: Clone> MemoryCache<K, V> {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            entries: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let mut entries = self.entries.lock().unwrap();

        if let Some(entry) = entries.get_mut(key) {
            if !entry.is_expired(self.ttl) {
                entry.hit_count += 1;
                return Some(entry.value.clone());
            } else {
                entries.remove(key);
            }
        }
        None
    }

    pub fn set(&self, key: K, value: V) {
        let mut entries = self.entries.lock().unwrap();
        entries.insert(key, CacheEntry::new(value));
    }

    pub fn delete(&self, key: &K) {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(key);
    }

    pub fn clear(&self) {
        let mut entries = self.entries.lock().unwrap();
        entries.clear();
    }

    pub fn len(&self) -> usize {
        let entries = self.entries.lock().unwrap();
        entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn cleanup_expired(&self) -> usize {
        let mut entries = self.entries.lock().unwrap();
        let before = entries.len();
        entries.retain(|_, entry| !entry.is_expired(self.ttl));
        before - entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_set_get() {
        let cache = MemoryCache::<String, Vec<String>>::new(60);
        let key = "query1".to_string();
        let value = vec!["result1".to_string(), "result2".to_string()];

        cache.set(key.clone(), value.clone());
        let retrieved = cache.get(&key);

        assert_eq!(retrieved, Some(value));
    }

    #[test]
    fn test_cache_miss() {
        let cache = MemoryCache::<String, Vec<String>>::new(60);
        let key = "nonexistent".to_string();

        let retrieved = cache.get(&key);
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_cache_delete() {
        let cache = MemoryCache::<String, Vec<String>>::new(60);
        let key = "query1".to_string();
        let value = vec!["result1".to_string()];

        cache.set(key.clone(), value);
        cache.delete(&key);

        let retrieved = cache.get(&key);
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_cache_stats() {
        let stats = CacheStats {
            l1_hits: 10,
            l1_misses: 5,
            ..Default::default()
        };

        assert_eq!(stats.total_hits(), 10);
        assert_eq!(stats.total_misses(), 5);
        assert_eq!(stats.hit_rate(), 10.0 / 15.0);
    }

    #[test]
    fn test_cache_clear() {
        let cache = MemoryCache::<String, String>::new(60);
        cache.set("key1".to_string(), "value1".to_string());
        cache.set("key2".to_string(), "value2".to_string());

        assert_eq!(cache.len(), 2);
        cache.clear();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_ttl_expiration() {
        let cache = MemoryCache::<String, String>::new(1); // 1 second TTL
        cache.set("key".to_string(), "value".to_string());

        // Should exist immediately
        assert!(cache.get(&"key".to_string()).is_some());

        // After cleanup, expired entries removed
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let removed = cache.cleanup_expired();
        assert_eq!(removed, 1);
        assert!(cache.get(&"key".to_string()).is_none());
    }

    #[test]
    fn test_hit_counting() {
        let cache = MemoryCache::<String, u32>::new(60);
        let key = "counter".to_string();

        cache.set(key.clone(), 42);
        cache.get(&key);
        cache.get(&key);
        cache.get(&key);

        let entries = cache.entries.lock().unwrap();
        let entry = entries.get(&key).unwrap();
        assert_eq!(entry.hit_count, 3);
    }
}
