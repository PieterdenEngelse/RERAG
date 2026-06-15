//! Screen 5 — First-Run Config.
//!
//! Collects the settings the user wants nailed down before ag's first
//! boot: Ollama model (from the user's real local install), FalkorDB
//! password, default agent mode, plus optional LLM API keys (OpenAI,
//! OpenRouter, Anthropic). On submit, atomically writes ag.env, applies
//! the FalkorDB password change if needed, starts ag.service, polls
//! /health up to 20s, and routes to the Done screen on success.
//!
//! Errors are surfaced inline in the form so the user can adjust without
//! losing what they typed.

use std::time::Duration;

use dioxus::prelude::*;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::sleep;

use crate::app::{use_screen, Screen};
use crate::first_run::{
    ag_env_path, backend_port, change_falkordb_password, probe_ollama_models,
    start_ag_and_wait, write_first_run_settings, FirstRunChoices, OllamaProbe,
};
use crate::install_steps::FALKORDB_PASS;
use crate::paths::Paths;

const AGENT_MODES: &[(&str, &str)] = &[
    ("Hybrid", "Search + LLM fallback. The \"just works\" choice."),
    ("Rag", "Retrieval only. No LLM needed; works without Ollama."),
    ("Llm", "LLM only. Ollama required; no retrieval."),
    ("RagStrict", "Grounded retrieval only — refuses to answer beyond the corpus."),
    ("Agentic", "Tool-calling loop via the Rig framework."),
];

