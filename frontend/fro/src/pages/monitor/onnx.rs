//! ONNX Embedding Runtime Monitor Page v1.0.0
//! Route: /monitor/onnx

use crate::{
    api,
    app::{PageErrors, Route},
    components::monitor::*,
};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

const BOARD_CLASS: &str = "rounded border border-gray-600 p-4 bg-gray-800/50";
const LABEL_CLASS: &str = "text-gray-400 text-xs";
const VALUE_CLASS: &str = "text-gray-100 text-sm font-mono";

#[derive(Clone, Default)]
struct OnnxPageState {
    loading: bool,
    error: Option<String>,
    data: api::OnnxMonitorStats,
}

#[component]
pub fn MonitorOnnx() -> Element {
    let mut state = use_signal(|| OnnxPageState {
        loading: true,
        ..Default::default()
    });
    let mut page_errors = use_context::<Signal<PageErrors>>();
    let mut show_help = use_signal(|| false);
    let mut show_batch_info = use_signal(|| false);

    use_future(move || async move {
        loop {
            match api::fetch_onnx_monitor_stats().await {
                Ok(data) => {
                    state.set(OnnxPageState {
                        loading: false,
                        error: None,
                        data,
                    });
                    page_errors.with_mut(|e| e.clear_error("onnx"));
                }
                Err(err) => {
                    state.with_mut(|s| {
                        s.loading = false;
                        s.error = Some(err.clone());
                    });
                    page_errors.with_mut(|e| e.set_error("onnx", &err));
                }
            }
            TimeoutFuture::new(5_000).await;
        }
    });

    let snap = state.read().clone();

    let status_color = if snap.data.status == "loaded" {
        "text-green-400"
    } else {
        "text-yellow-400"
    };

    let hit_rate_color = if snap.data.cache_hit_rate >= 80.0 {
        "text-green-400"
    } else if snap.data.cache_hit_rate >= 50.0 {
        "text-yellow-400"
    } else {
        "text-red-400"
    };

    rsx! {
        div { class: "space-y-6",

            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home",    Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("ONNX",    None),
                ],
            }

            NavTabs { active: Route::MonitorOnnx {} }


            div { class: "flex items-center gap-2 mb-3",
                h3 { class: "text-gray-100 text-base font-semibold", "ONNX Embedding Runtime" }
                button {
                    class: "w-6 h-6 min-w-6 min-h-6 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80",
                    style: "background-color:#7C2A02;border:1px solid #7C2A02;",
                    onclick: move |_| show_help.set(true),
                    svg {
                        class: "w-5 h-5 text-white",
                        xmlns: "http://www.w3.org/2000/svg",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke: "currentColor",
                        stroke_width: "1.5",
                        circle { cx: "12", cy: "12", r: "9" }
                        line { x1: "12", y1: "8", x2: "12", y2: "14", stroke_width: "1.5" }
                        circle { cx: "12", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                    }
                }
            }

            if show_batch_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_batch_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[90vw] max-w-lg shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-3",
                            h2 { class: "text-base font-semibold text-gray-100", "Texts via batch" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_batch_info.set(false),
                                "×"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-2",
                            p { "Counts how many texts were processed through the " code { "embed_batch()" } " code path — used during indexing/chunking when multiple chunks are embedded in one go." }
                            p { "When it is 0 it is because search uses " code { "embed_query()" } " which calls the single-text path. You will see it increment during a reindex or document upload, when the chunker processes many chunks at once and calls " code { "embed_batch_owned()" } " in " code { "EmbeddingRuntime" } "." }
                        }
                    }
                }
            }

            if show_help() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_help.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg w-[95vw] max-w-[min(90vw,1680px)] shadow-xl flex flex-col max-h-[90vh]",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between p-5 pb-3 shrink-0 border-b border-gray-600",
                            h2 { class: "text-base font-semibold text-gray-100", "ONNX Monitor" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_help.set(false),
                                "×"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-3 overflow-y-auto flex-1 p-5",
                            p { "Live stats for the ONNX embedding model. Monitors the ONNX embedding runtime used to generate vector embeddings for RAG retrieval." }
                            ul { class: "list-disc ml-5 space-y-1",
                                li { strong { "Model" } " – name, vector dimensions, and configured batch size." }
                                li { strong { "Cache" } " – LRU hit/miss counts and hit rate. High hit rate = repeated queries served without running the model." }
                                li { strong { "Throughput" } " – total single embeddings and batch calls since service start." }
                                li { strong { "Latency" } " – average and last single-embed inference time (cache misses only)." }
                            }
                            div { class: "border-t border-gray-600 pt-3",
                                p { class: "font-semibold text-gray-100 mb-1", "Semantic vectors" }
                                p { "Numerical representations where meaning is encoded geometrically. Each number is a learned feature — not human-interpretable individually, but collectively they place the text in an N-dimensional space where:" }
                                ul { class: "list-disc ml-5 space-y-1 mt-1",
                                    li { "How do I reset my password? and I forgot my login credentials land " strong { "near each other" } }
                                    li { "How do I reset my password? and What is the capital of France? land " strong { "far apart" } }
                                }
                                p { class: "mt-1", "Distance is measured by cosine similarity — the angle between two vectors. Small angle = similar meaning. Large angle = unrelated." }
                            }
                            div { class: "border-t border-gray-600 pt-3",
                                p { class: "font-semibold text-gray-100 mb-1", "What this means for RAG" }
                                p { "BM25 finds a word if the document contains that exact word. Vector search finds documents that mean the same thing even if they use completely different words — because their vectors cluster near the query vector in the semantic space." }
                            }
                            div { class: "border-t border-gray-600 pt-3",
                                p { class: "font-semibold text-gray-100 mb-1", "Number of dimensions" }
                                p { "A hyperparameter set before training. Larger = more expressive, slower. Smaller = faster, less expressive." }
                                ul { class: "list-disc ml-5 space-y-1 mt-1 font-mono text-xs",
                                    li { "BGE-small: 384" }
                                    li { "BGE-base: 768" }
                                    li { "BGE-large: 1024" }
                                    li { "OpenAI text-embedding-3-large: 3072" }
                                }
                            }
                            div { class: "border-t border-gray-600 pt-3",
                                p { class: "font-semibold text-gray-100 mb-1", "The numbers are not" }
                                ul { class: "list-disc ml-5 space-y-1",
                                    li { "Topic labels" }
                                    li { "Word counts" }
                                    li { "Keywords" }
                                }
                                p { class: "mt-1", "They are a compressed representation of the full contextual meaning of the input — position in a meaning space learned from millions of text examples during model training." }
                            }
                            div { class: "border-t border-gray-600 pt-3",
                                p { class: "font-semibold text-gray-100 mb-2", "Input" }
                                div { class: "overflow-x-auto",
                                    table { class: "w-full text-xs border-collapse",
                                        thead {
                                            tr { class: "border-b border-gray-600",
                                                th { class: "text-left py-1 pr-4 text-gray-400", "Input Type" }
                                                th { class: "text-left py-1 pr-4 text-gray-400", "Allowed?" }
                                                th { class: "text-left py-1 text-gray-400", "Notes" }
                                            }
                                        }
                                        tbody {
                                            tr { class: "border-b border-gray-700",
                                                td { class: "py-1 pr-4", "Single word" }
                                                td { class: "py-1 pr-4 text-green-400", "Yes" }
                                                td { class: "py-1 text-gray-400", "Common for keyword expansion" }
                                            }
                                            tr { class: "border-b border-gray-700",
                                                td { class: "py-1 pr-4", "Sentence" }
                                                td { class: "py-1 pr-4 text-green-400", "Yes" }
                                                td { class: "py-1 text-gray-400", "Most typical use case" }
                                            }
                                            tr { class: "border-b border-gray-700",
                                                td { class: "py-1 pr-4", "Long documents" }
                                                td { class: "py-1 pr-4 text-green-400", "Yes" }
                                                td { class: "py-1 text-gray-400", "Often chunked first" }
                                            }
                                            tr { class: "border-b border-gray-700",
                                                td { class: "py-1 pr-4", "JSON / structured data" }
                                                td { class: "py-1 pr-4 text-green-400", "Yes" }
                                                td { class: "py-1 text-gray-400", "Must be converted to text" }
                                            }
                                            tr { class: "border-b border-gray-700",
                                                td { class: "py-1 pr-4", "Images / audio" }
                                                td { class: "py-1 pr-4 text-yellow-400", "Conditional" }
                                                td { class: "py-1 text-gray-400", "Only if the model supports multimodal embeddings" }
                                            }
                                            tr {
                                                td { class: "py-1 pr-4", "Binary files" }
                                                td { class: "py-1 pr-4 text-red-400", "No" }
                                                td { class: "py-1 text-gray-400", "Must be converted to text or features first" }
                                            }
                                        }
                                    }
                                }
                            }
                            div { class: "border-t border-gray-600 pt-3",
                                p { class: "font-semibold text-gray-100 mb-2", "Output — where semantic vectors are used" }
                                ol { class: "list-decimal ml-5 space-y-2 text-xs",
                                    li {
                                        strong { "RAG (Retrieval-Augmented Generation)" }
                                        p { class: "text-gray-400 mt-0.5", "Embeddings let you search a vector DB by meaning, retrieve relevant text, and feed it to an LLM. They help find information, but they do not become K/V/Q vectors." }
                                    }
                                    li {
                                        strong { "Semantic Search" }
                                        p { class: "text-gray-400 mt-0.5", "Embeddings power search engines that match meaning instead of keywords." }
                                    }
                                    li {
                                        strong { "Recommendation Systems" }
                                        p { class: "text-gray-400 mt-0.5", "Users, items, and behaviors are embedded; similar vectors lead to good recommendations." }
                                    }
                                    li {
                                        strong { "Clustering & Topic Discovery" }
                                        p { class: "text-gray-400 mt-0.5", "Group documents, customers, or messages by semantic similarity." }
                                    }
                                    li {
                                        strong { "Classification" }
                                        p { class: "text-gray-400 mt-0.5", "Use embeddings as features for sentiment, intent, spam detection, etc." }
                                    }
                                    li {
                                        strong { "Anomaly Detection" }
                                        p { class: "text-gray-400 mt-0.5", "Vectors far from the normal cluster indicate fraud or unusual behavior." }
                                    }
                                    li {
                                        strong { "Agent Memory Systems" }
                                        p { class: "text-gray-400 mt-0.5", "Store past interactions as embeddings; retrieve the most relevant ones later." }
                                    }
                                }
                            }
                            p { class: "text-gray-400 text-xs border-t border-gray-600 pt-2", "Refreshes every 5 seconds." }
                            p { class: "text-xs pt-1", a { href: "/docu/index/embeddings", class: "text-blue-400 hover:underline", "More Info" } }
                        }
                        div { class: "shrink-0 border-t border-gray-600 p-4",
                            button {
                                class: "btn btn-primary btn-sm w-full",
                                onclick: move |_| show_help.set(false),
                                "Close"
                            }
                        }
                    }
                }
            }

            if let Some(ref err) = snap.error {
                div { class: "rounded border border-red-700 bg-red-900/30 p-3 text-sm text-red-300",
                    "Failed to load ONNX stats: {err}"
                }
            }

            if snap.loading {
                div { class: "text-gray-400 text-sm", "Loading…" }
            }

            div { class: BOARD_CLASS,
                h3 { class: "text-gray-200 text-sm font-semibold mb-3", "Model" }
                div { class: "grid grid-cols-2 gap-x-8 gap-y-2 sm:grid-cols-4",
                    div {
                        p { class: LABEL_CLASS, "Status" }
                        p { class: "{VALUE_CLASS} {status_color}", "{snap.data.status}" }
                    }
                    div {
                        p { class: LABEL_CLASS, "Name" }
                        p { class: VALUE_CLASS, "{snap.data.model_name}" }
                    }
                    div {
                        p { class: LABEL_CLASS, "Dimensions" }
                        p { class: VALUE_CLASS, "{snap.data.model_dims}" }
                    }
                    div {
                        p { class: LABEL_CLASS, "Batch size" }
                        p { class: VALUE_CLASS, "{snap.data.batch_size}" }
                    }
                }
            }

            div { class: BOARD_CLASS,
                h3 { class: "text-gray-200 text-sm font-semibold mb-3", "Embedding Cache (LRU)" }
                div { class: "grid grid-cols-2 gap-x-8 gap-y-2 sm:grid-cols-4",
                    div {
                        p { class: LABEL_CLASS, "Hit rate" }
                        p { class: "{VALUE_CLASS} {hit_rate_color}", "{snap.data.cache_hit_rate:.1}%" }
                    }
                    div {
                        p { class: LABEL_CLASS, "Hits" }
                        p { class: VALUE_CLASS, "{snap.data.cache_hits}" }
                    }
                    div {
                        p { class: LABEL_CLASS, "Misses" }
                        p { class: VALUE_CLASS, "{snap.data.cache_misses}" }
                    }
                    div {
                        p { class: LABEL_CLASS, "Total lookups" }
                        p { class: VALUE_CLASS, "{snap.data.cache_hits + snap.data.cache_misses}" }
                    }
                }
                div { class: "mt-4",
                    p { class: "text-xs text-gray-400 mb-1", "Cache efficiency" }
                    div { class: "w-full bg-gray-700 rounded h-2",
                        div {
                            class: "h-2 rounded bg-green-500",
                            style: "width: {snap.data.cache_hit_rate.min(100.0).max(0.0)}%",
                        }
                    }
                }
            }

            div { class: BOARD_CLASS,
                h3 { class: "text-gray-200 text-sm font-semibold mb-3", "Throughput (since start)" }
                div { class: "grid grid-cols-2 gap-x-8 gap-y-2 sm:grid-cols-3",
                    div {
                        p { class: LABEL_CLASS, "Single embeds" }
                        p { class: VALUE_CLASS, "{snap.data.total_embeddings}" }
                    }
                    div {
                        p { class: LABEL_CLASS, "Batch calls" }
                        p { class: VALUE_CLASS, "{snap.data.total_batches}" }
                    }
                    div {
                        div { class: "flex items-center gap-1",
                            p { class: LABEL_CLASS, "Texts via batch" }
                            button {
                                class: "w-6 h-6 min-w-6 min-h-6 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;",
                                onclick: move |_| show_batch_info.set(true),
                                svg {
                                    class: "w-5 h-5 text-white",
                                    xmlns: "http://www.w3.org/2000/svg",
                                    fill: "none",
                                    view_box: "0 0 24 24",
                                    stroke: "currentColor",
                                    stroke_width: "1.5",
                                    circle { cx: "12", cy: "12", r: "9" }
                                    line { x1: "12", y1: "8", x2: "12", y2: "14", stroke_width: "1.5" }
                                    circle { cx: "12", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                }
                            }
                        }
                        p { class: VALUE_CLASS, "{snap.data.total_batch_texts}" }
                    }
                }
            }

            div { class: BOARD_CLASS,
                h3 { class: "text-gray-200 text-sm font-semibold mb-3", "Inference Latency (cache misses only)" }
                div { class: "grid grid-cols-2 gap-x-8 gap-y-2",
                    div {
                        p { class: LABEL_CLASS, "Avg embed" }
                        p { class: VALUE_CLASS, "{snap.data.avg_embed_ms:.2} ms" }
                    }
                    div {
                        p { class: LABEL_CLASS, "Last embed" }
                        p { class: VALUE_CLASS, "{snap.data.last_embed_ms:.2} ms" }
                    }
                }
            }
        }
    }
}
