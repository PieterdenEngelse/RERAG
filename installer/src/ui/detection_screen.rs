//! Screen 2 — Detection.
//!
//! Runs the real probes from `crate::detection` once on mount, then renders
//! the result as a table. While probes are pending the screen shows a
//! spinner — keeps the user informed when the probes take a couple of seconds
//! on a busy docker daemon.

use dioxus::prelude::*;

use crate::app::{detection_rows, DetectionStatus};
use crate::detection::{self, DetectionResult};
use crate::ui::components::{IconKind, NavFooter, StatusIcon};

#[component]
pub fn DetectionScreen() -> Element {
    let mut detection_signal = use_context::<Signal<Option<DetectionResult>>>();

    // Resource runs once on mount and stores the result in the shared signal
    // so the Prompts screen (and anything else downstream) can read it.
    let resource = use_resource(move || async move {
        let result = detection::run().await;
        detection_signal.set(Some(result.clone()));
        result
    });

    let value_state = resource.value();
    let value = value_state.read();

    rsx! {
        div { class: "screen",
            div { class: "screen-header",
                h1 { class: "screen-title", "Detection" }
                p { class: "screen-subtitle",
                    "Checking what's already on this host. Anything yellow may "
                    "need a choice on the next screen."
                }
            }
            div { class: "screen-body",
                match value.as_ref() {
                    None => rsx! {
                        div { class: "detection-pending",
                            div { class: "detection-spinner" }
                            p { class: "detection-pending-label",
                                "Probing docker, systemd units, ports, disk and RAM…"
                            }
                        }
                    },
                    Some(result) => {
                        let rows = detection_rows(result);
                        rsx! {
                            table { class: "detection-table",
                                tbody {
                                    for row in rows.iter() {
                                        tr { key: "{row.label}", class: row_class(row.status),
                                            td { class: "detection-icon",
                                                StatusIcon { kind: icon_for(row.status) }
                                            }
                                            td { class: "detection-label", "{row.label}" }
                                            td { class: "detection-value", "{row.value}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            NavFooter { next_label: "Continue".to_string() }
        }
    }
}

fn icon_for(s: DetectionStatus) -> IconKind {
    match s {
        DetectionStatus::Ok => IconKind::Ok,
        DetectionStatus::Warn => IconKind::Warn,
    }
}

fn row_class(s: DetectionStatus) -> &'static str {
    match s {
        DetectionStatus::Ok => "detection-row detection-row-ok",
        DetectionStatus::Warn => "detection-row detection-row-warn",
    }
}
