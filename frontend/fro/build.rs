//! Build script — keeps `assets/styling/output.css` in sync with the source
//! Tailwind classes referenced from Rust files. Without this, adding a new
//! Tailwind class to a component compiles fine but renders unstyled until
//! someone remembers to run `npm run css:build` manually.
//!
//! Behaviour:
//!   * Re-runs only when files Tailwind actually scans have changed.
//!   * Skips silently (with a `cargo:warning=`) when npm is missing, so CI
//!     / Docker images without Node still build.
//!   * On a fresh checkout where `node_modules/` doesn't exist, warns rather
//!     than crashing — the dev does one `npm install` and the next build
//!     picks it up.
//!
//! If you need to bypass this for a tight inner loop (e.g. running
//! `npm run css:watch` in another terminal), set `SKIP_TAILWIND_BUILD=1`.

use std::path::Path;
use std::process::Command;

fn main() {
    // Tailwind scans Rust source files for class string literals + reads its
    // own config + the source CSS. Cargo recurses into directories listed
    // here, so one `src` line covers every .rs file under it.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=assets/styling/index.css");
    println!("cargo:rerun-if-changed=tailwind.config.js");
    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SKIP_TAILWIND_BUILD");

    if std::env::var("SKIP_TAILWIND_BUILD").is_ok() {
        return;
    }

    if Command::new("npm").arg("--version").output().is_err() {
        println!(
            "cargo:warning=npm not found — Tailwind output.css will be stale until you run `npm run css:build`"
        );
        return;
    }

    if !Path::new("node_modules").exists() {
        println!(
            "cargo:warning=node_modules missing — run `cd frontend/fro && npm install` once before building (Tailwind CSS rebuild skipped)"
        );
        return;
    }

    match Command::new("npm")
        .args(["run", "--silent", "css:build"])
        .status()
    {
        Ok(s) if s.success() => {}
        Ok(s) => println!("cargo:warning=Tailwind CSS build failed (exit {s})"),
        Err(e) => println!("cargo:warning=Failed to run `npm run css:build`: {e}"),
    }
}
