//! Screen 4 — Install Progress.
//!
//! Two-pane layout: step list on the left, scrolling log view on the right.
//! On mount, spawns an install task that emits `ProgressEvent`s; this
//! component receives them via mpsc and updates the step-list / log-view
//! signals in real time. A failure modal opens on any step error.
//!
//! Phase B's static mock data is gone. **D.1 scope:** the install task is
//! `install_steps::run` which only *describes* what it would do — no
//! writes. D.2 swaps the bodies for real filesystem / systemctl work.
//!
//! Continue button is disabled until the install task emits
//! `InstallComplete`. No Back button — once writes start in D.2 there's
//! nothing useful to go back to.

use dioxus::prelude::*;
use tokio::sync::mpsc::unbounded_channel;

use crate::app::{InstallStep, StepStatus};
use crate::install_steps::{
    self, ProgressEvent, INSTALL_DOCKER_STEP_NAME, INSTALL_WSL2_DOCKER_STEP_NAME,
    INSTALL_WSL2_ENABLE_STEP_NAME, STEP_NAMES,
};
use crate::prompts::PromptAnswers;
use crate::ui::components::{FailureInfo, FailureModal, LogView, NavFooter, StepListView};

#[component]
pub fn ProgressScreen() -> Element {
    let answers_signal = use_context::<Signal<PromptAnswers>>();

    let answers_for_steps = answers_signal.read().clone();
    let mut steps = use_signal(move || initial_steps(&answers_for_steps));
    let mut log_lines = use_signal(Vec::<String>::new);
    let mut error_signal = use_signal::<Option<FailureInfo>>(|| None);
    let mut complete = use_signal(|| false);
    // Set when the install paused for a Windows restart (WSL2 enablement).
    let mut reboot = use_signal::<Option<String>>(|| None);

    // Spawn the install task once on mount. Resource runs once even though
    // its body closes over the signals we mutate from event handling.
    let _install_resource = use_resource(move || {
        let answers = answers_signal.read().clone();
        async move {
            let (tx, mut rx) = unbounded_channel::<ProgressEvent>();
            // Run the install on a separate task so the receive loop below
            // isn't blocked by step bodies.
            tokio::spawn(async move {
                install_steps::run(answers, tx).await;
            });
            while let Some(event) = rx.recv().await {
                match event {
                    ProgressEvent::StepStart { name } => {
                        steps.with_mut(|s| set_step(s, name, StepStatus::Running, 0));
                        log_lines.with_mut(|l| l.push(format!("[▶] {name}")));
                    }
                    ProgressEvent::StepLog { name: _, line } => {
                        log_lines.with_mut(|l| l.push(line));
                    }
                    ProgressEvent::StepDone { name, duration } => {
                        let secs = duration.as_secs() as u32;
                        steps.with_mut(|s| set_step(s, name, StepStatus::Done, secs));
                        log_lines.with_mut(|l| l.push(format!("[✓] {name}  ({secs}s)")));
                    }
                    ProgressEvent::StepFailed { name, error } => {
                        steps.with_mut(|s| set_step(s, name, StepStatus::Failed, 0));
                        log_lines.with_mut(|l| l.push(format!("[✗] {name}: {error}")));
                        error_signal.set(Some(FailureInfo {
                            step: name.to_string(),
                            message: error,
                            log_path: None,
                        }));
                    }
                    ProgressEvent::InstallComplete => {
                        complete.set(true);
                    }
                    ProgressEvent::RebootRequired { message } => {
                        log_lines.with_mut(|l| l.push(format!("[⟳] {message}")));
                        reboot.set(Some(message));
                        complete.set(true);
                    }
                }
            }
        }
    });

    let is_complete = *complete.read();
    let reboot_msg = reboot.read().clone();
    // Sandbox (SKIP_SCHTASKS): the WSL2-enable + resume hooks ran as no-ops, so
    // the reboot footer must not offer a real restart that would only dead-end.
    let sandbox = crate::platform::skip_systemctl();
    let step_count = steps.read().len();

    rsx! {
        div { class: "screen",
            div { class: "screen-header",
                h1 { class: "screen-title", "Installing" }
                p { class: "screen-subtitle",
                    "{step_count} steps. Output streams below — your install log is "
                    "preserved under "
                    code {
                        if cfg!(windows) { "%LOCALAPPDATA%\\ag\\logs\\" } else { "~/.local/share/ag/logs/" }
                    }
                    "."
                }
            }
            div { class: "screen-body progress-body",
                div { class: "progress-pane progress-left",
                    StepListView { steps: steps.read().clone() }
                }
                div { class: "progress-pane progress-right",
                    LogView { lines: log_lines.read().clone() }
                }
            }
            FailureModal { error: error_signal }
            if let Some(msg) = reboot_msg {
                div {
                    style: "margin: 0.75rem 1.5rem; padding: 0.75rem 1rem; border: 1px solid \
                        #7C2A02; border-radius: 6px; background: rgba(124,42,2,0.15); \
                        color: #d1d5db;",
                    p { style: "margin: 0;", "{msg}" }
                }
                div { class: "screen-footer",
                    div { class: "screen-footer-left" }
                    div { class: "screen-footer-right",
                        if sandbox {
                            // Dry run: nothing was enabled or registered, so a
                            // real restart would dead-end (no RunOnce hook to
                            // relaunch). Offer a plain close, not "Restart now".
                            button {
                                class: "btn btn-primary",
                                onclick: move |_| quit_installer(),
                                "Close (dry run)"
                            }
                        } else {
                            button {
                                class: "btn btn-secondary",
                                onclick: move |_| quit_installer(),
                                "I'll restart later"
                            }
                            button {
                                class: "btn btn-primary",
                                onclick: move |_| {
                                    // Best-effort immediate restart; the OS tears
                                    // this process down. The HKCU RunOnce hook
                                    // relaunches the installer after logon.
                                    let _ = std::process::Command::new("shutdown")
                                        .args(["/r", "/t", "0"])
                                        .spawn();
                                },
                                "Restart now"
                            }
                        }
                    }
                }
            } else {
                NavFooter {
                    next_label: if is_complete { "Continue".to_string() } else { "Installing…".to_string() },
                    next_enabled: is_complete,
                    hide_back: true,
                }
            }
        }
    }
}

/// Quit the installer. Wrapped in a `()`-returning fn so the click handler
/// doesn't trip never-type fallback — `std::process::exit` returns `!`.
fn quit_installer() {
    std::process::exit(0);
}

fn initial_steps(answers: &PromptAnswers) -> Vec<InstallStep> {
    let mut names: Vec<&'static str> = Vec::new();
    #[cfg(windows)]
    match answers.docker_setup_choice() {
        Some("install_docker_desktop") => names.push(INSTALL_DOCKER_STEP_NAME),
        Some("install_wsl2_docker") => names.push(INSTALL_WSL2_DOCKER_STEP_NAME),
        Some("enable_wsl2_docker") => {
            names.push(INSTALL_WSL2_ENABLE_STEP_NAME);
            names.push(INSTALL_WSL2_DOCKER_STEP_NAME);
        }
        _ => {}
    }
    names.extend_from_slice(STEP_NAMES);
    names
        .iter()
        .map(|&name| InstallStep {
            name,
            status: StepStatus::Pending,
            duration_s: 0,
        })
        .collect()
}

fn set_step(steps: &mut [InstallStep], name: &str, status: StepStatus, duration_s: u32) {
    if let Some(step) = steps.iter_mut().find(|s| s.name == name) {
        step.status = status;
        step.duration_s = duration_s;
    }
}