#[component]
pub fn FirstRunForm() -> Element {
    // Field state.
    let model = use_signal(String::new);
    let mut password = use_signal(|| FALKORDB_PASS.to_string());
    let mode = use_signal(|| "Hybrid".to_string());
    let openai_key = use_signal(String::new);
    let openrouter_key = use_signal(String::new);
    let anthropic_key = use_signal(String::new);

    // Submit state.
    let mut submitting = use_signal(|| false);
    let mut error = use_signal::<Option<String>>(|| None);

    // Probe Ollama on mount.
    let probe = use_resource(probe_ollama_models);
    let probe_value = probe.value();
    let probe_state = probe_value.read();
    let ollama_models: Vec<String> = match probe_state.as_ref() {
        Some(OllamaProbe::Ok(m)) => m.clone(),
        _ => Vec::new(),
    };
    let ollama_unreachable = matches!(probe_state.as_ref(), Some(OllamaProbe::Unreachable));
    let probe_pending = probe_state.is_none();
    drop(probe_state);

    // Seed model with first available once probe completes.
    {
        let mut model_handle = model;
        let first_model = ollama_models.first().cloned();
        use_effect(move || {
            if model_handle.read().is_empty() {
                if let Some(first) = first_model.clone() {
                    model_handle.set(first);
                }
            }
        });
    }

    let mut screen = use_screen();

    let on_start = move |_| {
        if *submitting.read() {
            return;
        }
        submitting.set(true);
        error.set(None);
        let choices = FirstRunChoices {
            ollama_model: model.read().clone(),
            falkordb_password: password.read().clone(),
            agent_mode: mode.read().clone(),
            openai_api_key: openai_key.read().clone(),
            openrouter_api_key: openrouter_key.read().clone(),
            anthropic_api_key: anthropic_key.read().clone(),
        };
        spawn(async move {
            match run_submit(choices).await {
                Ok(()) => {
                    screen.set(Screen::Done);
                }
                Err(msg) => {
                    error.set(Some(msg));
                    submitting.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "screen",
            div { class: "screen-header",
                h1 { class: "screen-title", "First-run configuration" }
                p { class: "screen-subtitle",
                    "Confirm a few settings before ag starts. Everything else "
                    "stays default and is editable in the dashboard's Config "
                    "section."
                }
            }
            div { class: "screen-body",
                div { class: "first-run-form",

                    OllamaModelField {
                        model,
                        pending: probe_pending,
                        unreachable: ollama_unreachable,
                        models: ollama_models,
                    }

                    PasswordField {
                        password,
                        on_reset_default: move |_| password.set(FALKORDB_PASS.to_string()),
                    }

                    AgentModeField { mode }

                    h2 { class: "first-run-section-header", "Optional: LLM API keys" }
                    p { class: "first-run-section-note",
                        "Leave blank to stay on Ollama only. Keys land in "
                        code { "~/.config/ag/ag.env" }
                        " with mode 0600 and are never sent anywhere except the "
                        "provider you typed."
                    }

                    SecretField {
                        label: "OPENAI_API_KEY",
                        placeholder: "sk-…",
                        value: openai_key,
                    }
                    SecretField {
                        label: "OPENROUTER_API_KEY",
                        placeholder: "sk-or-v1-…",
                        value: openrouter_key,
                    }
                    SecretField {
                        label: "ANTHROPIC_API_KEY",
                        placeholder: "sk-ant-…",
                        value: anthropic_key,
                    }
                }

                if let Some(msg) = error.read().clone() {
                    div { class: "first-run-error",
                        strong { "Couldn't start ag." }
                        pre { "{msg}" }
                    }
                }
            }
            div { class: "screen-footer",
                div { class: "screen-footer-left" }
                div { class: "screen-footer-right",
                    button {
                        class: "btn btn-primary",
                        disabled: *submitting.read() || probe_pending,
                        onclick: on_start,
                        if *submitting.read() { "Starting ag…" } else { "Start ag" }
                    }
                }
            }
        }
    }
}

// =============================================================================
// Submit
// =============================================================================

async fn run_submit(choices: FirstRunChoices) -> Result<(), String> {
    let env_path = ag_env_path();
    write_first_run_settings(&env_path, &choices).map_err(|e| format!("{e:#}"))?;

    let paths = Paths::resolve();
    let (tx, mut rx) = unbounded_channel();
    let needs_password_change =
        !choices.falkordb_password.is_empty() && choices.falkordb_password != FALKORDB_PASS;

    // Run the work in this task; events go to tx which we drain after.
    // First-Run doesn't render a live log pane (unlike Progress) — the
    // events get discarded; Phase E only cares about the final result.
    let work = async move {
        if needs_password_change {
            change_falkordb_password(&paths, &tx, &choices.falkordb_password)
                .await
                .map_err(|e| format!("{e:#}"))?;
        }
        start_ag_and_wait(&tx, backend_port())
            .await
            .map_err(|e| format!("{e:#}"))
    };
    let work_task = tokio::spawn(work);

    // Drain the channel as events fire so the sender doesn't fill an
    // unbounded queue. The loop ends when the sender drops (work task
    // finishes).
    while rx.recv().await.is_some() {}
    // Tiny pause so the work task's Result is ready when we await it.
    sleep(Duration::from_millis(10)).await;
    work_task.await.map_err(|e| format!("join: {e}"))?
}

// =============================================================================
// Fields
// =============================================================================

#[component]
fn OllamaModelField(
    model: Signal<String>,
    pending: bool,
    unreachable: bool,
    models: Vec<String>,
) -> Element {
    let mut m = model;
    rsx! {
        div { class: "field-group",
            label { class: "field-label", "Ollama model" }
            if pending {
                p { class: "field-hint", "Probing Ollama at http://127.0.0.1:11434…" }
            } else if unreachable {
                select {
                    class: "field-select",
                    disabled: true,
                    option { "(Ollama not reachable)" }
                }
                p { class: "field-hint field-hint-warn",
                    "Ollama isn't responding on :11434. LLM-backed agent modes "
                    "(Hybrid / Llm / Agentic) will return 503 until it's up. "
                    "Start it with: "
                    code { "systemctl --user enable --now ollama" }
                    " — then revisit this screen on the next install run."
                }
            } else if models.is_empty() {
                select {
                    class: "field-select",
                    disabled: true,
                    option { "(no models pulled)" }
                }
                p { class: "field-hint field-hint-warn",
                    "Ollama is running but has no models. Pull one first: "
                    code { "ollama pull phi" }
                    "."
                }
            } else {
                select {
                    class: "field-select",
                    value: "{m.read()}",
                    onchange: move |evt| m.set(evt.value().clone()),
                    for name in models.iter() {
                        option { key: "{name}", value: "{name}", "{name}" }
                    }
                }
                p { class: "field-hint",
                    {format!("{} model(s) available on this host. Writes OLLAMA_MODEL=… to ag.env.", models.len())}
                }
            }
        }
    }
}

#[component]
fn PasswordField(password: Signal<String>, on_reset_default: EventHandler<()>) -> Element {
    let mut p = password;
    let mut show = use_signal(|| false);
    let is_default = *p.read() == FALKORDB_PASS;
    rsx! {
        div { class: "field-group",
            label { class: "field-label", "FalkorDB password" }
            div { class: "field-input-with-toggle",
                input {
                    class: "field-input",
                    r#type: if *show.read() { "text" } else { "password" },
                    value: "{p.read()}",
                    oninput: move |evt| p.set(evt.value().clone()),
                }
                button {
                    class: "field-toggle-btn",
                    r#type: "button",
                    onclick: move |_| {
                        let cur = *show.read();
                        show.set(!cur);
                    },
                    if *show.read() { "Hide" } else { "Show" }
                }
            }
            if is_default {
                p { class: "field-hint field-hint-warn",
                    "Default is the public sample value (agpassword123). "
                    "Change it if this host is ever exposed beyond localhost."
                }
            } else {
                p { class: "field-hint",
                    "On Start, the falkordb.service unit will be re-rendered "
                    "with this password and restarted; redis-cli ping will "
                    "verify the new password works."
                }
                button {
                    class: "field-link-btn",
                    r#type: "button",
                    onclick: move |_| on_reset_default.call(()),
                    "Reset to default"
                }
            }
        }
    }
}

#[component]
fn AgentModeField(mode: Signal<String>) -> Element {
    let mut m = mode;
    let current = m.read().clone();
    let desc = AGENT_MODES
        .iter()
        .find(|(k, _)| *k == current.as_str())
        .map(|(_, d)| *d)
        .unwrap_or("");

    rsx! {
        div { class: "field-group",
            label { class: "field-label", "Default agent mode" }
            select {
                class: "field-select",
                value: "{m.read()}",
                onchange: move |evt| m.set(evt.value().clone()),
                for (k, _) in AGENT_MODES.iter() {
                    option { key: "{k}", value: "{k}", "{k}" }
                }
            }
            p { class: "field-hint",
                "{desc} "
                em { "Informational only — selectable per-chat in the dashboard." }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct SecretFieldProps {
    label: &'static str,
    placeholder: &'static str,
    value: Signal<String>,
}

#[component]
fn SecretField(props: SecretFieldProps) -> Element {
    let mut v = props.value;
    let mut show = use_signal(|| false);
    rsx! {
        div { class: "field-group",
            label { class: "field-label", "{props.label}" }
            div { class: "field-input-with-toggle",
                input {
                    class: "field-input",
                    r#type: if *show.read() { "text" } else { "password" },
                    placeholder: "{props.placeholder}",
                    value: "{v.read()}",
                    oninput: move |evt| v.set(evt.value().clone()),
                }
                button {
                    class: "field-toggle-btn",
                    r#type: "button",
                    onclick: move |_| {
                        let cur = *show.read();
                        show.set(!cur);
                    },
                    if *show.read() { "Hide" } else { "Show" }
                }
            }
        }
    }
}

