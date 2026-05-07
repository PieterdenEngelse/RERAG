use crate::path_manager::PathManager;
use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use num_cpus;
use serde::{Deserialize, Serialize};
use std::fs;
use sysinfo::System;
use wgpu::Instance;

#[derive(Serialize)]
struct GpuInfo {
    index: usize,
    name: String,
    vendor: String,
    backend: String,
    device_type: String,
}

#[derive(Serialize)]
struct SystemInfo {
    os: String,
    os_family: String,
    arch: String,
    physical_cores: usize,
    logical_cores: usize,
}

/// Memory information for quantization recommendations
#[derive(Serialize)]
struct MemoryInfo {
    /// Total system RAM in bytes
    total_memory_bytes: u64,
    /// Available (free) RAM in bytes
    available_memory_bytes: u64,
    /// Used RAM in bytes
    used_memory_bytes: u64,
    /// Total RAM in GB (for display)
    total_memory_gb: f64,
    /// Available RAM in GB (for display)
    available_memory_gb: f64,
    /// Memory usage percentage
    usage_percent: f64,
}

// Vendor IDs from PCI database
fn get_vendor_name(vendor_id: u32) -> String {
    match vendor_id {
        0x1002 => "AMD".to_string(),
        0x10DE => "NVIDIA".to_string(),
        0x8086 => "Intel".to_string(),
        0x13B5 => "ARM".to_string(),
        0x5143 => "Qualcomm".to_string(),
        0x1414 => "Microsoft".to_string(),
        0x106B => "Apple".to_string(),
        0x14E4 => "Broadcom".to_string(),
        0x1AE0 => "Google".to_string(),
        0x144D => "Samsung".to_string(),
        _ => format!("Unknown (0x{:04X})", vendor_id),
    }
}

fn get_device_type_name(device_type: wgpu::DeviceType) -> String {
    match device_type {
        wgpu::DeviceType::IntegratedGpu => "Integrated GPU".to_string(),
        wgpu::DeviceType::DiscreteGpu => "Discrete GPU".to_string(),
        wgpu::DeviceType::VirtualGpu => "Virtual GPU".to_string(),
        wgpu::DeviceType::Cpu => "CPU (Software)".to_string(),
        wgpu::DeviceType::Other => "Other".to_string(),
    }
}

fn get_backend_name(backend: wgpu::Backend) -> String {
    match backend {
        wgpu::Backend::Vulkan => "Vulkan".to_string(),
        wgpu::Backend::Metal => "Metal".to_string(),
        wgpu::Backend::Dx12 => "DirectX 12".to_string(),
        wgpu::Backend::Gl => "OpenGL".to_string(),
        wgpu::Backend::BrowserWebGpu => "WebGPU".to_string(),
        wgpu::Backend::Empty => "Empty".to_string(),
    }
}

async fn get_physical_cores() -> impl Responder {
    HttpResponse::Ok().json(num_cpus::get_physical())
}

/// Get system memory information for quantization recommendations
async fn get_memory() -> impl Responder {
    let mut sys = System::new_all();
    sys.refresh_memory();

    let total = sys.total_memory();
    let available = sys.available_memory();
    let used = sys.used_memory();

    let total_gb = total as f64 / 1_073_741_824.0;
    let available_gb = available as f64 / 1_073_741_824.0;
    let usage_percent = if total > 0 {
        (used as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    HttpResponse::Ok().json(MemoryInfo {
        total_memory_bytes: total,
        available_memory_bytes: available,
        used_memory_bytes: used,
        total_memory_gb: (total_gb * 10.0).round() / 10.0, // Round to 1 decimal
        available_memory_gb: (available_gb * 10.0).round() / 10.0,
        usage_percent: (usage_percent * 10.0).round() / 10.0,
    })
}

async fn get_gpus() -> impl Responder {
    let instance = Instance::new(wgpu::InstanceDescriptor::default());
    let adapters: Vec<GpuInfo> = instance
        .enumerate_adapters(wgpu::Backends::all())
        .into_iter()
        .enumerate()
        .map(|(index, adapter)| {
            let info = adapter.get_info();
            GpuInfo {
                index,
                name: info.name,
                vendor: get_vendor_name(info.vendor),
                backend: get_backend_name(info.backend),
                device_type: get_device_type_name(info.device_type),
            }
        })
        .collect();
    HttpResponse::Ok().json(adapters)
}

/// Returns simple GPU names list (for backward compatibility)
async fn get_gpu_names() -> impl Responder {
    let instance = Instance::new(wgpu::InstanceDescriptor::default());
    let names: Vec<String> = instance
        .enumerate_adapters(wgpu::Backends::all())
        .into_iter()
        .map(|adapter| adapter.get_info().name)
        .collect();
    HttpResponse::Ok().json(names)
}

/// Model info returned by the models endpoint
#[derive(Serialize, Clone)]
struct ModelInfo {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    modified_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default)]
    is_custom: bool,
    #[serde(default)]
    is_active: bool,
}

