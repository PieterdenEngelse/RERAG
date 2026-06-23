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
use crate::install_steps::{self, ProgressEvent, STEP_NAMES};
use crate::prompts::PromptAnswers;
use crate::ui::components::{FailureInfo, FailureModal, LogView, NavFooter, StepListView};

#[component]
pub fn ProgressScreen() -> Element {
    let answers_signal = use_context::<Signal<PromptAnswers>>();

    let mut steps = use_signal(initial_steps);
    let mut log_lines = use_signal(Vec::<String>::new);
    let mut error_signal = use_signal::<Option<FailureInfo>>(|| None);
    let mut complete = use_signal(|| false);

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
                }
            }
        }
    });

    let is_complete = *complete.read();

    rsx! {
        div { class: "screen",
            div { class: "screen-header",
                h1 { class: "screen-title", "Installing" }
                p { class: "screen-subtitle",
                    "Six steps. Output streams below — your install log is "
                    "preserved under "
                    code { "~/.local/share/ag/logs/" }
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
            NavFooter {
                next_label: if is_complete { "Continue".to_string() } else { "Installing…".to_string() },
                next_enabled: is_complete,
                hide_back: true,
            }
        }
    }
}

fn initial_steps() -> Vec<InstallStep> {
    STEP_NAMES
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
