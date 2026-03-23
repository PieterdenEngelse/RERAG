#!/usr/bin/env python3
"""
Batch 4: LLM default one-liners + io_uring typed struct
  - Condense 15 LLM default_* fns to one-liners (~30 lines saved)
  - Replace save_io_uring_config with typed IoUringConfig struct (~110 lines saved)

Expected savings: ~140 lines
"""
import sys, os, shutil
from datetime import datetime

MOD_RS = os.path.expanduser("~/ag/backend/src/api/mod.rs")
ts = datetime.now().strftime("%Y%m%d_%H%M%S")
errors = []
changes = []

if not os.path.exists(MOD_RS):
    print(f"FATAL: {MOD_RS} not found")
    sys.exit(1)

shutil.copy2(MOD_RS, f"{MOD_RS}.bak.{ts}")
print(f"[OK] Backed up mod.rs")

with open(MOD_RS, 'r') as f:
    content = f.read()

original_lines = content.count('\n')

# ═══════════════════════════════════════════════════════════════
# CHANGE 1: LLM default fns → one-liners
# ═══════════════════════════════════════════════════════════════

old_defaults = """fn default_min_p() -> f32 {
    llm_settings::DEFAULT_MIN_P
}
fn default_typical_p() -> f32 {
    llm_settings::DEFAULT_TYPICAL_P
}
fn default_tfs_z() -> f32 {
    llm_settings::DEFAULT_TFS_Z
}
fn default_mirostat() -> i32 {
    llm_settings::DEFAULT_MIROSTAT
}
fn default_mirostat_eta() -> f32 {
    llm_settings::DEFAULT_MIROSTAT_ETA
}
fn default_mirostat_tau() -> f32 {
    llm_settings::DEFAULT_MIROSTAT_TAU
}
fn default_repeat_last_n() -> usize {
    llm_settings::DEFAULT_REPEAT_LAST_N
}
fn default_num_keep() -> i64 {
    llm_settings::DEFAULT_NUM_KEEP
}
fn default_penalize_newline() -> bool {
    llm_settings::DEFAULT_PENALIZE_NEWLINE
}
fn default_ignore_eos() -> bool {
    llm_settings::DEFAULT_IGNORE_EOS
}
fn default_dry_multiplier() -> f32 {
    llm_settings::DEFAULT_DRY_MULTIPLIER
}
fn default_dry_base() -> f32 {
    llm_settings::DEFAULT_DRY_BASE
}
fn default_dry_allowed_length() -> usize {
    llm_settings::DEFAULT_DRY_ALLOWED_LENGTH
}
fn default_xtc_probability() -> f32 {
    llm_settings::DEFAULT_XTC_PROBABILITY
}
fn default_xtc_threshold() -> f32 {
    llm_settings::DEFAULT_XTC_THRESHOLD
}"""

new_defaults = """fn default_min_p() -> f32 { llm_settings::DEFAULT_MIN_P }
fn default_typical_p() -> f32 { llm_settings::DEFAULT_TYPICAL_P }
fn default_tfs_z() -> f32 { llm_settings::DEFAULT_TFS_Z }
fn default_mirostat() -> i32 { llm_settings::DEFAULT_MIROSTAT }
fn default_mirostat_eta() -> f32 { llm_settings::DEFAULT_MIROSTAT_ETA }
fn default_mirostat_tau() -> f32 { llm_settings::DEFAULT_MIROSTAT_TAU }
fn default_repeat_last_n() -> usize { llm_settings::DEFAULT_REPEAT_LAST_N }
fn default_num_keep() -> i64 { llm_settings::DEFAULT_NUM_KEEP }
fn default_penalize_newline() -> bool { llm_settings::DEFAULT_PENALIZE_NEWLINE }
fn default_ignore_eos() -> bool { llm_settings::DEFAULT_IGNORE_EOS }
fn default_dry_multiplier() -> f32 { llm_settings::DEFAULT_DRY_MULTIPLIER }
fn default_dry_base() -> f32 { llm_settings::DEFAULT_DRY_BASE }
fn default_dry_allowed_length() -> usize { llm_settings::DEFAULT_DRY_ALLOWED_LENGTH }
fn default_xtc_probability() -> f32 { llm_settings::DEFAULT_XTC_PROBABILITY }
fn default_xtc_threshold() -> f32 { llm_settings::DEFAULT_XTC_THRESHOLD }"""

if old_defaults in content:
    content = content.replace(old_defaults, new_defaults, 1)
    changes.append("Condensed 15 LLM default fns to one-liners")
else:
    errors.append("FATAL: LLM default fns block not found")

