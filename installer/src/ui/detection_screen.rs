//! Screen 2 — Detection.
//!
//! Renders the mock detection results as a table with green checkmarks /
//! yellow warning icons. Phase C replaces mock_detections() with real probes.

use dioxus::prelude::*;

use crate::app::{mock_detections, DetectionStatus};
use crate::ui::components::{IconKind, NavFooter, StatusIcon};

#[component]
pub fn DetectionScreen() -> Element {
    let rows = use_memo(mock_detections);

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
                table { class: "detection-table",
                    tbody {
                        for row in rows.read().iter() {
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
                p { class: "detection-footnote",
                    "(Phase B: results are mocked. Phase C wires the same labels "
                    "to real probes against your machine.)"
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
