//! Locate bundled artifacts (the AppImage's payload) so install_steps.rs can
//! copy them into XDG locations.
//!
//! In AppImage mode, the AppRun shim exports `AG_INSTALLER_BUNDLE_ROOT`
//! pointing at `$APPDIR/usr/share/ag/`. From there the bin/ and lib/
//! directories are siblings under `$APPDIR/usr/`.
//!
//! In dev mode (`cargo run -p ag-installer`) `AG_INSTALLER_BUNDLE_ROOT`
//! is unset; we walk up from `current_exe()` to find the repo root and
//! resolve paths against the in-tree layout (target/release/ag,
//! frontend/fro/dist/, systemd/*.tmpl, etc.). This lets D.1's skeleton
//! and D.2's real writes both run with `cargo run` without packaging.
//!
//! Functions return `PathBuf` unconditionally; callers check `exists()`
//! before reading. Missing artifacts (e.g. `target/release/ag` before the
//! backend has been built) are install_steps' problem to surface, not
//! bundled's.

use std::path::PathBuf;

/// Directory containing bundled non-binary artifacts: web/, falkordb/,
/// systemd/, docker-compose.yml, .env.example.
pub fn share_dir() -> PathBuf {
    if let Ok(p) = std::env::var("AG_INSTALLER_BUNDLE_ROOT") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    // Dev fallback: pull from in-tree sources. install_steps treats each
    // sub-path independently, so a partial dev tree (e.g. no FalkorDB
    // staging) just surfaces a missing-file error on the relevant step.
    repo_root()
}

pub fn ag_binary_path() -> PathBuf {
    if let Some(usr) = appimage_usr_dir() {
        return usr.join("bin/ag");
    }
    repo_root().join("target/release/ag")
}

pub fn libtika_path() -> Option<PathBuf> {
    if let Some(usr) = appimage_usr_dir() {
        let p = usr.join("lib/libtika_native.so");
        return p.exists().then_some(p);
    }
    find_dev_libtika()
}

pub fn frontend_dist_dir() -> PathBuf {
    if appimage_usr_dir().is_some() {
        share_dir().join("web")
    } else {
        repo_root().join("frontend/fro/dist")
    }
}

pub fn falkordb_stage_dir() -> PathBuf {
    if appimage_usr_dir().is_some() {
        share_dir().join("falkordb")
    } else {
        repo_root().join("installer/stage/falkordb")
    }
}

pub fn docker_compose_path() -> PathBuf {
    if appimage_usr_dir().is_some() {
        share_dir().join("docker-compose.yml")
    } else {
        repo_root().join("docker-compose.yml")
    }
}

pub fn env_example_path() -> PathBuf {
    if appimage_usr_dir().is_some() {
        share_dir().join(".env.example")
    } else {
        repo_root().join(".env.example")
    }
}

pub fn systemd_template_dir() -> PathBuf {
    if appimage_usr_dir().is_some() {
        share_dir().join("systemd")
    } else {
        repo_root().join("systemd")
    }
}

// --- helpers --------------------------------------------------------------

fn appimage_usr_dir() -> Option<PathBuf> {
    let bundle_root = std::env::var("AG_INSTALLER_BUNDLE_ROOT").ok()?;
    if bundle_root.is_empty() {
        return None;
    }
    // bundle_root = $APPDIR/usr/share/ag → grandparent = $APPDIR/usr.
    let p = PathBuf::from(bundle_root);
    p.parent()?.parent().map(PathBuf::from)
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
    // libtika is built as a side effect of `cargo build -p ag`. Its location
    // is target/release/build/extractous-<hash>/out/libs/libtika_native.so.
    // We pick the newest matching dir to avoid stale artifacts from old
    // checkouts.
    let build_dir = repo_root().join("target/release/build");
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(entries) = std::fs::read_dir(&build_dir) {
        for entry in entries.flatten() {
            if !entry.file_name().to_string_lossy().starts_with("extractous-") {
                continue;
            }
            let candidate = entry.path().join("out/libs/libtika_native.so");
            if !candidate.exists() {
                continue;
            }
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            if newest.as_ref().map_or(true, |(t, _)| mtime > *t) {
                newest = Some((mtime, candidate));
            }
        }
    }
    newest.map(|(_, p)| p)
}
