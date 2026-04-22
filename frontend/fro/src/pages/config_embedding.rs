//! Embedding model configuration — /config/embedding

use crate::{
    api,
    app::Route,
    components::config_nav::{ConfigNav, ConfigTab},
    components::monitor::*,
};
use dioxus::prelude::*;
use dioxus_router::Link;

const PARAM_BLOCK_CLASS: &str = "flex flex-col gap-1 text-xs text-gray-200";
const PARAM_COLUMN_CLASS: &str = "param-column-spacing";
const PARAM_LABEL_CLASS: &str = "text-gray-400 whitespace-nowrap";

#[component]
pub fn ConfigEmbedding() -> Element {
    // ─── Embedding model ──────────────────────────────────────
    let mut embed_model = use_signal(|| "bge-small-en-v1.5".to_string());
    let mut embed_model_pending = use_signal(|| "bge-small-en-v1.5".to_string());
    let mut embed_dim = use_signal(|| 384usize);
    let mut embed_tokenizer_ok = use_signal(|| false);
    let mut embed_model_file_ok = use_signal(|| false);
    let mut embed_saving = use_signal(|| false);
    let mut embed_save_msg = use_signal(|| Option::<String>::None);

    // ─── Tokenizer download ───────────────────────────────────
    let mut tok_downloading = use_signal(|| false);
    let mut tok_download_msg = use_signal(|| Option::<String>::None);
    let mut restarting = use_signal(|| false);

    // ─── ONNX model info (read-only) ──────────────────────────
    let mut model_path = use_signal(|| String::new());
    let mut onnx_max_length = use_signal(|| 0usize);
    let mut onnx_loading = use_signal(|| true);

    use_effect(move || {
        spawn(async move {
            if let Ok(emb) = api::fetch_embedding_config().await {
                embed_model.set(emb.model.clone());
                embed_model_pending.set(emb.model);
                embed_dim.set(emb.dimension);
                embed_tokenizer_ok.set(emb.tokenizer_exists);
                embed_model_file_ok.set(emb.onnx.model_exists);
            }
            if let Ok(resp) = api::fetch_onnx_config().await {
                model_path.set(resp.config.model_path);
                onnx_max_length.set(resp.config.max_length);
                onnx_loading.set(false);
            } else {
                onnx_loading.set(false);
            }
        });
    });

    rsx! {
        div { class: "space-y-5",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::Config {})),
                    BreadcrumbItem::new("Embedding", Some(Route::ConfigEmbedding {})),
                ],
            }

            ConfigNav { active: ConfigTab::Embedding }

            // ─── MODEL PICKER ─────────────────────────────────────────
            Panel { title: None, refresh: None,
                div { class: "flex flex-col gap-2",
                    span { class: "text-base text-gray-100 font-semibold", "Embedding Model" }
                    span { class: "text-xs text-gray-400",
                        "Produces vectors for both document ingestion and query search. \
                        Also drives semantic chunk-boundary detection when the Semantic or Pipeline chunker mode is active. \
                        Changing the model requires a full re-index."
                    }
                }

                div { class: "mt-4 flex flex-wrap gap-8",

                    div { class: "rounded border border-gray-600 p-4 flex-1 min-w-64",
                        span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Active Model" }
                        div { class: "flex flex-col gap-3",

                            // Active model badge
                            div { class: "flex items-center gap-2 flex-wrap",
                                span { class: "text-xs text-gray-400", "Active:" }
                                span { class: "text-xs font-mono text-cyan-300 bg-gray-800 px-2 py-0.5 rounded",
                                    "{embed_model()}"
                                }
                                span { class: "text-xs text-gray-500", "· {embed_dim()} dims" }
                            }

                            // Status indicators
                            div { class: "flex gap-3 text-xs",
                                div { class: "flex items-center gap-1",
                                    span {
                                        class: if embed_model_file_ok() { "text-green-400" } else { "text-red-400" },
                                        if embed_model_file_ok() { "● model file" } else { "○ model file" }
                                    }
                                }
                                div { class: "flex items-center gap-1",
                                    span {
                                        class: if embed_tokenizer_ok() { "text-green-400" } else { "text-yellow-400" },
                                        if embed_tokenizer_ok() { "● tokenizer" } else { "○ tokenizer" }
                                    }
                                    if !embed_tokenizer_ok() {
                                        button {
                                            class: "btn btn-xs text-white ml-1",
                                            style: "background-color:#7C2A02;border-color:#7C2A02;",
                                            disabled: tok_downloading(),
                                            onclick: move |_| {
                                                spawn(async move {
                                                    tok_downloading.set(true);
                                                    tok_download_msg.set(None);
                                                    match api::download_tokenizer().await {
                                                        Ok(msg) => {
                                                            embed_tokenizer_ok.set(true);
                                                            tok_download_msg.set(Some(msg));
                                                        }
                                                        Err(e) => tok_download_msg.set(Some(format!("Error: {e}"))),
                                                    }
                                                    tok_downloading.set(false);
                                                });
                                            },
                                            if tok_downloading() { "Downloading…" } else { "Download" }
                                        }
                                    }
                                }
                            }
                            if let Some(msg) = tok_download_msg() {
                                div { class: "flex items-center gap-2 flex-wrap",
                                    span { class: "text-xs text-amber-400 leading-tight", "{msg}" }
                                    if msg.contains("Restart to activate") {
                                        button {
                                            class: "btn btn-xs text-white",
                                            style: "background-color:#7C2A02;border-color:#7C2A02;",
                                            disabled: restarting(),
                                            onclick: move |_| {
                                                spawn(async move {
                                                    restarting.set(true);
                                                    let _ = api::restart_process().await;
                                                });
                                            },
                                            if restarting() { "Restarting…" } else { "Restart" }
                                        }
                                    }
                                }
                            }

                            // Model picker
                            div { class: "flex flex-col gap-1",
                                label { class: "text-xs text-gray-400", "Model" }
                                select {
                                    class: "select select-xs select-bordered bg-gray-700 text-gray-200 w-full",
                                    value: "{embed_model_pending()}",
                                    onchange: move |e| {
                                        embed_save_msg.set(None);
                                        embed_model_pending.set(e.value());
                                    },
                                    option { value: "bge-small-en-v1.5",  "bge-small-en-v1.5 — 384d · 33 MB" }
                                    option { value: "bge-small-en-v1.5q", "bge-small-en-v1.5q — 384d · 8 MB (INT8)" }
                                    option { value: "all-minilm-l6-v2",   "all-MiniLM-L6-v2 — 384d · 22 MB" }
                                    option { value: "bge-base-en-v1.5",   "bge-base-en-v1.5 — 768d · 109 MB ⚠ re-index" }
                                    option { value: "e5-small-v2",        "e5-small-v2 — 384d · 33 MB" }
                                }
                            }

                            // Apply button
                            div { class: "flex items-center gap-2 flex-wrap",
                                button {
                                    class: "btn btn-xs text-white",
                                    style: "background-color:#7C2A02;border-color:#7C2A02;",
                                    disabled: embed_saving() || embed_model_pending() == embed_model(),
                                    onclick: move |_| {
                                        let selected = embed_model_pending();
                                        spawn(async move {
                                            embed_saving.set(true);
                                            embed_save_msg.set(None);
                                            match api::set_embedding_model(&selected).await {
                                                Ok(msg) => {
                                                    embed_model.set(selected);
                                                    embed_save_msg.set(Some(msg));
                                                }
                                                Err(e) => embed_save_msg.set(Some(format!("Error: {e}"))),
                                            }
                                            embed_saving.set(false);
                                        });
                                    },
                                    if embed_saving() { "Saving…" } else { "Apply" }
                                }
                                if let Some(msg) = embed_save_msg() {
                                    span { class: "text-xs text-amber-400", "{msg}" }
                                }
                            }

                            if embed_model_pending() == "bge-base-en-v1.5" && embed_model() != "bge-base-en-v1.5" {
                                span { class: "text-xs text-amber-400 leading-tight",
                                    "⚠ 768-dim model — existing index must be deleted and rebuilt after restart."
                                }
                            }
                        }
                    }

                    // ─── Model info (read-only) ──────────────────────────
                    div { class: "rounded border border-gray-600 p-4 flex-1 min-w-64",
                        span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Runtime Info" }
                        if onnx_loading() {
                            p { class: "text-xs text-gray-500", "Loading…" }
                        } else {
                            div { class: PARAM_COLUMN_CLASS,
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "model_path" }
                                    span { class: "text-gray-200 text-xs font-mono", "{model_path()}" }
                                }
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "embedding_dim" }
                                    span { class: "text-gray-200 text-xs", "{embed_dim()}" }
                                }
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "max_length" }
                                    span { class: "text-gray-200 text-xs", "{onnx_max_length()}" }
                                }
                            }
                        }
                        p { class: "text-xs text-gray-500 mt-3 italic",
                            "Runtime parameters (threading, memory, optimization) are on the "
                            Link { to: Route::ConfigOnnx {}, class: "text-cyan-400 hover:underline", "ONNX" }
                            " page."
                        }
                    }
                }
            }
        }
    }
}
