//! Screen 3 — Prompts.
//!
//! Reads the `DetectionResult` from context, asks `prompts::required_prompts`
//! which forms to show, renders one card per prompt, and writes the user's
//! choices into the shared `PromptAnswers` signal on submit / NavFooter Next.
//!
//! If detection didn't flag anything, the screen surfaces a single
//! "nothing to decide" card and the user proceeds straight to Install.

use dioxus::prelude::*;

use crate::detection::DetectionResult;
use crate::prompts::{self, PromptAnswers, PromptId, PromptOption};
use crate::ui::components::prompt_radio::RadioOption;
use crate::ui::components::{NavFooter, PromptRadio};

#[component]
pub fn PromptsScreen() -> Element {
    let detection_signal = use_context::<Signal<Option<DetectionResult>>>();
    let mut answers_signal = use_context::<Signal<PromptAnswers>>();

    let detection_state = detection_signal.read();
    let prompts_to_show: Vec<PromptId> = detection_state
        .as_ref()
        .map(prompts::required_prompts)
        .unwrap_or_default();

    // Seed defaults on first render so even the no-interaction path leaves
    // sensible values in PromptAnswers for Phase D to read.
    {
        let current = answers_signal.read().clone();
        let mut updated = current.clone();
        let mut changed = false;
        for id in &prompts_to_show {
            if updated.choice(*id).is_none() {
                updated.set_choice(*id, id.default_choice().to_string());
                changed = true;
            }
        }
        if changed {
            answers_signal.set(updated);
        }
    }

    rsx! {
        div { class: "screen",
            div { class: "screen-header",
                h1 { class: "screen-title",
                    {
                        match prompts_to_show.len() {
                            0 => "Nothing to decide".to_string(),
                            1 => "One choice to make".to_string(),
                            n => format!("{n} choices to make"),
                        }
                    }
                }
                p { class: "screen-subtitle",
                    if prompts_to_show.is_empty() {
                        "Detection didn't flag anything that needs a decision before install."
                    } else {
                        "Each card below was triggered by something on the previous screen."
                    }
                }
            }
            div { class: "screen-body",
                if prompts_to_show.is_empty() {
                    div { class: "prompt-card",
                        div { class: "prompt-card-header",
                            h2 { "All clear" }
                            p { class: "prompt-card-context",
                                "Click Begin install to proceed."
                            }
                        }
                    }
                } else {
                    div { class: "prompts-grid",
                        for id in prompts_to_show.iter().copied() {
                            PromptCard {
                                key: "{id_key(id)}",
                                id: id,
                                detection: detection_state.as_ref().cloned().unwrap_or_default(),
                            }
                        }
                    }
                }
            }
            NavFooter { next_label: "Begin install".to_string() }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct PromptCardProps {
    id: PromptId,
    detection: DetectionResult,
}

#[component]
fn PromptCard(props: PromptCardProps) -> Element {
    let id = props.id;
    let mut answers_signal = use_context::<Signal<PromptAnswers>>();

    let current_choice = answers_signal
        .read()
        .choice(id)
        .map(str::to_string)
        .unwrap_or_else(|| id.default_choice().to_string());
    let selected = use_signal(|| current_choice.clone());

    // Mirror local radio state → shared PromptAnswers signal whenever it
    // changes. use_effect re-runs every time `selected` updates.
    {
        use_effect(move || {
            let value = selected.read().clone();
            let mut answers = answers_signal.read().clone();
            if answers.choice(id) != Some(value.as_str()) {
                answers.set_choice(id, value);
                answers_signal.set(answers);
            }
        });
    }

    let options: Vec<RadioOption> = id
        .options()
        .into_iter()
        .map(
            |PromptOption {
                 key,
                 label,
                 description,
             }| RadioOption {
                key: key.to_string(),
                label: label.to_string(),
                description: description.to_string(),
            },
        )
        .collect();

    rsx! {
        div { class: "prompt-card",
            div { class: "prompt-card-header",
                h2 { "{id.title()}" }
                p { class: "prompt-card-context", "{id.context(&props.detection)}" }
            }
            PromptRadio {
                name: id_key(id).to_string(),
                options: options,
                selected: selected,
            }
            if id == PromptId::PortBusy && *selected.read() == "pick" {
                PortPicker {}
            }
        }
    }
}

/// Optional sub-form for the PortBusy "pick" choice. Validates the port falls
/// in [1024, 65535] and stores it in `PromptAnswers.backend_port`. We don't
/// re-probe `ss -tln` here — Phase D does that just before binding so the
/// check is fresh.
#[component]
fn PortPicker() -> Element {
    let mut answers_signal = use_context::<Signal<PromptAnswers>>();
    let initial = answers_signal
        .read()
        .backend_port
        .map(|p| p.to_string())
        .unwrap_or_default();
    let raw = use_signal(|| initial);
    let mut error = use_signal(String::new);

    let mut raw_handle = raw;
    let oninput = move |evt: Event<FormData>| {
        let v = evt.value();
        raw_handle.set(v.clone());
        match parse_port(&v) {
            Ok(Some(port)) => {
                let mut answers = answers_signal.read().clone();
                answers.backend_port = Some(port);
                answers_signal.set(answers);
                error.set(String::new());
            }
            Ok(None) => {
                // Empty input — clear stored value, no error.
                let mut answers = answers_signal.read().clone();
                answers.backend_port = None;
                answers_signal.set(answers);
                error.set(String::new());
            }
            Err(msg) => {
                error.set(msg.to_string());
            }
        }
    };

    rsx! {
        div { class: "prompt-card-subform",
            label { class: "prompt-card-sublabel", "New backend port (1024-65535):" }
            input {
                r#type: "text",
                class: "prompt-card-input",
                aria_label: "New backend port (1024-65535)",
                value: "{raw.read()}",
                oninput: oninput,
                placeholder: "e.g. 3011",
            }
            if !error.read().is_empty() {
                div { class: "prompt-card-error", "{error}" }
            }
        }
    }
}

fn parse_port(s: &str) -> Result<Option<u16>, &'static str> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let n: u32 = trimmed.parse().map_err(|_| "must be a number")?;
    if !(1024..=65535).contains(&n) {
        return Err("must be between 1024 and 65535");
    }
    Ok(Some(n as u16))
}

fn id_key(id: PromptId) -> &'static str {
    match id {
        PromptId::DiskLow => "disk_low",
        PromptId::DockerMissing => "docker_missing",
        PromptId::PortBusy => "port_busy",
        PromptId::LowRam => "low_ram",
        PromptId::NativeObs => "native_obs",
        PromptId::SystemRedis => "system_redis",
        PromptId::AgInstallDrift => "ag_install_drift",
    }
}
