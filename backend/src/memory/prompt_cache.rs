// src/memory/prompt_cache.rs
// Prompt Caching Support for LLM Providers
// 
// Implements KV cache optimization hints for:
// - Anthropic: Explicit cache_control breakpoints
// - OpenAI: Automatic caching (prefix matching)
//
// Reference: https://ngrok.com/blog-post/prompt-caching-10x-cheaper-llm-tokens
//
// Key insight: LLMs cache K (Key) and V (Value) matrices from attention mechanism.
// Cached tokens are 10x cheaper and reduce latency by up to 85% for long prompts.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tracing::{debug, info};

/// Represents a cacheable prompt segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheableSegment {
    /// The text content of this segment
    pub content: String,
    /// Whether this segment should be marked for caching (Anthropic)
    pub cache_control: bool,
    /// Segment type for organization
    pub segment_type: SegmentType,
}

/// Types of prompt segments for caching strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentType {
    /// System instructions - highly cacheable, rarely changes
    SystemPrompt,
    /// RAG context documents - cacheable if same docs retrieved
    Context,
    /// Conversation history - partially cacheable (older messages)
    History,
    /// Current user query - never cached
    UserQuery,
    /// Assistant response prefix - never cached
    AssistantPrefix,
}

impl SegmentType {
    /// Returns true if this segment type benefits from caching
    pub fn is_cacheable(&self) -> bool {
        matches!(
            self,
            SegmentType::SystemPrompt | SegmentType::Context | SegmentType::History
        )
    }

    /// Suggested minimum token count for caching (Anthropic requires 1024+)
    pub fn min_cache_tokens(&self) -> usize {
        match self {
            SegmentType::SystemPrompt => 1024,
            SegmentType::Context => 2048,
            SegmentType::History => 1024,
            _ => usize::MAX, // Never cache
        }
    }
}

/// Prompt structure optimized for caching
#[derive(Debug, Clone, Default)]
pub struct CacheOptimizedPrompt {
    segments: Vec<CacheableSegment>,
    /// Hash of cacheable content for cache key generation
    cache_key: Option<u64>,
}

impl CacheOptimizedPrompt {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a system prompt segment (highest cache priority)
    pub fn with_system_prompt(mut self, content: impl Into<String>) -> Self {
        let content = content.into();
        if !content.is_empty() {
            self.segments.push(CacheableSegment {
                content,
                cache_control: true, // Always cache system prompts
                segment_type: SegmentType::SystemPrompt,
            });
        }
        self
    }

    /// Add RAG context documents
    pub fn with_context(mut self, content: impl Into<String>) -> Self {
        let content = content.into();
        if !content.is_empty() {
            self.segments.push(CacheableSegment {
                content,
                cache_control: true, // Cache context if long enough
                segment_type: SegmentType::Context,
            });
        }
        self
    }

    /// Add conversation history
    pub fn with_history(mut self, messages: Vec<(String, String)>) -> Self {
        if !messages.is_empty() {
            let history_text = messages
                .iter()
                .map(|(role, content)| format!("{}: {}", role, content))
                .collect::<Vec<_>>()
                .join("\n\n");

            self.segments.push(CacheableSegment {
                content: history_text,
                cache_control: true, // Cache older history
                segment_type: SegmentType::History,
            });
        }
        self
    }

    /// Add the current user query (never cached)
    pub fn with_user_query(mut self, query: impl Into<String>) -> Self {
        self.segments.push(CacheableSegment {
            content: query.into(),
            cache_control: false,
            segment_type: SegmentType::UserQuery,
        });
        self
    }

    /// Generate a cache key based on cacheable content
    pub fn compute_cache_key(&mut self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for segment in &self.segments {
            if segment.cache_control {
                segment.content.hash(&mut hasher);
            }
        }
        let key = hasher.finish();
        self.cache_key = Some(key);
        key
    }

    /// Get the cache key (computes if not already done)
    pub fn cache_key(&mut self) -> u64 {
        self.cache_key.unwrap_or_else(|| self.compute_cache_key())
    }

    /// Estimate token count (rough: ~4 chars per token)
    pub fn estimate_tokens(&self) -> usize {
        self.segments
            .iter()
            .map(|s| s.content.len() / 4)
            .sum()
    }

    /// Estimate cacheable token count
    pub fn estimate_cacheable_tokens(&self) -> usize {
        self.segments
            .iter()
            .filter(|s| s.cache_control)
            .map(|s| s.content.len() / 4)
            .sum()
    }

