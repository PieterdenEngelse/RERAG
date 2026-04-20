//! GGUF-based tokenizer for exact token counting
//! Loads vocab from the active LLM's GGUF file via shimmytok.
//! Falls back to heuristic counting when no GGUF is available.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

// ============================================================================
// Status — what mode the counter is in and (if heuristic) why it fell back
// ============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenCounterMode {
    Exact,
    Heuristic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FallbackReason {
    NotAttempted,
    CloudBackend,
    NoModelConfigured,
    PathNotFound { detail: String },
    LoadFailed { detail: String },
}

impl FallbackReason {
    pub fn discriminant(&self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::CloudBackend => "cloud_backend",
            Self::NoModelConfigured => "no_model_configured",
            Self::PathNotFound { .. } => "path_not_found",
            Self::LoadFailed { .. } => "load_failed",
        }
    }
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::PathNotFound { detail } | Self::LoadFailed { detail } => Some(detail),
            _ => None,
        }
    }
    /// True when exact tokenization was expected but didn't happen — these
    /// degrade retrieval fidelity and should be surfaced loudly.
    pub fn is_unexpected(&self) -> bool {
        matches!(self, Self::PathNotFound { .. } | Self::LoadFailed { .. })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCounterStatus {
    pub mode: TokenCounterMode,
    pub fallback_reason: Option<FallbackReason>,
    pub attempted_path: Option<String>,
    pub attempted_at: Option<DateTime<Utc>>,
}

impl TokenCounterStatus {
    fn initial_heuristic() -> Self {
        Self {
            mode: TokenCounterMode::Heuristic,
            fallback_reason: Some(FallbackReason::NotAttempted),
            attempted_path: None,
            attempted_at: None,
        }
    }
}

// ============================================================================
// Token Counter trait
// ============================================================================

/// Trait for token counting — allows swapping implementations at runtime
pub trait TokenCounter: Send + Sync {
    fn count_tokens(&self, text: &str) -> usize;
    fn model_name(&self) -> &str;
    fn vocab_size(&self) -> usize;
    fn is_exact(&self) -> bool;
    /// Encode to token IDs. Returns `None` for counters that don't have a real
    /// vocabulary (heuristic fallback). Used by the golden-sample baseline and
    /// by the diff engine.
    fn encode_ids(&self, _text: &str) -> Option<Vec<u32>> {
        None
    }
}

// ============================================================================
// Exact counter: shimmytok from GGUF
// ============================================================================

pub struct GgufTokenCounter {
    tokenizer: shimmytok::Tokenizer,
    model_name: String,
    vocab_size: usize,
}

impl GgufTokenCounter {
    pub fn from_gguf_file(path: &Path) -> Result<Self> {
        let tokenizer = shimmytok::Tokenizer::from_gguf_file(path)
            .with_context(|| format!("Failed to load tokenizer from {:?}", path))?;
        let vocab_size = tokenizer.vocab_size();
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());
        info!(
            model = %name,
            vocab_size = vocab_size,
            path = %path.display(),
            "Loaded GGUF tokenizer for exact token counting"
        );
        Ok(Self {
            tokenizer,
            model_name: name,
            vocab_size,
        })
    }
}

impl TokenCounter for GgufTokenCounter {
    fn count_tokens(&self, text: &str) -> usize {
        match self.tokenizer.encode(text, false) {
            Ok(tokens) => tokens.len(),
            Err(e) => {
                warn!(error = %e, "GGUF encode failed, falling back to heuristic");
                heuristic_count(text)
            }
        }
    }
    fn model_name(&self) -> &str {
        &self.model_name
    }
    fn vocab_size(&self) -> usize {
        self.vocab_size
    }
    fn is_exact(&self) -> bool {
        true
    }
    fn encode_ids(&self, text: &str) -> Option<Vec<u32>> {
        self.tokenizer.encode(text, false).ok()
    }
}

// ============================================================================
// Heuristic fallback (existing behavior)
// ============================================================================

pub struct HeuristicTokenCounter;