/// Response from Ollama /api/tags endpoint
#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    modified_at: Option<String>,
    #[serde(default)]
    details: Option<OllamaModelDetails>,
}

#[derive(Deserialize)]
struct OllamaModelDetails {
    #[serde(default)]
    family: Option<String>,
}

/// Query params for models endpoint
#[derive(Deserialize)]
struct ModelsQuery {
    backend: Option<String>,
}

/// Fetch available models based on backend type
async fn get_models(query: web::Query<ModelsQuery>) -> impl Responder {
    let backend = query.backend.as_deref().unwrap_or("ollama");

    match backend {
        "ollama" => {
            // Try to fetch from Ollama API
            let ollama_url = std::env::var("OLLAMA_HOST")
                .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
            let url = format!("{}/api/tags", ollama_url);

            match reqwest::get(&url).await {
                Ok(response) => match response.json::<OllamaTagsResponse>().await {
                    Ok(tags) => {
                        let models: Vec<ModelInfo> = tags
                            .models
                            .into_iter()
                            .map(|m| ModelInfo {
                                name: m.name,
                                size: m.size,
                                modified_at: m.modified_at,
                                family: m.details.and_then(|d| d.family),
                                description: None,
                                is_custom: false,
                                is_active: false,
                            })
                            .collect();
                        HttpResponse::Ok().json(models)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse Ollama response: {}", e);
                        HttpResponse::Ok().json(Vec::<ModelInfo>::new())
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to connect to LLM backend: {}", e);
                    HttpResponse::Ok().json(Vec::<ModelInfo>::new())
                }
            }
        }
        "openai" => {
            // Return common OpenAI models as static list
            let models = vec![
                ModelInfo {
                    name: "gpt-4o".to_string(),
                    size: None,
                    modified_at: None,
                    family: Some("GPT-4".to_string()),
                    description: None,
                    is_custom: false,
                    is_active: false,
                },
                ModelInfo {
                    name: "gpt-4o-mini".to_string(),
                    size: None,
                    modified_at: None,
                    family: Some("GPT-4".to_string()),
                    description: None,
                    is_custom: false,
                    is_active: false,
                },
                ModelInfo {
                    name: "gpt-4-turbo".to_string(),
                    size: None,
                    modified_at: None,
                    family: Some("GPT-4".to_string()),
                    description: None,
                    is_custom: false,
                    is_active: false,
                },
                ModelInfo {
                    name: "gpt-4".to_string(),
                    size: None,
                    modified_at: None,
                    family: Some("GPT-4".to_string()),
                    description: None,
                    is_custom: false,
                    is_active: false,
                },
                ModelInfo {
                    name: "gpt-3.5-turbo".to_string(),
                    size: None,
                    modified_at: None,
                    family: Some("GPT-3.5".to_string()),
                    description: None,
                    is_custom: false,
                    is_active: false,
                },
            ];
            HttpResponse::Ok().json(models)
        }
        "anthropic" => {
            // Return common Anthropic models as static list
            let models = vec![
                ModelInfo {
                    name: "claude-3-5-sonnet-latest".to_string(),
                    size: None,
                    modified_at: None,
                    family: Some("Claude 3.5".to_string()),
                    description: None,
                    is_custom: false,
                    is_active: false,
                },
                ModelInfo {
                    name: "claude-3-5-haiku-latest".to_string(),
                    size: None,
                    modified_at: None,
                    family: Some("Claude 3.5".to_string()),
                    description: None,
                    is_custom: false,
                    is_active: false,
                },
                ModelInfo {
                    name: "claude-3-opus-latest".to_string(),
                    size: None,
                    modified_at: None,
                    family: Some("Claude 3".to_string()),
                    description: None,
                    is_custom: false,
                    is_active: false,
                },
                ModelInfo {
                    name: "claude-3-sonnet-20240229".to_string(),
                    size: None,
                    modified_at: None,
                    family: Some("Claude 3".to_string()),
                    description: None,
                    is_custom: false,
                    is_active: false,
                },
                ModelInfo {
                    name: "claude-3-haiku-20240307".to_string(),
                    size: None,
                    modified_at: None,
                    family: Some("Claude 3".to_string()),
                    description: None,
                    is_custom: false,
                    is_active: false,
                },
            ];
            HttpResponse::Ok().json(models)
        }
        "llama_cpp" => {
            // Scan ~/llama.cpp/models/ for .gguf files
            let models_dir = dirs::home_dir()
                .unwrap_or_default()
                .join("llama.cpp/models");
            let mut models: Vec<ModelInfo> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&models_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("gguf") {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            let size = path.metadata().ok().map(|m| m.len());
                            models.push(ModelInfo {
                                name: name.to_string(),
                                size,
                                modified_at: None,
                                family: Some("llama.cpp".to_string()),
                                description: None,
                                is_custom: false,
                                is_active: false,
                            });
                        }
                    }
                }
            }
            models.sort_by(|a, b| a.name.cmp(&b.name));
            HttpResponse::Ok().json(models)
        }
        "vllm" | "custom" => {
            // For other local backends, return empty list - user enters model path manually
            HttpResponse::Ok().json(Vec::<ModelInfo>::new())
        }
        _ => HttpResponse::Ok().json(Vec::<ModelInfo>::new()),
    }
}

