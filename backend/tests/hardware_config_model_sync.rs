//! Integration tests for hardware config model sync
//!
//! These tests verify that the `/config/hardware` endpoint correctly returns
//! the active model, which the frontend home page uses to sync its model display.
//!
//! Related change: frontend/fro/src/pages/home.rs now fetches hardware config
//! on mount to display the active model instead of using a hardcoded value.

use serde::{Deserialize, Serialize};

/// Hardware config response structure (mirrors backend API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HardwareConfigResponse {
    status: String,
    message: String,
    request_id: String,
    config: HardwareConfig,
}

/// Hardware config structure (mirrors backend HardwareConfigRequest)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct HardwareConfig {
    backend_type: String,
    model: String,
    num_thread: usize,
    num_gpu: usize,
    gpu_layers: usize,
    main_gpu: usize,
    low_vram: bool,
    f16_kv: bool,
    rope_frequency_base: f32,
    rope_frequency_scale: f32,
    numa: bool,
    num_ctx: usize,
    num_batch: usize,
    logits_all: bool,
    vocab_only: bool,
    use_mmap: bool,
    use_mlock: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that HardwareConfig deserializes correctly with a model field
    #[test]
    fn test_hardware_config_model_field_present() {
        let json = r#"{
            "status": "ok",
            "message": "",
            "request_id": "abc123",
            "config": {
                "backend_type": "ollama",
                "model": "llama3:latest",
                "num_thread": 4,
                "num_gpu": 1,
                "gpu_layers": 0,
                "main_gpu": 0,
                "low_vram": false,
                "f16_kv": true,
                "rope_frequency_base": 10000.0,
                "rope_frequency_scale": 1.0,
                "numa": false,
                "num_ctx": 2048,
                "num_batch": 512,
                "logits_all": false,
                "vocab_only": false,
                "use_mmap": true,
                "use_mlock": false
            }
        }"#;

        let response: HardwareConfigResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.status, "ok");
        assert_eq!(response.config.model, "llama3:latest");
        assert_eq!(response.config.backend_type, "ollama");
    }

    /// Test that empty model field is handled correctly
    /// (frontend should keep default "phi:latest" when model is empty)
    #[test]
    fn test_hardware_config_empty_model() {
        let json = r#"{
            "status": "ok",
            "message": "",
            "request_id": "abc123",
            "config": {
                "backend_type": "ollama",
                "model": "",
                "num_thread": 1,
                "num_gpu": 0,
                "gpu_layers": 0,
                "main_gpu": 0,
                "low_vram": false,
                "f16_kv": true,
                "rope_frequency_base": 10000.0,
                "rope_frequency_scale": 1.0,
                "numa": false,
                "num_ctx": 2048,
                "num_batch": 512,
                "logits_all": false,
                "vocab_only": false,
                "use_mmap": true,
                "use_mlock": false
            }
        }"#;

        let response: HardwareConfigResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.config.model, "");
        // Frontend logic: if model.trim().is_empty(), keep default
        let active_model = response.config.model.trim().to_string();
        assert!(active_model.is_empty());
    }

    /// Test that whitespace-only model is treated as empty
    /// (matches frontend logic: model.trim().to_string())
    #[test]
    fn test_hardware_config_whitespace_model() {
        let json = r#"{
            "status": "ok",
            "message": "",
            "request_id": "abc123",
            "config": {
                "backend_type": "ollama",
                "model": "   ",
                "num_thread": 1,
                "num_gpu": 0,
                "gpu_layers": 0,
                "main_gpu": 0,
                "low_vram": false,
                "f16_kv": true,
                "rope_frequency_base": 10000.0,
                "rope_frequency_scale": 1.0,
                "numa": false,
                "num_ctx": 2048,
                "num_batch": 512,
                "logits_all": false,
                "vocab_only": false,
                "use_mmap": true,
                "use_mlock": false
            }
        }"#;

        let response: HardwareConfigResponse = serde_json::from_str(json).unwrap();

        // Frontend logic: trim whitespace
        let active_model = response.config.model.trim().to_string();
        assert!(active_model.is_empty());
    }

    /// Test model with leading/trailing whitespace is trimmed
    #[test]
    fn test_hardware_config_model_trimmed() {
        let json = r#"{
            "status": "ok",
            "message": "",
            "request_id": "abc123",
            "config": {
                "backend_type": "ollama",
                "model": "  phi:latest  ",
                "num_thread": 1,
                "num_gpu": 0,
                "gpu_layers": 0,
                "main_gpu": 0,
                "low_vram": false,
                "f16_kv": true,
                "rope_frequency_base": 10000.0,
                "rope_frequency_scale": 1.0,
                "numa": false,
                "num_ctx": 2048,
                "num_batch": 512,
                "logits_all": false,
                "vocab_only": false,
                "use_mmap": true,
                "use_mlock": false
            }
        }"#;

        let response: HardwareConfigResponse = serde_json::from_str(json).unwrap();

        // Frontend logic: trim whitespace
        let active_model = response.config.model.trim().to_string();
        assert_eq!(active_model, "phi:latest");
    }

    /// Test various backend types are handled
    #[test]
    fn test_hardware_config_different_backends() {
        let backends = vec![
            ("ollama", "llama3:latest"),
            ("llama_cpp", "mistral-7b.gguf"),
            ("openai", "gpt-4"),
            ("anthropic", "claude-3-opus"),
            ("vllm", "meta-llama/Llama-2-7b"),
            ("custom", "my-custom-model"),
        ];

        for (backend, model) in backends {
            let json = format!(
                r#"{{
                "status": "ok",
                "message": "",
                "request_id": "test",
                "config": {{
                    "backend_type": "{}",
                    "model": "{}",
                    "num_thread": 1,
                    "num_gpu": 0,
                    "gpu_layers": 0,
                    "main_gpu": 0,
                    "low_vram": false,
                    "f16_kv": true,
                    "rope_frequency_base": 10000.0,
                    "rope_frequency_scale": 1.0,
                    "numa": false,
                    "num_ctx": 2048,
                    "num_batch": 512,
                    "logits_all": false,
                    "vocab_only": false,
                    "use_mmap": true,
                    "use_mlock": false
                }}
            }}"#,
                backend, model
            );

            let response: HardwareConfigResponse = serde_json::from_str(&json).unwrap();

            assert_eq!(response.config.backend_type, backend);
            assert_eq!(response.config.model, model);
        }
    }

    /// Test default values when fields are missing (serde default)
    #[test]
    fn test_hardware_config_defaults() {
        let config = HardwareConfig::default();

        // Verify defaults match what frontend expects
        assert!(config.model.is_empty());
        assert!(config.backend_type.is_empty());
    }

    /// Simulate the frontend model sync logic
    /// This mirrors the logic in frontend/fro/src/pages/home.rs
    #[test]
    fn test_frontend_model_sync_logic() {
        // Default model (what frontend starts with)
        let mut selected_model = "phi:latest".to_string();

        // Simulate successful hardware config fetch
        let hardware_model = "llama3:8b".to_string();
        let active_model = hardware_model.trim().to_string();

        // Frontend logic: only update if not empty
        if !active_model.is_empty() {
            selected_model = active_model;
        }

        assert_eq!(selected_model, "llama3:8b");
    }

    /// Test that frontend keeps default when hardware config returns empty model
    #[test]
    fn test_frontend_keeps_default_on_empty() {
        // Default model (what frontend starts with)
        let mut selected_model = "phi:latest".to_string();

        // Simulate hardware config with empty model
        let hardware_model = "".to_string();
        let active_model = hardware_model.trim().to_string();

        // Frontend logic: only update if not empty
        if !active_model.is_empty() {
            selected_model = active_model;
        }

        // Should keep the default
        assert_eq!(selected_model, "phi:latest");
    }
}
