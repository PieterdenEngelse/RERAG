//! Screen 5 — First-Run Config.
//!
//! Three knobs the user picks once before ag's first boot:
//! Ollama model, FalkorDB password, agent mode. Everything else stays at
//! default and is editable in the dashboard.
//!
//! Phase B: form is wired and selections persist in local signals, but the
//! values aren't yet written to ~/.config/ag/ag.env. Phase E adds the
//! probe of /api/tags for the Ollama list, the atomic env write, and the
//! falkordb password-change-with-restart flow.

use dioxus::prelude::*;

use crate::ui::components::NavFooter;

#[component]
pub fn FirstRunForm() -> Element {
    let model = use_signal(|| "phi:latest".to_string());
    let password = use_signal(|| "agpassword123".to_string());
    let mode = use_signal(|| "Hybrid".to_string());

    rsx! {
        div { class: "screen",
            div { class: "screen-header",
                h1 { class: "screen-title", "First-run configuration" }
                p { class: "screen-subtitle",
                    "Three settings you'll want to confirm before ag starts. "
                    "Everything else stays default and is editable in the "
                    "dashboard."
                }
            }
            div { class: "screen-body",
                div { class: "first-run-form",
                    OllamaModelField { model }
                    FalkordbPasswordField { password }
                    AgentModeField { mode }
                }
                p { class: "first-run-footnote",
                    "(Phase B: defaults are pre-filled. Phase E queries "
                    "Ollama's /api/tags for the real model list, writes the "
                    "selected values into ~/.config/ag/ag.env, and restarts "
                    "FalkorDB if the password changed.)"
                }
            }
            NavFooter {
                next_label: "Start ag".to_string(),
                hide_back: true,
                hide_cancel: true,
            }
        }
    }
}

#[component]
fn OllamaModelField(model: Signal<String>) -> Element {
    let m = model;
    rsx! {
        div { class: "field-group",
            label { class: "field-label", "Ollama model" }
            select {
                class: "field-select",
                value: "{m.read()}",
                onchange: move |evt| m.clone().set(evt.value().clone()),
                option { value: "phi:latest", "phi:latest" }
                option { value: "llama3.2:3b", "llama3.2:3b" }
                option { value: "qwen2.5:7b", "qwen2.5:7b" }
                option { value: "mistral:7b", "mistral:7b" }
                option { value: "gemma2:2b", "gemma2:2b" }
            }
            p { class: "field-hint",
                "Phase E populates this from your local Ollama install."
            }
        }
    }
}

#[component]
fn FalkordbPasswordField(password: Signal<String>) -> Element {
    let p = password;
    rsx! {
        div { class: "field-group",
            label { class: "field-label", "FalkorDB password" }
            input {
                class: "field-input",
                r#type: "text",
                value: "{p.read()}",
                oninput: move |evt| p.clone().set(evt.value().clone()),
            }
            p { class: "field-hint field-hint-warn",
                "Default is the public sample value. Change it if this host "
                "is ever exposed beyond localhost."
            }
        }
    }
}

#[component]
fn AgentModeField(mode: Signal<String>) -> Element {
    let m = mode;
    let descriptions = [
        ("Hybrid", "Search + LLM fallback. The \"just works\" choice."),
        ("Rag", "Retrieval only. No LLM needed; works without Ollama."),
        ("Llm", "LLM only. Ollama required; no retrieval."),
        ("RagStrict", "Grounded retrieval only — refuses to answer beyond corpus."),
        ("Agentic", "Tool-calling loop via Rig framework."),
    ];
    let current = m.read().clone();
    let desc = descriptions
        .iter()
        .find(|(k, _)| *k == current.as_str())
        .map(|(_, d)| *d)
        .unwrap_or("");

    rsx! {
        div { class: "field-group",
            label { class: "field-label", "Agent mode" }
            select {
                class: "field-select",
                value: "{m.read()}",
                onchange: move |evt| m.clone().set(evt.value().clone()),
                for (k, _) in descriptions.iter() {
                    option { key: "{k}", value: "{k}", "{k}" }
                }
            }
            p { class: "field-hint", "{desc}" }
        }
    }
}