    /// Build prompt for Ollama (simple concatenation, no cache hints)
    pub fn build_ollama_prompt(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Build system prompt separately for Ollama
    pub fn build_ollama_system(&self) -> Option<String> {
        self.segments
            .iter()
            .find(|s| s.segment_type == SegmentType::SystemPrompt)
            .map(|s| s.content.clone())
    }

    /// Build messages array for Ollama /api/chat endpoint (enables context caching)
    /// 
    /// Ollama 0.3+ supports KV cache reuse when using the chat endpoint with
    /// consistent message history. The server keeps the K/V cache warm for
    /// the conversation prefix.
    ///
    /// Key settings for Ollama caching:
    /// - `keep_alive`: Keep model loaded (default "5m", use "24h" for persistent)
    /// - Use /api/chat instead of /api/generate for multi-turn conversations
    /// - Send full conversation history each request (Ollama caches the prefix)
    pub fn build_ollama_chat_messages(&self) -> Vec<serde_json::Value> {
        let mut messages = Vec::new();

        for segment in &self.segments {
            match segment.segment_type {
                SegmentType::SystemPrompt => {
                    // System message (cached across conversation)
                    messages.push(serde_json::json!({
                        "role": "system",
                        "content": segment.content
                    }));
                }
                SegmentType::Context => {
                    // RAG context as user message
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": format!("Here is relevant context:\n\n{}", segment.content)
                    }));
                    // Assistant acknowledgment helps maintain cache prefix
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": "I'll use this context to help answer your questions."
                    }));
                }
                SegmentType::History => {
                    // Parse history into alternating user/assistant messages
                    // For now, add as context
                    messages.push(serde_json::json!({
                        "role": "user", 
                        "content": format!("Previous conversation:\n{}", segment.content)
                    }));
                }
                SegmentType::UserQuery => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": segment.content
                    }));
                }
                SegmentType::AssistantPrefix => {
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": segment.content
                    }));
                }
            }
        }

        messages
    }

    /// Build complete Ollama /api/chat request with caching options
    pub fn build_ollama_chat_request(
        &self,
        model: &str,
        stream: bool,
        options: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let mut request = serde_json::json!({
            "model": model,
            "messages": self.build_ollama_chat_messages(),
            "stream": stream,
            // Keep model loaded for 1 hour to maximize cache hits
            "keep_alive": "1h"
        });

        if let Some(opts) = options {
            request["options"] = opts;
        }

        request
    }

    /// Build messages array for Anthropic API with cache_control hints
    pub fn build_anthropic_messages(&self) -> serde_json::Value {
        let mut messages = Vec::new();
        let mut system_content = Vec::new();

        for segment in &self.segments {
            match segment.segment_type {
                SegmentType::SystemPrompt => {
                    // System goes in separate field for Anthropic
                    let mut block = serde_json::json!({
                        "type": "text",
                        "text": segment.content
                    });
                    if segment.cache_control && segment.content.len() / 4 >= 1024 {
                        block["cache_control"] = serde_json::json!({"type": "ephemeral"});
                    }
                    system_content.push(block);
                }
                SegmentType::Context => {
                    // Context as user message with cache hint
                    let mut content_block = serde_json::json!({
                        "type": "text",
                        "text": format!("Context:\n{}", segment.content)
                    });
                    if segment.cache_control && segment.content.len() / 4 >= 1024 {
                        content_block["cache_control"] = serde_json::json!({"type": "ephemeral"});
                    }
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": [content_block]
                    }));
                }
                SegmentType::History => {
                    // Parse history back into messages
                    // For simplicity, add as single user context
                    let mut content_block = serde_json::json!({
                        "type": "text",
                        "text": format!("Previous conversation:\n{}", segment.content)
                    });
                    if segment.cache_control && segment.content.len() / 4 >= 1024 {
                        content_block["cache_control"] = serde_json::json!({"type": "ephemeral"});
                    }
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": [content_block]
                    }));
                }
                SegmentType::UserQuery => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": segment.content
                    }));
                }
                SegmentType::AssistantPrefix => {
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": segment.content
                    }));
                }
            }
        }

        serde_json::json!({
            "system": system_content,
            "messages": messages
        })
    }

    /// Build messages array for OpenAI API
    /// OpenAI does automatic prefix caching, so we just structure for best cache hits
    pub fn build_openai_messages(&self) -> Vec<serde_json::Value> {
        let mut messages = Vec::new();

        // Combine system prompt segments
        let system_text: String = self
            .segments
            .iter()
            .filter(|s| s.segment_type == SegmentType::SystemPrompt)
            .map(|s| s.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        if !system_text.is_empty() {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system_text
            }));
        }

        // Add context as user message (OpenAI caches by prefix)
        for segment in &self.segments {
            match segment.segment_type {
                SegmentType::Context => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": format!("Context:\n{}", segment.content)
                    }));
                    // Add assistant acknowledgment to maintain cache prefix
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": "I'll use this context to answer your question."
                    }));
                }
                SegmentType::History => {
                    // History should already be in message format
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": format!("Previous conversation:\n{}", segment.content)
                    }));
                }
                SegmentType::UserQuery => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": segment.content
                    }));
                }
                _ => {}
            }
        }

        messages
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub tokens_saved: u64,
    pub estimated_cost_saved_usd: f64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.total_requests as f64
        }
    }

    pub fn record_hit(&mut self, cached_tokens: u64) {
        self.total_requests += 1;
        self.cache_hits += 1;
        self.tokens_saved += cached_tokens;
        // Rough estimate: $0.01 per 1K tokens saved (10x discount)
        self.estimated_cost_saved_usd += (cached_tokens as f64 / 1000.0) * 0.009;
    }

    pub fn record_miss(&mut self) {
        self.total_requests += 1;
        self.cache_misses += 1;
    }
}

