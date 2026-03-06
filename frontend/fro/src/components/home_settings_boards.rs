use crate::api::{self, RagMemoryItem};
use crate::app::ShowRagInfo;
use crate::components::BackendSelector;
use crate::pages::hardware::constants::INFO_ICON_SVG_CLASS;
use dioxus::prelude::*;

/// Extracted settings boards (Runtime / Mode / RAG Add's / KV Cache) for the Home page.
/// Rendered unconditionally so they are always visible.
#[component]
pub fn HomeSettingsBoards(
    current_backend: Signal<String>,
    show_backend_info: Signal<bool>,
    chat_mode: Signal<String>,
    show_rag_info: Signal<bool>,
    show_llm_info: Signal<bool>,
    show_hybrid_info: Signal<bool>,
    show_auto_info: Signal<bool>,
    show_strict_info: Signal<bool>,
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
) -> Element {
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
                            class: "bg-white/5 border border-white/10 rounded-2xl px-5 py-4 flex flex-col items-center gap-3 pointer-events-auto",
                            style: "min-width: 12rem;",
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
                            }
                        }

                        // Mode board
                        div {
                            class: "bg-white/5 border border-white/10 rounded-2xl px-5 py-4 flex flex-col items-center gap-3 pointer-events-auto",
                            label {
                                class: "font-medium text-center",
                                style: "color: white; font-size: 1.1rem;",
                                "Mode"
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
                                            onclick: move |_| chat_mode.set("auto".to_string()),
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
                                    // RAG mode button with info
                                    div {
                                        class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm rounded-lg px-3",
                                            style: if chat_mode() == "rag" {
                                                "background-color:#7C2A02; border-color:#7C2A02; color:white; box-shadow:none;"
                                            } else {
                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.3); color:white; box-shadow:none;"
                                            },
                                            onclick: move |_| chat_mode.set("rag".to_string()),
                                            title: "Search documents only",
                                            span { style: "font-size: 0.75em;", "\u{1F4DA}" }
                                            " RAG"
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_rag_info.set(true),
                                            title: "Info about RAG mode",
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
                                            onclick: move |_| chat_mode.set("ragstrict".to_string()),
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
                                            onclick: move |_| chat_mode.set("llm".to_string()),
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
                                    // Hybrid mode button with info
                                    div {
                                        class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm rounded-lg px-3",
                                            style: if chat_mode() == "hybrid" {
                                                "background-color:#7C2A02; border-color:#7C2A02; color:white; box-shadow:none;"
                                            } else {
                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.3); color:white; box-shadow:none;"
                                            },
                                            onclick: move |_| chat_mode.set("hybrid".to_string()),
                                            title: "Search documents + LLM enhancement",
                                            span { style: "font-size: 0.75em;", "\u{26A1}" }
                                            " Hybrid"
                                        }
                                        button {
                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                                            style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: transparent; border: 1.5px solid #026B7C;",
                                            onclick: move |_| show_hybrid_info.set(true),
                                            title: "Info about Hybrid mode",
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
                            p {
                                class: "text-xs font-medium text-center",
                                style: "color: white;",
                                match chat_mode().as_str() {
                                    "auto" => "Auto mode - prefers RAG, falls back to Hybrid",
                                    "rag" => "RAG mode - searches your documents only",
                                    "ragstrict" => "Strict RAG - answers only from documents",
                                    "llm" => "LLM mode - uses AI without document search",
                                    "hybrid" => "Hybrid mode - documents + AI fallback",
                                    _ => "Select a mode"
                                }
                            }
                        }
                    }

                    // Row 2: RAG Add's (own row)
                    div {
                        class: "flex justify-center gap-4 w-full",
                        div {
                            class: "bg-white/5 border border-white/10 rounded-2xl px-5 py-4 flex flex-col items-center gap-3 pointer-events-auto",
                            style: "min-width: calc(12rem + 2cm); padding-left: calc(1.25rem + 1cm); padding-right: calc(1.25rem + 1cm);",
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
                    }

                    // Row 3: KV Cache
                    div {
                        class: "flex justify-center gap-4 w-full pointer-events-auto",
                        style: "margin-top: 1cm;",
                        div {
                            class: "bg-white/5 border border-white/10 rounded-2xl px-5 py-4 flex flex-col items-center gap-2",
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
