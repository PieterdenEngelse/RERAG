//! Screen 6 — Done / Summary.
//!
//! Four-bucket summary view (Reused silent / Reused confirmed / Installed
//! fresh / Reused with assumption) + the configured first-run settings,
//! mirroring the bash installer's terminal output.
//!
//! Buttons: Open ag dashboard / View install log / Close. Phase B wires
//! the buttons to placeholders; Phase D/E hook them up to real `xdg-open`
//! calls.

use dioxus::prelude::*;

use crate::app::{
    mock_assumptions, mock_installed_fresh, mock_reused_confirmed, mock_reused_silent,
    SummaryItem,
};
use crate::ui::components::{IconKind, StatusIcon};

#[component]
pub fn SummaryScreen() -> Element {
    rsx! {
        div { class: "screen",
            div { class: "screen-header",
                h1 { class: "screen-title", "Installed" }
                p { class: "screen-subtitle",
                    "ag is set up. The summary below shows exactly what was "
                    "reused vs installed fresh; any "
                    span { class: "icon icon-warn", "⚠" }
                    " lines are configuration notes worth a glance."
                }
            }
            div { class: "screen-body summary-body",
                SummaryBucket {
                    title: "Reused (silent)".to_string(),
                    icon: IconKind::Ok,
                    items: mock_reused_silent(),
                }
                SummaryBucket {
                    title: "Reused (confirmed)".to_string(),
                    icon: IconKind::Ok,
                    items: mock_reused_confirmed(),
                }
                SummaryBucket {
                    title: "Installed fresh".to_string(),
                    icon: IconKind::Active,
                    items: mock_installed_fresh(),
                }
                SummaryBucket {
                    title: "Reused with assumption".to_string(),
                    icon: IconKind::Warn,
                    items: mock_assumptions(),
                }
                div { class: "first-run-summary",
                    h3 { "Configured for first run" }
                    ul {
                        li { "Ollama model: " span { class: "value", "phi:latest" } }
                        li { "FalkorDB password: " span { class: "value", "(default — consider changing)" } }
                        li { "Agent mode: " span { class: "value", "Hybrid" } }
                        li { "Backend port: " span { class: "value", "3010" } }
                    }
                }
            }
            div { class: "screen-footer",
                div { class: "screen-footer-left",
                    button { class: "btn btn-ghost", "View install log" }
                }
                div { class: "screen-footer-right",
                    button { class: "btn btn-secondary", "Close" }
                    button { class: "btn btn-primary", "Open ag dashboard" }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct SummaryBucketProps {
    title: String,
    icon: IconKind,
    items: Vec<SummaryItem>,
}

#[component]
fn SummaryBucket(props: SummaryBucketProps) -> Element {
    if props.items.is_empty() {
        return rsx! { Fragment {} };
    }
    rsx! {
        div { class: "summary-bucket",
            div { class: "summary-bucket-header",
                StatusIcon { kind: props.icon }
                span { class: "summary-bucket-title", "{props.title}" }
            }
            ul { class: "summary-bucket-items",
                for item in props.items.iter() {
                    li { key: "{item.key}",
                        span { class: "summary-bucket-key", "{item.key}" }
                        span { class: "summary-bucket-detail", "{item.detail}" }
                    }
                }
            }
        }
    }
}
