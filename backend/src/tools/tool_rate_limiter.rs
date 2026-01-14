// src/tools/tool_rate_limiter.rs
// Feature #3: Per-tool rate limiting for expensive tools

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Rate limit configuration for a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRateLimit {
    /// Maximum requests per window
    pub max_requests: usize,
    /// Time window in seconds
    pub window_secs: u64,
    /// Whether this tool is rate limited
    pub enabled: bool,
}

impl Default for ToolRateLimit {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window_secs: 60,
            enabled: true,
        }
    }
}

/// Token bucket for rate limiting
struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    fn new(max_tokens: usize, window_secs: u64) -> Self {
        let refill_rate = max_tokens as f64 / window_secs as f64;
        Self {
            tokens: max_tokens as f64,
            max_tokens: max_tokens as f64,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn try_acquire(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }

    fn tokens_available(&mut self) -> f64 {
        self.refill();
        self.tokens
    }

    fn time_until_available(&mut self) -> Duration {
        self.refill();
        if self.tokens >= 1.0 {
            Duration::ZERO
        } else {
            let needed = 1.0 - self.tokens;
            Duration::from_secs_f64(needed / self.refill_rate)
        }
    }
}

/// Global rate limiter state
struct ToolRateLimiterState {
    buckets: HashMap<String, TokenBucket>,
    configs: HashMap<String, ToolRateLimit>,
}

impl Default for ToolRateLimiterState {
    fn default() -> Self {
        let mut configs = HashMap::new();

        // Default rate limits for each tool type
        // Expensive/external tools get stricter limits
        configs.insert(
            "CodeExecution".to_string(),
            ToolRateLimit {
                max_requests: 10,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "WebSearch".to_string(),
            ToolRateLimit {
                max_requests: 30,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "ImageGeneration".to_string(),
            ToolRateLimit {
                max_requests: 5,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "URLFetch".to_string(),
            ToolRateLimit {
                max_requests: 60,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "DatabaseQuery".to_string(),
            ToolRateLimit {
                max_requests: 50,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "Notification".to_string(),
            ToolRateLimit {
                max_requests: 20,
                window_secs: 60,
                enabled: true,
            },
        );
        // Local/cheap tools get generous limits
        configs.insert(
            "Calculator".to_string(),
            ToolRateLimit {
                max_requests: 1000,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "SemanticSearch".to_string(),
            ToolRateLimit {
                max_requests: 100,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "Summarizer".to_string(),
            ToolRateLimit {
                max_requests: 50,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "QueryRewriter".to_string(),
            ToolRateLimit {
                max_requests: 100,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "Classifier".to_string(),
            ToolRateLimit {
                max_requests: 100,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "FileAnalyzer".to_string(),
            ToolRateLimit {
                max_requests: 50,
                window_secs: 60,
                enabled: true,
            },
        );
        // New tools
        configs.insert(
            "Translator".to_string(),
            ToolRateLimit {
                max_requests: 50,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "SentimentAnalyzer".to_string(),
            ToolRateLimit {
                max_requests: 100,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "EntityExtractor".to_string(),
            ToolRateLimit {
                max_requests: 100,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "SpellChecker".to_string(),
            ToolRateLimit {
                max_requests: 200,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "Scheduler".to_string(),
            ToolRateLimit {
                max_requests: 30,
                window_secs: 60,
                enabled: true,
            },
        );
        configs.insert(
            "Memory".to_string(),
            ToolRateLimit {
                max_requests: 100,
                window_secs: 60,
                enabled: true,
            },
        );

        Self {
            buckets: HashMap::new(),
            configs,
        }
    }
}

static RATE_LIMITER: OnceLock<Mutex<ToolRateLimiterState>> = OnceLock::new();

fn get_state() -> &'static Mutex<ToolRateLimiterState> {
    RATE_LIMITER.get_or_init(|| Mutex::new(ToolRateLimiterState::default()))
}

/// Check if a tool execution is allowed under rate limits
pub fn check_rate_limit(tool_type: &str) -> RateLimitResult {
    let mut state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return RateLimitResult::Allowed, // Fail open
    };

    // Get or create config
    let config = state.configs.get(tool_type).cloned().unwrap_or_default();

    if !config.enabled {
        return RateLimitResult::Allowed;
    }

    // Get or create bucket
    let bucket = state
        .buckets
        .entry(tool_type.to_string())
        .or_insert_with(|| TokenBucket::new(config.max_requests, config.window_secs));

    if bucket.try_acquire() {
        debug!(tool = tool_type, "Rate limit check passed");
        RateLimitResult::Allowed
    } else {
        let wait_time = bucket.time_until_available();
        warn!(
            tool = tool_type,
            wait_ms = wait_time.as_millis(),
            "Rate limit exceeded"
        );
        RateLimitResult::Limited {
            retry_after: wait_time,
            tokens_available: bucket.tokens_available(),
        }
    }
}

/// Result of rate limit check
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    Allowed,
    Limited {
        retry_after: Duration,
        tokens_available: f64,
    },
}

impl RateLimitResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, RateLimitResult::Allowed)
    }
}

/// Get current rate limit status for a tool
pub fn get_rate_limit_status(tool_type: &str) -> ToolRateLimitStatus {
    let mut state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return ToolRateLimitStatus::default(),
    };

    let config = state.configs.get(tool_type).cloned().unwrap_or_default();

    let (tokens_available, tokens_max) = if let Some(bucket) = state.buckets.get_mut(tool_type) {
        (bucket.tokens_available(), bucket.max_tokens)
    } else {
        (config.max_requests as f64, config.max_requests as f64)
    };

    ToolRateLimitStatus {
        tool_type: tool_type.to_string(),
        enabled: config.enabled,
        max_requests: config.max_requests,
        window_secs: config.window_secs,
        tokens_available,
        tokens_max,
        utilization: 1.0 - (tokens_available / tokens_max),
    }
}

/// Get rate limit status for all tools
pub fn get_all_rate_limit_status() -> Vec<ToolRateLimitStatus> {
    let mut state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    state
        .configs
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .map(|tool_type| {
            let config = state.configs.get(&tool_type).cloned().unwrap_or_default();
            let (tokens_available, tokens_max) =
                if let Some(bucket) = state.buckets.get_mut(&tool_type) {
                    (bucket.tokens_available(), bucket.max_tokens)
                } else {
                    (config.max_requests as f64, config.max_requests as f64)
                };

            ToolRateLimitStatus {
                tool_type,
                enabled: config.enabled,
                max_requests: config.max_requests,
                window_secs: config.window_secs,
                tokens_available,
                tokens_max,
                utilization: 1.0 - (tokens_available / tokens_max),
            }
        })
        .collect()
}

/// Update rate limit configuration for a tool
pub fn set_rate_limit(tool_type: &str, config: ToolRateLimit) {
    if let Ok(mut state) = get_state().lock() {
        // Update config
        state.configs.insert(tool_type.to_string(), config.clone());
        // Reset bucket with new config
        state.buckets.insert(
            tool_type.to_string(),
            TokenBucket::new(config.max_requests, config.window_secs),
        );
    }
}

/// Rate limit status for API response
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolRateLimitStatus {
    pub tool_type: String,
    pub enabled: bool,
    pub max_requests: usize,
    pub window_secs: u64,
    pub tokens_available: f64,
    pub tokens_max: f64,
    pub utilization: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_allowed() {
        let result = check_rate_limit("Calculator");
        assert!(result.is_allowed());
    }

    #[test]
    fn test_rate_limit_status() {
        let status = get_rate_limit_status("Calculator");
        assert!(status.enabled);
        assert!(status.tokens_available > 0.0);
    }
}
