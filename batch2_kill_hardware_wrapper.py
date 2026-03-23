#!/usr/bin/env python3
"""
Batch 2: Kill HardwareConfigRequest wrapper
  - Delete HardwareConfigRequest struct + Default + From impls + converter fns (~211 lines)
  - Delete HardwareConfigResponse struct (~8 lines)
  - Delete validate_hardware_request (~89 lines)
  - Rewrite handlers to use HardwareParams directly
  - Extend HardwareParams::validate() with missing checks

Expected savings: ~190 lines (net, after adding extended validation)
"""
import sys, os, shutil
from datetime import datetime

MOD_RS = os.path.expanduser("~/ag/backend/src/api/mod.rs")
HARDWARE_RS = os.path.expanduser("~/ag/backend/src/db/param_hardware.rs")
ts = datetime.now().strftime("%Y%m%d_%H%M%S")
errors = []
changes = []

for f in [MOD_RS, HARDWARE_RS]:
    if not os.path.exists(f):
        print(f"FATAL: {f} not found")
        sys.exit(1)
    shutil.copy2(f, f"{f}.bak.{ts}")
    print(f"[OK] Backed up {os.path.basename(f)}")

# ═══════════════════════════════════════════════════════════════
# FILE 1: param_hardware.rs — extend validate()
# ═══════════════════════════════════════════════════════════════

with open(HARDWARE_RS, 'r') as f:
    hw_content = f.read()

old_validate = '''    pub fn validate(&self) -> Result<()> {
        if self.num_thread == 0 {
            return Err(HardwareParamError::Validation(
                "num_thread must be greater than 0".into(),
            ));
        }
        if self.rope_frequency_base <= 0.0 {
            return Err(HardwareParamError::Validation(
                "rope_frequency_base must be positive".into(),
            ));
        }
        if self.rope_frequency_scale <= 0.0 {
            return Err(HardwareParamError::Validation(
                "rope_frequency_scale must be positive".into(),
            ));
        }
        if self.num_ctx == 0 {
            return Err(HardwareParamError::Validation(
                "num_ctx must be greater than 0".into(),
            ));
        }
        if self.num_batch == 0 {
            return Err(HardwareParamError::Validation(
                "num_batch must be greater than 0".into(),
            ));
        }
        if self.num_ubatch == 0 {
            return Err(HardwareParamError::Validation(
                "num_ubatch must be greater than 0".into(),
            ));
        }
        if self.mask_valid && self.cpumask.is_empty() {
            return Err(HardwareParamError::Validation(
                "cpumask cannot be empty when mask_valid is true".into(),
            ));
        }
        Ok(())
    }'''

new_validate = '''    pub fn validate(&self) -> Result<()> {
        // Thread validation
        if self.num_thread == 0 {
            return Err(HardwareParamError::Validation(
                "num_thread must be greater than 0".into(),
            ));
        }
        if self.num_thread_batch == 0 {
            return Err(HardwareParamError::Validation(
                "num_thread_batch must be greater than 0".into(),
            ));
        }

        // GPU validation
        if self.num_gpu > 64 {
            return Err(HardwareParamError::Validation(
                "num_gpu must be 64 or less".into(),
            ));
        }
        if self.main_gpu > 64 {
            return Err(HardwareParamError::Validation(
                "main_gpu index must be 64 or less".into(),
            ));
        }
        if self.gpu_layers > 1000 {
            return Err(HardwareParamError::Validation(
                "gpu_layers must be 1000 or less".into(),
            ));
        }

        // RoPE validation
        if self.rope_frequency_base <= 0.0 {
            return Err(HardwareParamError::Validation(
                "rope_frequency_base must be positive".into(),
            ));
        }
        if self.rope_frequency_scale <= 0.0 {
            return Err(HardwareParamError::Validation(
                "rope_frequency_scale must be positive".into(),
            ));
        }

        // Context/batch validation
        if self.num_ctx == 0 {
            return Err(HardwareParamError::Validation(
                "num_ctx must be greater than 0".into(),
            ));
        }
        if self.num_batch == 0 {
            return Err(HardwareParamError::Validation(
                "num_batch must be greater than 0".into(),
            ));
        }
        if self.num_ubatch == 0 {
            return Err(HardwareParamError::Validation(
                "num_ubatch must be greater than 0".into(),
            ));
        }
        if self.num_ubatch > self.num_batch {
            return Err(HardwareParamError::Validation(
                "num_ubatch must be <= num_batch".into(),
            ));
        }
        if self.num_seq_max == 0 {
            return Err(HardwareParamError::Validation(
                "num_seq_max must be greater than 0".into(),
            ));
        }

        // CPU mask validation
        if self.mask_valid && self.cpumask.is_empty() {
            return Err(HardwareParamError::Validation(
                "cpumask cannot be empty when mask_valid is true".into(),
            ));
        }

        // Defrag threshold validation
        if self.defrag_thold < 0.0 || self.defrag_thold > 1.0 {
            return Err(HardwareParamError::Validation(
                "defrag_thold must be between 0.0 and 1.0".into(),
            ));
        }

        // Tensor split validation
        if !self.tensor_split.is_empty() {
            let sum: f32 = self.tensor_split.iter().sum();
            if (sum - 1.0).abs() > 0.01 && sum > 0.0 {
                let all_positive = self.tensor_split.iter().all(|&x| x > 0.0);
                if all_positive {
                    return Err(HardwareParamError::Validation(
                        "tensor_split values should sum to approximately 1.0".into(),
                    ));
                }
            }
        }

        // Split mode validation
        let valid_split_modes = ["none", "layer", "row"];
        if !valid_split_modes.contains(&self.split_mode.as_str()) {
            return Err(HardwareParamError::Validation(
                format!("split_mode must be one of: {}", valid_split_modes.join(", ")),
            ));
        }

        // Priority validation
        let valid_priorities = ["low", "normal", "high", "realtime"];
        if !valid_priorities.contains(&self.priority.as_str()) {
            return Err(HardwareParamError::Validation(
                format!("priority must be one of: {}", valid_priorities.join(", ")),
            ));
        }

        Ok(())
    }'''

