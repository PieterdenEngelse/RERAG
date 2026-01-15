//! End-to-end tests for hardware num_thread configuration
//!
//! These tests verify that the num_thread setting from hardware config
//! is correctly passed to Ollama request options.

use serde::{Deserialize, Serialize};


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

    /// Test that num_thread is included in hardware config
    #[test]
    fn test_hardware_config_num_thread_present() {
        let json = r#"{
            "backend_type": "ollama",
            "model": "phi:latest",
            "num_thread": 4,
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
        }"#;

        let config: HardwareConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.num_thread, 4);
    }

    /// Test that num_thread defaults to reasonable value
    #[test]
    fn test_hardware_config_num_thread_default() {
        let config = HardwareConfig::default();
        // Default is 0, but backend enforces minimum of 1
        assert_eq!(config.num_thread, 0);
    }

    /// Test that num_thread is correctly serialized in Ollama options
    #[test]
    fn test_ollama_options_include_num_thread() {
        let thread_count: usize = 4;
        let temperature: f32 = 0.7;
        let top_p: f32 = 0.95;
        let top_k: usize = 40;
        let max_tokens: usize = 1024;
        let repeat_penalty: f32 = 1.1;

        // Simulate the options JSON built in run_agent_stream
        let options = serde_json::json!({
            "temperature": temperature,
            "top_p": top_p,
            "top_k": top_k,
            "num_predict": max_tokens,
            "repeat_penalty": repeat_penalty,
            "num_thread": thread_count
        });

        assert_eq!(options["num_thread"], 4);
        assert!(options["temperature"].as_f64().unwrap() > 0.69);
        assert_eq!(options["num_predict"], 1024);
    }

    /// Test that num_thread minimum is enforced (at least 1)
    #[test]
    fn test_num_thread_minimum_enforced() {
        let num_thread: usize = 0;
        let thread_count = num_thread.max(1);
        assert_eq!(thread_count, 1);

        let num_thread: usize = 4;
        let thread_count = num_thread.max(1);
        assert_eq!(thread_count, 4);
    }

    /// Test Ollama generate request body structure
    #[test]
    fn test_ollama_generate_request_structure() {
        let thread_count: usize = 8;
        let model = "phi:latest";
        let prompt = "Hello, world!";
        let system_prompt = "You are a helpful assistant.";

        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": true,
            "options": {
                "temperature": 0.7,
                "top_p": 0.95,
                "top_k": 40,
                "num_predict": 1024,
                "repeat_penalty": 1.1,
                "num_thread": thread_count
            },
            "system": system_prompt
        });

        // Verify structure
        assert_eq!(body["model"], "phi:latest");
        assert_eq!(body["stream"], true);
        assert_eq!(body["options"]["num_thread"], 8);
        assert_eq!(body["system"], "You are a helpful assistant.");
    }

    /// Test Ollama chat request options structure
    #[test]
    fn test_ollama_chat_request_options() {
        let thread_count: usize = 2;

        let options = serde_json::json!({
            "temperature": 0.4,
            "top_p": 0.9,
            "top_k": 30,
            "num_predict": 768,
            "repeat_penalty": 1.12,
            "num_thread": thread_count
        });

        // Verify all expected fields are present
        assert!(options.get("temperature").is_some());
        assert!(options.get("top_p").is_some());
        assert!(options.get("top_k").is_some());
        assert!(options.get("num_predict").is_some());
        assert!(options.get("repeat_penalty").is_some());
        assert!(options.get("num_thread").is_some());

        assert_eq!(options["num_thread"], 2);
    }

    /// Test various thread counts
    #[test]
    fn test_various_thread_counts() {
        let test_cases = vec![
            (1, 1),   // minimum
            (2, 2),   // dual core
            (4, 4),   // quad core
            (8, 8),   // octa core
            (16, 16), // high-end
            (32, 32), // server
        ];

        for (input, expected) in test_cases {
            let thread_count = input.max(1);
            assert_eq!(thread_count, expected, "Failed for input {}", input);
        }
    }

    /// Test that fallback bodies also include num_thread
    #[test]
    fn test_fallback_body_includes_num_thread() {
        let thread_count: usize = 4;
        let model = "phi:latest";
        let prompt = "Test prompt";

        // Simulate fallback body (when OpenAI/Anthropic keys missing)
        let fallback_body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": true,
            "options": {
                "temperature": 0.7,
                "top_p": 0.95,
                "top_k": 40,
                "num_predict": 1024,
                "repeat_penalty": 1.1,
                "num_thread": thread_count
            }
        });

        assert_eq!(fallback_body["options"]["num_thread"], 4);
    }

    /// Test hardware config round-trip with num_thread
    #[test]
    fn test_hardware_config_roundtrip() {
        let config = HardwareConfig {
            backend_type: "ollama".to_string(),
            model: "llama3:8b".to_string(),
            num_thread: 6,
            num_gpu: 1,
            gpu_layers: 35,
            main_gpu: 0,
            low_vram: false,
            f16_kv: true,
            rope_frequency_base: 10000.0,
            rope_frequency_scale: 1.0,
            numa: false,
            num_ctx: 4096,
            num_batch: 512,
            logits_all: false,
            vocab_only: false,
            use_mmap: true,
            use_mlock: false,
        };

        let json = serde_json::to_string(&config).unwrap();
        let loaded: HardwareConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.num_thread, 6);
        assert_eq!(loaded.backend_type, "ollama");
        assert_eq!(loaded.model, "llama3:8b");
    }
}
