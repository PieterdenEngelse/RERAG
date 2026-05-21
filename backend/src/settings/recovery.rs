//! Boot-failure recovery for runtime overrides.
//!
//! Before applying overrides, ag writes `overrides.boot.marker`. The marker
//! is cleared after the first non-/healthz request returns 2xx (or after a
//! configurable uptime). If the marker still exists at the next startup the
//! previous boot crashed before reaching healthy — `overrides.json` is moved
//! aside as `overrides.json.bad-<timestamp>` and ag boots with no overrides.

use parking_lot::RwLock;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize)]
pub struct RollbackInfo {
    pub rolled_back_at: String,
    pub last_bad_file: String,
}

pub struct Recovery {
    marker: PathBuf,
    pub healthy: AtomicBool,
    pub last_rollback: RwLock<Option<RollbackInfo>>,
}

impl Recovery {
    /// Run the boot check before `Settings::load`. Returns the path that
    /// `Settings::load` should consume (unchanged in the happy path; the
    /// rolled-back file is renamed aside).
    pub fn boot_check(base_dir: &Path, overrides_path: &Path) -> (PathBuf, Self) {
        let marker = base_dir.join("overrides.boot.marker");
        let last_rollback = RwLock::new(None);

        if marker.exists() && overrides_path.exists() {
            let ts = timestamp_now();
            let mut bad = overrides_path.to_path_buf();
            let bad_name = format!(
                "{}.bad-{ts}",
                overrides_path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "overrides.json".to_string())
            );
            bad.set_file_name(bad_name);
            match std::fs::rename(overrides_path, &bad) {
                Ok(()) => {
                    warn!(
                        "settings: previous boot did not reach healthy — rolled back overrides to {}",
                        bad.display()
                    );
                    *last_rollback.write() = Some(RollbackInfo {
                        rolled_back_at: ts,
                        last_bad_file: bad.display().to_string(),
                    });
                }
                Err(e) => {
                    warn!("settings: rollback rename failed ({e}); leaving overrides in place")
                }
            }
        }

        // Create the marker for this boot (best-effort).
        if let Err(e) = std::fs::write(&marker, b"") {
            warn!(
                "settings: could not create boot marker {}: {e}",
                marker.display()
            );
        } else {
            info!("settings: boot marker created at {}", marker.display());
        }

        (
            overrides_path.to_path_buf(),
            Self {
                marker,
                healthy: AtomicBool::new(false),
                last_rollback,
            },
        )
    }

    /// Idempotent: mark this boot "known good" and clear the marker. Safe to
    /// call from any handler; the swap ensures the file op runs only once.
    pub fn mark_healthy(&self) {
        if self.healthy.swap(true, Ordering::SeqCst) {
            return;
        }
        match std::fs::remove_file(&self.marker) {
            Ok(()) => info!("settings: boot marker cleared — overrides committed"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => warn!("settings: failed to clear boot marker: {e}"),
        }
    }
}

fn timestamp_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Compact UTC-ish stamp; not a full RFC3339 to avoid pulling in chrono.
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let h = rem / 3_600;
    let m = (rem % 3_600) / 60;
    let s = rem % 60;
    format!("{days}d{h:02}{m:02}{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn paths(td: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
        (
            td.path().to_path_buf(),
            td.path().join("overrides.json"),
        )
    }

    #[test]
    fn clean_boot_creates_marker_and_keeps_overrides() {
        let td = TempDir::new().unwrap();
        let (base, overrides) = paths(&td);
        std::fs::write(&overrides, r#"{"FOO":"bar"}"#).unwrap();

        let (returned_path, recovery) = Recovery::boot_check(&base, &overrides);
        assert_eq!(returned_path, overrides);
        assert!(base.join("overrides.boot.marker").exists());
        assert!(overrides.exists(), "overrides should not have been moved");
        assert!(recovery.last_rollback.read().is_none());
    }

    #[test]
    fn surviving_marker_triggers_rollback() {
        let td = TempDir::new().unwrap();
        let (base, overrides) = paths(&td);
        let marker = base.join("overrides.boot.marker");

        // Simulate: previous boot wrote a marker, wrote overrides, then
        // crashed before mark_healthy.
        std::fs::write(&marker, b"").unwrap();
        std::fs::write(&overrides, r#"{"BAD":"setting"}"#).unwrap();

        let (returned_path, recovery) = Recovery::boot_check(&base, &overrides);
        assert_eq!(returned_path, overrides);
        assert!(!overrides.exists(), "overrides should have been moved aside");

        let rollback = recovery
            .last_rollback
            .read()
            .clone()
            .expect("rollback should be recorded");
        assert!(rollback.last_bad_file.contains("overrides.json.bad-"));

        // The .bad-<ts> file actually exists on disk.
        let bad_path = std::path::Path::new(&rollback.last_bad_file);
        assert!(bad_path.exists(), "bad file: {}", rollback.last_bad_file);
        let preserved = std::fs::read_to_string(bad_path).unwrap();
        assert!(preserved.contains("BAD"));

        // A fresh marker was written for this new boot.
        assert!(marker.exists());
    }

    #[test]
    fn mark_healthy_clears_marker_and_is_idempotent() {
        let td = TempDir::new().unwrap();
        let (base, overrides) = paths(&td);
        let (_, recovery) = Recovery::boot_check(&base, &overrides);
        let marker = base.join("overrides.boot.marker");
        assert!(marker.exists());

        recovery.mark_healthy();
        assert!(!marker.exists());

        // Second call is a no-op (no panic, no error).
        recovery.mark_healthy();
        assert!(!marker.exists());
    }

    #[test]
    fn marker_without_overrides_is_treated_as_clean() {
        // Edge case: marker present but no overrides.json — nothing to roll
        // back, just proceed.
        let td = TempDir::new().unwrap();
        let (base, overrides) = paths(&td);
        std::fs::write(base.join("overrides.boot.marker"), b"").unwrap();

        let (_, recovery) = Recovery::boot_check(&base, &overrides);
        assert!(recovery.last_rollback.read().is_none());
    }
}
