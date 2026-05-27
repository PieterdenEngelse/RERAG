//! Runtime settings store + boot-failure recovery + known-keys registry.
//!
//! See `persisentenc.md` for the full design. The summary:
//!   - Layer 1 (this module): file-backed override store with effective-value
//!     lookup, change subscriptions, and atomic persistence.
//!   - The registry (`registry.rs`) declares known keys for UI discoverability.
//!   - The recovery layer (`recovery.rs`) implements last-known-good rollback
//!     so a bad override cannot brick startup in a binary deployment.
//!
//! `install_global` wires a process-wide instance so callers (`api::*`,
//! `config::*`, subsystems) can reach the store without threading a handle
//! through every signature.

pub mod kind;
pub mod recovery;
pub mod registry;
pub mod store;

pub use kind::Kind;
pub use recovery::{Recovery, RollbackInfo};
pub use registry::{lookup, KnownKey, KNOWN_KEYS};
pub use store::{SettingEntry, Settings, SettingsSnapshot, Source};

use std::sync::Arc;
use std::sync::OnceLock;

static GLOBAL_SETTINGS: OnceLock<Arc<Settings>> = OnceLock::new();
static GLOBAL_RECOVERY: OnceLock<Arc<Recovery>> = OnceLock::new();

/// Install the process-wide settings + recovery handles. Idempotent —
/// subsequent calls are no-ops (first writer wins).
pub fn install_global(settings: Arc<Settings>, recovery: Arc<Recovery>) {
    let _ = GLOBAL_SETTINGS.set(settings);
    let _ = GLOBAL_RECOVERY.set(recovery);
}

/// Fetch the global settings store, if installed.
pub fn global() -> Option<Arc<Settings>> {
    GLOBAL_SETTINGS.get().cloned()
}

/// Convenience: effective string with default. Falls back to env::var and
/// then to `default` if no global is installed yet (early startup).
pub fn effective_or(key: &str, default: &str) -> String {
    if let Some(s) = GLOBAL_SETTINGS.get() {
        s.effective_or(key, default)
    } else {
        std::env::var(key).unwrap_or_else(|_| default.to_string())
    }
}

pub fn effective_bool(key: &str, default: bool) -> bool {
    if let Some(s) = GLOBAL_SETTINGS.get() {
        s.effective_bool(key, default)
    } else {
        match std::env::var(key) {
            Ok(v) => {
                let v = v.trim().to_lowercase();
                matches!(v.as_str(), "true" | "1" | "yes" | "on")
            }
            Err(_) => default,
        }
    }
}

pub fn effective_u64(key: &str, default: u64) -> u64 {
    if let Some(s) = GLOBAL_SETTINGS.get() {
        s.effective_u64(key, default)
    } else {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(default)
    }
}

pub fn effective_f64(key: &str, default: f64) -> f64 {
    if let Some(s) = GLOBAL_SETTINGS.get() {
        s.effective_f64(key, default)
    } else {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(default)
    }
}

/// Most recent rollback (if `Recovery::boot_check` moved a bad overrides
/// file aside on this boot).
pub fn last_rollback() -> Option<RollbackInfo> {
    GLOBAL_RECOVERY
        .get()
        .and_then(|r| r.last_rollback.read().clone())
}

/// Mark the current boot "known good" — clears the marker so the next boot
/// trusts overrides.json. Safe to call from many handlers.
pub fn mark_healthy() {
    if let Some(r) = GLOBAL_RECOVERY.get() {
        r.mark_healthy();
    }
}
