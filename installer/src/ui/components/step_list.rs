//! Vertical step indicator used on the Install Progress screen.

use dioxus::prelude::*;

use crate::app::{InstallStep, StackPull, StepStatus};
use crate::install_steps::STEP_STACK;
use crate::ui::components::{IconKind, StatusIcon};

// --- Docker-compose stack pull estimate --------------------------------------
// On a fresh distro the Windows stack pulls ~3 GB of container images (ollama
// ~1.5 GB dominates, plus grafana / otel-contrib / prometheus / tempo / loki /
// falkordb / redis). It's a one-time cost — re-runs hit Docker's layer cache.
// `docker compose up` streams no intermediate progress back to the installer,
// so the bar below is a *time estimate*, not live byte progress: ETA ≈ image
// size ÷ download speed, plus a factor for unpacking layers onto the ext4 disk.
//
// Calibrated against a real cold run: ~3 GB pulled + unpacked in ~305 s (~5 min)
// over a ~44 Mbps single-stream link — Docker's parallel layer fetches reached
// ~80 Mbps effective and extraction overlapped the download, so the fast bound
// is generous and the overhead factor small.
const STACK_IMAGE_MB: f64 = 3072.0; // ~3 GB
const STACK_FAST_MBPS: f64 = 100.0; // fast broadband, parallel pulls → ETA low bound
const STACK_SLOW_MBPS: f64 = 25.0; // modest connection → ETA high bound
const STACK_NOMINAL_MBPS: f64 = 55.0; // drives the bar's fill rate
const STACK_OVERHEAD: f64 = 1.1; // serial tail: last-layer unpack, dockerd wait

/// Rough seconds to pull + unpack the stack images at `mbps` megabits/s.
fn stack_eta_secs(mbps: f64) -> u32 {
    (STACK_IMAGE_MB * 8.0 / mbps * STACK_OVERHEAD) as u32
}

/// Compact byte size: MB under 1 GB, else GB.
fn human_bytes(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{mb:.0} MB")
    }
}

#[component]
pub fn StepListView(steps: Vec<InstallStep>, stack: StackPull) -> Element {
    rsx! {
        ul { class: "step-list",
            for step in steps.iter() {
                StepListItem { step: step.clone(), stack }
            }
        }
    }
}

#[component]
fn StepListItem(step: InstallStep, stack: StackPull) -> Element {
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
    // The multi-GB image pull only happens on the Windows docker-compose stack
    // step (Linux's STEP_STACK is a quick native FalkorDB service), and only
    // while it's actually running.
    let show_stack_progress =
        cfg!(windows) && step.name == STEP_STACK && step.status == StepStatus::Running;
    rsx! {
        li { class: "step-list-item {class_state}",
            div { class: "step-icon", StatusIcon { kind } }
            div { class: "step-name", "{step.name}" }
            div { class: "step-duration", "{duration_text}" }
            if show_stack_progress {
                StackProgress { stack }
            }
        }
    }
}

/// Progress bar shown under the running docker-compose stack step. Once the
/// pull streams byte counts it shows a *measured* bar — real %, live MB/s, ETA
/// from the actual download. Before any bytes arrive (Docker starting, first
/// layers) — and on the non-WSL2 path that doesn't stream — it falls back to a
/// time-based estimate that fills toward (never reaching) 100% over the nominal
/// ETA. Either way the step flipping to Done removes it.
#[component]
fn StackProgress(stack: StackPull) -> Element {
    if stack.total_bytes > 0 {
        // Measured: drive everything from the live byte stream.
        let done = stack.done_bytes.min(stack.total_bytes);
        let pct = (done as f64 / stack.total_bytes as f64 * 100.0).round() as u32;
        let mib_s = stack.bytes_per_sec / 1_048_576.0;
        let eta = if stack.bytes_per_sec > 1.0 {
            let secs = (stack.total_bytes.saturating_sub(done) as f64 / stack.bytes_per_sec) as u32;
            format!("ETA {}:{:02}", secs / 60, secs % 60)
        } else {
            "ETA —".to_string()
        };
        let label = format!(
            "{} / {} · {:.1} MB/s · {} · pulling images",
            human_bytes(done),
            human_bytes(stack.total_bytes),
            mib_s,
            eta,
        );
        rsx! {
            div { class: "stack-progress",
                div { class: "stack-bar-track",
                    div { class: "stack-bar-fill", style: "width: {pct}%;" }
                }
                div { class: "stack-bar-label", "{label}" }
            }
        }
    } else {
        // Estimate: no measured bytes yet — fill by elapsed time vs nominal ETA.
        let lo_min = stack_eta_secs(STACK_FAST_MBPS).div_ceil(60);
        let hi_min = stack_eta_secs(STACK_SLOW_MBPS).div_ceil(60);
        let nominal = stack_eta_secs(STACK_NOMINAL_MBPS).max(1);
        let pct = ((stack.elapsed_s as f64 / nominal as f64).min(0.95) * 100.0).round() as u32;
        let mm = stack.elapsed_s / 60;
        let ss = stack.elapsed_s % 60;
        rsx! {
            div { class: "stack-progress",
                div { class: "stack-bar-track",
                    div { class: "stack-bar-fill", style: "width: {pct}%;" }
                }
                div { class: "stack-bar-label",
                    "{mm}:{ss:02} elapsed · est. ~{lo_min}–{hi_min} min · pulling ~3 GB of images (one-time)"
                }
            }
        }
    }
}