async fn get_system_info() -> impl Responder {
    let os = if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Unknown"
    };

    let os_family = if cfg!(target_family = "windows") {
        "windows"
    } else if cfg!(target_family = "unix") {
        "unix"
    } else {
        "unknown"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "unknown"
    };

    let info = SystemInfo {
        os: os.to_string(),
        os_family: os_family.to_string(),
        arch: arch.to_string(),
        physical_cores: num_cpus::get_physical(),
        logical_cores: num_cpus::get(),
    };

    HttpResponse::Ok().json(info)
}

fn custom_models_dir() -> std::io::Result<std::path::PathBuf> {
    let manager = PathManager::new().map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(manager.base_dir().join("models"))
}

fn is_custom_model_enabled() -> bool {
    std::env::var("CUSTOM_MODEL_ENABLED")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false)
}

fn active_custom_model_name() -> Option<String> {
    if !is_custom_model_enabled() {
        return None;
    }
    std::env::var("CUSTOM_MODEL_NAME")
        .ok()
        .filter(|s| !s.is_empty())
}

fn list_custom_models() -> std::io::Result<Vec<ModelInfo>> {
    let dir = custom_models_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let active_name = active_custom_model_name();
    let mut models = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if !matches!(ext, "gguf" | "bin" | "mlmodelc") {
                continue;
            }
        } else {
            continue;
        }
        if let Some(file_name) = path.file_stem().and_then(|s| s.to_str()) {
            let metadata = entry.metadata()?;
            let modified_at = metadata.modified().ok().and_then(|ts| {
                DateTime::<Utc>::from(ts)
                    .format("%Y-%m-%d %H:%M:%S UTC")
                    .to_string()
                    .into()
            });
            let name = file_name.to_string();
            let is_active = active_name
                .as_ref()
                .map(|active| active.eq_ignore_ascii_case(&name))
                .unwrap_or(false);
            models.push(ModelInfo {
                name: name.clone(),
                size: Some(metadata.len()),
                modified_at,
                family: Some("custom".to_string()),
                description: Some(format!("Local file ({})", path.display())),
                is_custom: true,
                is_active,
            });
        }
    }
    Ok(models)
}

