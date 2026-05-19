//! Speed (ONNX Embedding) Status Component
//! Shows ONNX embedding status - no toggle needed since ONNX is the only provider

use crate::api;
use crate::pages::hardware::constants::INFO_ICON_SVG_CLASS;
use dioxus::prelude::*;

#[component]
pub fn EmbeddingToggle() -> Element {
    let mut status = use_signal(|| None::<String>);
    let mut model_path = use_signal(String::new);
    let mut ready = use_signal(|| false);
    let mut loading = use_signal(|| true);
    let mut show_info = use_signal(|| false);

    // Fetch config on mount
    use_effect(move || {
        spawn(async move {
            loading.set(true);
            match api::fetch_embedding_config().await {
                Ok(cfg) => {
                    ready.set(cfg.onnx.ready);
                    model_path.set(cfg.onnx.model_path);
                    status.set(None);
                }
                Err(e) => {
                    status.set(Some(e));
                }
            }
            loading.set(false);
        });
    });

    rsx! {
        // Title
        label {
            class: "font-medium text-center",
            style: "color: white; font-size: 1.1rem;",
            "⚡ Speed"
        }

        // Content
        div {
            class: "flex flex-col items-center gap-2",

            // Loading state
            if loading() {
                span {
                    class: "loading loading-spinner loading-xs",
                    style: "color: white;"
                }
            } else if let Some(err) = status() {
                // Error state
                p {
                    class: "text-xs text-center",
                    style: "color: #ef4444;",
                    "{err}"
                }
            } else {
                // ONNX status
                div {
                    class: "flex items-center gap-2",

                    // Status badge
                    div {
                        class: "flex items-center gap-1",
                        style: if ready() {
                            "background: white; color: black; padding: 0.25rem 0.75rem; border-radius: 0.5rem;"
                        } else {
                            "background: transparent; color: #ef4444; padding: 0.25rem 0.75rem; border-radius: 0.5rem; border: 1px solid #ef4444;"
                        },
                        span {
                            class: "text-sm font-medium",
                            if ready() { "ONNX ⚡" } else { "ONNX ⚠" }
                        }
                    }

                    // Info button
                    button {
                        class: "shrink-0 rounded flex items-center justify-center cursor-pointer pointer-events-auto",
                        style: "width: 1.5rem; height: 1.5rem; min-width: 1.5rem; min-height: 1.5rem; background-color: #1D6B9A; border: 1px solid #1D6B9A;",
                        onclick: move |_| show_info.set(true),
                        title: "Info about ONNX embeddings",
                        svg {
                            class: INFO_ICON_SVG_CLASS,
                            view_box: "0 0 20 20",
                            fill: "none",
                            stroke: "currentColor",
                            circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                            line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                            circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                        }
                    }
                }

                // Status text
                p {
                    class: "text-xs text-center",
                    style: if ready() {
                        "color: #22c55e;"
                    } else {
                        "color: #ef4444;"
                    },
                    if ready() {
                        "Optimized embeddings active"
                    } else {
                        "Model not found"
                    }
                }
            }
        }

        // Info Modal
        if show_info() {
            div {
                class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
                onclick: move |_| show_info.set(false),

                div {
                    class: "bg-base-100 rounded-lg p-5 max-w-sm mx-4 shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),

                    div {
                        class: "flex justify-between items-center mb-3",
                        h3 { class: "text-base font-bold", "⚡ ONNX Embeddings" }
                        button {
                            class: "btn btn-ghost btn-xs",
                            onclick: move |_| show_info.set(false),
                            "✕"
                        }
                    }

                    div {
                        class: "space-y-3 text-sm",
                        p {
                            strong { "What is this?" }
                            br {}
                            "Embeddings convert your text into numbers that the AI uses for search and understanding."
                        }
                        p {
                            strong { "Why ONNX?" }
                            br {}
                            "ONNX Runtime is a highly optimized engine that runs 2-10x faster than alternatives, with the same accuracy."
                        }
                        div {
                            class: "bg-base-200 rounded p-2",
                            p { class: "text-success font-semibold text-xs", "✓ Benefits" }
                            ul {
                                class: "list-disc list-inside ml-2 text-xs",
                                li { "2-10x faster processing" }
                                li { "Lower memory usage" }
                                li { "Production optimized" }
                                li { "Same accuracy as standard" }
                            }
                        }
                        p {
                            class: "text-xs text-base-content/70",
                            "Model: {model_path}"
                        }
                    }

                    button {
                        class: "btn btn-primary btn-sm mt-4 w-full",
                        onclick: move |_| show_info.set(false),
                        "Got it!"
                    }
                }
            }
        }
    }
}
