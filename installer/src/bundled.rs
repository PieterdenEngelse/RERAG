//! Locate bundled artifacts so install_steps can copy them into the
//! per-user install tree.
//!
//! Two packaged-install modes:
//!
//! - **Linux AppImage**: AppRun shim exports `AG_INSTALLER_BUNDLE_ROOT`
//!   pointing at `$APPDIR/usr/share/ag/`. From there `bin/` and `lib/`
//!   are siblings under `$APPDIR/usr/`.
//! - **Windows MSI**: cargo-wix lays the payload under
//!   `%PROGRAMFILES%\ag\`. We resolve relative to `current_exe()` —
//!   `%PROGRAMFILES%\ag\bin\ag-installer.exe` → walk up two parents →
//!   `%PROGRAMFILES%\ag` → join `share\ag`. Same on-disk shape as the
//!   AppImage's `$APPDIR/usr/share/ag/`.
//!
//! In dev mode (`cargo run -p ag-installer`) neither signal is set; we
//! walk up from `current_exe()` to find the repo root and resolve
//! paths against the in-tree layout (`target/release/`, `frontend/fro/
//! dist/`, `systemd/*.tmpl`, `installer/scheduled-tasks/*.tmpl`).
//!
//! Functions return `PathBuf` unconditionally; callers check `exists()`
//! before reading. Missing artifacts (e.g. `target/release/ag` before
//! the backend has been built) are install_steps' problem to surface,
//! not bundled's.

use std::path::PathBuf;

#[cfg(unix)]
const AG_BIN_NAME: &str = "ag";
#[cfg(windows)]
const AG_BIN_NAME: &str = "ag.exe";

#[cfg(unix)]
const LIBTIKA_NAME: &str = "libtika_native.so";
#[cfg(windows)]
const LIBTIKA_NAME: &str = "tika_native.dll";

/// Directory containing bundled non-binary artifacts: `web/`,
/// `docker-compose.yml`, `.env.example`, plus platform-specific service
/// templates (`systemd/` on Linux, `scheduled-tasks/` on Windows). In
/// packaged mode this is the bundle's `share/ag/` payload root; in dev
/// mode it falls back to the repo root so each in-tree path resolver
/// can pluck its own sub-path.
pub fn share_dir() -> PathBuf {
    bundle_share_dir().unwrap_or_else(repo_root)
}

pub fn ag_binary_path() -> PathBuf {
    if let Some(root) = bundle_install_root() {
        return root.join("bin").join(AG_BIN_NAME);
    }
    let root = repo_root();
    // In dev mode the installer may be built with --target <triple>, placing
    // the ag binary under target/<triple>/release/ rather than target/release/.
    // Scan one level deep so sandbox runs work without a separate host build.
    if let Ok(entries) = std::fs::read_dir(root.join("target")) {
        for entry in entries.flatten() {
            let candidate = entry.path().join("release").join(AG_BIN_NAME);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    root.join("target/release").join(AG_BIN_NAME)
}

pub fn libtika_path() -> Option<PathBuf> {
    if let Some(root) = bundle_install_root() {
        let p = root.join("lib").join(LIBTIKA_NAME);
        return p.exists().then_some(p);
    }
    find_dev_libtika()
}

pub fn frontend_dist_dir() -> PathBuf {
    if bundle_share_dir().is_some() {
        share_dir().join("web")
    } else {
        repo_root().join("frontend/fro/dist")
    }
}

#[cfg(unix)]
pub fn falkordb_stage_dir() -> PathBuf {
    if bundle_share_dir().is_some() {
        share_dir().join("falkordb")
    } else {
        repo_root().join("installer/stage/falkordb")
    }
}

pub fn docker_compose_path() -> PathBuf {
    if bundle_share_dir().is_some() {
        share_dir().join("docker-compose.yml")
    } else {
        repo_root().join("docker-compose.yml")
    }
}

pub fn env_example_path() -> PathBuf {
    if bundle_share_dir().is_some() {
        share_dir().join(".env.example")
    } else {
        repo_root().join(".env.example")
    }
}

#[cfg(unix)]
pub fn systemd_template_dir() -> PathBuf {
    if bundle_share_dir().is_some() {
        share_dir().join("systemd")
    } else {
        repo_root().join("systemd")
    }
}

/// Windows analog of `systemd_template_dir`. Holds `ag.xml.tmpl` and
/// `ag-stack.xml.tmpl` — the Scheduled-Task XML templates rendered by
/// `platform::windows::install_service`.
#[cfg(windows)]
pub fn scheduled_tasks_template_dir() -> PathBuf {
    if bundle_share_dir().is_some() {
        share_dir().join("scheduled-tasks")
    } else {
        repo_root().join("installer/scheduled-tasks")
    }
}

// --- helpers --------------------------------------------------------------

/// `Some(share/ag/)` when running from a packaged install (AppImage or
/// MSI); `None` in dev mode. The Linux AppImage explicitly exports
/// `AG_INSTALLER_BUNDLE_ROOT`; the Windows MSI relies on a fixed layout
/// relative to `current_exe()` instead.
fn bundle_share_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("AG_INSTALLER_BUNDLE_ROOT") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    #[cfg(windows)]
    {
        // %PROGRAMFILES%\ag\bin\ag-installer.exe → %PROGRAMFILES%\ag → \share\ag
        let exe = std::env::current_exe().ok()?;
        let share_ag = exe.parent()?.parent()?.join("share").join("ag");
        if share_ag.exists() {
            return Some(share_ag);
        }
    }
    None
}

/// `Some(<install_root>)` — the directory that holds `bin/`, `lib/`,
/// and `share/ag/` siblings — when running from a packaged install.
/// `None` in dev mode. Used to find the installed `ag` binary and
/// `libtika`/`tika_native` library next to the bundle.
fn bundle_install_root() -> Option<PathBuf> {
    let share = bundle_share_dir()?;
    // share = .../share/ag → parent.parent = the install root
    // (`$APPDIR/usr` on Linux AppImage, `%PROGRAMFILES%\ag` on Windows MSI).
    share.parent()?.parent().map(PathBuf::from)
}

fn repo_root() -> PathBuf {
    // current_exe is typically <repo>/target/{debug,release}/ag-installer.
    // Walk up looking for a workspace Cargo.toml with backend/ as a sibling.
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = exe.as_path();
    for _ in 0..8 {
        if dir.join("Cargo.toml").exists() && dir.join("backend").is_dir() {
            return dir.to_path_buf();
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    // Last resort: CWD. install_steps will surface missing files clearly.
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn find_dev_libtika() -> Option<PathBuf> {
    // libtika / tika_native is built as a side effect of `cargo build -p ag`.
    // Its location is target/release/build/extractous-<hash>/out/libs/<name>.
    // We pick the newest matching dir to avoid stale artifacts from old
    // checkouts.
    let build_dir = repo_root().join("target/release/build");
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(entries) = std::fs::read_dir(&build_dir) {
        for entry in entries.flatten() {
            if !entry
                .file_name()
                .to_string_lossy()
                .starts_with("extractous-")
            {
                continue;
            }
            let candidate = entry.path().join("out/libs").join(LIBTIKA_NAME);
            if !candidate.exists() {
                continue;
            }
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            if newest.as_ref().is_none_or(|(t, _)| mtime > *t) {
                newest = Some((mtime, candidate));
            }
        }
    }
    newest.map(|(_, p)| p)
}
