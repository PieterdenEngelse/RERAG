//! Screen 4 — Install Progress.
//!
//! Two-pane layout: step list on the left, scrolling log view on the right.
//! Phase B: both are populated with mock_install_steps() and mock_log_lines().
//! The step list shows one step "running" (FalkorDB) with the rest a mix of
//! done/pending — same shape Phase D drives from real subprocess events.
//!
//! No Back button — once Phase D is wired, writes start happening on this
//! screen. The Next button is a stand-in for "wait for install to complete"
//! and routes to First-Run Config.

use dioxus::prelude::*;

use crate::app::{mock_install_steps, mock_log_lines};
use crate::ui::components::{LogView, NavFooter, StepListView};

#[component]
pub fn ProgressScreen() -> Element {
    let steps = use_memo(mock_install_steps);
    let log_lines: Vec<String> = mock_log_lines().into_iter().map(String::from).collect();

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
                    LogView { lines: log_lines }
                }
            }
            div { class: "progress-footnote",
                "(Phase B: mock animation. Phase D wires real step transitions, "
                "subprocess output streaming, and the failure modal.)"
            }
            NavFooter {
                next_label: "Skip mock → First-Run".to_string(),
                hide_back: true,
            }
        }
    }
}
