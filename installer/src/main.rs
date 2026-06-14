//! ag-installer — GUI installer for ag, distributed as an AppImage.
//!
//! Phase B scope: all six screens render with mocked data; Back/Next
//! navigation between them works; brand colors match the dashboard.
//! Real detection / install / first-run config land in Phases C, D, E.
//!
//! See `docs/bin3` for the full design and execution plan.

#![allow(non_snake_case)]

mod app;
mod bundled;
mod detection;
mod install_steps;
mod paths;
mod prompts;
mod ui;

use dioxus::prelude::*;

use app::{use_screen, Screen};
use detection::DetectionResult;
use prompts::PromptAnswers;
use ui::{DetectionScreen, FirstRunForm, ProgressScreen, PromptsScreen, SummaryScreen, Welcome};

/// Bake-time constants from build.rs.
const GIT_SHA: &str = env!("AG_INSTALLER_GIT_SHA");
const BUILT_AT: &str = env!("AG_INSTALLER_BUILT_AT");
const RUNNER: &str = env!("AG_INSTALLER_RUNNER");
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    // CLI flags short-circuit before Dioxus boots.
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

    let cfg = dioxus::desktop::Config::new().with_window(
        dioxus::desktop::WindowBuilder::new()
            .with_title(format!("ag installer {VERSION}"))
            // 1400x900 is the fallback size if the WM rejects maximize.
            // Min 1100x700 keeps the Detection two-column layout and the
            // 2x2 prompts grid from collapsing onto one column.
            .with_inner_size(dioxus::desktop::LogicalSize::new(1400.0, 900.0))
            .with_min_inner_size(dioxus::desktop::LogicalSize::new(1100.0, 700.0))
            .with_maximized(true)
            .with_resizable(true),
    );
    dioxus::LaunchBuilder::desktop().with_cfg(cfg).launch(App);
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
fn App() -> Element {
    // Top-level screen signal. Every component can navigate by mutating it.
    use_context_provider(|| Signal::new(Screen::Welcome));
    // Detection result: None until probes complete on the Detection screen;
    // Prompts screen reads from this to decide which forms to show.
    use_context_provider(|| Signal::new(Option::<DetectionResult>::None));
    // Prompt answers: filled in as the user submits each form on the Prompts
    // screen. Phase D's installer reads from this.
    use_context_provider(|| Signal::new(PromptAnswers::default()));
    let screen = use_screen();
    let current = *screen.read();

    rsx! {
        style { {include_str!("../assets/style.css")} }
        div { class: "app",
            ProgressBar { current: current }
            div { class: "screen-host",
                match current {
                    Screen::Welcome => rsx! { Welcome {} },
                    Screen::Detection => rsx! { DetectionScreen {} },
                    Screen::Prompts => rsx! { PromptsScreen {} },
                    Screen::Progress => rsx! { ProgressScreen {} },
                    Screen::FirstRun => rsx! { FirstRunForm {} },
                    Screen::Done => rsx! { SummaryScreen {} },
                }
            }
            FooterMeta {}
        }
    }
}

/// Top-of-window progress bar showing 6 dots — one per screen.
#[component]
fn ProgressBar(current: Screen) -> Element {
    let screens = [
        Screen::Welcome,
        Screen::Detection,
        Screen::Prompts,
        Screen::Progress,
        Screen::FirstRun,
        Screen::Done,
    ];
    rsx! {
        div { class: "progress-bar",
            for (i, s) in screens.iter().enumerate() {
                {
                    let class_state = if *s == current {
                        "progress-dot progress-dot-active"
                    } else if (*s as usize) < (current as usize) {
                        "progress-dot progress-dot-done"
                    } else {
                        "progress-dot"
                    };
                    rsx! {
                        div { key: "{i}", class: "{class_state}",
                            span { class: "progress-dot-number", "{s.step_number()}" }
                            span { class: "progress-dot-label", "{s.title()}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FooterMeta() -> Element {
    rsx! {
        div { class: "footer-meta",
            span { "ag-installer " span { class: "footer-version", "{VERSION}" } }
            span { class: "footer-sep", "·" }
            span { class: "footer-dim", "git {GIT_SHA}" }
        }
    }
}
