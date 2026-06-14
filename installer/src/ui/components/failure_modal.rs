//! Modal overlay shown when an install step fails.
//!
//! Driven by a `Signal<Option<FailureInfo>>` owned by the Progress screen.
//! Setting `Some(info)` opens the modal; the close button clears it back to
//! `None`. The "Open log" button is D.2 scope — in D.1 there is no log file
//! to open (no real writes happen), so the button surfaces a placeholder
//! message instead of being silently broken.

use dioxus::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureInfo {
    pub step: String,
    pub message: String,
    /// Optional absolute path to the install log. D.2 sets this when the
    /// `ensure_xdg` step finishes; D.1 always leaves it `None`.
    pub log_path: Option<String>,
}

#[derive(Props, Clone, PartialEq)]
pub struct FailureModalProps {
    pub error: Signal<Option<FailureInfo>>,
}

#[component]
pub fn FailureModal(props: FailureModalProps) -> Element {
    let mut error_signal = props.error;
    let snapshot = error_signal.read().clone();
    let Some(info) = snapshot else {
        return rsx! {};
    };

    let log_path = info.log_path.clone();
    let log_button_label = match log_path.as_deref() {
        Some(_) => "Open log",
        None => "Open log (D.1: no log written yet)",
    };
    let log_button_enabled = log_path.is_some();

    let on_open_log = move |_| {
        if let Some(path) = &log_path {
            // xdg-open is the standard portable opener on Linux desktops.
            // We don't await or error-check: best-effort is fine for a
            // failure-modal escape hatch.
            let _ = std::process::Command::new("xdg-open").arg(path).spawn();
        }
    };

    rsx! {
        div { class: "modal-overlay",
            div { class: "modal-content failure-modal",
                h2 { class: "modal-title", "Install failed" }
                p { class: "modal-step", "Step: " span { class: "modal-step-name", "{info.step}" } }
                pre { class: "modal-message", "{info.message}" }
                div { class: "modal-actions",
                    button {
                        class: "btn btn-link",
                        disabled: !log_button_enabled,
                        onclick: on_open_log,
                        "{log_button_label}"
                    }
                    button {
                        class: "btn btn-primary",
                        onclick: move |_| error_signal.set(None),
                        "Close"
                    }
                }
            }
        }
    }
}