# ═══════════════════════════════════════════════════════════════
# CHANGE 2: Type save_io_uring_config with IoUringConfig struct
# ═══════════════════════════════════════════════════════════════

# Find and replace the entire function
old_io_uring = r'''async fn save_io_uring_config(body: web::Json<serde_json::Value>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 1: QUEUE & BUFFERS
    // ═══════════════════════════════════════════════════════════════
    let ring_size = body
        .get("ring_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(256) as u32;
    let cq_size = body.get("cq_size").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let buffer_size = body
        .get("buffer_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(65536) as usize;
    let buffer_pool_size = body
        .get("buffer_pool_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(64) as usize;
    let clamp = body.get("clamp").and_then(|v| v.as_bool()).unwrap_or(false);

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 2: POLLING
    // ═══════════════════════════════════════════════════════════════
    let sqpoll = body
        .get("sqpoll")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let sqpoll_idle_ms = body
        .get("sqpoll_idle_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(1000) as u32;
    let sqpoll_cpu = body
        .get("sqpoll_cpu")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1) as i32;
    let iopoll = body
        .get("iopoll")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 3: OPTIMIZATION
    // ═══════════════════════════════════════════════════════════════
    let single_issuer = body
        .get("single_issuer")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let coop_taskrun = body
        .get("coop_taskrun")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let defer_taskrun = body
        .get("defer_taskrun")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let submit_all = body
        .get("submit_all")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let taskrun_flag = body
        .get("taskrun_flag")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 4: ADVANCED
    // ═══════════════════════════════════════════════════════════════
    let r_disabled = body
        .get("r_disabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let attach_wq_fd = body
        .get("attach_wq_fd")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1) as i32;
    let dontfork = body
        .get("dontfork")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Validate ring_size is power of 2
    if !ring_size.is_power_of_two() || ring_size < 1 || ring_size > 32768 {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "ring_size must be a power of 2 between 1 and 32768",
            "request_id": request_id
        })));
    }

    // Validate buffer_size
    if buffer_size < 4096 || buffer_size > 16 * 1024 * 1024 {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": "buffer_size must be between 4096 and 16MB",
            "request_id": request_id
        })));
    }

    // Build env content with all parameters
    let env_content = format!(
        "# io_uring Configuration (saved by UI)\n\
         \n\
         # Category 1: Queue & Buffers\n\
         IO_URING_RING_SIZE={}\n\
         IO_URING_CQ_SIZE={}\n\
         IO_URING_BUFFER_SIZE={}\n\
         IO_URING_BUFFER_POOL_SIZE={}\n\
         IO_URING_CLAMP={}\n\
         \n\
         # Category 2: Polling\n\
         IO_URING_SQPOLL={}\n\
         IO_URING_SQPOLL_IDLE_MS={}\n\
         IO_URING_SQPOLL_CPU={}\n\
         IO_URING_IOPOLL={}\n\
         \n\
         # Category 3: Optimization\n\
         IO_URING_SINGLE_ISSUER={}\n\
         IO_URING_COOP_TASKRUN={}\n\
         IO_URING_DEFER_TASKRUN={}\n\
         IO_URING_SUBMIT_ALL={}\n\
         IO_URING_TASKRUN_FLAG={}\n\
         \n\
         # Category 4: Advanced\n\
         IO_URING_R_DISABLED={}\n\
         IO_URING_ATTACH_WQ_FD={}\n\
         IO_URING_DONTFORK={}\n",
        ring_size,
        cq_size,
        buffer_size,
        buffer_pool_size,
        clamp,
        sqpoll,
        sqpoll_idle_ms,
        sqpoll_cpu,
        iopoll,
        single_issuer,
        coop_taskrun,
        defer_taskrun,
        submit_all,
        taskrun_flag,
        r_disabled,
        attach_wq_fd,
        dontfork
    );

    // Save to .env.io_uring file
    let env_path = std::path::Path::new(".env.io_uring");
    match std::fs::write(env_path, &env_content) {
        Ok(_) => {
            info!("Saved io_uring config to .env.io_uring");
            Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "message": "io_uring configuration saved to .env.io_uring",
                "request_id": request_id,
                "config": {
                    "ring_size": ring_size,
                    "cq_size": cq_size,
                    "buffer_size": buffer_size,
                    "buffer_pool_size": buffer_pool_size,
                    "clamp": clamp,
                    "sqpoll": sqpoll,
                    "sqpoll_idle_ms": sqpoll_idle_ms,
                    "sqpoll_cpu": sqpoll_cpu,
                    "iopoll": iopoll,
                    "single_issuer": single_issuer,
                    "coop_taskrun": coop_taskrun,
                    "defer_taskrun": defer_taskrun,
                    "submit_all": submit_all,
                    "taskrun_flag": taskrun_flag,
                    "r_disabled": r_disabled,
                    "attach_wq_fd": attach_wq_fd,
                    "dontfork": dontfork
                },
                "note": "Restart backend to apply changes"
            })))
        }
        Err(e) => {
            error!("Failed to save io_uring config: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save config: {}", e),
                "request_id": request_id
            })))
        }
    }
}'''

