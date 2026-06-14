// build.rs — stamps git SHA and build timestamp into the binary.
// Phase A: implemented here for the installer; the bin3 plan calls for the same
// stamping in backend/build.rs once we want skip-rebuild-on-SHA-match in the
// terminal installer (Phase 2 deferral from bin2).

use std::process::Command;

fn main() {
    let git_sha = Command::new("git")
        .args(["rev-parse", "--short=10", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let built_at = chrono::Utc::now().to_rfc3339();
    let runner = std::env::var("RUNNER_OS")
        .or_else(|_| std::env::var("RUNNER_IMAGE_OS"))
        .unwrap_or_else(|_| "local".to_string());

    println!("cargo:rustc-env=AG_INSTALLER_GIT_SHA={git_sha}");
    println!("cargo:rustc-env=AG_INSTALLER_BUILT_AT={built_at}");
    println!("cargo:rustc-env=AG_INSTALLER_RUNNER={runner}");

    // Re-run if HEAD changes (so SHA stamps stay fresh during dev).
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=build.rs");
}
