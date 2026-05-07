// src/cache/mod.rs - Version 1.0.0
// Phase 11: Caching Layer Module
// Uses async-trait to properly handle async functions in traits

pub mod cache_layer;
pub mod redis_cache;
pub use redis_cache::RedisCache;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use lazy_static::lazy_static;
use std::collections::HashMap;

pub struct ResultCache {
    cache: HashMap<String, String>,
}

impl Default for ResultCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub async fn get(&self, tool_type: &str, query: &str) -> Option<String> {
        let key = format!("{}_{}", tool_type, query);
        self.cache.get(&key).cloned()
    }

    pub async fn set(&mut self, tool_type: &str, query: String, result: String) {
        let key = format!("{}_{}", tool_type, query);
        self.cache.insert(key, result);
    }

    pub async fn clear(&mut self) {
        self.cache.clear();
    }

    pub async fn size(&self) -> usize {
        self.cache.len()
    }
}

lazy_static! {
    pub static ref RESULT_CACHE: tokio::sync::Mutex<ResultCache> =
        tokio::sync::Mutex::new(ResultCache::new());
}

#[derive(Debug, Clone)]
pub enum CacheLayer {
    L1, // In-process LRU (existing)
    L2, // Persistent SQLite cache
    L3, // Distributed Redis (optional)
}

pub struct CacheConfig {
    pub l1_capacity: usize,
    pub l1_ttl: Duration,
    pub l2_ttl: Duration,
    pub l3_enabled: bool,
    pub l3_redis_url: Option<String>,
}

#[async_trait]
pub trait CacheBackend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>>;
    async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Duration,
    ) -> Result<(), Box<dyn std::error::Error>>;
    async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error>>;
    async fn clear(&self) -> Result<(), Box<dyn std::error::Error>>;
    async fn stats(&self) -> CacheStats;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub size_bytes: u64,
    pub item_count: usize,
}
