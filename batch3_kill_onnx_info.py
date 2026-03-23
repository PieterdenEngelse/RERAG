#!/usr/bin/env python3
"""
Batch 3: Kill OnnxConfigInfo wrapper
  - Add Serialize to OnnxConfig and its enums in onnx_embedder.rs
  - Delete OnnxConfigResponse + OnnxConfigInfo structs from mod.rs
  - Delete onnx_*_to_str converter functions
  - Rewrite get_onnx_config and set_onnx_config response to serialize OnnxConfig directly

Expected savings: ~130 lines
"""
import sys, os, shutil
from datetime import datetime

MOD_RS = os.path.expanduser("~/ag/backend/src/api/mod.rs")
ONNX_RS = os.path.expanduser("~/ag/backend/src/perf/onnx_embedder.rs")
ts = datetime.now().strftime("%Y%m%d_%H%M%S")
errors = []
changes = []

for f in [MOD_RS, ONNX_RS]:
    if not os.path.exists(f):
        print(f"FATAL: {f} not found")
        sys.exit(1)
    shutil.copy2(f, f"{f}.bak.{ts}")
    print(f"[OK] Backed up {os.path.basename(f)}")

# ═══════════════════════════════════════════════════════════════
# FILE 1: onnx_embedder.rs — add Serialize to enums + OnnxConfig
# ═══════════════════════════════════════════════════════════════

with open(ONNX_RS, 'r') as f:
    onnx_content = f.read()

# Add serde import
old_imports = 'use std::path::Path;\nuse tracing::info;'
new_imports = 'use serde::Serialize;\nuse std::path::Path;\nuse tracing::info;'

if old_imports in onnx_content:
    onnx_content = onnx_content.replace(old_imports, new_imports, 1)
    changes.append("Added serde::Serialize import to onnx_embedder.rs")
else:
    errors.append("WARNING: Could not find import block in onnx_embedder.rs")

# Add Serialize + serde rename to OnnxOptimizationLevel
old_opt = '#[derive(Debug, Clone, Copy, Default)]\npub enum OnnxOptimizationLevel {'
new_opt = '#[derive(Debug, Clone, Copy, Default, Serialize)]\n#[serde(rename_all = "lowercase")]\npub enum OnnxOptimizationLevel {'

if old_opt in onnx_content:
    onnx_content = onnx_content.replace(old_opt, new_opt, 1)
    changes.append("Added Serialize to OnnxOptimizationLevel")
else:
    errors.append("WARNING: OnnxOptimizationLevel derive not found")

# Add Serialize + serde rename to OnnxExecutionMode
old_exec = '#[derive(Debug, Clone, Copy, Default)]\npub enum OnnxExecutionMode {'
new_exec = '#[derive(Debug, Clone, Copy, Default, Serialize)]\n#[serde(rename_all = "lowercase")]\npub enum OnnxExecutionMode {'

if old_exec in onnx_content:
    onnx_content = onnx_content.replace(old_exec, new_exec, 1)
    changes.append("Added Serialize to OnnxExecutionMode")
else:
    errors.append("WARNING: OnnxExecutionMode derive not found")

# Add Serialize + serde rename to OnnxLogLevel
old_log = '#[derive(Debug, Clone, Copy, Default)]\npub enum OnnxLogLevel {'
new_log = '#[derive(Debug, Clone, Copy, Default, Serialize)]\n#[serde(rename_all = "lowercase")]\npub enum OnnxLogLevel {'

if old_log in onnx_content:
    onnx_content = onnx_content.replace(old_log, new_log, 1)
    changes.append("Added Serialize to OnnxLogLevel")
else:
    errors.append("WARNING: OnnxLogLevel derive not found")

# Add Serialize to OnnxConfig
old_config = '#[derive(Debug, Clone)]\npub struct OnnxConfig {'
new_config = '#[derive(Debug, Clone, Serialize)]\npub struct OnnxConfig {'

if old_config in onnx_content:
    onnx_content = onnx_content.replace(old_config, new_config, 1)
    changes.append("Added Serialize to OnnxConfig")
else:
    errors.append("WARNING: OnnxConfig derive not found")

with open(ONNX_RS, 'w') as f:
    f.write(onnx_content)
print("[OK] Updated onnx_embedder.rs")

# ═══════════════════════════════════════════════════════════════
# FILE 2: mod.rs — delete wrappers and rewrite handlers
# ═══════════════════════════════════════════════════════════════

with open(MOD_RS, 'r') as f:
    content = f.read()

original_lines = content.count('\n')