impl TokenCounter for HeuristicTokenCounter {
    fn count_tokens(&self, text: &str) -> usize {
        heuristic_count(text)
    }
    fn model_name(&self) -> &str {
        "heuristic"
    }
    fn vocab_size(&self) -> usize {
        0
    }
    fn is_exact(&self) -> bool {
        false
    }
}

fn heuristic_count(text: &str) -> usize {
    let char_est = text.len() / 4;
    let word_est = text.split_whitespace().count() * 4 / 3;
    (char_est + word_est) / 2
}

// ============================================================================
// Shared handle — swappable at runtime when model changes
// ============================================================================

pub struct TokenCounterHandle {
    inner: RwLock<Arc<dyn TokenCounter>>,
    status: RwLock<TokenCounterStatus>,
}

impl TokenCounterHandle {
    pub fn new_heuristic() -> Self {
        info!("TokenCounterHandle initialized with heuristic counter");
        Self {
            inner: RwLock::new(Arc::new(HeuristicTokenCounter)),
            status: RwLock::new(TokenCounterStatus::initial_heuristic()),
        }
    }

    pub fn load_from_gguf(&self, path: &Path) -> Result<()> {
        let path_str = path.display().to_string();
        match GgufTokenCounter::from_gguf_file(path) {
            Ok(counter) => {
                let mut inner = self.inner.write().map_err(|_| anyhow!("Lock poisoned"))?;
                *inner = Arc::new(counter);
                if let Ok(mut s) = self.status.write() {
                    *s = TokenCounterStatus {
                        mode: TokenCounterMode::Exact,
                        fallback_reason: None,
                        attempted_path: Some(path_str),
                        attempted_at: Some(Utc::now()),
                    };
                }
                Ok(())
            }
            Err(e) => {
                let detail = format!("{:#}", e);
                warn!(path = %path_str, error = %detail, "GGUF tokenizer load failed");
                if let Ok(mut s) = self.status.write() {
                    *s = TokenCounterStatus {
                        mode: TokenCounterMode::Heuristic,
                        fallback_reason: Some(FallbackReason::LoadFailed {
                            detail: detail.clone(),
                        }),
                        attempted_path: Some(path_str),
                        attempted_at: Some(Utc::now()),
                    };
                }
                if let Ok(mut inner) = self.inner.write() {
                    *inner = Arc::new(HeuristicTokenCounter);
                }
                Err(e)
            }
        }
    }

    pub fn reset_to_heuristic(&self) {
        if let Ok(mut inner) = self.inner.write() {
            *inner = Arc::new(HeuristicTokenCounter);
            info!("TokenCounterHandle reset to heuristic");
        }
        // Caller-owned reason: prefer mark_fallback for richer context. This
        // path is used when state should be cleared without a structured cause.
        if let Ok(mut s) = self.status.write() {
            s.mode = TokenCounterMode::Heuristic;
            if s.fallback_reason.is_none() {
                s.fallback_reason = Some(FallbackReason::NotAttempted);
            }
        }
    }

    /// Record a fallback to heuristic with a structured cause. Logs at warn
    /// level when the cause is unexpected (path/load failures) so operators
    /// see degraded retrieval fidelity in the log stream as well as the UI.
    pub fn mark_fallback(&self, reason: FallbackReason, attempted_path: Option<String>) {
        if reason.is_unexpected() {
            warn!(
                reason = reason.discriminant(),
                detail = reason.detail().unwrap_or(""),
                path = attempted_path.as_deref().unwrap_or(""),
                "Tokenizer fell back to heuristic — token counts are approximate"
            );
        } else {
            info!(
                reason = reason.discriminant(),
                "Token counter using heuristic mode"
            );
        }
        if let Ok(mut inner) = self.inner.write() {
            *inner = Arc::new(HeuristicTokenCounter);
        }
        if let Ok(mut s) = self.status.write() {
            *s = TokenCounterStatus {
                mode: TokenCounterMode::Heuristic,
                fallback_reason: Some(reason),
                attempted_path,
                attempted_at: Some(Utc::now()),
            };
        }
    }

    pub fn status(&self) -> TokenCounterStatus {
        self.status.read().unwrap().clone()
    }

