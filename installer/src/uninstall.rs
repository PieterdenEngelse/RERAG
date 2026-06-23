//! Inverse of install_steps. CLI-only path triggered by
//! `ag-installer --uninstall` (or `--uninstall --purge`).
//!
//! Two modes:
//!
//! - **Default** removes ag's binaries, the bundled libtika, the OS-
//!   managed service registrations (systemd units + drop-ins on Linux,
//!   Scheduled Tasks on Windows), and the copied compose file.
//!   **Preserves** `ag.env` (the user's API keys, FalkorDB password)
//!   and `$AG_HOME` (data, indexes, logs, FalkorDB store).
//! - **`--purge`** additionally removes `ag.env` and the entire
//!   `$AG_HOME` tree. Destructive — confirmed via terminal prompt
//!   regardless of how the installer was invoked.
//!
//! Honors `SKIP_SYSTEMCTL=1` (Linux) / `SKIP_SCHTASKS=1` (Windows) the
//! same way install_steps does — the service-management shellouts log
//! what they would run instead of touching real systemd / Task
//! Scheduler. Combined with `HOME=/tmp/ag-test` (Linux) or
//! `AG_HOME=C:\Temp\ag-test` (Windows) this makes uninstall testable
//! in a sandbox without disturbing a real ag deployment.
//!
//! The OS-specific bodies — which units / tasks to stop, which files to
//! remove — live under `crate::platform::{linux,windows}`. This file
//! owns the user-facing intro, the y/N confirm, and the shared cleanup
//! of docker-compose.yml / ag.env / ag_home.

use std::fs;
use std::io::Write as _;
use std::path::Path;

use anyhow::Result;

use crate::paths::Paths;

pub async fn run(purge: bool) -> Result<()> {
    let paths = Paths::resolve();

    println!("RERAG uninstall");
    println!();
    println!("Will remove:");
    for target in crate::platform::uninstall_targets(&paths) {
        println!("  • {}", target.display());
    }
    println!("  • {}", paths.docker_compose().display());
    println!();
    if purge {
        println!("--purge ALSO removes (DESTRUCTIVE):");
        println!(
            "  • {}  (ag.env — API keys, FalkorDB password)",
            paths.ag_env().display()
        );
        println!(
            "  • {}  (data, indexes, logs, FalkorDB store)",
            paths.ag_home.display()
        );
    } else {
        println!("Preserved (re-run with --purge to also remove):");
        println!("  • {}", paths.ag_env().display());
        println!("  • {}", paths.ag_home.display());
    }
    println!();

    // Confirm interactively. `--non-interactive` would be Phase H polish
    // for scripted uninstalls; for now we always ask, since this is the
    // standard expectation for an irreversible action.
    print!("Continue? [y/N] ");
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
        println!("Aborted.");
        return Ok(());
    }
    println!();

    // OS-specific stop + remove: systemd units on Linux, Scheduled Tasks
    // + compose-down on Windows. Plus the binaries / native libs whose
    // filenames differ per platform (ag vs ag.exe, .so vs .dll).
    crate::platform::uninstall_managed(&paths).await;

    // Shared cleanup — same on both platforms.
    rm_quiet(&paths.docker_compose());

    if purge {
        rm_quiet(&paths.ag_env());
        rm_dir_quiet(&paths.ag_home);
    }

    println!();
    println!("Uninstall complete.");
    if !purge {
        println!();
        println!("To also remove your ag.env and data:");
        println!("  ag-installer --uninstall --purge");
    }
    Ok(())
}

// --- helpers --------------------------------------------------------------
//
// `pub(crate)` so `platform::{linux,windows}` can reuse the same
// idempotent rm helpers from their `uninstall_managed` impls.

pub(crate) fn rm_quiet(path: &Path) {
    if !path.exists() {
        return;
    }
    match fs::remove_file(path) {
        Ok(()) => println!("  removed {}", path.display()),
        Err(e) => println!("  ! could not remove {}: {}", path.display(), e),
    }
}

pub(crate) fn rm_dir_quiet(path: &Path) {
    if !path.exists() {
        return;
    }
    match fs::remove_dir_all(path) {
        Ok(()) => println!("  removed {}/", path.display()),
        Err(e) => println!("  ! could not remove {}/: {}", path.display(), e),
    }
}