async fn get_custom_models() -> impl Responder {
    match list_custom_models() {
        Ok(models) => HttpResponse::Ok().json(models),
        Err(err) => {
            tracing::warn!("Failed to list custom models: {}", err);
            HttpResponse::Ok().json(Vec::<ModelInfo>::new())
        }
    }
}

/// Runtime health check response
#[derive(serde::Serialize)]
pub struct RuntimeHealth {
    pub ollama_available: bool,
    pub llama_cpp_available: bool,
    pub active_backend: Option<String>,
}

/// GET /system/runtime-health
pub async fn runtime_health() -> impl Responder {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();

    // Probe Ollama (11434)
    let ollama_available = client
        .get("http://127.0.0.1:11434/api/tags")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    // Probe llama-server (11435)
    let llama_cpp_available = client
        .get("http://127.0.0.1:11435/health")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    // Determine active backend (prefer configured, fallback to available)
    let active_backend = if llama_cpp_available {
        Some("llama_cpp".to_string())
    } else if ollama_available {
        Some("ollama".to_string())
    } else {
        None
    };

    HttpResponse::Ok().json(RuntimeHealth {
        ollama_available,
        llama_cpp_available,
        active_backend,
    })
}

/// Request body for runtime actions
#[derive(serde::Deserialize)]
pub struct RuntimeActionRequest {
    pub action: String,
}

/// POST /sys/runtime/action
/// Switch between LLM backends (ollama, llama_cpp)
pub async fn runtime_action(body: web::Json<RuntimeActionRequest>) -> impl Responder {
    let action = body.action.as_str();

    // Handle backend switching: stop one, start the other
    let commands: Vec<(&str, &str)> = match action {
        "stop" => vec![("stop", "ollama.service")],
        "start" => vec![("start", "ollama.service")],
        "switch_ollama" => vec![
            ("stop", "llama-server.service"),
            ("start", "ollama.service"),
        ],
        "switch_llama_cpp" => vec![
            ("stop", "ollama.service"),
            ("start", "llama-server.service"),
        ],
        _ => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "error": format!("Unknown runtime action: {}", action),
            }));
        }
    };

    // Execute commands sequentially
    for (cmd, service) in &commands {
        let output = tokio::process::Command::new("systemctl")
            .arg("--user")
            .args([*cmd, *service])
            .output()
            .await;

        if let Err(e) = output {
            tracing::warn!("Failed to {} {}: {}", cmd, service, e);
        }
    }

    // Give service time to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Return current status
    let ollama_running = tokio::process::Command::new("systemctl")
        .args(["--user", "is-active", "ollama.service"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    let llama_running = tokio::process::Command::new("systemctl")
        .args(["--user", "is-active", "llama-server.service"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    let active_backend = if llama_running {
        "llama_cpp"
    } else if ollama_running {
        "ollama"
    } else {
        "none"
    };

    // Reload tokenizer for new backend
    reload_token_counter();
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "active_backend": active_backend,
        "ollama_running": ollama_running,
        "llama_cpp_running": llama_running,
    }))
}

// ── Loaded model endpoint ── v1.1.0 ─────────────────────────────────

/// What's currently loaded in GPU/CPU memory
#[derive(Serialize)]
pub struct LoadedModelResponse {
    /// Which backend is serving ("ollama", "llama_cpp", or "none")
    pub backend: String,
    /// Model identifier currently in memory (None if nothing loaded)
    pub model: Option<String>,
    /// Model file size in bytes (when available)
    pub size: Option<u64>,
    /// VRAM or RAM used by the model in bytes (Ollama only)
    pub memory_used: Option<u64>,
    /// Time the model expires from memory (Ollama only, ISO 8601)
    pub expires_at: Option<String>,
}

/// Ollama /api/ps response
#[derive(Deserialize)]
struct OllamaPsResponse {
    models: Vec<OllamaPsModel>,
}

#[derive(Deserialize)]
struct OllamaPsModel {
    name: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    size_vram: Option<u64>,
    #[serde(default)]
    expires_at: Option<String>,
}

/// llama-server /props response (subset we care about)
#[derive(Deserialize)]
struct LlamaServerProps {
    #[serde(default, alias = "default_generation_settings")]
    default_generation_settings: Option<LlamaGenSettings>,
}

