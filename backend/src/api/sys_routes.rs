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
        total_memory_gb: (total_gb * 10.0).round() / 10.0,  // Round to 1 decimal
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
                    tracing::warn!("Failed to connect to Ollama: {}", e);
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
        "llama_cpp" | "vllm" | "custom" => {
            // For local backends, return empty list - user enters model path manually
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
    let manager = PathManager::new()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
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

pub fn sys_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/cores").route(web::get().to(get_physical_cores)));
    cfg.service(web::resource("/memory").route(web::get().to(get_memory)));
    cfg.service(web::resource("/gpus").route(web::get().to(get_gpus)));
    cfg.service(web::resource("/gpu-names").route(web::get().to(get_gpu_names)));
    cfg.service(web::resource("/info").route(web::get().to(get_system_info)));
    cfg.service(web::resource("/models").route(web::get().to(get_models)));
    cfg.service(web::resource("/models/custom").route(web::get().to(get_custom_models)));
}
