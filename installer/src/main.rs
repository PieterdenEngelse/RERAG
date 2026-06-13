//! ag-installer — GUI installer for ag, distributed as an AppImage.
//!
//! Phase A scope (minimum-viable foundation): a Dioxus desktop window that
//! shows "Hello ag installer" + version info. No real install logic yet —
//! the screens get fleshed out in Phase B (mocked data) and Phase C+ (wired
//! to real detection + steps).
//!
//! See `docs/bin3` for the full design and execution plan.

use dioxus::prelude::*;

/// Bake-time constants from build.rs.
const GIT_SHA: &str = env!("AG_INSTALLER_GIT_SHA");
const BUILT_AT: &str = env!("AG_INSTALLER_BUILT_AT");
const RUNNER: &str = env!("AG_INSTALLER_RUNNER");
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    // Honor a CLI-style --version flag without spinning up the GUI. The bin3
    // plan calls for this for two reasons: (a) sanity-check the AppImage from
    // a terminal, and (b) the installer compares its own version against the
    // bundled ag binary's version at launch.
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        print_version();
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("ag-installer {VERSION} (git: {GIT_SHA}, built: {BUILT_AT})");

    let cfg = dioxus::desktop::Config::new()
        .with_window(
            dioxus::desktop::WindowBuilder::new()
                .with_title(format!("ag installer {VERSION}"))
                .with_inner_size(dioxus::desktop::LogicalSize::new(900.0, 650.0))
                .with_resizable(true),
        );
    dioxus::LaunchBuilder::desktop().with_cfg(cfg).launch(app);
}

fn print_version() {
    println!("ag-installer {VERSION}");
    println!("git: {GIT_SHA}");
    println!("built: {BUILT_AT} ({RUNNER})");
}

fn print_help() {
    println!("ag-installer {VERSION} — GUI installer for ag");
    println!();
    println!("Usage: ag-installer [OPTIONS]");
    println!();
    println!("  --version, -V    Print version + git SHA + build timestamp and exit");
    println!("  --help, -h       This help");
    println!();
    println!("Without flags: opens the GUI installer window.");
    println!();
    println!("Design: docs/bin3 in the repo.");
}

#[component]
fn app() -> Element {
    rsx! {
        style { {include_str!("../assets/style.css")} }
        div { class: "container",
            div { class: "card",
                h1 { "ag installer" }
                p { class: "tagline",
                    "Foundation scaffold — Phase A. The six screens land in Phase B."
                }
                div { class: "meta",
                    div { class: "meta-row",
                        span { class: "label", "Version" }
                        span { class: "value", "{VERSION}" }
                    }
                    div { class: "meta-row",
                        span { class: "label", "Git" }
                        span { class: "value", "{GIT_SHA}" }
                    }
                    div { class: "meta-row",
                        span { class: "label", "Built" }
                        span { class: "value", "{BUILT_AT} ({RUNNER})" }
                    }
                }
                p { class: "next",
                    "Next: Phase B scaffolds Welcome → Detection → Prompts → "
                    "Install Progress → First-Run Config → Done."
                }
            }
        }
    }
}
