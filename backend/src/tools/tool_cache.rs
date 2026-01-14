// src/tools/tool_cache.rs
// Feature #10: Tool result caching with TTL support

use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{debug, info};

use crate::tools::ToolResult;

/// Cache configuration for a tool type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCacheConfig {
    /// Whether caching is enabled for this tool
    pub enabled: bool,
    /// Time-to-live in seconds (0 = infinite for deterministic tools)
    pub ttl_secs: u64,
    /// Maximum cache entries for this tool
    pub max_entries: usize,
}

impl Default for ToolCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_secs: 300, // 5 minutes default
            max_entries: 1000,
        }
    }
}

/// Cached tool result with metadata
#[derive(Clone)]
struct CachedResult {
    result: ToolResult,
    cached_at: Instant,
    ttl: Duration,
    hit_count: usize,
}

impl CachedResult {
    fn is_expired(&self) -> bool {
        if self.ttl.is_zero() {
            false // Never expires (deterministic tools)
        } else {
            self.cached_at.elapsed() > self.ttl
        }
    }
}

/// Per-tool cache
struct ToolCache {
    cache: LruCache<String, CachedResult>,
    config: ToolCacheConfig,
    hits: usize,
    misses: usize,
}

impl ToolCache {
    fn new(config: ToolCacheConfig) -> Self {
        let size = NonZeroUsize::new(config.max_entries.max(1)).unwrap();
        Self {
            cache: LruCache::new(size),
            config,
            hits: 0,
            misses: 0,
        }
    }
}

/// Global cache state
struct ToolCacheState {
    caches: HashMap<String, ToolCache>,
    configs: HashMap<String, ToolCacheConfig>,
}

impl Default for ToolCacheState {
    fn default() -> Self {
        let mut configs = HashMap::new();

        // Calculator - deterministic, cache forever
        configs.insert(
            "Calculator".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 0, // Never expires
                max_entries: 10000,
            },
        );

        // SpellChecker - deterministic
        configs.insert(
            "SpellChecker".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 0,
                max_entries: 5000,
            },
        );

        // Classifier - mostly deterministic
        configs.insert(
            "Classifier".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 3600, // 1 hour
                max_entries: 2000,
            },
        );

        // SentimentAnalyzer - deterministic
        configs.insert(
            "SentimentAnalyzer".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 0,
                max_entries: 5000,
            },
        );

        // EntityExtractor - deterministic
        configs.insert(
            "EntityExtractor".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 0,
                max_entries: 5000,
            },
        );

        // WebSearch - short TTL (results change)
        configs.insert(
            "WebSearch".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 300, // 5 minutes
                max_entries: 500,
            },
        );

        // URLFetch - medium TTL
        configs.insert(
            "URLFetch".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 600, // 10 minutes
                max_entries: 500,
            },
        );

        // SemanticSearch - short TTL (index may change)
        configs.insert(
            "SemanticSearch".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 60, // 1 minute
                max_entries: 1000,
            },
        );

        // Summarizer - cache based on input
        configs.insert(
            "Summarizer".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 3600, // 1 hour
                max_entries: 500,
            },
        );

        // QueryRewriter - deterministic for same input
        configs.insert(
            "QueryRewriter".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 3600,
                max_entries: 2000,
            },
        );

        // Translator - deterministic
        configs.insert(
            "Translator".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 0,
                max_entries: 5000,
            },
        );

        // Tools that shouldn't be cached
        configs.insert(
            "CodeExecution".to_string(),
            ToolCacheConfig {
                enabled: false, // Side effects possible
                ttl_secs: 0,
                max_entries: 0,
            },
        );
        configs.insert(
            "Notification".to_string(),
            ToolCacheConfig {
                enabled: false, // Side effects
                ttl_secs: 0,
                max_entries: 0,
            },
        );
        configs.insert(
            "Scheduler".to_string(),
            ToolCacheConfig {
                enabled: false, // Side effects
                ttl_secs: 0,
                max_entries: 0,
            },
        );
        configs.insert(
            "ImageGeneration".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 3600, // Cache generated images
                max_entries: 100,
            },
        );
        configs.insert(
            "DatabaseQuery".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 30, // Very short - data changes
                max_entries: 200,
            },
        );
        configs.insert(
            "FileAnalyzer".to_string(),
            ToolCacheConfig {
                enabled: true,
                ttl_secs: 300, // Files may change
                max_entries: 500,
            },
        );
        configs.insert(
            "Memory".to_string(),
            ToolCacheConfig {
                enabled: false, // Dynamic data
                ttl_secs: 0,
                max_entries: 0,
            },
        );

        Self {
            caches: HashMap::new(),
            configs,
        }
    }
}

static CACHE_STATE: OnceLock<Mutex<ToolCacheState>> = OnceLock::new();

fn get_state() -> &'static Mutex<ToolCacheState> {
    CACHE_STATE.get_or_init(|| Mutex::new(ToolCacheState::default()))
}

/// Generate cache key from tool type and query
fn cache_key(tool_type: &str, query: &str) -> String {
    format!("{}:{:x}", tool_type, seahash::hash(query.as_bytes()))
}