#[derive(Deserialize)]
struct LlamaGenSettings {
    #[serde(default)]
    model: Option<String>,
}

/// GET /sys/loaded-model
pub async fn loaded_model() -> impl Responder {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();

    // ── Try Ollama /api/ps first ──
    if let Ok(resp) = client.get("http://127.0.0.1:11434/api/ps").send().await {
        if resp.status().is_success() {
            if let Ok(ps) = resp.json::<OllamaPsResponse>().await {
                if let Some(m) = ps.models.first() {
                    return HttpResponse::Ok().json(LoadedModelResponse {
                        backend: "ollama".into(),
                        model: Some(m.name.clone()),
                        size: m.size,
                        memory_used: m.size_vram,
                        expires_at: m.expires_at.clone(),
                    });
                }
                // Ollama is running but no model loaded
                return HttpResponse::Ok().json(LoadedModelResponse {
                    backend: "ollama".into(),
                    model: None,
                    size: None,
                    memory_used: None,
                    expires_at: None,
                });
            }
        }
    }

    // ── Try llama-server /props ──
    if let Ok(resp) = client.get("http://127.0.0.1:11435/props").send().await {
        if resp.status().is_success() {
            let model_name = resp
                .json::<LlamaServerProps>()
                .await
                .ok()
                .and_then(|p| p.default_generation_settings)
                .and_then(|g| g.model);

            return HttpResponse::Ok().json(LoadedModelResponse {
                backend: "llama_cpp".into(),
                model: model_name,
                size: None,
                memory_used: None,
                expires_at: None,
            });
        }
    }

    // ── Neither backend responding ──
    HttpResponse::Ok().json(LoadedModelResponse {
        backend: "none".into(),
        model: None,
        size: None,
        memory_used: None,
        expires_at: None,
    })
}

/// Request body for llama model update
#[derive(Deserialize)]
pub struct LlamaModelRequest {
    pub model: String,
}

/// POST /sys/llama-model
/// Update llama-server model in env file and restart service
pub async fn set_llama_model(body: web::Json<LlamaModelRequest>) -> impl Responder {
    let model_name = body.model.trim();
    if model_name.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "error": "Model name cannot be empty",
        }));
    }

    // Resolve full path
    let models_dir = dirs::home_dir()
        .unwrap_or_default()
        .join("llama.cpp/models");
    let model_path = models_dir.join(model_name);

    if !model_path.exists() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "error": format!("Model not found: {}", model_path.display()),
        }));
    }

    // Write env file
    let env_dir = dirs::home_dir().unwrap_or_default().join(".config/ag");
    let env_path = env_dir.join("llama-server.env");

    if let Err(e) = std::fs::create_dir_all(&env_dir) {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "status": "error",
            "error": format!("Failed to create config dir: {}", e),
        }));
    }

    let env_content = format!(
        "LLAMA_MODEL={}
",
        model_path.display()
    );
    if let Err(e) = std::fs::write(&env_path, &env_content) {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "status": "error",
            "error": format!("Failed to write env file: {}", e),
        }));
    }

    // Restart llama-server
    let _ = tokio::process::Command::new("systemctl")
        .args(["--user", "restart", "llama-server.service"])
        .output()
        .await;

    tracing::info!("Updated llama-server model to: {}", model_path.display());
    reload_token_counter();

    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "model": model_name,
        "model_path": model_path.display().to_string(),
    }))
}