if old_validate in hw_content:
    hw_content = hw_content.replace(old_validate, new_validate, 1)
    changes.append("Extended HardwareParams::validate() with all validation checks")
else:
    errors.append("FATAL: Could not find validate() in param_hardware.rs")
    print("FATAL: validate() not found in param_hardware.rs")
    sys.exit(1)

with open(HARDWARE_RS, 'w') as f:
    f.write(hw_content)
print("[OK] Updated param_hardware.rs")

# ═══════════════════════════════════════════════════════════════
# FILE 2: mod.rs — delete wrapper types and rewrite handlers
# ═══════════════════════════════════════════════════════════════

with open(MOD_RS, 'r') as f:
    content = f.read()

original_lines = content.count('\n')

# --- DELETE 1: HardwareConfigRequest struct + Default + both From impls + converter fns ---
# This is one big contiguous block from the #[derive] before the struct to end of string_to_backend_type

delete_block_1_start = '#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n#[serde(default)]\nstruct HardwareConfigRequest {'
delete_block_1_end = '''fn string_to_backend_type(s: &str) -> crate::db::param_hardware::BackendType {
    use crate::db::param_hardware::BackendType;
    match s {
        "ollama" => BackendType::Ollama,
        "llama_cpp" => BackendType::LlamaCpp,
        "openai" => BackendType::OpenAi,
        "anthropic" => BackendType::Anthropic,
        "vllm" => BackendType::Vllm,
        "custom" => BackendType::Custom,
        _ => BackendType::Ollama, // default fallback
    }
}'''

idx_start = content.find(delete_block_1_start)
idx_end = content.find(delete_block_1_end)
if idx_start == -1 or idx_end == -1:
    errors.append("FATAL: Could not find HardwareConfigRequest block boundaries")
    print("FATAL: HardwareConfigRequest block not found")
    sys.exit(1)

# Delete from start of the derive annotation to end of string_to_backend_type + trailing newlines
end_of_block = idx_end + len(delete_block_1_end)
# Skip trailing newlines
while end_of_block < len(content) and content[end_of_block] == '\n':
    end_of_block += 1

content = content[:idx_start] + content[end_of_block:]
changes.append("Deleted HardwareConfigRequest struct + Default + From impls + converter fns")

# --- DELETE 2: HardwareConfigResponse struct ---
hw_response_block = '''#[derive(Debug, Serialize)]
struct HardwareConfigResponse {
    status: String,
    message: String,
    request_id: String,
    config: HardwareConfigRequest,
}

'''

if hw_response_block in content:
    content = content.replace(hw_response_block, '', 1)
    changes.append("Deleted HardwareConfigResponse struct")
else:
    errors.append("WARNING: HardwareConfigResponse not found")

# --- DELETE 3: validate_hardware_request function ---
validate_start = 'fn validate_hardware_request(req: &HardwareConfigRequest) -> Result<(), String> {'
validate_end_marker = '    Ok(())\n}'

idx_vs = content.find(validate_start)
if idx_vs == -1:
    errors.append("WARNING: validate_hardware_request not found")
