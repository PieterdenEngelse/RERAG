//! Screen 2 — Detection.
//!
//! Runs the real probes from `crate::detection` once on mount, then renders
//! the result as two side-by-side groups: OK on the left, "Need attention"
//! (Warn) on the right. Warn-on-the-right keeps the urgent items where the
//! eye lands first when the user is looking for what they need to act on.
//! While probes are pending the screen shows a spinner over a pulsing
//! checklist of the checks being run, so the wait reads as "scanning the
//! host" rather than a frozen screen.

use dioxus::prelude::*;

use crate::app::{detection_rows, DetectionRow, DetectionStatus};
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
                    "Checking what's already on this host. Anything in the right "
                    "column may need a choice on the next screen."
                }
            }
            div { class: "screen-body",
                match value.as_ref() {
                    None => {
                        // Reuse detection_rows so the pending checklist always
                        // matches the real row set: labels are cfg-branched but
                        // value-independent, so a default result yields exactly
                        // the labels (and order) that will appear once probes
                        // resolve — no second list to keep in sync.
                        let probing: Vec<&'static str> = detection_rows(&DetectionResult::default())
                            .into_iter()
                            .map(|r| r.label)
                            .collect();
                        rsx! {
                            div { class: "detection-pending",
                                div { class: "detection-spinner" }
                                p { class: "detection-pending-label",
                                    if cfg!(windows) {
                                        "Probing docker, scheduled tasks, ports, disk, RAM and WSL2…"
                                    } else {
                                        "Probing docker, systemd units, ports, disk and RAM…"
                                    }
                                }
                                ul { class: "detection-probe-grid",
                                    for label in probing {
                                        li { key: "{label}", class: "detection-probe-item",
                                            span { class: "detection-probe-dot" }
                                            span { class: "detection-probe-name", "{label}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some(result) => {
                        let rows = detection_rows(result);
                        // Only Warn rows go in the right "needs attention" column;
                        // Ok (✓) and Info (○) both mean "no action needed" and
                        // share the left column.
                        let (cleared_rows, warn_rows): (Vec<DetectionRow>, Vec<DetectionRow>) = rows
                            .into_iter()
                            .partition(|r| !matches!(r.status, DetectionStatus::Warn));
                        rsx! {
                            div { class: "detection-groups",
                                DetectionGroup {
                                    kind: GroupKind::Ok,
                                    rows: cleared_rows,
                                }
                                DetectionGroup {
                                    kind: GroupKind::Warn,
                                    rows: warn_rows,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum GroupKind {
    Ok,
    Warn,
}

#[derive(Props, Clone, PartialEq)]
struct DetectionGroupProps {
    kind: GroupKind,
    rows: Vec<DetectionRow>,
}

#[component]
fn DetectionGroup(props: DetectionGroupProps) -> Element {
    let count = props.rows.len();
    let (class, header) = match props.kind {
        GroupKind::Ok => (
            "detection-group detection-group-ok",
            format!("{count} no action needed"),
        ),
        GroupKind::Warn => (
            "detection-group detection-group-warn",
            if count == 0 {
                "All clear".to_string()
            } else {
                format!("{count} need attention")
            },
        ),
    };

    rsx! {
        div { class: "{class}",
            h2 { class: "detection-group-header", "{header}" }
            if props.rows.is_empty() {
                p { class: "detection-group-empty",
                    "Nothing flagged. Click Continue to proceed."
                }
            } else {
                table { class: "detection-table",
                    tbody {
                        for row in props.rows.iter() {
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

fn icon_for(s: DetectionStatus) -> IconKind {
    match s {
        DetectionStatus::Ok => IconKind::Ok,
        // Neutral ○ (gray) — "noted, not active", never a green pass.
        DetectionStatus::Info => IconKind::Pending,
        DetectionStatus::Warn => IconKind::Warn,
    }
}

fn row_class(s: DetectionStatus) -> &'static str {
    match s {
        DetectionStatus::Ok => "detection-row detection-row-ok",
        DetectionStatus::Info => "detection-row detection-row-info",
        DetectionStatus::Warn => "detection-row detection-row-warn",
    }
}