/// Reload the global token counter from the current model's GGUF.
pub fn reload_token_counter() {
    use crate::gguf_tokenizer::FallbackReason;
    let hw = crate::db::param_hardware::global_config();
    if let Some(handle) = crate::api::get_token_counter() {
        let (result, expected_local_gguf) = match hw.backend_type {
            crate::db::param_hardware::BackendType::Ollama => {
                if !hw.model.is_empty() {
                    (
                        crate::gguf_tokenizer::resolve_ollama_gguf_path(&hw.model),
                        true,
                    )
                } else {
                    handle.mark_fallback(FallbackReason::NoModelConfigured, None);
                    (Err(anyhow::anyhow!("No model configured")), false)
                }
            }
            crate::db::param_hardware::BackendType::LlamaCpp => (
                crate::gguf_tokenizer::resolve_llama_server_gguf_path(),
                true,
            ),
            _ => {
                handle.mark_fallback(FallbackReason::CloudBackend, None);
                (Err(anyhow::anyhow!("Cloud backend, no local GGUF")), false)
            }
        };
        if expected_local_gguf {
            match result {
                Ok(path) => {
                    if let Ok(()) = handle.load_from_gguf(&path) {
                        tracing::info!(
                            model = %handle.model_name(),
                            vocab = handle.vocab_size(),
                            "Token counter reloaded"
                        );
                    } // else: load_from_gguf recorded the fallback + warned
                }
                Err(e) => handle.mark_fallback(
                    FallbackReason::PathNotFound {
                        detail: format!("{:#}", e),
                    },
                    None,
                ),
            }
        }
    }
}

/// Request body for explicit tokenizer swap
#[derive(Deserialize)]
pub struct TokenizerSwapRequest {
    pub candidate_path: Option<String>,
    pub candidate_ollama_model: Option<String>,
    pub candidate_llama_cpp: Option<bool>,
}

/// POST /sys/tokenizer/swap
/// Apply an explicit GGUF (by path, Ollama model, or llama.cpp active model)
/// to the live token counter without touching the configured backend.
pub async fn swap_tokenizer(body: web::Json<TokenizerSwapRequest>) -> impl Responder {
    let req = body.into_inner();
    let llama_cpp = req.candidate_llama_cpp.unwrap_or(false);
    let sources = [
        req.candidate_path.is_some(),
        req.candidate_ollama_model.is_some(),
        llama_cpp,
    ]
    .iter()
    .filter(|&&v| v)
    .count();
    if sources > 1 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "error": "Specify exactly one of candidate_path, candidate_ollama_model, or candidate_llama_cpp",
        }));
    }
    let candidate = if let Some(p) = req.candidate_path {
        let pb = std::path::PathBuf::from(&p);
        if !pb.exists() {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": "error",
                "error": format!("candidate_path does not exist: {}", p),
            }));
        }
        pb
    } else if let Some(m) = req.candidate_ollama_model {
        match crate::gguf_tokenizer::resolve_ollama_gguf_path(&m) {
            Ok(p) => p,
            Err(e) => {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": "error",
                    "error": format!("Failed to resolve Ollama model {:?}: {:#}", m, e),
                }));
            }
        }
    } else if llama_cpp {
        match crate::gguf_tokenizer::resolve_llama_server_gguf_path() {
            Ok(p) => p,
            Err(e) => {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": "error",
                    "error": format!("Failed to resolve llama.cpp model: {:#}", e),
                }));
            }
        }
    } else {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "error": "Must specify candidate_path, candidate_ollama_model, or candidate_llama_cpp",
        }));
    };

    let handle = match crate::api::get_token_counter() {
        Some(h) => h,
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "error": "Token counter not initialized",
            }));
        }
    };

    match handle.load_from_gguf(&candidate) {
        Ok(()) => {
            tracing::info!(
                model = %handle.model_name(),
                vocab = handle.vocab_size(),
                path = %candidate.display(),
                "Tokenizer swapped via /sys/tokenizer/swap"
            );
            let status = handle.status();
            HttpResponse::Ok().json(serde_json::json!({
                "status": "ok",
                "model": handle.model_name(),
                "vocab_size": handle.vocab_size(),
                "is_exact": handle.is_exact(),
                "mode": status.mode,
                "candidate_path": candidate.display().to_string(),
                "note": "Tokenizer swapped. Re-capture the golden sample so the new baseline is captured under this tokenizer.",
            }))
        }
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "error": format!("{:#}", e),
        })),
    }
}