new_io_uring = r'''#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(default)]
struct IoUringConfig {
    // Queue & Buffers
    ring_size: u32,
    cq_size: u32,
    buffer_size: usize,
    buffer_pool_size: usize,
    clamp: bool,
    // Polling
    sqpoll: bool,
    sqpoll_idle_ms: u32,
    sqpoll_cpu: i32,
    iopoll: bool,
    // Optimization
    single_issuer: bool,
    coop_taskrun: bool,
    defer_taskrun: bool,
    submit_all: bool,
    taskrun_flag: bool,
    // Advanced
    r_disabled: bool,
    attach_wq_fd: i32,
    dontfork: bool,
}

impl Default for IoUringConfig {
    fn default() -> Self {
        Self {
            ring_size: 256, cq_size: 0, buffer_size: 65536, buffer_pool_size: 64, clamp: false,
            sqpoll: false, sqpoll_idle_ms: 1000, sqpoll_cpu: -1, iopoll: false,
            single_issuer: true, coop_taskrun: false, defer_taskrun: false,
            submit_all: false, taskrun_flag: false,
            r_disabled: false, attach_wq_fd: -1, dontfork: false,
        }
    }
}

impl IoUringConfig {
    fn validate(&self) -> Result<(), String> {
        if !self.ring_size.is_power_of_two() || self.ring_size < 1 || self.ring_size > 32768 {
            return Err("ring_size must be a power of 2 between 1 and 32768".into());
        }
        if self.buffer_size < 4096 || self.buffer_size > 16 * 1024 * 1024 {
            return Err("buffer_size must be between 4096 and 16MB".into());
        }
        Ok(())
    }

    fn to_env_string(&self) -> String {
        format!(
            "# io_uring Configuration (saved by UI)\n\
             \n# Queue & Buffers\n\
             IO_URING_RING_SIZE={}\nIO_URING_CQ_SIZE={}\nIO_URING_BUFFER_SIZE={}\n\
             IO_URING_BUFFER_POOL_SIZE={}\nIO_URING_CLAMP={}\n\
             \n# Polling\n\
             IO_URING_SQPOLL={}\nIO_URING_SQPOLL_IDLE_MS={}\nIO_URING_SQPOLL_CPU={}\n\
             IO_URING_IOPOLL={}\n\
             \n# Optimization\n\
             IO_URING_SINGLE_ISSUER={}\nIO_URING_COOP_TASKRUN={}\nIO_URING_DEFER_TASKRUN={}\n\
             IO_URING_SUBMIT_ALL={}\nIO_URING_TASKRUN_FLAG={}\n\
             \n# Advanced\n\
             IO_URING_R_DISABLED={}\nIO_URING_ATTACH_WQ_FD={}\nIO_URING_DONTFORK={}\n",
            self.ring_size, self.cq_size, self.buffer_size, self.buffer_pool_size, self.clamp,
            self.sqpoll, self.sqpoll_idle_ms, self.sqpoll_cpu, self.iopoll,
            self.single_issuer, self.coop_taskrun, self.defer_taskrun,
            self.submit_all, self.taskrun_flag,
            self.r_disabled, self.attach_wq_fd, self.dontfork,
        )
    }
}

async fn save_io_uring_config(body: web::Json<IoUringConfig>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = body.into_inner();

    if let Err(msg) = config.validate() {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "error", "message": msg, "request_id": request_id
        })));
    }

    let env_path = std::path::Path::new(".env.io_uring");
    match std::fs::write(env_path, config.to_env_string()) {
        Ok(_) => {
            info!("Saved io_uring config to .env.io_uring");
            Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "message": "io_uring configuration saved to .env.io_uring",
                "request_id": request_id,
                "config": config,
                "note": "Restart backend to apply changes"
            })))
        }
        Err(e) => {
            error!("Failed to save io_uring config: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save config: {}", e),
                "request_id": request_id
            })))
        }
    }
}'''

if old_io_uring in content:
    content = content.replace(old_io_uring, new_io_uring, 1)
    changes.append("Replaced save_io_uring_config with typed IoUringConfig struct")
else:
    errors.append("FATAL: save_io_uring_config function not found")

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
