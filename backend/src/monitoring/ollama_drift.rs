// Detect drift between `hardware_config.num_thread` and the Ollama runner
// process that's currently serving requests. Ollama bakes num_thread (and
// other model-load options) into the runner at the moment it loads the
// model and ignores subsequent config changes until that runner exits and
// a fresh one starts.
//
// This Ollama version doesn't expose --threads on the runner's argv, so we
// can't read the live value from /proc/<pid>/cmdline directly. Instead we
// watch the runner PID: when it changes, we know a new runner was spawned
// and is using the currently-configured num_thread, so the drift flag can
// be cleared. PID-change is a reliable proxy for "config was re-applied".

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

static NEEDS_RESTART: AtomicBool = AtomicBool::new(false);
static CONFIGURED_THREADS: AtomicUsize = AtomicUsize::new(0);
static LAST_RUNNER_PID: AtomicU32 = AtomicU32::new(0);

#[derive(serde::Serialize)]
pub struct DriftSnapshot {
    pub drift: bool,
    pub configured: usize,
    /// PID of the currently-observed `ollama runner` process, or `None`
    /// when no model is loaded. Surfaced for debugging; the frontend
    /// banner only keys on `drift`.
    pub runner_pid: Option<u32>,
}

/// Called from the hardware-config save handler when `num_thread` changes
/// AND the backend is Ollama. Sets the banner-driving flag.
pub fn mark_config_change(configured: usize) {
    CONFIGURED_THREADS.store(configured, Ordering::Relaxed);
    NEEDS_RESTART.store(true, Ordering::Relaxed);
    tracing::info!(
        configured_threads = configured,
        "ollama_drift: config change recorded — banner will show until runner reloads"
    );
}

pub fn snapshot() -> DriftSnapshot {
    let pid = LAST_RUNNER_PID.load(Ordering::Relaxed);
    DriftSnapshot {
        drift: NEEDS_RESTART.load(Ordering::Relaxed),
        configured: CONFIGURED_THREADS.load(Ordering::Relaxed),
        runner_pid: if pid == 0 { None } else { Some(pid) },
    }
}

/// Walk /proc, find the process whose argv is `ollama runner ...` and
/// return its PID. The check is anchored on argv[0] + argv[1] so it can't
/// be fooled by another process whose command line happens to contain the
/// substring "ollama runner" (e.g. a debug shell, this very poller's own
/// future invocation, etc).
fn detect_runner_pid() -> Option<u32> {
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let name_os = entry.file_name();
        let name = name_os.to_string_lossy();
        let pid: u32 = match name.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let cmdline = match std::fs::read(entry.path().join("cmdline")) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let args: Vec<String> = cmdline
            .split(|&b| b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect();
        let argv0_is_ollama = args
            .first()
            .map(|a| a.ends_with("/ollama") || a == "ollama")
            .unwrap_or(false);
        let argv1_is_runner = args.get(1).map(|s| s == "runner").unwrap_or(false);
        if argv0_is_ollama && argv1_is_runner {
            return Some(pid);
        }
    }
    None
}

/// Periodic poll: tracks the live runner PID. The flag is cleared
/// whenever the observed PID changes after the flag was set, because the
/// flag can only be set via `mark_config_change`, and any runner that
/// spawns AFTER that call necessarily uses the currently-configured
/// `num_thread`.
///
/// This includes the 0→Some transition (no runner before, runner now):
/// that fresh runner is the post-save load, so the flag should clear.
/// It does NOT include the very first observation at ag startup when a
/// runner was already there and the flag hasn't been set — without a save
/// event the flag is false anyway, so the guard is implicit.
pub fn poll_once() {
    let current = detect_runner_pid();
    let last = LAST_RUNNER_PID.load(Ordering::Relaxed);
    match current {
        Some(pid) => {
            if pid != last && NEEDS_RESTART.load(Ordering::Relaxed) {
                NEEDS_RESTART.store(false, Ordering::Relaxed);
                tracing::info!(
                    old_runner_pid = last,
                    new_runner_pid = pid,
                    configured_threads = CONFIGURED_THREADS.load(Ordering::Relaxed),
                    "ollama_drift: runner PID changed — config re-applied, banner cleared"
                );
            }
            LAST_RUNNER_PID.store(pid, Ordering::Relaxed);
        }
        None => {
            // Runner gone (model unloaded). Keep the flag and the last
            // PID — next runner's appearance with a different PID is what
            // clears the flag.
        }
    }
}
