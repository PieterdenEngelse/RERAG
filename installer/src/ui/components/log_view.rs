//! Scrolling log pane for the Install Progress screen.

use dioxus::prelude::*;

#[component]
pub fn LogView(lines: Vec<String>) -> Element {
    rsx! {
        div { class: "log-view",
            for (i, line) in lines.iter().enumerate() {
                div { key: "{i}", class: log_class(line), "{line}" }
            }
        }
    }
}

fn log_class(line: &str) -> &'static str {
    if line.contains("✓ ") {
        "log-line log-success"
    } else if line.contains("✗ ") || line.contains("error") || line.contains("ERROR") {
        "log-line log-error"
    } else if line.contains("⚠ ") || line.contains("warning") || line.contains("WARN") {
        "log-line log-warn"
    } else if line.starts_with("[") {
        "log-line log-step"
    } else {
        "log-line"
    }
}