# --- DELETE 1: OnnxConfigResponse + OnnxConfigInfo structs ---
delete_structs = '''#[derive(Debug, Serialize)]
struct OnnxConfigResponse {
    status: String,
    message: String,
    request_id: String,
    config: OnnxConfigInfo,
}

#[derive(Debug, Serialize)]
struct OnnxConfigInfo {
    model_path: String,
    max_length: usize,
    embedding_dim: usize,
    num_threads: usize,
    inter_op_num_threads: usize,
    optimization_level: String,
    execution_mode: String,
    enable_mem_pattern: bool,
    enable_cpu_mem_arena: bool,
    deterministic_compute: bool,
    optimized_model_path: Option<String>,
    enable_profiling: bool,
    profiling_output_path: Option<String>,
    log_id: Option<String>,
    log_level: String,
    log_verbosity: i32,
    use_env_allocators: bool,
    denormal_as_zero: bool,
    enable_quant_qdq: bool,
    enable_double_qdq_remover: bool,
    enable_qdq_cleanup: bool,
    approximate_gelu: bool,
    enable_aot_inlining: bool,
    disabled_optimizers: Vec<String>,
    use_device_allocator_for_initializers: bool,
    allow_inter_op_spinning: bool,
    allow_intra_op_spinning: bool,
    use_prepacking: bool,
    independent_thread_pool: bool,
    no_env_execution_providers: bool,
}

'''

if delete_structs in content:
    content = content.replace(delete_structs, '', 1)
    changes.append("Deleted OnnxConfigResponse + OnnxConfigInfo structs")
else:
    errors.append("FATAL: OnnxConfigResponse + OnnxConfigInfo block not found")

# --- DELETE 2: onnx_*_to_str converter functions ---
delete_converters = '''fn onnx_opt_level_to_str(level: OnnxOptimizationLevel) -> &'static str {
    match level {
        OnnxOptimizationLevel::Disable => "disable",
        OnnxOptimizationLevel::Basic => "basic",
        OnnxOptimizationLevel::Extended => "extended",
        OnnxOptimizationLevel::All => "all",
    }
}

fn onnx_exec_mode_to_str(mode: OnnxExecutionMode) -> &'static str {
    match mode {
        OnnxExecutionMode::Sequential => "sequential",
        OnnxExecutionMode::Parallel => "parallel",
    }
}

fn onnx_log_level_to_str(level: OnnxLogLevel) -> &'static str {
    match level {
        OnnxLogLevel::Verbose => "verbose",
        OnnxLogLevel::Info => "info",
        OnnxLogLevel::Warning => "warning",
        OnnxLogLevel::Error => "error",
        OnnxLogLevel::Fatal => "fatal",
    }
}

'''

if delete_converters in content:
    content = content.replace(delete_converters, '', 1)
    changes.append("Deleted onnx_*_to_str converter functions")
else:
    errors.append("FATAL: onnx_*_to_str converter block not found")

# --- REPLACE 3: get_onnx_config handler ---
old_get_onnx = '''async fn get_onnx_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = get_onnx_config_storage().read().unwrap();

    let opt_level_str = onnx_opt_level_to_str(config.optimization_level);
    let exec_mode_str = onnx_exec_mode_to_str(config.execution_mode);
    let log_level_str = onnx_log_level_to_str(config.log_level);

    Ok(HttpResponse::Ok().json(OnnxConfigResponse {
        status: "ok".into(),
        message: "".into(),
        request_id,
        config: OnnxConfigInfo {
            model_path: config.model_path.clone(),
            max_length: config.max_length,
            embedding_dim: config.embedding_dim,
            num_threads: config.num_threads,
            inter_op_num_threads: config.inter_op_num_threads,
            optimization_level: opt_level_str.to_string(),
            execution_mode: exec_mode_str.to_string(),
            enable_mem_pattern: config.enable_mem_pattern,
            enable_cpu_mem_arena: config.enable_cpu_mem_arena,
            deterministic_compute: config.deterministic_compute,
            optimized_model_path: config.optimized_model_path.clone(),
            enable_profiling: config.enable_profiling,
            profiling_output_path: config.profiling_output_path.clone(),
            log_id: config.log_id.clone(),
            log_level: log_level_str.to_string(),
            log_verbosity: config.log_verbosity,
            use_env_allocators: config.use_env_allocators,
            denormal_as_zero: config.denormal_as_zero,
            enable_quant_qdq: config.enable_quant_qdq,
            enable_double_qdq_remover: config.enable_double_qdq_remover,
            enable_qdq_cleanup: config.enable_qdq_cleanup,
            approximate_gelu: config.approximate_gelu,
            enable_aot_inlining: config.enable_aot_inlining,
            disabled_optimizers: config.disabled_optimizers.clone(),
            use_device_allocator_for_initializers: config.use_device_allocator_for_initializers,
            allow_inter_op_spinning: config.allow_inter_op_spinning,
            allow_intra_op_spinning: config.allow_intra_op_spinning,
            use_prepacking: config.use_prepacking,
            independent_thread_pool: config.independent_thread_pool,
            no_env_execution_providers: config.no_env_execution_providers,
        },
    }))
}'''