/// Helper to log cache usage
pub fn log_cache_usage(prompt: &CacheOptimizedPrompt, provider: &str) {
    let total_tokens = prompt.estimate_tokens();
    let cacheable_tokens = prompt.estimate_cacheable_tokens();
    let cache_ratio = if total_tokens > 0 {
        cacheable_tokens as f64 / total_tokens as f64 * 100.0
    } else {
        0.0
    };

    info!(
        provider = %provider,
        total_tokens = total_tokens,
        cacheable_tokens = cacheable_tokens,
        cache_ratio = format!("{:.1}%", cache_ratio),
        "Prompt cache analysis"
    );

    if cacheable_tokens < 1024 && provider == "anthropic" {
        debug!(
            "Anthropic requires 1024+ tokens for caching. Current cacheable: {}",
            cacheable_tokens
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_optimized_prompt_building() {
        let prompt = CacheOptimizedPrompt::new()
            .with_system_prompt("You are a helpful assistant.")
            .with_context("Document 1: Some content here...")
            .with_user_query("What is the meaning of life?");

        let ollama_prompt = prompt.build_ollama_prompt();
        assert!(ollama_prompt.contains("helpful assistant"));
        assert!(ollama_prompt.contains("Document 1"));
        assert!(ollama_prompt.contains("meaning of life"));
    }

    #[test]
    fn test_cache_key_generation() {
        let mut prompt1 = CacheOptimizedPrompt::new()
            .with_system_prompt("System prompt")
            .with_context("Same context");

        let mut prompt2 = CacheOptimizedPrompt::new()
            .with_system_prompt("System prompt")
            .with_context("Same context");

        let mut prompt3 = CacheOptimizedPrompt::new()
            .with_system_prompt("Different system")
            .with_context("Same context");

        assert_eq!(prompt1.cache_key(), prompt2.cache_key());
        assert_ne!(prompt1.cache_key(), prompt3.cache_key());
    }

    #[test]
    fn test_anthropic_message_format() {
        let prompt = CacheOptimizedPrompt::new()
            .with_system_prompt("You are helpful.")
            .with_user_query("Hello");

        let result = prompt.build_anthropic_messages();
        assert!(result.get("system").is_some());
        assert!(result.get("messages").is_some());
    }

    #[test]
    fn test_openai_message_format() {
        let prompt = CacheOptimizedPrompt::new()
            .with_system_prompt("You are helpful.")
            .with_user_query("Hello");

        let messages = prompt.build_openai_messages();
        assert!(!messages.is_empty());
        assert_eq!(messages[0]["role"], "system");
    }

    #[test]
    fn test_token_estimation() {
        let prompt = CacheOptimizedPrompt::new()
            .with_system_prompt("a".repeat(4000)); // ~1000 tokens

        assert!(prompt.estimate_tokens() >= 900);
        assert!(prompt.estimate_tokens() <= 1100);
    }

    #[test]
    fn test_ollama_chat_format() {
        let prompt = CacheOptimizedPrompt::new()
            .with_system_prompt("You are helpful.")
            .with_context("Document content here.")
            .with_user_query("What is this about?");

        let messages = prompt.build_ollama_chat_messages();
        
        // Should have: system, user (context), assistant (ack), user (query)
        assert!(messages.len() >= 3);
        assert_eq!(messages[0]["role"], "system");
    }

    #[test]
    fn test_ollama_chat_request() {
        let prompt = CacheOptimizedPrompt::new()
            .with_system_prompt("You are helpful.")
            .with_user_query("Hello");

        let request = prompt.build_ollama_chat_request("llama3", true, None);
        
        assert_eq!(request["model"], "llama3");
        assert_eq!(request["stream"], true);
        assert_eq!(request["keep_alive"], "1h");
        assert!(request["messages"].is_array());
    }
}
