// src/cache/redis_cache.rs - Phase 12: Redis L3 Cache - Version 1.0.0

use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisCacheSummary {
    pub enabled: bool,
    pub connected: bool,
    pub ttl_seconds: u64,
}

/// Redis-backed distributed L3 cache
#[derive(Clone)]
pub struct RedisCache {
    client: Option<ConnectionManager>,
    ttl: Duration,
    enabled: bool,
}

impl RedisCache {
    /// Create new Redis cache connection
    /// Uses a 5-second timeout to prevent blocking if Redis is unavailable
    pub async fn new(redis_url: &str, ttl_secs: u64) -> Result<Self, Box<dyn std::error::Error>> {
        const CONNECTION_TIMEOUT_SECS: u64 = 5;
        
        match redis::Client::open(redis_url) {
            Ok(client) => {
                // Wrap connection in timeout to prevent indefinite blocking
                match timeout(
                    Duration::from_secs(CONNECTION_TIMEOUT_SECS),
                    ConnectionManager::new(client),
                )
                .await
                {
                    Ok(Ok(manager)) => {
                        info!("Redis L3 cache connected (TTL: {} seconds)", ttl_secs);
                        Ok(Self {
                            client: Some(manager),
                            ttl: Duration::from_secs(ttl_secs),
                            enabled: true,
                        })
                    }
                    Ok(Err(e)) => {
                        error!("Failed to create Redis connection manager: {}", e);
                        warn!("Continuing without L3 cache...");
                        Ok(Self::disabled())
                    }
                    Err(_) => {
                        error!(
                            "Redis connection timed out after {} seconds - is Redis running?",
                            CONNECTION_TIMEOUT_SECS
                        );
                        warn!("Continuing without L3 cache...");
                        Ok(Self::disabled())
                    }
                }
            }
            Err(e) => {
                error!("Failed to open Redis client: {}", e);
                warn!("Continuing without L3 cache...");
                Ok(Self::disabled())
            }
        }
    }

    /// Create disabled Redis cache (fallback)
    pub fn disabled() -> Self {
        Self {
            client: None,
            ttl: Duration::from_secs(3600),
            enabled: false,
        }
    }

    /// Check if Redis is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled && self.client.is_some()
    }

    pub fn summary(&self) -> RedisCacheSummary {
        RedisCacheSummary {
            enabled: self.enabled,
            connected: self.client.is_some(),
            ttl_seconds: self.ttl.as_secs(),
        }
    }

    /// Get value from Redis
    pub async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
    ) -> Result<Option<T>, Box<dyn std::error::Error>> {
        if !self.is_enabled() {
            return Ok(None);
        }

        if let Some(client) = &self.client {
            let mut conn = client.clone();
            let value: Option<String> = conn.get(key).await?;

            match value {
                Some(json) => {
                    let deserialized = serde_json::from_str(&json)?;
                    Ok(Some(deserialized))
                }
                None => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    /// Set value in Redis with TTL
    pub async fn set<T: Serialize>(
        &self,
        key: &str,
        value: &T,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_enabled() {
            return Ok(());
        }

        if let Some(client) = &self.client {
            let mut conn = client.clone();
            let json = serde_json::to_string(value)?;
            let ttl_secs = self.ttl.as_secs();

            conn.set_ex::<_, _, ()>(key, json, ttl_secs).await?;
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Delete key from Redis
    pub async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_enabled() {
            return Ok(());
        }

        if let Some(client) = &self.client {
            let mut conn = client.clone();
            conn.del::<_, ()>(key).await?;
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Clear all keys matching pattern
    pub async fn clear_pattern(&self, pattern: &str) -> Result<u32, Box<dyn std::error::Error>> {
        if !self.is_enabled() {
            return Ok(0);
        }

        if let Some(client) = &self.client {
            let mut conn = client.clone();
            let keys: Vec<String> = conn.keys(pattern).await?;
            let count = keys.len() as u32;

            if !keys.is_empty() {
                conn.del::<_, ()>(keys).await?;
            }

            Ok(count)
        } else {
            Ok(0)
        }
    }

    /// Get cache health/ping
    pub async fn health_check(&self) -> Result<String, Box<dyn std::error::Error>> {
        if !self.is_enabled() {
            return Ok("Redis disabled".to_string());
        }

        if let Some(client) = &self.client {
            let mut conn = client.clone();
            let result: String = redis::cmd("PING").query_async::<String>(&mut conn).await?;
            Ok(result)
        } else {
            Ok("Redis disabled".to_string())
        }
    }

    /// Get Redis info/stats
    pub async fn get_info(&self) -> Result<String, Box<dyn std::error::Error>> {
        if !self.is_enabled() {
            return Ok("Redis disabled".to_string());
        }

        if let Some(client) = &self.client {
            let mut conn = client.clone();
            let result: String = redis::cmd("INFO")
                .arg("stats")
                .query_async::<String>(&mut conn)
                .await?;
            Ok(result)
        } else {
            Ok("Redis disabled".to_string())
        }
    }

    /// Get number of keys in Redis
    pub async fn key_count(&self) -> Result<usize, Box<dyn std::error::Error>> {
        if !self.is_enabled() {
            return Ok(0);
        }

        if let Some(client) = &self.client {
            let mut conn = client.clone();
            let count: usize = redis::cmd("DBSIZE").query_async::<usize>(&mut conn).await?;
            Ok(count)
        } else {
            Ok(0)
        }
    }

    /// Flush all keys
    pub async fn flush_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_enabled() {
            return Ok(());
        }

        if let Some(client) = &self.client {
            let mut conn = client.clone();
            redis::cmd("FLUSHDB").query_async::<()>(&mut conn).await?;
            Ok(())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_redis_connection() {
        let cache = RedisCache::new("redis://127.0.0.1:6379/", 60)
            .await
            .unwrap();
        assert!(cache.is_enabled());
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_set_get() {
        let cache = RedisCache::new("redis://127.0.0.1:6379/", 60)
            .await
            .unwrap();

        let test_value = vec!["result1".to_string(), "result2".to_string()];
        cache.set("test_key", &test_value).await.unwrap();

        let retrieved: Vec<String> = cache.get("test_key").await.unwrap().unwrap();
        assert_eq!(retrieved.len(), 2);

        cache.delete("test_key").await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_health_check() {
        let cache = RedisCache::new("redis://127.0.0.1:6379/", 60)
            .await
            .unwrap();
        let health = cache.health_check().await;
        assert!(health.is_ok());
    }
}