new_get_onnx = '''async fn get_onnx_config() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = get_onnx_config_storage().read().unwrap();
    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "message": "",
        "request_id": request_id,
        "config": *config
    })))
}'''

if old_get_onnx in content:
    content = content.replace(old_get_onnx, new_get_onnx, 1)
    changes.append("Rewrote get_onnx_config to serialize OnnxConfig directly")
else:
    errors.append("FATAL: get_onnx_config handler not found")

# --- REPLACE 4: set_onnx_config response tail ---
# Replace from the _to_str calls to end of the function
old_set_tail = '''    let opt_level_str = onnx_opt_level_to_str(config.optimization_level);
    let exec_mode_str = onnx_exec_mode_to_str(config.execution_mode);
    let log_level_str = onnx_log_level_to_str(config.log_level);

    tracing::info!(
        request_id = %request_id,
        num_threads = config.num_threads,
        inter_op_threads = config.inter_op_num_threads,
        optimization_level = opt_level_str,
        execution_mode = exec_mode_str,
        deterministic_compute = config.deterministic_compute,
        enable_profiling = config.enable_profiling,
        log_level = log_level_str,
        "ONNX config updated (restart required to apply)"
    );

    Ok(HttpResponse::Ok().json(OnnxConfigResponse {
        status: "ok".into(),
        message: "ONNX config updated. Restart backend to apply changes to embedder.".into(),
        request_id,
        config: OnnxConfigInfo {
            model_path: config.model_path.clone(),
            max_length: config.max_length,
            embedding_dim: config.embedding_dim,
            num_threads: config.num_threads,
            inter_op_num_threads: config.inter_op_num_threads,
            optimization_level: opt_level_str.to_string(),
            execution_mode: exec_mode_str.to_string(),
            enable_mem_pattern: config.enable_mem_pattern,
            enable_cpu_mem_arena: config.enable_cpu_mem_arena,
            deterministic_compute: config.deterministic_compute,
            optimized_model_path: config.optimized_model_path.clone(),
            enable_profiling: config.enable_profiling,
            profiling_output_path: config.profiling_output_path.clone(),
            log_id: config.log_id.clone(),
            log_level: log_level_str.to_string(),
            log_verbosity: config.log_verbosity,
            use_env_allocators: config.use_env_allocators,
            denormal_as_zero: config.denormal_as_zero,
            enable_quant_qdq: config.enable_quant_qdq,
            enable_double_qdq_remover: config.enable_double_qdq_remover,
            enable_qdq_cleanup: config.enable_qdq_cleanup,
            approximate_gelu: config.approximate_gelu,
            enable_aot_inlining: config.enable_aot_inlining,
            disabled_optimizers: config.disabled_optimizers.clone(),
            use_device_allocator_for_initializers: config.use_device_allocator_for_initializers,
            allow_inter_op_spinning: config.allow_inter_op_spinning,
            allow_intra_op_spinning: config.allow_intra_op_spinning,
            use_prepacking: config.use_prepacking,
            independent_thread_pool: config.independent_thread_pool,
            no_env_execution_providers: config.no_env_execution_providers,
        },
    }))
}'''

new_set_tail = '''    tracing::info!(
        request_id = %request_id,
        num_threads = config.num_threads,
        inter_op_threads = config.inter_op_num_threads,
        deterministic_compute = config.deterministic_compute,
        enable_profiling = config.enable_profiling,
        "ONNX config updated (restart required to apply)"
    );

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "message": "ONNX config updated. Restart backend to apply changes to embedder.",
        "request_id": request_id,
        "config": *config
    })))
}'''

if old_set_tail in content:
    content = content.replace(old_set_tail, new_set_tail, 1)
    changes.append("Rewrote set_onnx_config response to serialize OnnxConfig directly")
else:
    errors.append("FATAL: set_onnx_config response tail not found")

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