/// GET /sys/tokenizer-info
pub async fn tokenizer_info() -> impl Responder {
    match crate::api::get_token_counter() {
        Some(handle) => {
            let status = handle.status();
            HttpResponse::Ok().json(serde_json::json!({
                "model": handle.model_name(),
                "vocab_size": handle.vocab_size(),
                "is_exact": handle.is_exact(),
                "mode": status.mode,
                "fallback_reason": status.fallback_reason.as_ref().map(|r| r.discriminant()),
                "fallback_detail": status.fallback_reason.as_ref().and_then(|r| r.detail().map(|s| s.to_string())),
                "attempted_path": status.attempted_path,
                "attempted_at": status.attempted_at,
            }))
        }
        None => HttpResponse::Ok().json(serde_json::json!({
            "model": "unavailable",
            "vocab_size": 0,
            "is_exact": false,
            "mode": "heuristic",
            "fallback_reason": "not_attempted",
            "fallback_detail": null,
            "attempted_path": null,
            "attempted_at": null,
        })),
    }
}

/// POST /sys/restart — restart the ag.service unit and respond before the process dies.
pub async fn restart_backend() -> HttpResponse {
    tracing::info!("Backend restart requested via API");
    // Spawn the restart with a short delay so the HTTP response can be flushed first.
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        let _ = tokio::process::Command::new("systemctl")
            .args(["--user", "restart", "ag.service"])
            .output()
            .await;
    });
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "message": "Restart initiated — ag.service will restart momentarily."
    }))
}

/// POST /sys/restart-process — re-exec just the ag binary without touching ag.service or Docker.
/// Reads /proc/self/exe to get the current binary path, flushes the response, then exec-replaces
/// the process. Docker services and systemd are untouched.
pub async fn restart_process() -> HttpResponse {
    tracing::info!("Process re-exec requested via API");
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        // Read the current executable path from /proc/self/exe (follows the symlink)
        let exe = match std::fs::read_link("/proc/self/exe") {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to read /proc/self/exe: {e}");
                return;
            }
        };
        // Build null-terminated argv and envp for execve
        let to_cstring = |s: String| std::ffi::CString::new(s).ok();
        let args: Vec<std::ffi::CString> = std::env::args().filter_map(to_cstring).collect();
        let env: Vec<std::ffi::CString> = std::env::vars()
            .filter_map(|(k, v)| to_cstring(format!("{k}={v}")))
            .collect();
        let exe_cstr = match std::ffi::CString::new(exe.to_string_lossy().as_bytes()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Invalid exe path: {e}");
                return;
            }
        };

        // Build raw pointer arrays (null-terminated)
        let mut argv: Vec<*const libc::c_char> = args.iter().map(|s| s.as_ptr()).collect();
        argv.push(std::ptr::null());
        let mut envp: Vec<*const libc::c_char> = env.iter().map(|s| s.as_ptr()).collect();
        envp.push(std::ptr::null());

        tracing::info!(exe = %exe.display(), "Exec-replacing process");
        // execve replaces the process image; returns only on failure
        unsafe { libc::execve(exe_cstr.as_ptr(), argv.as_ptr(), envp.as_ptr()) };
        tracing::error!("execve failed: {}", std::io::Error::last_os_error());
    });
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "message": "Process restart initiated — binary will re-exec momentarily."
    }))
}

pub fn sys_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/cores").route(web::get().to(get_physical_cores)));
    cfg.service(web::resource("/memory").route(web::get().to(get_memory)));
    cfg.service(web::resource("/gpus").route(web::get().to(get_gpus)));
    cfg.service(web::resource("/gpu-names").route(web::get().to(get_gpu_names)));
    cfg.service(web::resource("/info").route(web::get().to(get_system_info)));
    cfg.service(web::resource("/models").route(web::get().to(get_models)));
    cfg.service(web::resource("/models/custom").route(web::get().to(get_custom_models)));
    cfg.service(web::resource("/runtime-health").route(web::get().to(runtime_health)));
    cfg.service(web::resource("/runtime/action").route(web::post().to(runtime_action)));
    cfg.service(web::resource("/loaded-model").route(web::get().to(loaded_model)));
    cfg.service(web::resource("/tokenizer-info").route(web::get().to(tokenizer_info)));
    cfg.service(web::resource("/tokenizer/swap").route(web::post().to(swap_tokenizer)));
    cfg.service(web::resource("/llama-model").route(web::post().to(set_llama_model)));
    cfg.service(web::resource("/restart").route(web::post().to(restart_backend)));
    cfg.service(web::resource("/restart-process").route(web::post().to(restart_process)));
}