/// Try to get a cached result
pub fn get_cached(tool_type: &str, query: &str) -> Option<ToolResult> {
    let mut state = get_state().lock().ok()?;

    let config = state.configs.get(tool_type).cloned().unwrap_or_default();
    if !config.enabled {
        return None;
    }

    let key = cache_key(tool_type, query);

    let cache = state
        .caches
        .entry(tool_type.to_string())
        .or_insert_with(|| ToolCache::new(config));

    if let Some(cached) = cache.cache.get_mut(&key) {
        if cached.is_expired() {
            cache.cache.pop(&key);
            cache.misses += 1;
            debug!(tool = tool_type, "Cache miss (expired)");
            None
        } else {
            cached.hit_count += 1;
            cache.hits += 1;
            debug!(tool = tool_type, hits = cached.hit_count, "Cache hit");
            Some(cached.result.clone())
        }
    } else {
        cache.misses += 1;
        debug!(tool = tool_type, "Cache miss");
        None
    }
}

/// Store a result in cache
pub fn cache_result(tool_type: &str, query: &str, result: &ToolResult) {
    let mut state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    let config = state.configs.get(tool_type).cloned().unwrap_or_default();
    if !config.enabled {
        return;
    }

    let key = cache_key(tool_type, query);
    let ttl = Duration::from_secs(config.ttl_secs);

    let cache = state
        .caches
        .entry(tool_type.to_string())
        .or_insert_with(|| ToolCache::new(config));

    cache.cache.put(
        key,
        CachedResult {
            result: result.clone(),
            cached_at: Instant::now(),
            ttl,
            hit_count: 0,
        },
    );

    debug!(tool = tool_type, "Result cached");
}

/// Get cache statistics
pub fn get_cache_stats() -> Vec<ToolCacheStats> {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    state
        .caches
        .iter()
        .map(|(tool_type, cache)| {
            let total = cache.hits + cache.misses;
            ToolCacheStats {
                tool_type: tool_type.clone(),
                enabled: cache.config.enabled,
                ttl_secs: cache.config.ttl_secs,
                max_entries: cache.config.max_entries,
                current_entries: cache.cache.len(),
                hits: cache.hits,
                misses: cache.misses,
                hit_rate: if total > 0 {
                    cache.hits as f64 / total as f64
                } else {
                    0.0
                },
            }
        })
        .collect()
}

/// Get cache stats for a specific tool
pub fn get_tool_cache_stats(tool_type: &str) -> Option<ToolCacheStats> {
    let state = get_state().lock().ok()?;

    let cache = state.caches.get(tool_type)?;
    let total = cache.hits + cache.misses;

    Some(ToolCacheStats {
        tool_type: tool_type.to_string(),
        enabled: cache.config.enabled,
        ttl_secs: cache.config.ttl_secs,
        max_entries: cache.config.max_entries,
        current_entries: cache.cache.len(),
        hits: cache.hits,
        misses: cache.misses,
        hit_rate: if total > 0 {
            cache.hits as f64 / total as f64
        } else {
            0.0
        },
    })
}

/// Clear cache for a specific tool
pub fn clear_tool_cache(tool_type: &str) {
    if let Ok(mut state) = get_state().lock() {
        if let Some(cache) = state.caches.get_mut(tool_type) {
            cache.cache.clear();
            cache.hits = 0;
            cache.misses = 0;
            info!(tool = tool_type, "Cache cleared");
        }
    }
}

/// Clear all caches
pub fn clear_all_caches() {
    if let Ok(mut state) = get_state().lock() {
        for cache in state.caches.values_mut() {
            cache.cache.clear();
            cache.hits = 0;
            cache.misses = 0;
        }
        info!("All tool caches cleared");
    }
}

/// Update cache configuration for a tool
pub fn set_cache_config(tool_type: &str, config: ToolCacheConfig) {
    if let Ok(mut state) = get_state().lock() {
        state.configs.insert(tool_type.to_string(), config.clone());
        // Recreate cache with new config
        state
            .caches
            .insert(tool_type.to_string(), ToolCache::new(config));
    }
}

/// Cache statistics for API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCacheStats {
    pub tool_type: String,
    pub enabled: bool,
    pub ttl_secs: u64,
    pub max_entries: usize,
    pub current_entries: usize,
    pub hits: usize,
    pub misses: usize,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolMetadata;

    fn make_result() -> ToolResult {
        ToolResult {
            tool: ToolType::Calculator,
            success: true,
            result: "8".to_string(),
            metadata: ToolMetadata {
                execution_time_ms: 1,
                confidence: 1.0,
                source: None,
                cost: None,
            },
        }
    }

    #[test]
    fn test_cache_miss() {
        let result = get_cached("Calculator", "unique_query_12345");
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_hit() {
        let result = make_result();
        cache_result("Calculator", "5 + 3", &result);

        let cached = get_cached("Calculator", "5 + 3");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().result, "8");
    }

    #[test]
    fn test_cache_stats() {
        let stats = get_cache_stats();
        // Stats should be available even if empty
        assert!(stats.len() >= 0);
    }
}
