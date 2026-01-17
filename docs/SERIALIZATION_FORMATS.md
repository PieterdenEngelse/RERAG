# Serialization Format Decisions

This document describes how complex types are serialized for storage and API communication between the backend and frontend.

## Overview

The system uses **JSON** as the primary serialization format for:
1. **Database storage** - via `param_store` (JSON blobs in SQLite)
2. **API communication** - HTTP JSON payloads between frontend and backend

## Type Serialization Decisions

### 1. Arrays/Vectors → JSON Arrays

All array types are serialized as **native JSON arrays**, not comma-separated strings.

| Rust Type | JSON Format | Example |
|-----------|-------------|---------|
| `Vec<f32>` | `[1.0, 2.0, 3.0]` | `tensor_split: [0.5, 0.5]` |
| `Vec<String>` | `["a", "b", "c"]` | `devices: ["gpu:0", "gpu:1"]` |
| `Vec<bool>` | `[true, false, true]` | `cpumask: [true, false, true, false]` |
| `Vec<KvOverride>` | `[{...}, {...}]` | See below |

**Rationale**: Native JSON arrays are:
- Type-safe (no parsing errors)
- Self-describing (no delimiter ambiguity)
- Directly usable by serde without custom parsers

### 2. Structs → JSON Objects

Complex structs are serialized as nested JSON objects.

```json
// KvOverride
{
  "key": "llama.context_length",
  "value": "4096"
}

// Array of KvOverride
"kv_overrides": [
  { "key": "llama.context_length", "value": "4096" },
  { "key": "llama.embedding_length", "value": "768" }
]
```

### 3. Enums → Snake Case Strings

Enums use `#[serde(rename_all = "snake_case")]` for consistent JSON representation.

| Rust Enum | JSON String |
|-----------|-------------|
| `BackendType::LlamaCpp` | `"llama_cpp"` |
| `BackendType::OpenAi` | `"openai"` |
| `BackendType::Ollama` | `"ollama"` |

### 4. String Type Aliases

Some types are string aliases for flexibility:

| Type Alias | Rust Type | Example Values |
|------------|-----------|----------------|
| `RopeScalingType` | `String` | `"unspecified"`, `"none"`, `"linear"`, `"yarn"` |
| `KvDataType` | `String` | `"f16"`, `"f32"`, `"q8_0"`, `"q4_0"` |
| `DeviceTarget` | `String` | `"gpu:0"`, `"cpu"`, `"cuda:1"` |

**Rationale**: Using strings instead of enums allows:
- Forward compatibility with new llama.cpp options
- No backend changes needed when llama.cpp adds new values
- Frontend can display unknown values gracefully

### 5. Optional Fields

Optional fields use `#[serde(default)]` and `Option<T>`:

```rust
pub seed: Option<i64>,  // null or integer in JSON
```

```json
{ "seed": null }     // No seed
{ "seed": 42 }       // Specific seed
```

## Full Example: HardwareParams JSON

```json
{
  "backend_type": "llama_cpp",
  "model": "phi3",
  
  "gpu_layers": 35,
  "main_gpu": 0,
  "split_mode": "layer",
  "tensor_split": [0.6, 0.4],
  "use_mmap": true,
  "use_mlock": false,
  "vocab_only": false,
  "devices": ["cuda:0", "cuda:1"],
  "kv_overrides": [
    { "key": "llama.context_length", "value": "8192" }
  ],
  "swa_full": false,
  "no_perf": false,
  
  "num_ctx": 4096,
  "num_batch": 512,
  "num_ubatch": 512,
  "num_seq_max": 1,
  "rope_scaling_type": "yarn",
  "rope_frequency_base": 10000.0,
  "rope_frequency_scale": 1.0,
  "yarn_ext_factor": -1.0,
  "yarn_attn_factor": 1.0,
  "yarn_beta_fast": 32.0,
  "yarn_beta_slow": 1.0,
  "yarn_orig_ctx": 0,
  "pooling_type": "unspecified",
  "attention_type": "unspecified",
  "flash_attn": true,
  "type_k": "f16",
  "type_v": "f16",
  "embeddings": false,
  "offload_kqv": true,
  "defrag_thold": 0.1,
  "logits_all": false,
  "f16_kv": true,
  "low_vram": false,
  
  "num_thread": 8,
  "num_thread_batch": 8,
  "numa": false,
  "cpu_strict": false,
  "cpumask": [true, true, true, true, false, false, false, false],
  "mask_valid": true,
  "poll": 50,
  "priority": "normal",
  
  "num_gpu": 2
}
```

## Full Example: LlmConfig JSON

```json
{
  "temperature": 0.7,
  "top_p": 0.95,
  "top_k": 40,
  "max_tokens": 1024,
  "repeat_penalty": 1.1,
  "frequency_penalty": 0.0,
  "presence_penalty": 0.0,
  "stop_sequences": ["\n\n", "User:"],
  "seed": 42,
  "min_p": 0.05,
  "typical_p": 1.0,
  "tfs_z": 1.0,
  
  "mirostat": 0,
  "mirostat_eta": 0.1,
  "mirostat_tau": 5.0,
  
  "repeat_last_n": 64,
  "penalize_newline": true,
  
  "num_predict": 1024,
  "num_keep": 0,
  "ignore_eos": false,
  
  "dry_multiplier": 0.0,
  "dry_base": 1.75,
  "dry_allowed_length": 2,
  
  "xtc_probability": 0.0,
  "xtc_threshold": 0.1
}
```

## Frontend Parsing Helpers

The frontend should handle these formats directly since they're native JSON:

```rust
// No special parsing needed - serde handles it
let config: HardwareConfig = serde_json::from_str(&json)?;

// Arrays are directly accessible
for device in &config.devices {
    println!("Device: {}", device);
}

// KvOverrides are structured
for kv in &config.kv_overrides {
    println!("{} = {}", kv.key, kv.value);
}
```

## Migration Notes

If migrating from comma-separated string formats:

```rust
// Old format (avoid)
"devices": "gpu:0,gpu:1"

// New format (use this)
"devices": ["gpu:0", "gpu:1"]
```

The `#[serde(default)]` attribute ensures backward compatibility - missing fields get default values.

## Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Array format | JSON arrays | Type-safe, no parsing |
| Struct format | JSON objects | Self-describing |
| Enum format | Snake case strings | Consistent, readable |
| String aliases | Plain strings | Forward compatible |
| Optional fields | `Option<T>` + `#[serde(default)]` | Backward compatible |
