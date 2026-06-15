//! Inverse of install_steps. CLI-only path triggered by
//! `ag-installer --uninstall` (or `--uninstall --purge`).
//!
//! Two modes:
//!
//! - **Default** removes ag's binaries, the bundled libtika, the three
//!   rendered systemd units + drop-ins, and the copied compose file.
//!   **Preserves** `ag.env` (the user's API keys, FalkorDB password)
//!   and `$AG_HOME` (data, indexes, logs, FalkorDB store).
//! - **`--purge`** additionally removes `ag.env` and the entire
//!   `$AG_HOME` tree. Destructive — confirmed via terminal prompt
//!   regardless of how the installer was invoked.
//!
//! Honors `SKIP_SYSTEMCTL=1` the same way install_steps does — the
//! systemctl calls log what they would run instead of touching real
//! systemd. Combined with `HOME=/tmp/ag-test` this makes uninstall
//! testable in a sandbox without disturbing a real ag deployment.

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::Stdio;

use anyhow::Result;
use tokio::process::Command;

use crate::paths::{self, Paths};

pub async fn run(purge: bool) -> Result<()> {
    let paths = Paths::resolve();

    println!("RERAG uninstall");
    println!();
    println!("Will remove:");
    println!("  • {}", paths.bin_dir.join("ag").display());
    println!("  • {}", paths.lib_dir.join("libtika_native.so").display());
    println!("  • {}", paths.ag_service().display());
    println!("  • {}", paths.ag_stack_service().display());
    println!("  • {}", paths.falkordb_service().display());
    println!("  • {}", paths.ag_service_drop_in_dir().display());
    println!("  • {}", paths.docker_compose().display());
    println!();
    if purge {
        println!("--purge ALSO removes (DESTRUCTIVE):");
        println!("  • {}  (ag.env — API keys, FalkorDB password)", paths.ag_env().display());
        println!("  • {}  (data, indexes, logs, FalkorDB store)", paths.ag_home.display());
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

    // 1. Stop + disable services. Best-effort — a stop on a service
    //    that isn't running, or a disable on one that isn't enabled,
    //    isn't an error worth bailing on.
    for unit in ["ag.service", "ag-stack.service", "falkordb.service"] {
        systemctl_user(&["stop", unit]).await;
        systemctl_user(&["disable", unit]).await;
    }
    systemctl_user(&["daemon-reload"]).await;

    // 2. Remove rendered unit files + the drop-in dir.
    rm_quiet(&paths.ag_service());
    rm_quiet(&paths.ag_stack_service());
    rm_quiet(&paths.falkordb_service());
    rm_dir_quiet(&paths.ag_service_drop_in_dir());

    // 3. Binaries + bundled libs.
    rm_quiet(&paths.bin_dir.join("ag"));
    rm_quiet(&paths.lib_dir.join("libtika_native.so"));

    // 4. Rendered config (compose file). ag.env preserved unless --purge.
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

fn rm_quiet(path: &Path) {
    if !path.exists() {
        return;
    }
    match fs::remove_file(path) {
        Ok(()) => println!("  removed {}", path.display()),
        Err(e) => println!("  ! could not remove {}: {}", path.display(), e),
    }
}

fn rm_dir_quiet(path: &Path) {
    if !path.exists() {
        return;
    }
    match fs::remove_dir_all(path) {
        Ok(()) => println!("  removed {}/", path.display()),
        Err(e) => println!("  ! could not remove {}/: {}", path.display(), e),
    }
}

async fn systemctl_user(args: &[&str]) {
    let pretty = format!("systemctl --user {}", args.join(" "));
    if paths::skip_systemctl() {
        println!("  SKIP_SYSTEMCTL=1 — would run: {pretty}");
        return;
    }
    let result = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    match result {
        Ok(status) if status.success() => println!("  {pretty}"),
        Ok(_) => {
            // Common case: stop/disable on a unit that isn't there.
            // Don't surface as an error — uninstall is idempotent.
        }
        Err(e) => println!("  ! {pretty} — spawn failed: {e}"),
    }
}
