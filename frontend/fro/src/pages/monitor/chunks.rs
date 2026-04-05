use crate::{
    api,
    app::Route,
    components::monitor::*,
    pages::hardware::constants::{
        PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE, INFO_ICON_SVG_CLASS,
    },
};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

#[component]
fn InfoIcon() -> Element {
    rsx! {
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

#[component]
pub fn MonitorChunks() -> Element {
    let mut tokenizer = use_signal(|| None::<api::TokenizerInfo>);
    let mut stats = use_signal(|| None::<Vec<api::ChunkingStatsSnapshot>>);
    let mut loading = use_signal(|| true);
    let mut show_info = use_signal(|| false);
    let mut show_shimmytok = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    use_future(move || async move {
        loop {
            // Fetch both in parallel
            let (tok_res, stats_res) = futures_util::join!(
                api::fetch_tokenizer_info(),
                api::fetch_chunking_stats(20),
            );

            if let Ok(tok) = tok_res {
                tokenizer.set(Some(tok));
            }
            match stats_res {
                Ok(resp) => {
                    stats.set(Some(resp.snapshots));
                    error.set(None);
                }
                Err(e) => error.set(Some(e)),
            }
            loading.set(false);
            TimeoutFuture::new(10_000).await;
        }
    });

    let tok = tokenizer();
    let tok_model = tok.as_ref().map(|t| t.model.clone()).unwrap_or_default();
    let tok_exact = tok.as_ref().map(|t| t.is_exact).unwrap_or(false);
    let tok_vocab = tok.as_ref().map(|t| t.vocab_size).unwrap_or(0);

    // Pre-compute tokenizer mismatch outside RSX
    let mismatch_models: String = stats()
        .as_ref()
        .map(|snaps| {
            let mut seen = std::collections::BTreeSet::new();
            for s in snaps {
                if let Some(ref m) = s.tokenizer_model {
                    if !tok_model.is_empty() && m != &tok_model {
                        seen.insert(m.clone());
                    }
                }
            }
            seen.into_iter().collect::<Vec<_>>().join(", ")
        })
        .unwrap_or_default();
    let has_mismatch = !mismatch_models.is_empty();

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Chunks", None),
                ],
            }

            NavTabs { active: Route::MonitorChunks {} }

            // Tokenizer status board
            Panel { title: None, refresh: None,
                div { class: "flex items-center gap-2 mb-3",
                    h3 { class: "text-sm font-semibold text-gray-200", "Token Counter" }
                    button {
                        class: PARAM_ICON_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        onclick: move |_| show_info.set(true),
                        title: "Token counter help",
                        InfoIcon {}
                    }
                }
                div { class: "flex flex-wrap gap-6 text-sm",
                    div { class: "flex flex-col gap-1",
                        span { class: "text-gray-400 text-xs", "Model" }
                        span { class: "text-gray-200 font-medium", "{tok_model}" }
                    }
                    div { class: "flex flex-col gap-1",
                        span { class: "text-gray-400 text-xs", "Vocab size" }
                        span { class: "text-gray-200 font-medium",
                            if tok_vocab > 0 {
                                "{tok_vocab}"
                            } else {
                                "N/A"
                            }
                        }
                    }
                    div { class: "flex flex-col gap-1",
                        span { class: "text-gray-400 text-xs", "Counting method" }
                        span {
                            class: if tok_exact { "text-green-400 font-medium" } else { "text-yellow-400 font-medium" },
                            if tok_exact { "Exact (GGUF)" } else { "Heuristic (approx)" }
                        }
                    }
                }
            }

            // Tokenizer mismatch warning
            if has_mismatch {
                Panel { title: None, refresh: None,
                    div { class: "flex items-start gap-3 p-3 rounded-lg",
                        style: "background-color: rgba(234,179,8,0.1); border: 1px solid rgba(234,179,8,0.3);",
                        span { class: "text-yellow-400 text-lg", "⚠" }
                        div { class: "text-sm text-yellow-300",
                            p { class: "font-medium mb-1",
                                "Tokenizer mismatch detected"
                            }
                            p { class: "text-yellow-400/80",
                                "Some chunks were indexed with a different tokenizer ({mismatch_models}) than the currently active one ({tok_model}). Token counts may be inaccurate. Consider re-indexing."
                            }
                        }
                    }
                }
            }

            // Chunking history
            Panel { title: Some("Recent Chunking Operations".into()), refresh: None,
                if loading() {
                    div { class: "text-sm text-gray-400", "Loading..." }
                } else if let Some(err) = error() {
                    div { class: "text-sm text-red-400", "{err}" }
                } else if let Some(snaps) = stats() {
                    if snaps.is_empty() {
                        div { class: "text-sm text-gray-400", "No chunking operations recorded yet. Upload a document to see stats." }
                    } else {
                        div { class: "overflow-x-auto",
                            table { class: "table table-xs w-full text-gray-300",
                                thead {
                                    tr {
                                        th { class: "text-gray-400", "Time" }
                                        th { class: "text-gray-400", "File" }
                                        th { class: "text-gray-400", "Mode" }
                                        th { class: "text-gray-400 text-right", "Chunks" }
                                        th { class: "text-gray-400 text-right", "Tokens" }
                                        th { class: "text-gray-400 text-right", "Duration" }
                                        th { class: "text-gray-400", "Format" }
                                        th { class: "text-gray-400", "Strategy" }
                                        th { class: "text-gray-400", "Tokenizer" }
                                    }
                                }
                                tbody {
                                    for snap in snaps.iter() {
                                        {
                                            let time_short = if snap.recorded_at.len() > 19 {
                                                &snap.recorded_at[11..19]
                                            } else {
                                                &snap.recorded_at
                                            };
                                            let file_short = snap.file.rsplit('/').next().unwrap_or(&snap.file);
                                            let detected_fmt = snap.detection.as_ref()
                                                .map(|d| d.detected_format.clone())
                                                .unwrap_or_default();
                                            let strategy = snap.detection.as_ref()
                                                .map(|d| d.chosen_strategy.clone())
                                                .unwrap_or_default();
                                            rsx! {
                                                tr { class: "hover:bg-gray-800/50",
                                                    td { class: "font-mono text-xs", "{time_short}" }
                                                    td { class: "max-w-48 truncate", title: "{snap.file}", "{file_short}" }
                                                    td { "{snap.chunker_mode}" }
                                                    td { class: "text-right", "{snap.chunks}" }
                                                    td { class: "text-right", "{snap.tokens}" }
                                                    td { class: "text-right", "{snap.duration_ms}ms" }
                                                    td { class: "text-xs", "{detected_fmt}" }
                                                    td { class: "text-xs", "{strategy}" }
                                                    {
                                                        let snap_tok = snap.tokenizer_model.as_deref().unwrap_or("unknown");
                                                        let matches_active = tok_model.is_empty() || snap_tok == tok_model;
                                                        let color_cls = if snap_tok == "unknown" {
                                                            "text-gray-500 text-xs"
                                                        } else if matches_active {
                                                            "text-green-400 text-xs"
                                                        } else {
                                                            "text-yellow-400 text-xs"
                                                        };
                                                        let icon = if snap_tok == "unknown" {
                                                            "●"
                                                        } else if matches_active {
                                                            "●"
                                                        } else {
                                                            "⚠"
                                                        };
                                                        rsx! {
                                                            td { class: "{color_cls}",
                                                                title: "{snap_tok}",
                                                                "{icon} {snap_tok}"
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
                }
            }

            // Token counter info modal
            if show_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-[90vw] max-h-[95vh] overflow-y-auto shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4",
                            h2 { class: "text-lg font-semibold text-gray-100", "Token Counter" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_info.set(false),
                                "x"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-3",
                            p { "The token counter measures how many tokens each chunk contains. Accurate token counts are essential for staying within LLM context windows and for fair chunk-size comparisons." }
                            p { "AG supports two counting methods:" }
                            p {
                                "Exact (GGUF): Loads the vocabulary from your active LLM's GGUF file via "
                                span {
                                    class: "text-blue-400 underline cursor-pointer hover:text-blue-300",
                                    onclick: move |_| show_shimmytok.set(!show_shimmytok()),
                                    "shimmytok"
                                }
                                ". Token counts match exactly what the model sees. This is the preferred method."
                            }
                            if show_shimmytok() {
                                div { class: "ml-2 p-3 rounded-lg text-xs text-gray-400 space-y-2",
                                    style: "background-color: rgba(96,165,250,0.08); border-left: 2px solid #60a5fa;",
                                    p { "shimmytok is a pure Rust tokenizer that reads the vocabulary directly from a GGUF model file. It's the companion tokenizer used by the Rust LLM runtime shimmy, and it removes the need for llama.cpp or external SentencePiece/BPE files." }
                                    p { class: "font-semibold text-gray-300 mt-2", "What shimmytok actually is" }
                                    p { "shimmytok is:" }
                                    p { "- A pure Rust tokenizer (no C++, no Python, no external libs)" }
                                    p { "- GGUF-native - it loads the tokenizer directly from the model.gguf" }
                                    p { "- llama.cpp-compatible - outputs identical token IDs" }
                                    p { "- Supports LLaMA, Mistral, Phi-3, Qwen2, Gemma and more" }
                                    p { "- MIT-licensed and designed to stay free forever" }
                                    p { class: "mt-2", "This means: If your active LLM is a GGUF model, shimmytok can read its tokenizer straight from the same file, without needing .model, .spm, or .tokenizer.json." }
                                }
                            }
                            p { "Heuristic: A fast approximation (roughly 1 token per 4 characters). Used when no GGUF file is available, for example with cloud backends." }
                            p { "When you switch models, the token counter automatically reloads with the new model's vocabulary. Chunks indexed under the old model keep their original token counts. The mismatch warning tells you when this has happened - token counts shown may not match the active model's tokenization." }
                            p { "To fix a mismatch, re-index your documents. This will re-chunk and re-count tokens using the active tokenizer." }
                        }
                    }
                }
            }
        }
    }
}
