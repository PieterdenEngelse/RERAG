//! Vertical step indicator used on the Install Progress screen.

use dioxus::prelude::*;

use crate::app::{InstallStep, StepStatus};
use crate::ui::components::{IconKind, StatusIcon};

#[component]
pub fn StepListView(steps: Vec<InstallStep>) -> Element {
    rsx! {
        ul { class: "step-list",
            for step in steps.iter() {
                StepListItem { step: step.clone() }
            }
        }
    }
}

#[component]
fn StepListItem(step: InstallStep) -> Element {
    let kind = match step.status {
        StepStatus::Pending => IconKind::Pending,
        StepStatus::Running => IconKind::Active,
        StepStatus::Done => IconKind::Ok,
        StepStatus::Failed => IconKind::Error,
    };
    let class_state = match step.status {
        StepStatus::Pending => "step-pending",
        StepStatus::Running => "step-running",
        StepStatus::Done => "step-done",
        StepStatus::Failed => "step-failed",
    };
    let duration_text = if step.duration_s > 0 {
        format!("{}s", step.duration_s)
    } else {
        String::new()
    };
    rsx! {
        li { class: "step-list-item {class_state}",
            div { class: "step-icon", StatusIcon { kind } }
            div { class: "step-name", "{step.name}" }
            div { class: "step-duration", "{duration_text}" }
        }
    }
}