else:
    # Find the Ok(()) } that closes this function
    idx_ve = content.find(validate_end_marker, idx_vs)
    if idx_ve == -1:
        errors.append("WARNING: Could not find end of validate_hardware_request")
    else:
        end_of_validate = idx_ve + len(validate_end_marker)
        # Skip trailing newlines
        while end_of_validate < len(content) and content[end_of_validate] == '\n':
            end_of_validate += 1
        content = content[:idx_vs] + content[end_of_validate:]
        changes.append("Deleted validate_hardware_request function")

# --- REPLACE 4: get_hardware_config handler ---
old_get_handler = '''async fn get_hardware_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = crate::db::param_hardware::global_config().into();
    Ok(HttpResponse::Ok().json(HardwareConfigResponse {
        status: "ok".into(),
        message: "".into(),
        request_id,
        config,
    }))
}'''

new_get_handler = '''async fn get_hardware_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = crate::db::param_hardware::global_config();
    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "message": "",
        "request_id": request_id,
        "config": config
    })))
}'''

if old_get_handler in content:
    content = content.replace(old_get_handler, new_get_handler, 1)
    changes.append("Rewrote get_hardware_config to use HardwareParams directly")
else:
    errors.append("WARNING: get_hardware_config handler not found for replacement")

# --- REPLACE 5: commit_hardware_config handler ---
old_commit_handler = '''async fn commit_hardware_config(
    payload: web::Json<HardwareConfigRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let body = payload.into_inner();

    if let Err(msg) = validate_hardware_request(&body) {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "invalid",
            "message": msg,
            "request_id": request_id
        })));
    }

    let params = crate::db::param_hardware::HardwareParams::from(body.clone());
    match crate::db::param_hardware::save_default_db(&params) {
        Ok(_) => {
            tracing::info!(
                request_id = %request_id,
                num_thread = params.num_thread,
                num_gpu = params.num_gpu,
                gpu_layers = params.gpu_layers,
                main_gpu = params.main_gpu,
                low_vram = params.low_vram,
                f16_kv = params.f16_kv,
                rope_frequency_base = params.rope_frequency_base,
                rope_frequency_scale = params.rope_frequency_scale,
                "Hardware config committed"
            );
            Ok(HttpResponse::Ok().json(HardwareConfigResponse {
                status: "ok".into(),
                message: "Hardware settings saved".into(),
                request_id,
                config: body,
            }))
        }
        Err(err) => {
            tracing::error!(
                request_id = %request_id,
                error = %err,
                "Failed to save hardware config"
            );
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save hardware config: {}", err),
                "request_id": request_id
            })))
        }
    }
}'''

new_commit_handler = '''async fn commit_hardware_config(
    payload: web::Json<crate::db::param_hardware::HardwareParams>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let params = payload.into_inner();
    if let Err(err) = params.validate() {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "invalid",
            "message": err.to_string(),
            "request_id": request_id
        })));
    }
    match crate::db::param_hardware::save_default_db(&params) {
        Ok(_) => {
            tracing::info!(
                request_id = %request_id,
                num_thread = params.num_thread,
                num_gpu = params.num_gpu,
                gpu_layers = params.gpu_layers,
                main_gpu = params.main_gpu,
                low_vram = params.low_vram,
                f16_kv = params.f16_kv,
                rope_frequency_base = params.rope_frequency_base,
                rope_frequency_scale = params.rope_frequency_scale,
                "Hardware config committed"
            );
            Ok(HttpResponse::Ok().json(json!({
                "status": "ok",
                "message": "Hardware settings saved",
                "request_id": request_id,
                "config": params
            })))
        }
        Err(err) => {
            tracing::error!(
                request_id = %request_id,
                error = %err,
                "Failed to save hardware config"
            );
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save hardware config: {}", err),
                "request_id": request_id
            })))
        }
    }
}'''

if old_commit_handler in content:
    content = content.replace(old_commit_handler, new_commit_handler, 1)
    changes.append("Rewrote commit_hardware_config to use HardwareParams directly")
else:
    errors.append("WARNING: commit_hardware_config handler not found for replacement")

# ═══════════════════════════════════════════════════════════════
# WRITE RESULT
# ═══════════════════════════════════════════════════════════════

new_lines = content.count('\n')
saved = original_lines - new_lines

with open(MOD_RS, 'w') as f:
    f.write(content)

print(f"\n{'='*60}")
print(f"CHANGES APPLIED:")
for c in changes:
    print(f"  ✓ {c}")

if errors:
    print(f"\nWARNINGS/ERRORS:")
    for e in errors:
        print(f"  ⚠ {e}")

fatal = [e for e in errors if e.startswith("FATAL")]
if fatal:
    print("\nFATAL errors occurred — check files and restore from .bak")
    sys.exit(1)

print(f"\nmod.rs: {original_lines} → {new_lines} (saved {saved})")
print(f"\nNext: cd ~/ag && cargo check 2>&1 | head -30")