    pub fn count_tokens(&self, text: &str) -> usize {
        self.inner.read().unwrap().count_tokens(text)
    }

    pub fn model_name(&self) -> String {
        self.inner.read().unwrap().model_name().to_string()
    }

    pub fn vocab_size(&self) -> usize {
        self.inner.read().unwrap().vocab_size()
    }

    pub fn is_exact(&self) -> bool {
        self.inner.read().unwrap().is_exact()
    }

    pub fn encode_ids(&self, text: &str) -> Option<Vec<u32>> {
        self.inner.read().unwrap().encode_ids(text)
    }
}

// ============================================================================
// GGUF path resolution helpers
// ============================================================================

pub fn resolve_ollama_gguf_path(model: &str) -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("No home directory"))?;
    let (name, tag) = model.split_once(':').unwrap_or((model, "latest"));
    let manifest_path = home
        .join(".ollama/models/manifests/registry.ollama.ai/library")
        .join(name)
        .join(tag);

    let manifest_str = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("No Ollama manifest at {:?}", manifest_path))?;
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_str).context("Failed to parse Ollama manifest")?;

    let layers = manifest["layers"]
        .as_array()
        .ok_or_else(|| anyhow!("No layers array in Ollama manifest"))?;

    let model_layer = layers
        .iter()
        .find(|l| {
            l["mediaType"]
                .as_str()
                .map(|m| m.contains("model"))
                .unwrap_or(false)
        })
        .ok_or_else(|| anyhow!("No model layer found in Ollama manifest"))?;

    let digest = model_layer["digest"]
        .as_str()
        .ok_or_else(|| anyhow!("No digest in model layer"))?;

    let blob_name = digest.replace(':', "-");
    let blob_path = home.join(".ollama/models/blobs").join(&blob_name);

    if blob_path.exists() {
        info!(model = %model, path = %blob_path.display(), "Resolved Ollama GGUF path");
        Ok(blob_path)
    } else {
        Err(anyhow!("Ollama blob not found at {:?}", blob_path))
    }
}

pub fn resolve_llama_server_gguf_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("No home directory"))?;
    let env_path = home.join(".config/ag/llama-server.env");
    let content = std::fs::read_to_string(&env_path)
        .with_context(|| format!("Cannot read {:?}", env_path))?;

    for line in content.lines() {
        // Support both env var names
        let stripped = line
            .strip_prefix("MODEL_PATH=")
            .or_else(|| line.strip_prefix("LLAMA_MODEL="));
        if let Some(path) = stripped {
            let path = path.trim().trim_matches('"');
            let pb = PathBuf::from(path);
            if pb.exists() {
                info!(path = %pb.display(), "Resolved llama-server GGUF path");
                return Ok(pb);
            } else {
                return Err(anyhow!("MODEL_PATH {:?} does not exist", pb));
            }
        }
    }
    Err(anyhow!("MODEL_PATH not found in {:?}", env_path))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_counter() {
        let counter = HeuristicTokenCounter;
        let count = counter.count_tokens("The quick brown fox jumps over the lazy dog");
        assert!(count >= 8 && count <= 15, "Heuristic count was {}", count);
        assert_eq!(counter.model_name(), "heuristic");
        assert!(!counter.is_exact());
        assert_eq!(counter.vocab_size(), 0);
    }

    #[test]
    fn test_handle_starts_heuristic() {
        let handle = TokenCounterHandle::new_heuristic();
        assert!(!handle.is_exact());
        assert_eq!(handle.model_name(), "heuristic");
    }

    #[test]
    fn test_handle_reset() {
        let handle = TokenCounterHandle::new_heuristic();
        handle.reset_to_heuristic();
        assert!(!handle.is_exact());
    }

    #[test]
    fn test_heuristic_empty_text() {
        let counter = HeuristicTokenCounter;
        assert_eq!(counter.count_tokens(""), 0);
    }

    #[test]
    fn test_heuristic_code_text() {
        let counter = HeuristicTokenCounter;
        let code = "self.config.max_seq_len()";
        let count = counter.count_tokens(code);
        assert!(count > 0);
    }
}
