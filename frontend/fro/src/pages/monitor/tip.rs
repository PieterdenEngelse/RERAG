use crate::api::{fetch_parser_stats, FileRecord, ParserStats};
use crate::app::Route;
use crate::components::monitor::*;
use crate::pages::hardware::constants::{
    INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
};
use dioxus::prelude::*;

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
pub fn MonitorTip() -> Element {
    let mut show_tip_info = use_signal(|| false);
    let mut show_parser_info = use_signal(|| false);
    let mut show_canon_info = use_signal(|| false);
    let parser_stats = use_resource(fetch_parser_stats);

    rsx! {
        div { class: "p-6 text-gray-300",
            NavTabs { active: Route::MonitorTip {} }

            // Page header
            div { class: "mt-6",
                div { class: "flex items-center gap-2 mb-6",
                    h2 { class: "text-xl font-semibold text-white", "Text Ingestion Pipeline (TIP)" }
                    button {
                        class: PARAM_ICON_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        onclick: move |_| show_tip_info.set(true),
                        title: "About the Text Ingestion Pipeline",
                        InfoIcon {}
                    }
                }
            }

            // Tiles row
            div { class: "flex flex-wrap gap-4",

                // Parser tile
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 w-full", style: "height:288px;",
                    div { class: "flex items-center justify-between mb-3",
                        div { class: "flex items-center gap-2",
                            h3 { class: "text-sm font-semibold text-gray-200", "Parser" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_parser_info.set(true),
                                title: "About the Parser",
                                InfoIcon {}
                            }
                        }
                        span { class: "text-xs text-gray-400", "7 days history / limit 1000" }
                    }
                    match &*parser_stats.read() {
                        Some(Ok(stats)) => rsx! {
                            ParserStatsView { stats: stats.clone() }
                        },
                        Some(Err(e)) => rsx! {
                            p { class: "text-xs text-red-400", "Error: {e}" }
                        },
                        None => rsx! {
                            p { class: "text-xs text-gray-500", "Loading…" }
                        },
                    }
                }

                // Canonicalization tile
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 w-full", style: "height:288px;",
                    div { class: "flex items-center gap-2",
                        h3 { class: "text-sm font-semibold text-gray-200", "Canonicalization" }
                        button {
                            class: PARAM_ICON_BUTTON_CLASS,
                            style: PARAM_ICON_BUTTON_STYLE,
                            onclick: move |_| show_canon_info.set(true),
                            title: "About Canonicalization",
                            InfoIcon {}
                        }
                    }
                }
            }

            // ── TIP info modal ──────────────────────────────────────────────
            if show_tip_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_tip_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg w-[90vw] max-w-[90vw] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),

                        // Sticky header
                        div { class: "flex items-center justify-between px-6 py-4 border-b border-gray-600 shrink-0",
                            h2 { class: "text-lg font-semibold text-gray-100",
                                "Text Ingestion Pipeline and Its Role in RAG"
                            }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                onclick: move |_| show_tip_info.set(false),
                                "✕"
                            }
                        }

                        // Scrollable body
                        div { class: "flex-1 overflow-y-auto px-6 py-4 text-sm text-gray-300 space-y-4",

                            p {
                                "A Text Ingestion Pipeline—structured into "
                                strong { class: "text-gray-100", "Canonicalization" }
                                ", "
                                strong { class: "text-gray-100", "Preprocessing" }
                                ", and "
                                strong { class: "text-gray-100", "Pipeline Orchestration" }
                                "—forms the foundational layer of any Retrieval-Augmented Generation (RAG) system. \
                                Each layer directly influences embedding quality, graph construction, clustering \
                                behavior, summarization accuracy, and retrieval performance."
                            }

                            hr { class: "border-gray-600" }

                            h3 { class: "text-base font-semibold text-gray-100", "1. Canonicalization Layer" }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "Role in RAG" }
                            p { "The canonicalization layer ensures that all incoming text is transformed into a \
                                uniform, normalized representation. This reduces surface-level variation and \
                                eliminates inconsistencies that would otherwise propagate through the RAG stack." }

                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "Effects on Downstream Components" }
                            TipSubSection { title: "Embeddings", items: vec![
                                "Reduces embedding variance caused by punctuation, casing, Unicode inconsistencies, or typographic noise.",
                                "Ensures semantically identical text maps to similar vectors.",
                                "Improves reproducibility of embedding generation.",
                            ]}
                            TipSubSection { title: "Graph Construction", items: vec![
                                "Prevents duplicate nodes caused by superficial text differences.",
                                "Ensures consistent node identity across re-indexing cycles.",
                                "Reduces graph fragmentation.",
                            ]}
                            TipSubSection { title: "Clustering", items: vec![
                                "Minimizes cluster splitting caused by inconsistent tokenization or text noise.",
                                "Produces more coherent and semantically aligned clusters.",
                            ]}
                            TipSubSection { title: "Summarization", items: vec![
                                "Provides clean, normalized input to summarizers.",
                                "Reduces hallucination risk caused by malformed or noisy text.",
                                "Improves sentence boundary detection.",
                            ]}
                            TipSubSection { title: "Retrieval", items: vec![
                                "Ensures queries and documents canonicalize to the same form.",
                                "Improves recall by eliminating mismatches such as \"AI\" vs \"A.I.\" vs \"Ai\".",
                                "Stabilizes hybrid search (keyword + vector) performance.",
                            ]}
                            p { class: "italic text-gray-400", "Canonicalization acts as the stability layer of the entire RAG system." }

                            hr { class: "border-gray-600" }

                            h3 { class: "text-base font-semibold text-gray-100", "2. Preprocessing Layer" }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "Role in RAG" }
                            p { "The preprocessing layer structures canonicalized text into semantic units suitable \
                                for embedding, indexing, and retrieval. This includes tokenization, sentence \
                                segmentation, chunking, boilerplate removal, and domain-specific cleanup." }

                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "Effects on Downstream Components" }
                            TipSubSection { title: "Embeddings", items: vec![
                                "Defines the granularity of embedding units (sentences, paragraphs, chunks).",
                                "Ensures embeddings represent coherent semantic content.",
                                "Reduces embedding drift caused by inconsistent chunk boundaries.",
                            ]}
                            TipSubSection { title: "Graph Construction", items: vec![
                                "Determines node boundaries in graph-based RAG systems.",
                                "Influences edge creation by shaping semantic adjacency.",
                                "Produces cleaner, more interpretable graph structures.",
                            ]}
                            TipSubSection { title: "Clustering", items: vec![
                                "Controls cluster density and purity by defining chunk size and segmentation.",
                                "Ensures clusters represent meaningful topics rather than mixed content.",
                            ]}
                            TipSubSection { title: "Summarization", items: vec![
                                "Provides well-formed, coherent chunks for summarization.",
                                "Improves summary quality by ensuring each chunk contains a unified topic.",
                                "Reduces summarizer confusion caused by boilerplate or HTML noise.",
                            ]}
                            TipSubSection { title: "Retrieval", items: vec![
                                "Enhances precision by ensuring each chunk corresponds to a single semantic idea.",
                                "Improves ranking stability by reducing noise in embedding vectors.",
                                "Enables more accurate context selection for generation.",
                            ]}
                            p { class: "italic text-gray-400", "Preprocessing acts as the semantic structuring layer of RAG." }

                            hr { class: "border-gray-600" }

                            h3 { class: "text-base font-semibold text-gray-100", "3. Pipeline Orchestration Layer" }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "Role in RAG" }
                            p { "The orchestration layer coordinates canonicalization and preprocessing into a \
                                deterministic, reproducible ingestion flow. It defines configuration, execution \
                                order, and output formats." }

                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "Effects on Downstream Components" }
                            TipSubSection { title: "Embeddings", items: vec![
                                "Guarantees deterministic embedding generation across runs.",
                                "Enables caching and incremental indexing.",
                            ]}
                            TipSubSection { title: "Graph Construction", items: vec![
                                "Ensures stable graph topology across re-indexing cycles.",
                                "Supports versioning and reproducible analytics.",
                            ]}
                            TipSubSection { title: "Clustering", items: vec![
                                "Produces consistent cluster assignments over time.",
                                "Facilitates comparison of cluster evolution.",
                            ]}
                            TipSubSection { title: "Summarization", items: vec![
                                "Ensures summaries remain stable when the underlying ingestion pipeline is unchanged.",
                                "Supports reproducible summary-first retrieval.",
                            ]}
                            TipSubSection { title: "Retrieval", items: vec![
                                "Produces consistent retrieval rankings.",
                                "Enables reliable evaluation and debugging of retrieval behavior.",
                                "Ensures that changes in retrieval quality can be attributed to real data or model changes, not ingestion drift.",
                            ]}
                            p { class: "italic text-gray-400", "Orchestration acts as the determinism layer of RAG." }

                            hr { class: "border-gray-600" }

                            h3 { class: "text-base font-semibold text-gray-100", "System-Level Summary" }
                            p { "The three ingestion layers collectively determine the stability, quality, and \
                                interpretability of the entire RAG pipeline." }

                            div { class: "overflow-x-auto",
                                table { class: "w-full text-sm border-collapse",
                                    thead {
                                        tr { class: "border-b border-gray-600",
                                            th { class: "text-left py-2 pr-4 text-gray-300 font-semibold", "Layer" }
                                            th { class: "text-left py-2 pr-4 text-gray-300 font-semibold", "Primary Function" }
                                            th { class: "text-left py-2 text-gray-300 font-semibold", "RAG Impact" }
                                        }
                                    }
                                    tbody {
                                        tr { class: "border-b border-gray-700",
                                            td { class: "py-2 pr-4 font-medium text-gray-200", "Canonicalization" }
                                            td { class: "py-2 pr-4 text-gray-400", "Normalize text" }
                                            td { class: "py-2 text-gray-400", "Stable embeddings, high recall, no duplicate nodes" }
                                        }
                                        tr { class: "border-b border-gray-700",
                                            td { class: "py-2 pr-4 font-medium text-gray-200", "Preprocessing" }
                                            td { class: "py-2 pr-4 text-gray-400", "Structure text" }
                                            td { class: "py-2 text-gray-400", "Coherent chunks, accurate embeddings, meaningful graph nodes" }
                                        }
                                        tr {
                                            td { class: "py-2 pr-4 font-medium text-gray-200", "Orchestration" }
                                            td { class: "py-2 pr-4 text-gray-400", "Coordinate pipeline" }
                                            td { class: "py-2 text-gray-400", "Deterministic indexing, reproducible retrieval, stable summaries" }
                                        }
                                    }
                                }
                            }

                            p { "A well-designed ingestion pipeline ensures that all downstream RAG \
                                components—embedding, graph construction, clustering, summarization, and \
                                retrieval—operate on clean, consistent, and semantically coherent data." }
                        }

                        // Sticky footer
                        div { class: "px-6 py-3 border-t border-gray-600 shrink-0 flex justify-end bg-gray-800 rounded-b-lg",
                            button {
                                class: "px-5 py-1.5 text-sm font-medium rounded text-white hover:opacity-80",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;",
                                onclick: move |_| show_tip_info.set(false),
                                "Got it"
                            }
                        }
                    }
                }
            }

            // ── Parser info modal ───────────────────────────────────────────
            if show_parser_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_parser_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg w-[70vw] max-w-2xl max-h-[80vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between px-6 py-4 border-b border-gray-600 shrink-0",
                            h2 { class: "text-lg font-semibold text-gray-100", "Parser" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                onclick: move |_| show_parser_info.set(false),
                                "✕"
                            }
                        }
                        div { class: "flex-1 overflow-y-auto px-6 py-4 text-sm text-gray-300 space-y-3",
                            p {
                                "The parser is the first stage of the ingestion pipeline. It reads raw input—files, \
                                URLs, or streams—and converts them into a "
                                strong { class: "text-gray-100", "plain-text representation" }
                                " that subsequent pipeline stages can process."
                            }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "Supported formats" }
                            ul { class: "ml-4 space-y-1 list-disc list-outside text-gray-400",
                                li { "Plain text and Markdown" }
                                li { "HTML and XML (tag-aware extraction)" }
                                li { "PDF (text layer extraction)" }
                                li { "Office formats: DOCX, XLSX, ODT, ODS, CSV" }
                                li { "Source code files (language-aware)" }
                                li { "JSON (structure-aware flattening)" }
                            }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "RAG impact" }
                            ul { class: "ml-4 space-y-1 list-disc list-outside text-gray-400",
                                li { "Determines what text is available for chunking and embedding." }
                                li { "Format-specific extraction preserves semantic structure." }
                                li { "Poor parsing propagates noise into every downstream stage." }
                            }
                            p { class: "italic text-gray-400 pt-1",
                                "The parser is the entry point of the entire ingestion pipeline."
                            }
                        }
                        div { class: "px-6 py-3 border-t border-gray-600 shrink-0 flex justify-end bg-gray-800 rounded-b-lg",
                            button {
                                class: "px-5 py-1.5 text-sm font-medium rounded text-white hover:opacity-80",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;",
                                onclick: move |_| show_parser_info.set(false),
                                "Got it"
                            }
                        }
                    }
                }
            }

            // ── Canonicalization info modal ─────────────────────────────────
            if show_canon_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_canon_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg w-[70vw] max-w-2xl max-h-[80vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),

                        // Sticky header
                        div { class: "flex items-center justify-between px-6 py-4 border-b border-gray-600 shrink-0",
                            h2 { class: "text-lg font-semibold text-gray-100", "Canonicalization" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                onclick: move |_| show_canon_info.set(false),
                                "✕"
                            }
                        }

                        // Scrollable body
                        div { class: "flex-1 overflow-y-auto px-6 py-4 text-sm text-gray-300 space-y-3",
                            p {
                                "The canonicalization layer transforms all incoming text into a "
                                strong { class: "text-gray-100", "uniform, normalized representation" }
                                " before it enters the chunking and embedding pipeline. \
                                Surface-level variation—punctuation differences, casing, Unicode inconsistencies, \
                                typographic noise—is eliminated at this stage so it never propagates downstream."
                            }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "What it normalizes" }
                            ul { class: "ml-4 space-y-1 list-disc list-outside text-gray-400",
                                li { "Unicode whitespace and typographic characters (curly quotes, em-dashes, ellipses)" }
                                li { "HTML and XML markup tags stripped to plain text" }
                                li { "Inconsistent casing and punctuation patterns" }
                                li { "Zero-width and non-breaking spaces" }
                            }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "RAG impact" }
                            ul { class: "ml-4 space-y-1 list-disc list-outside text-gray-400",
                                li { "Semantically identical text maps to the same embedding vector." }
                                li { "Retrieval recall improves — \"AI\", \"A.I.\", and \"Ai\" resolve to the same form." }
                                li { "Graph nodes stay consistent across re-indexing cycles." }
                                li { "Summarizers receive clean input, reducing hallucination risk." }
                            }
                            p { class: "italic text-gray-400 pt-1",
                                "Canonicalization is the stability layer of the entire RAG system."
                            }
                        }

                        // Sticky footer
                        div { class: "px-6 py-3 border-t border-gray-600 shrink-0 flex justify-end bg-gray-800 rounded-b-lg",
                            button {
                                class: "px-5 py-1.5 text-sm font-medium rounded text-white hover:opacity-80",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;",
                                onclick: move |_| show_canon_info.set(false),
                                "Got it"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct ParserStatsViewProps {
    stats: ParserStats,
}

#[component]
fn ParserStatsView(props: ParserStatsViewProps) -> Element {
    let stats = &props.stats;
    let mut show_empty_info = use_signal(|| false);

    // Deduplicate: keep only the most recent entry per (filename, format).
    // recent_files is newest-first, so the first occurrence wins.
    let mut seen = std::collections::HashSet::new();
    let deduped: Vec<&FileRecord> = stats
        .recent_files
        .iter()
        .filter(|r| seen.insert((r.filename.clone(), r.format.clone())))
        .collect();

    rsx! {
        div { class: "overflow-y-auto", style: "max-height:220px;",
            if stats.recent_files.is_empty() {
                div { class: "flex flex-col items-center justify-center h-24 gap-1",
                    p { class: "text-xs text-gray-400", "Upload a file to see extraction stats" }
                    p { class: "text-xs text-gray-400", "Stats reset on service restart" }
                }
            } else {
                table { class: "w-full text-xs",
                    thead {
                        tr { class: "text-gray-300 border-b border-gray-500",
                            th { class: "text-left pb-1 pr-3 font-medium", "Filename" }
                            th { class: "text-left pb-1 pr-3 font-medium", "Path" }
                            th { class: "text-left pb-1 pr-3 font-medium", "Format" }
                            th { class: "text-right pb-1 pr-3 font-medium",
                                div { class: "inline-flex items-center gap-1 justify-end",
                                    "Status"
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_empty_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                                if show_empty_info() {
                                    div {
                                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                                        onclick: move |_| show_empty_info.set(false),
                                        div {
                                            class: "bg-gray-800 border border-gray-600 rounded-lg w-80 flex flex-col shadow-xl",
                                            onclick: move |e| e.stop_propagation(),
                                            div { class: "flex items-center justify-between px-4 py-3 border-b border-gray-600 shrink-0",
                                                h2 { class: "text-sm font-semibold text-gray-100", "Extraction status" }
                                                button {
                                                    class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                                    onclick: move |_| show_empty_info.set(false),
                                                    "✕"
                                                }
                                            }
                                            div { class: "px-4 py-3 text-xs text-gray-300 space-y-2",
                                                p {
                                                    strong { class: "text-green-400", "ok" }
                                                    " — the parser returned text that was passed to chunking and embedding."
                                                }
                                                p {
                                                    strong { class: "text-yellow-500", "empty" }
                                                    " — the parser ran without error but returned no text. The file is silently absent from your RAG index."
                                                }
                                                p { class: "text-gray-400", "Common causes of empty:" }
                                                ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                    li { "PDF with only scanned images (no text layer)" }
                                                    li { "DOCX or ODT with no body content" }
                                                    li { "File misidentified as the wrong format" }
                                                    li { "ZIP-based format with empty content entries" }
                                                }
                                            }
                                            div { class: "px-4 py-2 border-t border-gray-600 flex justify-end bg-gray-800 rounded-b-lg",
                                                button {
                                                    class: "px-4 py-1 text-xs font-medium rounded text-white hover:opacity-80",
                                                    style: "background-color:#7C2A02;border:1px solid #7C2A02;",
                                                    onclick: move |_| show_empty_info.set(false),
                                                    "Got it"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            th { class: "text-right pb-1 font-medium", "Chars" }
                        }
                    }
                    tbody {
                        for rec in &deduped {
                            tr { class: "border-b border-gray-500/50",
                                td { class: "py-0.5 pr-3 text-gray-200 font-mono truncate", style: "max-width:120px;", title: "{rec.filename}", "{rec.filename}" }
                                td { class: "py-0.5 pr-3 text-gray-400 font-mono truncate", style: "max-width:160px;", title: "{rec.path}", "{rec.path}" }
                                td { class: "py-0.5 pr-3 text-gray-300 font-mono", "{rec.format}" }
                                td { class: "py-0.5 pr-3 text-right",
                                    if rec.ok {
                                        span { class: "text-green-400", "ok" }
                                    } else {
                                        span { class: "text-yellow-500", "empty" }
                                    }
                                }
                                td { class: "py-0.5 text-right text-gray-400",
                                    if rec.ok {
                                        "{format_chars(rec.chars)}"
                                    } else {
                                        "—"
                                    }
                                }
                            }
                        }
                    }
                }
                if stats.ocr.attempted > 0 {
                    div { class: "mt-2 pt-2 border-t border-gray-700 flex flex-wrap gap-x-4 gap-y-0.5 text-xs",
                        span { class: "text-gray-400",
                            "attempted " span { class: "text-white font-medium", "{stats.ocr.attempted}" }
                        }
                        span { class: "text-gray-400",
                            "ok " span { class: "text-green-400 font-medium", "{stats.ocr.ok}" }
                        }
                        if stats.ocr.no_text > 0 {
                            span { class: "text-gray-400",
                                "no_text " span { class: "text-yellow-500 font-medium", "{stats.ocr.no_text}" }
                            }
                        }
                        if stats.ocr.unavailable > 0 {
                            span { class: "text-gray-400",
                                "unavail " span { class: "text-red-400 font-medium", "{stats.ocr.unavailable}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn format_chars(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[derive(Props, Clone, PartialEq)]
struct TipSubSectionProps {
    title: &'static str,
    items: Vec<&'static str>,
}

#[component]
fn TipSubSection(props: TipSubSectionProps) -> Element {
    rsx! {
        div { class: "mt-2",
            span { class: "text-xs font-semibold text-gray-300", "{props.title}" }
            ul { class: "mt-1 ml-4 space-y-0.5 list-disc list-outside text-gray-400",
                for item in &props.items {
                    li { "{item}" }
                }
            }
        }
    }
}
