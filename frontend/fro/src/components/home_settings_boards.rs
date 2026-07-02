use crate::api::{self, RagMemoryItem};
use crate::app::{ActiveCorpus, Route, ShowRagInfo};
use crate::components::BackendSelector;
use crate::pages::hardware::constants::INFO_ICON_SVG_CLASS;
use dioxus::prelude::*;
use dioxus_router::hooks::use_navigator;

/// Extracted settings boards (Runtime / Mode / RAG Add's / KV Cache) for the Home page.
/// Rendered unconditionally so they are always visible.
#[component]
pub fn HomeSettingsBoards(
    current_backend: Signal<String>,
    #[props(default)] on_backend_changed: Option<EventHandler<String>>,
    show_backend_info: Signal<bool>,
    chat_mode: Signal<String>,
    show_llm_info: Signal<bool>,
    show_auto_info: Signal<bool>,
    show_strict_info: Signal<bool>,
    show_agentic_info: Signal<bool>,
    show_upload_panel: Signal<bool>,
    show_delete_docs_modal: Signal<bool>,
    documents: Signal<Vec<String>>,
    upload_status: Signal<Option<String>>,
    show_delete_memories_modal: Signal<bool>,
    memories_loading: Signal<bool>,
    memory_error: Signal<Option<String>>,
    rag_memories: Signal<Vec<RagMemoryItem>>,
    show_info: Signal<ShowRagInfo>,
    prompt_caching_enabled: Signal<bool>,
    show_cache_info: Signal<bool>,
    show_tune_panel: Signal<bool>,
    show_tune_info: Signal<bool>,
    rag_priority_override: Signal<Option<f64>>,
    pointer_gap_threshold: Signal<f64>,
    selected_model: Signal<String>,
) -> Element {
    let navigator = use_navigator();
    let mut show_no_tools_msg = use_signal(|| false);
    let mut active_corpus = use_context::<Signal<ActiveCorpus>>();
    let mut corpora = use_signal(Vec::<api::CorpusEntry>::new);
    let mut show_corpus_info = use_signal(|| false);
    let mut show_new_corpus = use_signal(|| false);
    let mut new_corpus_slug = use_signal(String::new);
    let mut new_corpus_description = use_signal(String::new);
    let mut new_corpus_error = use_signal(|| Option::<String>::None);
    let mut show_delete_confirm = use_signal(|| false);
    let mut delete_error = use_signal(|| Option::<String>::None);
    let mut show_corpus_dropdown = use_signal(|| false);
    // use_resource re-runs when active_corpus changes, refreshing the list
    // so newly created corpora appear without a page reload.
    let _corpus_res = use_resource(move || async move {
        let _ = active_corpus.read().slug().to_string(); // reactive dep
        if let Ok(list) = api::fetch_corpora().await {
            corpora.set(list);
        }
    });

    let model_supports_tools = {
        let model = selected_model().to_lowercase();
        // Deny-list of model name prefixes known to lack tool-calling support.
        // Everything else defaults to supported (safe for new/unknown models).
        let no_tools: &[&str] = &["phi:latest", "phi:mini", "phi2", "phi:2", "phi-1", "phi-2"];
        !no_tools
            .iter()
            .any(|&bad| model == bad || model.starts_with(bad))
    };

    if !model_supports_tools && chat_mode() == "agentic" {
        chat_mode.set("auto".to_string());
    }

    rsx! {
        div {
            class: "fixed inset-x-0 z-10 pointer-events-none",
            style: "top: 3rem;",
            div {
                class: "max-w-2xl mx-auto w-full flex flex-col items-center",
                style: "padding-left: 0.5cm; padding-right: 0.5cm; width: calc(min(90vw, 34rem));",
                div {
                    class: "flex flex-col items-center w-full gap-4 pointer-events-auto",
                    style: "transform: translateY(1cm);",
                    p { class: "text-xs text-base-content/60 text-center", "Use these to set the starting context" }

                    // Row 1: Runtime + Mode side by side
                    div {
                        class: "flex justify-center gap-4 w-full",

                        // Runtime board
                        div {
                            class: "rounded-2xl px-5 py-4 flex flex-col items-center gap-3 pointer-events-auto",
                            style: "min-width: 12rem; background-color: rgba(34,211,238,0.10); border: 1px solid rgba(34,211,238,0.20);",
                            div {
                                class: "flex items-center gap-2",
                                label {
                                    class: "font-medium text-center",
                                    style: "color: white; font-size: 1.1rem;",
                                    "Runtime"
                                }
                                button {
                                    class: "shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80 pointer-events-auto",
                                    style: "width: 1.5rem; height: 1.5rem; min-width: 1.5rem; min-height: 1.5rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                    onclick: move |evt| {
                                        evt.stop_propagation();
                                        show_backend_info.set(true);
                                    },
                                    title: "Info about runtime selection",
                                    svg {
                                        class: INFO_ICON_SVG_CLASS,
                                        view_box: "0 0 20 20",
                                        fill: "none",
                                        stroke: "#026B7C",
                                        stroke_width: "1.5",
                                        circle { cx: "10", cy: "10", r: "9" }
                                        line { x1: "10", y1: "8", x2: "10", y2: "14" }
                                        circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                    }
                                }
                            }
                            BackendSelector {
                                current_backend: current_backend,
                                clear_model_on_change: true,
                                show_save_button: true,
                                show_info_button: false,
                                on_backend_changed: on_backend_changed,
                            }
                        }

                        // Mode board
                        div {
                            class: "rounded-2xl px-5 py-4 flex flex-col items-center gap-3 pointer-events-auto",
                            style: "padding-right: calc(1.25rem + 1.5cm); background-color: rgba(34,211,238,0.10); border: 1px solid rgba(34,211,238,0.20);",
                            div {
                                class: "flex items-center gap-3 w-full",
                                label {
                                    class: "font-medium shrink-0",
                                    style: "color: white; font-size: 1.1rem;",
                                    "Mode"
                                }
                                p {
                                    class: "text-xs font-medium",
                                    style: "color: #9ca3af;",
                                    match chat_mode().as_str() {
                                        "agentic" => "Agentic mode - LLM decides when to search, recall memory, or answer",
                                        "auto" => "Auto mode - prefers RAG, falls back to Hybrid",
                                        "ragstrict" => "Strict RAG - answers only from documents",
                                        "llm" => "LLM mode - uses AI without document search",
                                        _ => "Select a mode"
                                    }
                                }
                            }
                            div {
                                class: "flex justify-center",
                                div {
                                    class: "flex",
                                    style: "gap: 1.08rem;",
                                    // Auto mode button with info
                                    div {
                                        class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm rounded-lg px-3",
                                            style: if chat_mode() == "auto" {
                                                "background-color:#7C2A02; border-color:#7C2A02; color:white; box-shadow:none;"
                                            } else {
                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.3); color:white; box-shadow:none;"
                                            },
                                            onclick: move |_| { chat_mode.set("auto".to_string()); show_tune_panel.set(false); },
                                            title: "Auto: prefers RAG, falls back to Hybrid",
                                            span { style: "font-size: 0.75em;", "\u{2728}" }
                                            " Auto"
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_auto_info.set(true),
                                            title: "Info about Auto mode",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                    }

                                    // RAG Strict mode button with info
                                    div {
                                        class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm rounded-lg px-3",
                                            style: if chat_mode() == "ragstrict" {
                                                "background-color:#7C2A02; border-color:#7C2A02; color:white; box-shadow:none;"
                                            } else {
                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.3); color:white; box-shadow:none;"
                                            },
                                            onclick: move |_| { chat_mode.set("ragstrict".to_string()); show_tune_panel.set(false); },
                                            title: "Strict RAG: answers only from documents, says 'I don't know' otherwise",
                                            span { style: "font-size: 0.75em;", "\u{1F512}" }
                                            " Strict"
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_strict_info.set(true),
                                            title: "Info about Strict RAG mode",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                    }
                                    // LLM mode button with info
                                    div {
                                        class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm rounded-lg px-3",
                                            style: if chat_mode() == "llm" {
                                                "background-color:#7C2A02; border-color:#7C2A02; color:white; box-shadow:none;"
                                            } else {
                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.3); color:white; box-shadow:none;"
                                            },
                                            onclick: move |_| { chat_mode.set("llm".to_string()); show_tune_panel.set(false); },
                                            title: "Use LLM only (no document search)",
                                            span { style: "font-size: 0.75em;", "\u{1F916}" }
                                            " LLM"
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_llm_info.set(true),
                                            title: "Info about LLM mode",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                    }

                                    // Agentic mode button with info
                                    div {
                                        class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm rounded-lg px-3",
                                            style: if !model_supports_tools {
                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.15); color:rgba(255,255,255,0.35); box-shadow:none; cursor:not-allowed;"
                                            } else if chat_mode() == "agentic" {
                                                "background-color:#7C2A02; border-color:#7C2A02; color:white; box-shadow:none;"
                                            } else {
                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.3); color:white; box-shadow:none;"
                                            },
                                            onclick: move |_| {
                                                if model_supports_tools {
                                                    chat_mode.set("agentic".to_string());
                                                    show_no_tools_msg.set(false);
                                                    show_tune_panel.set(false);
                                                } else {
                                                    show_no_tools_msg.set(true);
                                                }
                                            },
                                            title: if model_supports_tools { "Agentic: LLM decides which tools to call in a loop" } else { "Current model does not support tool calling" },
                                            span { style: "font-size: 0.75em;", "\u{1F9E0}" }
                                            " Agent"
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_agentic_info.set(true),
                                            title: "Info about Agentic mode",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                    }

                                    // (Pointer mode button removed — its behavior is
                                    // reachable via Auto with the Pointer-trigger
                                    // slider at 0.0 / "Always Pointer".)

                                    // Tune button
                                    div {
                                        class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm rounded-lg px-3",
                                            style: if show_tune_panel() {
                                                "background-color:#7C2A02; border-color:#7C2A02; color:white; box-shadow:none;"
                                            } else {
                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.3); color:white; box-shadow:none;"
                                            },
                                            onclick: move |_| show_tune_panel.set(!show_tune_panel()),
                                            title: "Fine-tune RAG priority for this query",
                                            "\u{1F39A} Tune"
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_tune_info.set(true),
                                            title: "Info about Tune",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                    }
                                }
                            }
                            // Tune slider panel
                            if show_tune_panel() {
                                div {
                                    class: "flex items-center gap-2 mt-2 px-2 py-2 rounded-lg",
                                    style: "background-color: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1);",
                                    span {
                                        class: "text-xs font-medium shrink-0",
                                        style: "color: #9ca3af;",
                                        "LLM"
                                    }
                                    input {
                                        r#type: "range",
                                        min: "0",
                                        max: "100",
                                        step: "5",
                                        value: if let Some(v) = rag_priority_override() {
                                            format!("{}", (v * 100.0) as i32)
                                        } else {
                                            match chat_mode().as_str() {
                                                "llm" => "0".to_string(),
                                                "ragstrict" => "100".to_string(),
                                                _ => "50".to_string(),
                                            }
                                        },
                                        class: "flex-1",
                                        style: "accent-color: #7C2A02; height: 6px;",
                                        oninput: move |evt| {
                                            if let Ok(v) = evt.value().parse::<f64>() {
                                                rag_priority_override.set(Some(v / 100.0));
                                            }
                                        },
                                    }
                                    span {
                                        class: "text-xs font-medium shrink-0",
                                        style: "color: #9ca3af;",
                                        "Strict"
                                    }
                                    span {
                                        class: "text-xs font-medium shrink-0",
                                        style: "color: #e5e7eb; min-width: 5rem; text-align: right;",
                                        if let Some(v) = rag_priority_override() {
                                            {
                                                let label = if v <= 0.0 {
                                                    "LLM Only"
                                                } else if v < 0.3 {
                                                    "LLM-lean"
                                                } else if v < 0.7 {
                                                    "Balanced"
                                                } else if v < 1.0 {
                                                    "Doc-lean"
                                                } else {
                                                    "Docs Only"
                                                };
                                                format!("{label} ({:.2})", v)
                                            }
                                        } else {
                                            {
                                                match chat_mode().as_str() {
                                                    "llm" => "LLM Only (0.00)".to_string(),
                                                    "ragstrict" => "Docs Only (1.00)".to_string(),
                                                    "auto" => "Auto (0.50)".to_string(),
                                                    _ => "Balanced (0.50)".to_string(),
                                                }
                                            }
                                        }
                                    }
                                    button {
                                        class: "btn btn-xs rounded-lg px-3",
                                        style: "background-color:#1D6B9A; border: 1px solid #1D6B9A; color:white; box-shadow:none;",
                                        onclick: move |_| rag_priority_override.set(None),
                                        "Reset"
                                    }
                                }
                            }
                            // Pointer-trigger slider — surfaced when Auto is selected
                            // because Auto is the only branch that actually consults
                            // the threshold (gates `Auto → PointerRag` routing).
                            // Persists to POINTERRAG_AUTO_GAP_THRESHOLD via the runtime
                            // overrides API; hot-reloaded by the agent on next query.
                            // The info button opens the Auto modal — same shared content.
                            if chat_mode() == "auto" {
                                div {
                                    class: "flex items-center gap-2 mt-2 px-2 py-2 rounded-lg",
                                    style: "background-color: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1);",
                                    span {
                                        class: "text-xs font-medium shrink-0",
                                        style: "color: #9ca3af;",
                                        "Eager"
                                    }
                                    input {
                                        r#type: "range",
                                        min: "0",
                                        max: "100",
                                        step: "5",
                                        value: format!("{}", (pointer_gap_threshold() * 100.0) as i32),
                                        class: "flex-1",
                                        style: "accent-color: #7C2A02; height: 6px;",
                                        oninput: move |evt| {
                                            if let Ok(v) = evt.value().parse::<f64>() {
                                                let f = v / 100.0;
                                                pointer_gap_threshold.set(f);
                                                let value_str = format!("{f:.2}");
                                                spawn(async move {
                                                    let _ = crate::api::put_runtime_setting(
                                                        "POINTERRAG_AUTO_GAP_THRESHOLD",
                                                        Some(value_str),
                                                    ).await;
                                                });
                                            }
                                        },
                                    }
                                    span {
                                        class: "text-xs font-medium shrink-0",
                                        style: "color: #e5e7eb; min-width: 6.5rem; text-align: right;",
                                        {
                                            let v = pointer_gap_threshold();
                                            let label = if v < 0.35 {
                                                "Eager"
                                            } else if v < 1.0 {
                                                "Balanced"
                                            } else {
                                                "Never"
                                            };
                                            format!("Pointer trigger: {label} ({:.2})", v)
                                        }
                                    }
                                    button {
                                        class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                        style: "width: 1.5rem; height: 1.5rem; min-width: 1.5rem; min-height: 1.5rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                        onclick: move |_| show_auto_info.set(true),
                                        title: "Info on Auto mode and the Pointer trigger threshold",
                                        svg {
                                            class: INFO_ICON_SVG_CLASS,
                                            view_box: "0 0 20 20",
                                            fill: "none",
                                            stroke: "#026B7C",
                                            circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                            line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                            circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                        }
                                    }
                                    button {
                                        class: "btn btn-xs rounded-lg px-3",
                                        style: "background-color:#1D6B9A; border: 1px solid #1D6B9A; color:white; box-shadow:none;",
                                        onclick: move |_| {
                                            pointer_gap_threshold.set(0.5);
                                            spawn(async move {
                                                let _ = crate::api::put_runtime_setting(
                                                    "POINTERRAG_AUTO_GAP_THRESHOLD",
                                                    Some("0.50".to_string()),
                                                ).await;
                                            });
                                        },
                                        "Reset"
                                    }
                                }
                            }
                            if show_no_tools_msg() {
                                p {
                                    class: "text-xs text-center mt-1",
                                    style: "color: #f59e0b;",
                                    "\u{26A0} Current model ({selected_model}) lacks tool-calling support. Switch to phi3.5, qwen2.5, or llama3."
                                }
                            }
                            if chat_mode() != "llm" {
                                p {
                                    class: "text-base font-medium text-center mt-1",
                                    style: "color: white;",
                                    "Corpus"
                                }
                                div { class: "w-full relative",
                                    // Trigger: shows active corpus, click to expand
                                    button {
                                        class: "w-full flex items-center justify-between gap-2 px-2 py-1 rounded",
                                        style: "background-color: rgba(124,42,2,0.35); border: 1px solid rgba(124,42,2,0.6); color: white;",
                                        onclick: move |_| show_corpus_dropdown.set(!show_corpus_dropdown()),
                                        span { class: "text-xs font-mono", "{active_corpus.read().slug()}" }
                                        span { class: "text-xs opacity-60", if show_corpus_dropdown() { "▲" } else { "▼" } }
                                    }
                                    // Expanded list
                                    if show_corpus_dropdown() {
                                        div {
                                            class: "absolute left-0 right-0 mt-1 flex flex-col gap-0.5 z-20 p-1 rounded-lg",
                                            style: "background-color: #1f2937; border: 1px solid rgba(255,255,255,0.12);",
                                            for corpus in corpora.read().clone() {
                                                {
                                                    let slug_sel = corpus.slug.clone();
                                                    let is_active = corpus.slug == active_corpus.read().slug();
                                                    rsx! {
                                                        div {
                                                            class: "flex items-center px-2 py-1 rounded cursor-pointer",
                                                            style: if is_active {
                                                                "background-color: rgba(124,42,2,0.35); border: 1px solid rgba(124,42,2,0.6);"
                                                            } else {
                                                                "background-color: transparent; border: 1px solid transparent;"
                                                            },
                                                            onclick: move |_| {
                                                                active_corpus.with_mut(|ac| ac.0 = slug_sel.clone());
                                                                show_corpus_dropdown.set(false);
                                                            },
                                                            span {
                                                                class: "text-xs font-mono",
                                                                style: if is_active { "color: white;" } else { "color: #d1d5db;" },
                                                                "{corpus.slug}"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Row 2: Corpus + RAG Add's side by side
                    div {
                        class: "flex justify-center gap-4 w-full",

                        // Corpus board — only shown when a RAG mode is active
                        if chat_mode() != "llm" {
                        div {
                            class: "rounded-2xl px-5 py-4 flex flex-col items-center gap-3 pointer-events-auto",
                            style: "min-width: 12rem; background-color: rgba(34,211,238,0.10); border: 1px solid rgba(34,211,238,0.20);",
                            div {
                                class: "flex items-center gap-2",
                                label {
                                    class: "font-medium text-center",
                                    style: "color: white; font-size: 1.1rem;",
                                    "Corpus"
                                }
                                button {
                                    class: "shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80 pointer-events-auto",
                                    style: "width: 1.5rem; height: 1.5rem; min-width: 1.5rem; min-height: 1.5rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                    onclick: move |evt| { evt.stop_propagation(); show_corpus_info.set(true); },
                                    title: "Info about corpora",
                                    svg {
                                        class: INFO_ICON_SVG_CLASS,
                                        view_box: "0 0 20 20",
                                        fill: "none",
                                        stroke: "#026B7C",
                                        stroke_width: "1.5",
                                        circle { cx: "10", cy: "10", r: "9" }
                                        line { x1: "10", y1: "8", x2: "10", y2: "14" }
                                        circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                    }
                                }
                            }
                            if show_corpus_info() {
                                div {
                                    class: "fixed inset-0 z-50 flex items-center justify-center",
                                    style: "background: rgba(0,0,0,0.6);",
                                    onclick: move |_| show_corpus_info.set(false),
                                    div {
                                        class: "bg-base-200 rounded-2xl p-6 max-w-sm w-full mx-4 text-left",
                                        onclick: move |evt| evt.stop_propagation(),
                                        h3 { class: "text-lg font-bold mb-3", "Corpus" }
                                        p { class: "text-sm text-gray-300 mb-3",
                                            "A corpus is a named, isolated collection of documents. Each corpus has its own Tantivy index, upload directory, and vector store — so documents in one corpus never pollute search results in another."
                                        }
                                        h4 { class: "text-sm font-semibold text-gray-100 mb-1", "Active corpus" }
                                        p { class: "text-sm text-gray-300 mb-3",
                                            "The active corpus is the one used by the chat window on the home page for retrieval. It is highlighted with an orange border. To switch, create a second corpus and click \"Use\" on the one you want to activate. The selection persists in your browser session."
                                        }
                                        h4 { class: "text-sm font-semibold text-gray-100 mb-1", "default" }
                                        p { class: "text-sm text-gray-300 mb-3",
                                            "The default corpus always exists and cannot be deleted. It maps to the same Tantivy index and upload dir that existed before corpora were introduced, so existing documents are automatically in it."
                                        }
                                        h4 { class: "text-sm font-semibold text-gray-100 mb-1", "Slug rules" }
                                        p { class: "text-sm text-gray-300 mb-4",
                                            "Slugs are 1–64 characters, lowercase alphanumeric and hyphens, starting and ending with an alphanumeric character. The slug is permanent — rename only changes the display name."
                                        }
                                        button {
                                            class: "btn btn-sm w-full",
                                            style: "background-color:#7C2A02; border-color:#7C2A02; color:white;",
                                            onclick: move |_| show_corpus_info.set(false),
                                            "Got it"
                                        }
                                    }
                                }
                            }
                            div { class: "flex gap-1",
                                button {
                                    class: "btn rounded-full px-4 text-xl font-bold",
                                    style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none;",
                                    onclick: move |_| {
                                        new_corpus_error.set(None);
                                        show_new_corpus.set(!show_new_corpus());
                                    },
                                    title: "New corpus",
                                    "+"
                                }
                                button {
                                    class: "btn rounded-full px-4 text-xl font-bold",
                                    style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none;",
                                    onclick: move |_| {
                                        delete_error.set(None);
                                        show_delete_confirm.set(true);
                                    },
                                    title: "Delete corpus",
                                    "-"
                                }
                                button {
                                    class: "btn rounded-full px-4 text-xl font-bold",
                                    style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none;",
                                    onclick: move |_| { navigator.push(Route::ConfigCorpus {}); },
                                    title: "Manage corpora",
                                    "≡"
                                }
                            }
                            if show_new_corpus() {
                                div { class: "flex flex-col gap-1.5 w-full mt-1",
                                    input {
                                        class: "input input-sm input-bordered bg-gray-700 text-gray-200 w-full",
                                        placeholder: "slug (e.g. research)",
                                        value: "{new_corpus_slug}",
                                        oninput: move |evt| {
                                            new_corpus_slug.set(evt.value());
                                            new_corpus_error.set(None);
                                        },
                                    }
                                    input {
                                        class: "input input-sm input-bordered bg-gray-700 text-gray-200 w-full",
                                        placeholder: "description (optional)",
                                        value: "{new_corpus_description}",
                                        oninput: move |evt| new_corpus_description.set(evt.value()),
                                    }
                                    if let Some(err) = new_corpus_error.read().as_ref() {
                                        p { class: "text-xs text-red-400", "{err}" }
                                    }
                                    div { class: "flex gap-1.5",
                                        button {
                                            class: "btn btn-sm flex-1",
                                            style: "background-color:#7C2A02;border-color:#7C2A02;color:white;",
                                            onclick: move |_| {
                                                let slug = new_corpus_slug.read().trim().to_string();
                                                let desc = new_corpus_description.read().trim().to_string();
                                                if slug.is_empty() {
                                                    new_corpus_error.set(Some("Slug required".into()));
                                                    return;
                                                }
                                                spawn(async move {
                                                    match api::create_corpus(&slug, &slug, &desc).await {
                                                        Ok(_) => {
                                                            if let Ok(list) = api::fetch_corpora().await {
                                                                corpora.set(list);
                                                            }
                                                            new_corpus_slug.set(String::new());
                                                            new_corpus_description.set(String::new());
                                                            show_new_corpus.set(false);
                                                        }
                                                        Err(e) => new_corpus_error.set(Some(e)),
                                                    }
                                                });
                                            },
                                            "Create"
                                        }
                                        button {
                                            class: "btn btn-sm btn-ghost text-gray-400",
                                            onclick: move |_| {
                                                show_new_corpus.set(false);
                                                new_corpus_slug.set(String::new());
                                                new_corpus_description.set(String::new());
                                                new_corpus_error.set(None);
                                            },
                                            "✕"
                                        }
                                    }
                                }
                            }
                        }

                        // Delete confirmation modal
                        if show_delete_confirm() {
                            div {
                                class: "fixed inset-0 z-50 flex items-center justify-center",
                                style: "background: rgba(0,0,0,0.6);",
                                onclick: move |_| show_delete_confirm.set(false),
                                div {
                                    class: "bg-base-200 rounded-2xl p-6 max-w-sm w-full mx-4 text-left",
                                    onclick: move |evt| evt.stop_propagation(),
                                    h3 { class: "text-base font-bold mb-2 text-white", "Delete corpus?" }
                                    p { class: "text-sm text-gray-300 mb-1",
                                        "This will permanently delete "
                                        span { class: "font-mono text-red-300", "{active_corpus.read().slug()}" }
                                        " and all its documents. This cannot be undone."
                                    }
                                    if let Some(err) = delete_error.read().as_ref() {
                                        p { class: "text-xs text-red-400 mb-2", "{err}" }
                                    }
                                    div { class: "flex gap-2 mt-4",
                                        button {
                                            class: "btn btn-sm flex-1",
                                            style: "background-color:#991b1b;border-color:#991b1b;color:white;",
                                            onclick: move |_| {
                                                let slug = active_corpus.read().slug().to_string();
                                                spawn(async move {
                                                    match api::delete_corpus(&slug).await {
                                                        Ok(_) => {
                                                            active_corpus.with_mut(|ac| ac.0 = "default".to_string());
                                                            if let Ok(list) = api::fetch_corpora().await {
                                                                corpora.set(list);
                                                            }
                                                            show_delete_confirm.set(false);
                                                        }
                                                        Err(e) => delete_error.set(Some(e)),
                                                    }
                                                });
                                            },
                                            "Yes, delete"
                                        }
                                        button {
                                            class: "btn btn-sm flex-1 btn-ghost text-gray-300",
                                            onclick: move |_| show_delete_confirm.set(false),
                                            "Cancel"
                                        }
                                    }
                                }
                            }
                        }

                        } // end corpus board if chat_mode != llm

                        // RAG Add's board
                        div {
                            class: "rounded-2xl px-5 py-4 flex flex-col items-center gap-3 pointer-events-auto",
                            style: "min-width: calc(12rem + 2cm); padding-left: calc(1.25rem + 1cm); padding-right: calc(1.25rem + 1cm); background-color: rgba(34,211,238,0.10); border: 1px solid rgba(34,211,238,0.20);",
                            label {
                                class: "font-medium text-center",
                                style: "color: white; font-size: 1.1rem;",
                                "RAG Add's"
                            }
                            div {
                                class: "flex justify-center",
                                style: "gap: 1.08rem;",
                                // Documents buttons
                                div {
                                    class: "flex flex-col items-center",
                                    style: "width: 7.5rem;",
                                    div { class: "flex gap-1",
                                        // Info (standard styling)
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 2rem; height: 2rem; min-width: 2rem; min-height: 2rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |evt| {
                                                evt.stop_propagation();
                                                show_info.set(ShowRagInfo(true));
                                            },
                                            title: "Info about documents",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                        button {
                                            class: "btn rounded-full px-4 text-xl font-bold",
                                            style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none;",
                                            onclick: move |_| show_upload_panel.set(!show_upload_panel()),
                                            title: "Toggle documents panel",
                                            "+"
                                        }
                                        button {
                                            class: "btn rounded-full px-4 text-xl font-bold text-white",
                                            style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none;",
                                            onclick: move |_| {
                                                show_delete_docs_modal.set(true);
                                                spawn(async move {
                                                    match api::list_documents().await {
                                                        Ok(mut resp) => {
                                                            resp.documents.sort();
                                                            documents.set(resp.documents);
                                                        }
                                                        Err(e) => upload_status.set(Some(format!("Failed to load: {}", e))),
                                                    }
                                                });
                                            },
                                            title: "Delete documents",
                                            "-"
                                        }
                                    }
                                    span {
                                        class: "text-sm mt-1 font-medium",
                                        style: "color: white;",
                                        "Documents"
                                    }
                                }
                                // Memories buttons
                                div {
                                    class: "flex flex-col items-center",
                                    style: "width: 7.5rem;",
                                    div { class: "flex gap-1",
                                        a {
                                            class: "btn rounded-full px-4 text-xl font-bold cursor-pointer",
                                            style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none; text-decoration: none;",
                                            href: "/config/memories",
                                            title: "Add RAG memories",
                                            "+"
                                        }
                                        button {
                                            class: "btn rounded-full px-4 text-xl font-bold text-white",
                                            style: "border: 1.5px solid rgba(255,255,255,0.3); background: transparent; color: white; min-height: 1.875rem; height: 1.875rem; box-shadow: none;",
                                            onclick: move |_| {
                                                show_delete_memories_modal.set(true);
                                                memories_loading.set(true);
                                                memory_error.set(None);
                                                spawn(async move {
                                                    match api::fetch_rag_memories(100).await {
                                                        Ok(resp) => rag_memories.set(resp.memories),
                                                        Err(e) => memory_error.set(Some(e)),
                                                    }
                                                    memories_loading.set(false);
                                                });
                                            },
                                            title: "Delete memories",
                                            "-"
                                        }
                                        // Info (standard styling)
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 2rem; height: 2rem; min-width: 2rem; min-height: 2rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |evt| {
                                                evt.stop_propagation();
                                                show_info.set(ShowRagInfo(true));
                                            },
                                            title: "Info about memories",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                    }
                                    span {
                                        class: "text-sm mt-1 font-medium",
                                        style: "color: white;",
                                        "Memories"
                                    }
                                }
                            }
                        }

                        // KV Cache board (moved from Row 3)
                        div {
                            class: "rounded-2xl px-5 py-4 flex flex-col items-center gap-2",
                            style: "background-color: rgba(34,211,238,0.10); border: 1px solid rgba(34,211,238,0.20);",
                            label {
                                class: "font-medium text-center",
                                style: "color: white; font-size: 1.1rem;",
                                "KV Cache"
                            }
                            div {
                                class: "flex items-center justify-center gap-6 w-full",
                                div {
                                    class: "flex flex-col items-center gap-1",
                                    div {
                                        class: "flex items-center gap-2",
                                        span {
                                            class: "text-sm font-medium",
                                            style: "color: white;",
                                            "KV Cache"
                                        }
                                        label {
                                            class: "flex items-center gap-2 cursor-pointer pointer-events-auto",
                                            input {
                                                r#type: "checkbox",
                                                class: "toggle toggle-sm !border !border-white",
                                                style: {
                                                    format!(
                                                        "border: 1px solid white; background-color: {};",
                                                        if prompt_caching_enabled() { "" } else { "#d1d5db" }
                                                    )
                                                },
                                                checked: prompt_caching_enabled(),
                                                onchange: move |evt| {
                                                    let new_value = evt.checked();
                                                    prompt_caching_enabled.set(new_value);
                                                    spawn(async move {
                                                        let _ = api::set_prompt_caching(new_value).await;
                                                    });
                                                }
                                            }
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer pointer-events-auto",
                                            style: "width: 1.5rem; height: 1.5rem; min-width: 1.5rem; min-height: 1.5rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_cache_info.set(true),
                                            title: "Info about KV caching",
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "#026B7C",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }
                                            }
                                        }
                                    }
                                    p {
                                        class: "text-xs text-center",
                                        style: if prompt_caching_enabled() {
                                            "color: #22c55e;"
                                        } else {
                                            "color: rgba(255,255,255,0.5);"
                                        },
                                        if prompt_caching_enabled() {
                                            "KV Cache enabled"
                                        } else {
                                            "KV Cache disabled"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
