// src/memory/llm_provider.rs
// LLM Provider abstraction - pluggable architecture
// Default: Phi 3.5 via Ollama

use crate::db::llm_settings::{self, LlmConfig};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// LLM Provider trait - implement this to support new models
#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    /// Generate text with default settings from global config
    async fn generate(&self, prompt: &str) -> Result<String, LLMError>;
    /// Generate text with custom settings
    async fn generate_with_config(
        &self,
        prompt: &str,
        config: &LlmConfig,
    ) -> Result<String, LLMError>;
    fn model_name(&self) -> &str;
}

/// Configuration for different LLM providers
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum LLMConfig {
    /// Phi 3.5 via Ollama (default, minimal resources)
    Phi35Ollama { ollama_url: String, model: String },
    /// Qwen 7B via Ollama (multilingual)
    QwenOllama { ollama_url: String, model: String },
    /// Mistral 7B via Ollama
    MistralOllama { ollama_url: String, model: String },
    /// OpenAI API (requires API key)
    OpenAI { api_key: String, model: String },
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self::Phi35Ollama {
            ollama_url: "http://localhost:11434".to_string(),
            model: "phi:latest".to_string(), // Phi-2 (3B) - fits in ~3GB RAM
        }
    }
}

/// Error types for LLM operations
#[derive(Debug, Clone)]
pub enum LLMError {
    ConnectionFailed(String),
    InvalidResponse(String),
    GenerationFailed(String),
    ConfigError(String),
}

impl std::fmt::Display for LLMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "LLM connection failed: {}", msg),
            Self::InvalidResponse(msg) => write!(f, "Invalid LLM response: {}", msg),
            Self::GenerationFailed(msg) => write!(f, "Generation failed: {}", msg),
            Self::ConfigError(msg) => write!(f, "Config error: {}", msg),
        }
    }
}

impl std::error::Error for LLMError {}

/// Ollama-based LLM provider
pub struct OllamaProvider {
    url: String,
    model: String,
    client: reqwest::Client,
}

/// Ollama API options for generation parameters
#[derive(Serialize, Default)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeat_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    typical_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tfs_z: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mirostat: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mirostat_eta: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mirostat_tau: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeat_last_n: Option<i32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
}

impl From<&LlmConfig> for OllamaOptions {
    fn from(cfg: &LlmConfig) -> Self {
        Self {
            temperature: Some(cfg.temperature),
            top_p: Some(cfg.top_p),
            top_k: Some(cfg.top_k as i32),
            repeat_penalty: Some(cfg.repeat_penalty),
            frequency_penalty: Some(cfg.frequency_penalty),
            presence_penalty: Some(cfg.presence_penalty),
            num_predict: Some(cfg.max_tokens as i32),
            seed: cfg.seed,
            min_p: Some(cfg.min_p),
            typical_p: Some(cfg.typical_p),
            tfs_z: Some(cfg.tfs_z),
            mirostat: Some(cfg.mirostat),
            mirostat_eta: Some(cfg.mirostat_eta),
            mirostat_tau: Some(cfg.mirostat_tau),
            repeat_last_n: Some(cfg.repeat_last_n as i32),
            stop: cfg.stop_sequences.clone(),
        }
    }
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

impl OllamaProvider {
    pub fn new(url: String, model: String) -> Self {
        Self {
            url,
            model,
            client: reqwest::Client::new(),
        }
    }

    async fn health_check(&self) -> Result<(), LLMError> {
        let health_url = format!("{}/api/tags", self.url);
        self.client.get(&health_url).send().await.map_err(|e| {
            LLMError::ConnectionFailed(format!("Cannot reach Ollama at {}: {}", self.url, e))
        })?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl LLMProvider for OllamaProvider {
    async fn generate(&self, prompt: &str) -> Result<String, LLMError> {
        // Use global config for default generation
        let config = llm_settings::global_config();
        self.generate_with_config(prompt, &config).await
    }

    async fn generate_with_config(
        &self,
        prompt: &str,
        config: &LlmConfig,
    ) -> Result<String, LLMError> {
        debug!(
            model = %self.model,
            prompt_len = prompt.len(),
            temperature = config.temperature,
            top_p = config.top_p,
            top_k = config.top_k,
            max_tokens = config.max_tokens,
            "Generating with Ollama"
        );

        // Check connection first
        if let Err(e) = self.health_check().await {
            warn!("Ollama health check failed: {}", e);
            return Err(e);
        }

        let url = format!("{}/api/generate", self.url);
        let options = OllamaOptions::from(config);
        let req = OllamaRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            options: Some(options),
        };

        let response = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| LLMError::ConnectionFailed(e.to_string()))?;

        let ollama_resp: OllamaResponse = response
            .json()
            .await
            .map_err(|e| LLMError::InvalidResponse(e.to_string()))?;

        info!(
            model = %self.model,
            response_len = ollama_resp.response.len(),
            temperature = config.temperature,
            "Generation complete"
        );
        Ok(ollama_resp.response.trim().to_string())
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

/// Factory function to create LLM provider from config
pub async fn create_llm_provider(config: LLMConfig) -> Result<Box<dyn LLMProvider>, LLMError> {
    match config {
        LLMConfig::Phi35Ollama { ollama_url, model } => {
            info!("Initializing Phi 3.5 via Ollama at {}", ollama_url);
            let provider = OllamaProvider::new(ollama_url, model);
            provider.health_check().await.map_err(|e| {
                warn!("Failed to connect to LLM backend. Make sure it's running: ollama serve");
                e
            })?;
            Ok(Box::new(provider))
        }
        LLMConfig::QwenOllama { ollama_url, model } => {
            info!("Initializing Qwen via Ollama at {}", ollama_url);
            let provider = OllamaProvider::new(ollama_url, model);
            provider.health_check().await?;
            Ok(Box::new(provider))
        }
        LLMConfig::MistralOllama { ollama_url, model } => {
            info!("Initializing Mistral via Ollama at {}", ollama_url);
            let provider = OllamaProvider::new(ollama_url, model);
            provider.health_check().await?;
            Ok(Box::new(provider))
        }
        LLMConfig::OpenAI {
            api_key: _,
            model: _,
        } => Err(LLMError::ConfigError(
            "OpenAI provider not yet implemented".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LLMConfig::default();
        match config {
            LLMConfig::Phi35Ollama { model, .. } => {
                assert_eq!(model, "phi:latest");
            }
            _ => panic!("Default should be Phi35Ollama"),
        }
    }

    #[test]
    fn test_ollama_provider_creation() {
        let provider = OllamaProvider::new(
            "http://localhost:11434".to_string(),
            "phi:latest".to_string(),
        );
        assert_eq!(provider.model_name(), "phi:latest");
    }

    #[test]
    fn test_llm_error_display() {
        let err = LLMError::ConnectionFailed("test".to_string());
        assert!(format!("{}", err).contains("connection failed"));
    }
}
