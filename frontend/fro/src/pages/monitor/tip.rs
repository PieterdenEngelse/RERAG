use crate::api::{fetch_canon_stats, fetch_chunking_stats, fetch_parser_stats, CanonStats, CallSiteStats, ChunkingStatsSnapshot, FileRecord, ParserStats, StoreRecord};
use crate::app::Route;
use crate::components::monitor::*;
use crate::pages::hardware::constants::{
    INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
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
pub fn MonitorTip() -> Element {
    let mut show_tip_info = use_signal(|| false);
    let mut tip_tab = use_signal(|| 0u8);
    let mut show_parser_info = use_signal(|| false);
    let mut show_preprocessing_info = use_signal(|| false);
    let mut show_nfc_info = use_signal(|| false);
    let mut show_nfkc_info = use_signal(|| false);
    let mut show_nfkc_punct_info = use_signal(|| false);
    let mut show_store_target_info = use_signal(|| false);
    let mut show_format_cleanup_info = use_signal(|| false);
    let mut show_dedupe_pdf_info = use_signal(|| false);
    let mut show_noise_nodes_info = use_signal(|| false);
    let mut show_boilerplate_nodes_info = use_signal(|| false);
    let mut show_kg_info = use_signal(|| false);
    let mut show_mojibake_info = use_signal(|| false);
    let mut show_clustering_info = use_signal(|| false);
    let mut show_centroid_info = use_signal(|| false);
    let mut show_pq_training_info = use_signal(|| false);
    let mut show_emdash_info = use_signal(|| false);
    let mut show_ligature_info = use_signal(|| false);
    let mut show_extractors_info = use_signal(|| false);
    let mut show_chunker_info = use_signal(|| false);
    let mut parser_stats: Signal<Option<Result<ParserStats, String>>> = use_signal(|| None);
    let mut chunking_stats: Signal<Option<Result<Vec<ChunkingStatsSnapshot>, String>>> = use_signal(|| None);
    let mut canon_stats: Signal<Option<Result<CanonStats, String>>> = use_signal(|| None);

    use_future(move || async move {
        loop {
            parser_stats.set(Some(fetch_parser_stats().await));
            chunking_stats.set(Some(
                fetch_chunking_stats(20).await.map(|r| r.snapshots)
            ));
            canon_stats.set(Some(fetch_canon_stats().await));
            TimeoutFuture::new(5_000).await;
        }
    });

    rsx! {
        div { class: "p-6 text-gray-300",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("TIP", None),
                ]
            }
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

            // Pipeline layout: Parser → Typography & Tag Cleanup → Canonicalize NFC → Chunker → ┬ Canonicalize NFKC
            //                                                                                    └ Canonicalize NFKC+punct
            div { class: "flex gap-2 items-stretch",

                // ── Parser ──
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 flex-1 min-w-0", style: "height:288px;",
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
                        span { class: "text-xs text-gray-400", "7 days" }
                    }
                    match &*parser_stats.read() {
                        Some(Ok(stats)) => rsx! { ParserStatsView { stats: stats.clone() } },
                        Some(Err(e)) => rsx! { p { class: "text-xs text-red-400", "Error: {e}" } },
                        None => rsx! { p { class: "text-xs text-gray-500", "Loading…" } },
                    }
                }

                // arrow
                div { class: "flex items-center text-gray-500 text-lg flex-shrink-0", "→" }

                // ── Typography & Tag Cleanup ──
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 flex-1 min-w-0", style: "height:288px;",
                    div { class: "flex items-center justify-between mb-3",
                        div { class: "flex items-center gap-2",
                            h3 { class: "text-sm font-semibold text-gray-200", "Typography & Tag Cleanup" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_preprocessing_info.set(true),
                                title: "About Typography & Tag Cleanup",
                                InfoIcon {}
                            }
                        }
                        span { class: "text-xs text-gray-400", "format-keyed" }
                    }
                    div { class: "text-xs text-gray-400 space-y-2",
                        div { class: "flex items-start gap-2",
                            span { class: "text-cyan-400 font-mono shrink-0", "HTML" }
                            span { "strip tags" }
                        }
                        div { class: "flex items-start gap-2",
                            span { class: "text-amber-400 font-mono shrink-0", "PDF" }
                            span { "unicode fix" }
                        }
                        div { class: "flex items-start gap-2",
                            span { class: "text-amber-400 font-mono shrink-0", "DOCX" }
                            span { "unicode fix" }
                        }
                        div { class: "flex items-start gap-2",
                            span { class: "text-amber-400 font-mono shrink-0", "ODT" }
                            span { "unicode fix" }
                        }
                        div { class: "flex items-start gap-2",
                            span { class: "text-amber-400 font-mono shrink-0", "EPUB" }
                            span { "unicode fix" }
                        }
                        div { class: "flex items-start gap-2",
                            span { class: "text-amber-400 font-mono shrink-0", "PPTX" }
                            span { "unicode fix" }
                        }
                        div { class: "flex items-start gap-2 pt-1 border-t border-gray-700",
                            span { class: "text-gray-500 font-mono shrink-0", "TXT/MD" }
                            span { class: "text-gray-500", "pass-through" }
                        }
                    }
                }

                // arrow
                div { class: "flex items-center text-gray-500 text-lg flex-shrink-0", "→" }

                // ── Canonicalize NFC ──
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 flex-1 min-w-0", style: "height:288px;",
                    div { class: "flex items-center justify-between mb-3",
                        div { class: "flex items-center gap-2",
                            h3 { class: "text-sm font-semibold text-gray-200", "Canonicalize NFC" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_nfc_info.set(true),
                                title: "About NFC canonicalization",
                                InfoIcon {}
                            }
                        }
                        span { class: "text-xs text-gray-400", "NFC + whitespace" }
                    }
                    match &*canon_stats.read() {
                        Some(Ok(stats)) => rsx! { StoreRecordsView { records: stats.store_records.clone() } },
                        Some(Err(e)) => rsx! { p { class: "text-xs text-red-400", "Error: {e}" } },
                        None => rsx! { p { class: "text-xs text-gray-500", "Loading…" } },
                    }
                }

                // arrow
                div { class: "flex items-center text-gray-500 text-lg flex-shrink-0", "→" }

                // ── Chunker ──
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 flex-1 min-w-0", style: "height:288px;",
                    div { class: "flex items-center justify-between mb-3",
                        div { class: "flex items-center gap-2",
                            h3 { class: "text-sm font-semibold text-gray-200", "Chunker" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_chunker_info.set(true),
                                title: "About the Chunker",
                                InfoIcon {}
                            }
                        }
                        span { class: "text-xs text-gray-400", "recent 20" }
                    }
                    match &*chunking_stats.read() {
                        Some(Ok(snaps)) => rsx! { ChunkerStatsView { snapshots: snaps.clone() } },
                        Some(Err(e)) => rsx! { p { class: "text-xs text-red-400", "Error: {e}" } },
                        None => rsx! { p { class: "text-xs text-gray-500", "Loading…" } },
                    }
                }

                // fork arrow
                div { class: "flex items-center text-gray-500 text-lg flex-shrink-0", "→" }

                // ── Parallel branches: NFKC and NFKC+punct ──
                div { class: "flex flex-col gap-2 flex-1 min-w-0",

                    // ── Canonicalize NFKC ──
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 flex-1 min-w-0",
                        div { class: "flex items-center justify-between mb-3",
                            div { class: "flex items-center gap-2",
                                h3 { class: "text-sm font-semibold text-gray-200", "Canonicalize NFKC" }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| show_nfkc_info.set(true),
                                    title: "About NFKC canonicalization",
                                    InfoIcon {}
                                }
                            }
                            span { class: "text-xs text-gray-400", "NFKC + whitespace" }
                        }
                        match &*canon_stats.read() {
                            Some(Ok(stats)) => rsx! {
                                if stats.embed_ingestion.calls == 0 && stats.embed_query.calls == 0 {
                                    div { class: "flex items-center justify-center h-16",
                                        p { class: "text-xs text-gray-500", "Upload a document or run a search" }
                                    }
                                } else {
                                    CanonMiniTable {
                                        rows: vec![
                                            CanonMiniRow { label: "ingest", description: "Applied once per chunk before the embedding model. NFKC strips compatibility differences (fi-ligature to fi, circled-1 to 1, fullwidth to ASCII) so embeddings don't diverge on irrelevant Unicode variants.", site: stats.embed_ingestion.clone() },
                                            CanonMiniRow { label: "query",  description: "Applied to each search query before the embedding model — identical normalization as ingest so query and document vectors are directly comparable.", site: stats.embed_query.clone() },
                                        ]
                                    }
                                }
                            },
                            Some(Err(e)) => rsx! { p { class: "text-xs text-red-400", "Error: {e}" } },
                            None => rsx! { p { class: "text-xs text-gray-500", "Loading…" } },
                        }
                    }

                    // ── Canonicalize NFKC+punct ──
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 flex-1 min-w-0",
                        div { class: "flex items-center justify-between mb-3",
                            div { class: "flex items-center gap-2",
                                h3 { class: "text-sm font-semibold text-gray-200", "Canonicalize NFKC+punct" }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| show_nfkc_punct_info.set(true),
                                    title: "About NFKC+punct canonicalization",
                                    InfoIcon {}
                                }
                            }
                            span { class: "text-xs text-gray-400", "NFKC + whitespace + punct" }
                        }
                        match &*canon_stats.read() {
                            Some(Ok(stats)) => rsx! {
                                if stats.index_ingestion.calls == 0 && stats.index_query.calls == 0 {
                                    div { class: "flex items-center justify-center h-16",
                                        p { class: "text-xs text-gray-500", "Upload a document or run a search" }
                                    }
                                } else {
                                    CanonMiniTable {
                                        rows: vec![
                                            CanonMiniRow { label: "ingest", description: "Upgrades the Embed-normalized chunk for BM25: adds punctuation canonicalization so smart-quoted and plain-quoted terms match the same token.", site: stats.index_ingestion.clone() },
                                            CanonMiniRow { label: "query",  description: "Upgrades the Embed-normalized query for BM25 — identical canonicalization as ingest so query tokens match indexed tokens.", site: stats.index_query.clone() },
                                        ]
                                    }
                                }
                            },
                            Some(Err(e)) => rsx! { p { class: "text-xs text-red-400", "Error: {e}" } },
                            None => rsx! { p { class: "text-xs text-gray-500", "Loading…" } },
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
                        class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),

                        // Sticky header
                        div { class: "flex items-center justify-between px-6 py-3 border-b border-gray-600 shrink-0",
                            h2 { class: "text-base font-semibold text-gray-100",
                                "Text Ingestion Pipeline and Its Role in RAG"
                            }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                onclick: move |_| show_tip_info.set(false),
                                "✕"
                            }
                        }

                        // Tab bar
                        div { class: "flex border-b border-gray-700 shrink-0 px-4 gap-1",
                            button {
                                class: if tip_tab() == 0 { "px-3 py-2 text-xs font-medium text-sky-400 border-b-2 border-sky-400 -mb-px bg-transparent" } else { "px-3 py-2 text-xs font-medium text-gray-400 hover:text-gray-200 border-b-2 border-transparent -mb-px" },
                                onclick: move |_| tip_tab.set(0),
                                "0 · Parser"
                            }
                            button {
                                class: if tip_tab() == 1 { "px-3 py-2 text-xs font-medium text-amber-400 border-b-2 border-amber-400 -mb-px bg-transparent" } else { "px-3 py-2 text-xs font-medium text-gray-400 hover:text-gray-200 border-b-2 border-transparent -mb-px" },
                                onclick: move |_| tip_tab.set(1),
                                "1 · Canonicalization"
                            }
                            button {
                                class: if tip_tab() == 2 { "px-3 py-2 text-xs font-medium text-emerald-400 border-b-2 border-emerald-400 -mb-px bg-transparent" } else { "px-3 py-2 text-xs font-medium text-gray-400 hover:text-gray-200 border-b-2 border-transparent -mb-px" },
                                onclick: move |_| tip_tab.set(2),
                                "2 · Typography & Tag Cleanup"
                            }
                            button {
                                class: if tip_tab() == 3 { "px-3 py-2 text-xs font-medium text-violet-400 border-b-2 border-violet-400 -mb-px bg-transparent" } else { "px-3 py-2 text-xs font-medium text-gray-400 hover:text-gray-200 border-b-2 border-transparent -mb-px" },
                                onclick: move |_| tip_tab.set(3),
                                "3 · Orchestration"
                            }
                            button {
                                class: if tip_tab() == 4 { "px-3 py-2 text-xs font-medium text-gray-200 border-b-2 border-gray-400 -mb-px bg-transparent" } else { "px-3 py-2 text-xs font-medium text-gray-400 hover:text-gray-200 border-b-2 border-transparent -mb-px" },
                                onclick: move |_| tip_tab.set(4),
                                "Pipeline Flow"
                            }
                        }

                        // Tab content — overflow-y-auto so Pipeline Flow tab can scroll if needed
                        div { class: "flex-1 overflow-y-auto min-h-0 px-6 py-4 text-xs text-gray-300",

                            // Intro (shown on all tabs except Pipeline)
                            if tip_tab() != 4 {
                                div { class: "text-xs text-gray-300 mb-3",
                                    "Four components that together form the ingestion pipeline: "
                                    span { class: "text-sky-400 font-semibold underline cursor-pointer hover:text-sky-300", onclick: move |_| tip_tab.set(0), "Parser" }
                                    ", "
                                    span { class: "text-amber-400 font-semibold underline cursor-pointer hover:text-amber-300", onclick: move |_| tip_tab.set(1), "Canonicalization" }
                                    ", "
                                    span { class: "text-emerald-400 font-semibold underline cursor-pointer hover:text-emerald-300", onclick: move |_| tip_tab.set(2), "Typography & Tag Cleanup" }
                                    ", "
                                    span { class: "text-violet-400 font-semibold underline cursor-pointer hover:text-violet-300", onclick: move |_| tip_tab.set(3), "Orchestration" }
                                    ". Canonicalization is not a single discrete stage — it is applied at multiple points around Typography & Tag Cleanup."
                                }
                            }

                            // ── Tab 0: Parser ──
                            if tip_tab() == 0 {
                                div { class: "space-y-2",
                                    h3 { class: "text-xs font-bold text-sky-400 uppercase tracking-wide", "0 · Parser" }
                                    p { class: "text-gray-400",
                                        "Entry point. Reads raw bytes and converts them to plain text via "
                                        span {
                                            class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                            onclick: move |_| show_extractors_info.set(!show_extractors_info()),
                                            "format-specific extractors"
                                        }
                                        " (PDF, HTML, DOCX, XLSX, EPUB, PPTX, text/code). Nothing downstream receives input if this fails."
                                    }
                                    if show_extractors_info() {
                                        div { class: "rounded bg-gray-900 border border-sky-900 p-3 text-xs text-gray-300 space-y-1.5",
                                            div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_extractors_info.set(false), "✕" }
                                            }
                                            // PDF — full width, complex
                                            div {
                                                p { class: "text-gray-200 font-semibold", "PDF — three-level cascade" }
                                                ol { class: "ml-3 list-decimal list-outside text-gray-400 space-y-0.5",
                                                    li { span { class: "font-mono text-gray-200", "pdftotext" } " (poppler-utils, " span { class: "font-mono", "-layout -enc UTF-8" } ") — best quality, handles multi-column layouts and complex fonts." }
                                                    li { span { class: "font-mono text-gray-200", "pdf_extract" } " (Rust crate) — pure-Rust fallback when poppler absent." }
                                                    li { span { class: "font-mono text-gray-200", "pdftoppm" } " + " span { class: "font-mono text-gray-200", "tesseract" } " — OCR last resort for scanned/image PDFs (300 dpi). Requires both on PATH." }
                                                }
                                                p { class: "text-gray-500 mt-0.5",
                                                    span {
                                                        class: "font-mono text-sky-400 underline cursor-pointer hover:text-sky-300",
                                                        onclick: move |_| show_dedupe_pdf_info.set(!show_dedupe_pdf_info()),
                                                        "dedupe_pdf_noise"
                                                    }
                                                    " strips lines appearing 4+ times and ≤80 chars (headers/footers)."
                                                }
                                                if show_dedupe_pdf_info() {
                                                    div { class: "rounded bg-gray-900 border border-gray-700 p-3 text-xs text-gray-300 space-y-2 mt-1",
                                                        div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                            button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_dedupe_pdf_info.set(false), "✕" }
                                                        }
                                                        p { "A heuristic pass that removes repeated boilerplate PDFs accumulate across pages — things like page headers, footers, running titles, and page numbers. These appear on every page, so in a 20-page PDF they repeat 20 times. Without this pass they pollute the index: a search for \"architecture\" starts matching chapter headers instead of content, and the LLM gets context stuffed with repeated boilerplate instead of substance." }
                                                        p { class: "text-gray-200 font-semibold pt-1", "The two-part heuristic" }
                                                        ul { class: "ml-3 space-y-1 list-disc list-outside text-gray-400",
                                                            li { span { class: "text-gray-200 font-medium", "4+ repetitions — " } "a line appearing that many times is almost certainly structural, not content. Real sentences rarely repeat verbatim across a document." }
                                                            li { span { class: "text-gray-200 font-medium", "≤80 chars — " } "actual content tends to be longer. This guard prevents accidentally removing a short sentence that genuinely repeats (e.g. a refrain in a poem, a repeated warning in a manual)." }
                                                        }
                                                        p { class: "text-gray-500 italic", "Not perfect — a long footer survives, a short genuine refrain gets stripped — but it's a cheap, zero-dependency pass that handles the majority of noisy PDFs." }
                                                    }
                                                }
                                            }
                                            // 2-column grid for the rest
                                            div { class: "grid grid-cols-2 gap-x-4 gap-y-1.5",
                                                div {
                                                    p { class: "text-gray-200 font-semibold", "HTML" }
                                                    p { class: "text-gray-400", "Custom smart extractor — strips tags, decodes entities." }
                                                }
                                                div {
                                                    p { class: "text-gray-200 font-semibold", "DOCX / ODT" }
                                                    p { class: "text-gray-400", "ZIP archive → reads " span { class: "font-mono", "word/document.xml" } " / " span { class: "font-mono", "content.xml" } " → char-by-char tag stripper." }
                                                }
                                                div {
                                                    p { class: "text-gray-200 font-semibold", "XLSX / ODS" }
                                                    p { class: "text-gray-400", span { class: "font-mono", "calamine" } " crate — all sheets, cells → tab-separated rows, " span { class: "font-mono", "[Sheet: name]" } " headers for multi-sheet files." }
                                                }
                                                div {
                                                    p { class: "text-gray-200 font-semibold", "EPUB" }
                                                    p { class: "text-gray-400", "ZIP → all " span { class: "font-mono", ".xhtml/.html/.htm" } " entries → tag-strip → concatenate." }
                                                }
                                                div {
                                                    p { class: "text-gray-200 font-semibold", "PPTX" }
                                                    p { class: "text-gray-400", "ZIP → " span { class: "font-mono", "ppt/slides/slide*.xml" } " → tag-strip → concatenate." }
                                                }
                                                div {
                                                    p { class: "text-gray-200 font-semibold", "Text / code / CSV / JSON / XML" }
                                                    p { class: "text-gray-400", span { class: "font-mono", "detect_and_decode" } " — charset detection + UTF-8 decode." }
                                                }
                                            }
                                            p { class: "text-gray-500 italic", "Every format then goes through Format cleanup → normalize(Store)." }
                                        }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Embeddings" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Failed parse → zero chunks, zero vectors." }
                                        li {
                                            span {
                                                class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                                onclick: move |_| show_extractors_info.set(!show_extractors_info()),
                                                "Format-specific extractors"
                                            }
                                            " preserve linear reading order and document boundaries (sheet names, slide sequence)."
                                        }
                                        li { "Headings, tables, and emphasis are not preserved — all text is flattened to plain text." }
                                        li { "OCR fallback (300 dpi) recovers scanned PDFs." }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Graph" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Empty parse → no graph nodes for that document." }
                                        li {
                                            span {
                                                class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                                onclick: move |_| show_dedupe_pdf_info.set(!show_dedupe_pdf_info()),
                                                "PDF header/footer dedup"
                                            }
                                            " prevents "
                                            span {
                                                class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                                onclick: move |_| show_noise_nodes_info.set(!show_noise_nodes_info()),
                                                "noise nodes"
                                            }
                                            " in the "
                                            span {
                                                class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                                onclick: move |_| show_kg_info.set(!show_kg_info()),
                                                "knowledge graph"
                                            }
                                            "."
                                        }
                                        if show_noise_nodes_info() {
                                            div { class: "rounded bg-gray-900 border border-gray-700 p-3 text-xs text-gray-300 space-y-2 mt-1",
                                                div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                    button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_noise_nodes_info.set(false), "✕" }
                                                }
                                                p { "Every chunk becomes a node in the "
                                                    span {
                                                        class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                                        onclick: move |_| show_kg_info.set(!show_kg_info()),
                                                        "knowledge graph"
                                                    }
                                                    " (when Neo4j is enabled). Without dedup, every page header and footer becomes a chunk, and each chunk becomes a node — so a 20-page PDF produces 20 identical nodes for " span { class: "font-mono", "\"Chapter 3 — Architecture\"" } " and 20 more for " span { class: "font-mono", "\"© 2024 Acme Corp\"" } " (when they are in a header or footer)." }
                                                p { "These noise nodes are structurally meaningless — they carry no content, but they connect to real content nodes via co-occurrence edges. Graph traversal and entity extraction then has to wade through dozens of "
                                                    span {
                                                        class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                                        onclick: move |_| show_boilerplate_nodes_info.set(!show_boilerplate_nodes_info()),
                                                        "boilerplate nodes"
                                                    }
                                                    " to reach the actual knowledge."
                                                }
                                                if show_boilerplate_nodes_info() {
                                                    div { class: "rounded bg-gray-900 border border-gray-700 p-3 text-xs text-gray-300 space-y-1.5 mt-1",
                                                        div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                            button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_boilerplate_nodes_info.set(false), "✕" }
                                                        }
                                                        p { "Boilerplate nodes are graph nodes that originate from repeated, non‑informative text such as:" }
                                                        ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                            li { "page headers (" span { class: "font-mono", "Chapter 3 — Architecture" } ")" }
                                                            li { "page footers (" span { class: "font-mono", "© 2024 Acme Corp" } ")" }
                                                            li { "navigation elements (" span { class: "font-mono", "Table of Contents" } ", " span { class: "font-mono", "Page 12 of 200" } ")" }
                                                            li { "standard disclaimers" }
                                                            li { "document metadata that appears on every page" }
                                                        }
                                                        p { class: "text-gray-200 italic", "They are syntactically present but semantically empty." }
                                                    }
                                                }
                                                p { "They also corrupt "
                                                    span {
                                                        class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                                        onclick: move |_| show_pq_training_info.set(!show_pq_training_info()),
                                                        "Product Quantization codebook training"
                                                    }
                                                    " — boilerplate text produces real embeddings, so noise nodes pull PQ centroids toward meaningless content. The codebook then encodes genuine content chunks less accurately, degrading vector search recall."
                                                }
                                                if show_pq_training_info() {
                                                    div { class: "rounded bg-gray-900 border border-gray-700 p-3 text-xs text-gray-300 space-y-2 mt-1",
                                                        div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                            button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_pq_training_info.set(false), "✕" }
                                                        }
                                                        p { span { class: "text-gray-200 font-medium", "Product Quantization (PQ)" } " compresses embedding vectors to ~1/32 of their original size. A 384-dim f32 vector (1536 bytes) becomes 48 bytes — one byte per subvector." }
                                                        p { class: "text-gray-200 font-medium pt-1", "Training" }
                                                        p { "Training builds a " span { class: "font-mono text-gray-200", "PQCodebook" } " — a lookup table of centroids. It runs once over all stored vectors:" }
                                                        ol { class: "ml-3 space-y-1 list-decimal list-outside text-gray-400",
                                                            li { "Split each 384-dim vector into " span { class: "font-mono", "num_subvectors" } " (default 48) equal sub-vectors of 8 dims each." }
                                                            li { "For each subspace, run k-means over all sub-vectors to find 256 centroids. Each centroid is the average position of the vectors assigned to it. Centroids are initialised from the first 256 vectors in the corpus, then refined over several iterations." }
                                                            li { "Store the resulting " span { class: "font-mono", "num_subvectors × 256" } " centroid table as the codebook." }
                                                        }
                                                        p { class: "text-gray-200 font-medium pt-1", "Encoding (after training)" }
                                                        p { class: "text-gray-400", "Each new vector is encoded by finding the nearest centroid in each subspace and storing its index (0–255) as one byte. At search time, distances are approximated from the codebook without decompressing." }
                                                        p { class: "text-gray-200 font-medium pt-1", "Why boilerplate corrupts training" }
                                                        p { class: "text-gray-400", "A centroid is the average of all vectors assigned to it. Boilerplate embeddings — which cluster tightly around phrases like " span { class: "font-mono", "\"© 2024 Acme Corp\"" } " — also get assigned to centroids and pull the average toward themselves. The centroid moves away from where it would have settled with content-only training." }
                                                        p { class: "text-gray-400", "At encode time, genuine content vectors get assigned to a centroid that no longer accurately represents them. Two chunks that should be close in vector space may get encoded to the same centroid ID; two dissimilar chunks may get IDs that happen to be near each other. Either way the distance estimates used at search time are wrong." }
                                                        p { class: "text-gray-200 italic", "Training is done once at reindex time — the damage is baked into the codebook until the next reindex." }
                                                    }
                                                }
                                                p { class: "text-gray-200 italic", "Noise nodes are a graph problem caused by a parsing problem — deduplication at the parser stage is cheaper than filtering them out of the graph later." }
                                            }
                                        }
                                        if show_kg_info() {
                                            div { class: "rounded bg-gray-900 border border-sky-900 p-3 text-xs text-gray-300 space-y-2 mt-1",
                                                div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                    button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_kg_info.set(false), "✕" }
                                                }
                                                p { class: "text-gray-200 font-semibold", "What Is a Knowledge Graph?" }
                                                p { "A knowledge graph is a structured representation of information as a network of entities and relationships." }
                                                ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                    li { span { class: "text-gray-200 font-medium", "Nodes" } " represent entities (people, places, concepts, documents)." }
                                                    li { span { class: "text-gray-200 font-medium", "Edges" } " represent relationships between them (" span { class: "font-mono", "works at" } ", " span { class: "font-mono", "located in" } ", " span { class: "font-mono", "mentions" } ", " span { class: "font-mono", "depends on" } ")." }
                                                }
                                                p { class: "text-gray-200 font-semibold pt-1", "Why Knowledge Graphs for RAG?" }
                                                p { "Standard RAG retrieves isolated chunks. Knowledge graphs add structure:" }
                                                ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                    li { span { class: "text-gray-200 font-medium", "Multi-hop reasoning" } " — follow relationships across documents." }
                                                    li { span { class: "text-gray-200 font-medium", "Entity disambiguation" } " — distinguish same-name entities by context." }
                                                    li { span { class: "text-gray-200 font-medium", "Cross-document connections" } " — link related chunks via shared entities." }
                                                    li { span { class: "text-gray-200 font-medium", "Structural context" } " — understand how concepts relate, not just what they are." }
                                                }
                                                p { class: "text-gray-200 font-semibold pt-1", "In the AG System" }
                                                p { "AG uses a two-tier graph architecture:" }
                                                ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                    li { span { class: "font-mono text-gray-200", "Neo4j" } " — ingestion-time graph building. Extracts entities, builds relationships, stores the full knowledge graph." }
                                                    li { span { class: "font-mono text-gray-200", "Petgraph" } " — runtime graph queries. Loads an exported JSON snapshot from Neo4j into RAM for fast, in-process traversal." }
                                                }
                                                p { class: "text-gray-200 italic", "Neo4j never runs at query time. All runtime graph traversal goes through petgraph — nanoseconds with no network overhead." }
                                            }
                                        }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1",
                                        span {
                                            class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                            onclick: move |_| show_clustering_info.set(!show_clustering_info()),
                                            "Clustering"
                                        }
                                    }
                                    if show_clustering_info() {
                                        div { class: "rounded bg-gray-900 border border-sky-900 p-3 text-xs text-gray-300 space-y-2 mb-1",
                                            div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_clustering_info.set(false), "✕" }
                                            }

                                            // 1. What clustering is
                                            p { class: "text-gray-200 font-semibold", "1 · What clustering is" }
                                            p { "Clustering groups embedding vectors so that semantically similar chunks end up together. It's unsupervised: no labels, no supervision — just geometry in vector space." }
                                            p { "In a GraphRAG pipeline, clustering is the backbone of:" }
                                            ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                li { "topic‑level summaries" }
                                                li { "hierarchical retrieval" }
                                                li { "global graph coherence" }
                                                li { "PQ codebook stability" }
                                                li { "deduplication and noise filtering" }
                                            }
                                            p { class: "text-gray-200 italic", "Cluster quality → summary quality → global search quality." }

                                            // 2. Why parse quality sets the ceiling
                                            p { class: "text-gray-200 font-semibold pt-1", "2 · Why parse quality sets the ceiling on cluster coherence" }
                                            p { "Clustering can only be as good as the text fed into the embedding model. If parsing is sloppy:" }
                                            ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                li { "broken sentences" }
                                                li { "duplicated boilerplate" }
                                                li { "HTML artifacts" }
                                                li { "OCR noise" }
                                                li { "mojibake" }
                                            }
                                            p { "…then embeddings scatter → clusters smear → summaries degrade." }

                                            // 3. Why encoding detection prevents mojibake fragmentation
                                            p { class: "text-gray-200 font-semibold pt-1", "3 · Why encoding detection prevents mojibake fragmentation" }
                                            p { "Mojibake = garbled text caused by decoding bytes with the wrong encoding (e.g., " span { class: "font-mono", "CafÃ©" } " instead of " span { class: "font-mono", "Café" } ", " span { class: "font-mono", "â€œHelloâ€" } " instead of " span { class: "font-mono", "\u{201c}Hello\u{201d}" } ")." }
                                            p { "Mojibake breaks characters into multiple meaningless tokens, which:" }
                                            ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                li { "distort embeddings" }
                                                li { "scatter chunks across clusters" }
                                                li { "pollute "
                                                    span {
                                                        class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                                        onclick: move |_| show_centroid_info.set(!show_centroid_info()),
                                                        "centroids"
                                                    }
                                                }
                                                li { "degrade PQ codebooks" }
                                            }
                                            p { "Encoding detection ensures clean Unicode before tokenization." }

                                            // 4. How clusters are defined and made
                                            p { class: "text-gray-200 font-semibold pt-1", "4 · How clusters are defined and made" }
                                            p { "A cluster is a region in embedding space where vectors are closer to each other than to vectors in other regions, with internal density and separation from others by lower‑density areas or distance boundaries. Different algorithms define \"region\" differently." }

                                            p { class: "text-gray-200 font-medium pt-0.5", "4.1 · k‑means — clusters = Voronoi cells around "
                                                span {
                                                    class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                                    onclick: move |_| show_centroid_info.set(!show_centroid_info()),
                                                    "centroids"
                                                }
                                            }
                                            if show_centroid_info() {
                                                div { class: "rounded bg-gray-900 border border-gray-700 p-3 text-xs text-gray-300 space-y-2 mt-1",
                                                    div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                        button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_centroid_info.set(false), "✕" }
                                                    }
                                                    p { "A centroid is the center point of a cluster — the average position of all vectors assigned to that cluster." }
                                                    p { "In embedding‑space terms, it's the mean vector: " span { class: "font-mono text-gray-200", "μ = (1/N) Σ xᵢ" } " where " span { class: "font-mono", "xᵢ" } " is an embedding vector, " span { class: "font-mono", "N" } " the cluster size, and " span { class: "font-mono", "μ" } " the centroid." }
                                                    p { class: "text-gray-200 font-semibold pt-0.5", "Intuitively" }
                                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                        li { "the semantic center of gravity of a cluster" }
                                                        li { "the point that best represents the \"topic\" of that cluster" }
                                                        li { "the anchor around which all cluster members are grouped" }
                                                    }
                                                    p { "If you average all embeddings of \"Rust async networking,\" the centroid becomes the prototype of that topic." }
                                                    p { class: "text-gray-200 font-semibold pt-0.5", "Why centroids matter" }
                                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                        li { "cluster boundaries (in k‑means they define Voronoi cells)" }
                                                        li { "which cluster a new point belongs to (nearest centroid)" }
                                                        li { "how stable a cluster is (centroid drift = instability)" }
                                                        li { "how summaries are generated (centroid ≈ semantic center)" }
                                                    }
                                                    p { "In PQ codebooks, centroids are even more literal: they are the codebook entries." }
                                                    p { class: "text-gray-200 font-semibold pt-0.5", "How centroids are used in k‑means" }
                                                    ol { class: "ml-3 space-y-0.5 list-decimal list-outside text-gray-400",
                                                        li { "Start with k initial centroids" }
                                                        li { "Assign each vector to the nearest centroid" }
                                                        li { "Recompute each centroid as the mean of its assigned vectors" }
                                                        li { "Repeat until centroids stop moving" }
                                                    }
                                                    p { class: "text-gray-200 font-semibold pt-0.5", "Not all algorithms use centroids" }
                                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                        li { span { class: "font-mono text-gray-200", "k‑means" } " → centroids exist and define clusters" }
                                                        li { span { class: "font-mono text-gray-200", "GMM" } " → centroids exist as Gaussian means" }
                                                        li { span { class: "font-mono text-gray-200", "DBSCAN / HDBSCAN" } " → no centroids; clusters are density regions" }
                                                        li { "Hierarchical clustering → no centroids; clusters are tree nodes" }
                                                    }
                                                    p { class: "text-gray-200 italic", "\"Centroid\" is algorithm‑specific." }
                                                }
                                            }
                                            ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                li { span { class: "text-gray-200", "Definition: " } "A cluster is the set of points closest to a centroid." }
                                                li { span { class: "text-gray-200", "Formation: " } "pick k centroids → assign points to nearest → recompute centroids → repeat." }
                                                li { span { class: "text-gray-200", "Overlap: " } "❌ No — hard partitions." }
                                            }

                                            p { class: "text-gray-200 font-medium pt-0.5", "4.2 · DBSCAN — clusters = dense regions separated by sparse regions" }
                                            ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                li { span { class: "text-gray-200", "Definition: " } "A cluster is a connected component of high‑density points." }
                                                li { span { class: "text-gray-200", "Formation: " } "identify core points (≥ min_samples neighbors) → expand outward → mark unreachable points as noise." }
                                                li { span { class: "text-gray-200", "Overlap: " } "❌ No — boundaries are fuzzy and shapes are irregular." }
                                            }

                                            p { class: "text-gray-200 font-medium pt-0.5", "4.3 · HDBSCAN — clusters = stable density regions across scales" }
                                            ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                li { span { class: "text-gray-200", "Definition: " } "A cluster is a persistent dense region that remains stable across multiple density thresholds." }
                                                li { span { class: "text-gray-200", "Formation: " } "build MST of distances → condense into hierarchy → extract stable regions." }
                                                li { span { class: "text-gray-200", "Overlap: " } "❌ No — but clusters can be nested (hierarchical)." }
                                            }

                                            p { class: "text-gray-200 font-medium pt-0.5", "4.4 · Gaussian Mixture Models (GMM) — clusters = overlapping probability distributions" }
                                            ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                li { span { class: "text-gray-200", "Definition: " } "Each cluster is a Gaussian distribution in embedding space." }
                                                li { span { class: "text-gray-200", "Formation: " } "EM algorithm fits multiple Gaussians." }
                                                li { span { class: "text-gray-200", "Overlap: " } "✅ Yes — soft clustering. A point can be 70% cluster A, 20% B, 10% C." }
                                            }

                                            // 5. Absolute or overlapping?
                                            p { class: "text-gray-200 font-semibold pt-1", "5 · So are clusters absolute or overlapping?" }
                                            div { class: "overflow-x-auto",
                                                table { class: "text-xs w-full border-collapse",
                                                    thead {
                                                        tr { class: "border-b border-gray-700",
                                                            th { class: "text-left text-gray-200 pr-4 pb-1", "Algorithm" }
                                                            th { class: "text-left text-gray-200 pr-4 pb-1", "Overlap" }
                                                            th { class: "text-left text-gray-200 pr-4 pb-1", "Membership" }
                                                            th { class: "text-left text-gray-200 pb-1", "Notes" }
                                                        }
                                                    }
                                                    tbody { class: "text-gray-400",
                                                        tr { class: "border-b border-gray-800",
                                                            td { class: "font-mono pr-4 py-0.5", "k‑means" }
                                                            td { class: "pr-4 py-0.5", "❌ No" }
                                                            td { class: "pr-4 py-0.5", "Hard" }
                                                            td { class: "py-0.5", "Voronoi cells" }
                                                        }
                                                        tr { class: "border-b border-gray-800",
                                                            td { class: "font-mono pr-4 py-0.5", "DBSCAN" }
                                                            td { class: "pr-4 py-0.5", "❌ No" }
                                                            td { class: "pr-4 py-0.5", "Hard" }
                                                            td { class: "py-0.5", "Density islands; noise allowed" }
                                                        }
                                                        tr { class: "border-b border-gray-800",
                                                            td { class: "font-mono pr-4 py-0.5", "HDBSCAN" }
                                                            td { class: "pr-4 py-0.5", "❌ No" }
                                                            td { class: "pr-4 py-0.5", "Hard + hierarchy" }
                                                            td { class: "py-0.5", "Nested clusters possible" }
                                                        }
                                                        tr {
                                                            td { class: "font-mono pr-4 py-0.5", "GMM" }
                                                            td { class: "pr-4 py-0.5", "✅ Yes" }
                                                            td { class: "pr-4 py-0.5", "Soft" }
                                                            td { class: "py-0.5", "Probabilistic membership" }
                                                        }
                                                    }
                                                }
                                            }
                                            p { "In most GraphRAG pipelines (k‑means or HDBSCAN): clusters do not overlap, but semantic boundaries are fuzzy, and cluster meaning is not absolute — it depends on embedding quality." }

                                            // 6. Pipeline summary
                                            p { class: "text-gray-200 font-semibold pt-1", "6 · Pipeline summary (GraphRAG‑style)" }
                                            ol { class: "ml-3 space-y-0.5 list-decimal list-outside text-gray-400",
                                                li { "Clean + normalize text" }
                                                li { "Detect encoding → prevent mojibake" }
                                                li { "Chunk" }
                                                li { "Embed" }
                                                li { "Cluster (k‑means / HDBSCAN)" }
                                                li { "Summarize clusters" }
                                                li { "Build graph edges" }
                                                li { "Use summaries for global retrieval" }
                                            }
                                            p { class: "text-gray-200 italic", "Cluster quality → summary quality → retrieval quality." }
                                        }
                                    }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Parse quality sets the ceiling on cluster coherence." }
                                        li { "Encoding detection prevents "
                                            span {
                                                class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                                onclick: move |_| show_mojibake_info.set(!show_mojibake_info()),
                                                "mojibake"
                                            }
                                            " token fragmentation."
                                        }
                                        if show_mojibake_info() {
                                            div { class: "rounded bg-gray-900 border border-sky-900 p-3 text-xs text-gray-300 space-y-2 mt-1",
                                                div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                    button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_mojibake_info.set(false), "✕" }
                                                }
                                                p { span { class: "text-gray-200 font-semibold", "Mojibake" } " = garbled, unreadable text caused by using the wrong character encoding. It happens when bytes written in one encoding (e.g., UTF‑8) are interpreted as another (e.g., Latin‑1)." }
                                                p { class: "text-gray-200 font-semibold pt-1", "What mojibake actually is" }
                                                p { "Mojibake (Japanese: 文字化け, \"character transformation\") refers to text that turns into nonsense symbols because software decodes it with the wrong encoding." }
                                                p { "Examples:" }
                                                ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                    li { span { class: "font-mono text-gray-200", "CafÃ©" } " instead of " span { class: "font-mono text-gray-200", "Café" } }
                                                    li { span { class: "font-mono text-gray-200", "â€œHelloâ€" } " instead of " span { class: "font-mono text-gray-200", "\u{201c}Hello\u{201d}" } }
                                                    li { "Japanese text turning into " span { class: "font-mono text-gray-200", "æ–‡åŒ–ã" } }
                                                }
                                                p { "This happens when the byte sequence is correct, but the decoder guesses the wrong character set. For example, UTF‑8 bytes interpreted as Windows‑1252 produce systematic corruption." }
                                            }
                                        }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Summarization" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Noisy extraction directly degrades summaries." }
                                        li { "Boilerplate removal reduces summarizer distraction." }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Retrieval" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Bad parsing is invisible at search time but explains most retrieval failures." }
                                        li { "Empty parse → document silently absent from all results." }
                                        li { "Format drift shifts text quality across the entire index." }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1",
                                        span {
                                            class: "text-sky-400 underline cursor-pointer hover:text-sky-300",
                                            onclick: move |_| show_format_cleanup_info.set(!show_format_cleanup_info()),
                                            "Format cleanup"
                                        }
                                    }
                                    p { class: "text-gray-400", "Runs immediately after format extraction, before Store normalization. Two passes, each applied only to the formats that need it." }
                                    if show_format_cleanup_info() {
                                        div { class: "rounded bg-gray-900 border border-sky-900 p-3 text-xs text-gray-300 space-y-2",
                                            div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_format_cleanup_info.set(false), "✕" }
                                            }
                                            p { "Removes artifacts that are byproducts of the source format, not the content itself. The extractor gives you text, format cleanup gives you " span { class: "italic", "clean" } " text." }
                                            p { class: "text-gray-400 pt-1 font-semibold text-gray-200", "Pass 1 — HTML tag stripping" }
                                            p { class: "text-gray-400", "HTML only. The extractor preserves markup to avoid losing structure; this pass removes all tags and decodes HTML entities, leaving only the text nodes." }
                                            p { class: "text-gray-400 pt-1 font-semibold text-gray-200", "Pass 2 — Unicode/typography cleanup" }
                                            p { class: "text-gray-400", "PDF, DOCX, ODT, EPUB, PPTX, HTML. Folds characters that publishing tools emit but that have no semantic value in plain text:" }
                                            ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                                li { "Curly quotes (\u{2018}\u{2019}\u{201C}\u{201D}) → straight ASCII ' \"" }
                                                li { "Em-dash (\u{2014}) / en-dash (\u{2013}) → \" - \"" }
                                                li { "Non-breaking hyphen (\u{2011}) → \"-\"" }
                                                li { "Ellipsis (\u{2026}) → \"...\"" }
                                                li { "PDF ligatures (ﬁ ﬂ ﬀ ﬃ ﬄ ﬆ) → letter pairs (fi fl ff ffi ffl st)" }
                                            }
                                            p { class: "text-gray-500 pt-1", "Runs before NFC so the canonicalizer sees consistent input regardless of source format." }
                                            p { class: "text-gray-500", "Text and code skip both passes — they arrive as clean UTF-8 with no format artifacts." }
                                        }
                                    }
                                    p { class: "italic text-gray-300 pt-1", "The unseeable first cause of retrieval quality." }
                                }
                            }

                            // ── Tab 1: Canonicalization ──
                            if tip_tab() == 1 {
                                div { class: "space-y-2",
                                    h3 { class: "text-xs font-bold text-amber-400 uppercase tracking-wide", "1 · Canonicalization" }
                                    p { class: "text-gray-400", "Three targets, applied at different stages. Each target is a strict superset of the previous." }

                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1",
                                        span {
                                            class: "text-amber-400 underline cursor-pointer hover:text-amber-300",
                                            onclick: move |_| show_store_target_info.set(!show_store_target_info()),
                                            "Store"
                                        }
                                        "  —  "
                                        a { href: "https://unicode.org/reports/tr15/#NFC", target: "_blank", class: "text-amber-400 hover:text-amber-300 underline", "NFC" }
                                        " + whitespace"
                                    }
                                    if show_store_target_info() {
                                        div { class: "rounded bg-gray-900 border border-amber-800 p-3 text-xs text-gray-300 space-y-2",
                                            div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_store_target_info.set(false), "✕" }
                                            }
                                            p { "\"Store\" is the first of three normalization targets in the TIP canonicalization stage. It's not a data structure — it's the label for how text is normalized when it's being written to disk (stored/persisted)." }
                                            p { class: "text-gray-400 pt-1", "The three targets are a hierarchy of increasing aggressiveness:" }
                                            ol { class: "ml-3 space-y-1.5 list-decimal list-outside text-gray-400",
                                                li {
                                                    span { class: "text-gray-200 font-semibold", "Store" }
                                                    " — NFC + whitespace collapse. Applied after text extraction, before chunking. The text that ends up persisted to disk and shown to users. Conservative: only unifies byte-level encodings of the same character (e.g. é as U+00E9 vs e+combining accent → both become U+00E9), leaving curly quotes, "
                                                    span {
                                                        class: "text-amber-400 underline cursor-pointer hover:text-amber-300",
                                                        onclick: move |_| show_emdash_info.set(!show_emdash_info()),
                                                        "em-dashes"
                                                    }
                                                    ", "
                                                    span {
                                                        class: "text-amber-400 underline cursor-pointer hover:text-amber-300",
                                                        onclick: move |_| show_ligature_info.set(!show_ligature_info()),
                                                        "ligatures"
                                                    }
                                                    " intact."
                                                    if show_emdash_info() {
                                                        div { class: "rounded bg-gray-900 border border-amber-800 p-3 text-xs text-gray-300 space-y-2 mt-2",
                                                            div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                                button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_emdash_info.set(false), "✕" }
                                                            }
                                                            p { "The long dash character — like the one in this sentence. Unicode U+2014. Used in prose for parenthetical asides or breaks in thought. Named \"em\" because it is roughly the width of the letter M." }
                                                            p { "The en-dash (U+2013) is the shorter sibling — used for ranges like \"pages 10–20\"." }
                                                            p { class: "text-gray-400", "NFC leaves both untouched. The "
                                                                span { class: "text-amber-400 font-medium", "Format cleanup" }
                                                                " step folds em/en-dashes to \" - \", but that happens before Store normalization. If a document reaches NFC with an em-dash still in it, NFC will not touch it."
                                                            }
                                                        }
                                                    }
                                                    if show_ligature_info() {
                                                        div { class: "rounded bg-gray-900 border border-amber-800 p-3 text-xs text-gray-300 space-y-2 mt-2",
                                                            div { class: "flex justify-end -mt-1 -mr-1 mb-1",
                                                                button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_ligature_info.set(false), "✕" }
                                                            }
                                                            p { "Typographic letter combinations encoded as a single Unicode character by PDF and publishing tools. Instead of two separate characters f + i, some fonts encode them as the single character " span { class: "font-mono", "ﬁ" } " (U+FB01)." }
                                                            p { class: "text-gray-400 font-mono", "ﬁ (fi)  ﬂ (fl)  ﬀ (ff)  ﬃ (ffi)  ﬄ (ffl)  ﬆ (st)" }
                                                            p { class: "text-gray-400", "NFC leaves ligatures intact — they are single canonical code points, not encoding variants. It is the "
                                                                span { class: "text-amber-400 font-medium", "Embed" }
                                                                " step (NFKC) that folds them into their letter pairs, so the embedder sees \"fi\" not \"ﬁ\"."
                                                            }
                                                        }
                                                    }
                                                }
                                                li {
                                                    span { class: "text-gray-200 font-semibold", "Embed" }
                                                    " — NFKC + whitespace. More aggressive: also folds compatibility equivalents (ﬁ→fi, ①→1, ａｂｃ→abc). Applied to each chunk before the embedder so vectors don't diverge on visually-identical content encoded differently."
                                                }
                                                li {
                                                    span { class: "text-gray-200 font-semibold", "Index" }
                                                    " — NFKC + whitespace + punct canonicalization. Most aggressive: also normalizes smart quotes to ASCII, em-dashes to -, etc. Applied to Tantivy BM25 chunks and matched at query time."
                                                }
                                            }
                                            p { class: "text-gray-500 pt-1 italic", "So when the modal says \"Store — NFC + whitespace\", it means: this is the normalization form used for the copy that gets persisted to disk." }
                                        }
                                    }
                                    p { class: "text-gray-400", "Applied after extraction, before chunking. Persisted to disk and shown to users." }
                                    p { class: "text-gray-400", "NFC = canonical composition. It only normalizes canonical equivalences — different byte encodings of the same character — leaving typography intact. Example: \"é\" can be stored as a single code point (U+00E9) or as \"e\" + combining accent (U+0301). NFC rewrites both to U+00E9. Curly quotes, em-dashes, ligatures, and other typographic characters are left untouched." }

                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1",
                                        "Embed  —  "
                                        a { href: "https://unicode.org/reports/tr15/#NFKC", target: "_blank", class: "text-amber-400 hover:text-amber-300 underline", "NFKC" }
                                        " + whitespace"
                                    }
                                    p { class: "text-gray-400", "Applied to each chunk before the embedder and NER. No punct stripping — the embedding model's tokenizer handles that." }
                                    p { class: "text-gray-400", "NFKC = compatibility composition. Goes further than NFC: it also folds characters that look different but mean the same thing for search. This prevents duplicate embeddings for visually identical content encoded differently." }
                                    p { class: "text-gray-400 font-mono", "① → 1  Ⅳ → IV  ａｂｃ → abc  ﬁ → fi  ² → 2" }

                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Why Store uses NFC and Embed uses NFKC" }
                                    p { class: "text-gray-400", "Store keeps text human-readable and typographically faithful. Embed strips compatibility differences so embeddings don't diverge on irrelevant Unicode quirks. Without this split: \"ﬁ\" and \"fi\" produce different vectors; \"①\" and \"1\" don't match; full-width vs half-width characters cause embedding drift." }

                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Index  —  NFKC + whitespace + punct canon" }
                                    p { class: "text-gray-400", "Applied to Tantivy BM25 chunks and to the query at search time. Punct canon: smart quotes → ASCII, en/em-dash → \"-\", ellipsis → \"...\"" }

                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Whitespace (all targets)" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Strips U+00AD, U+200B–D, U+2060, U+FEFF (zero-width/invisible)" }
                                        li { "CR+LF, lone CR → LF" }
                                        li { "Form feed, vertical tab → LF" }
                                        li { "All Unicode space variants → ASCII space" }
                                        li { "Space runs collapsed — \\n\\n preserved for chunker" }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Query time" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "normalize(Embed) → embedder" }
                                        li { "normalize(Index) → Tantivy BM25" }
                                        li { "Raw query → NER (has its own tokenizer)" }
                                    }
                                    p { class: "italic text-gray-300 pt-1", "The stability layer of the entire RAG system." }
                                }
                            }

                            // ── Tab 2: Typography & Tag Cleanup ──
                            if tip_tab() == 2 {
                                div { class: "space-y-2",
                                    h3 { class: "text-xs font-bold text-emerald-400 uppercase tracking-wide", "2 · Typography & Tag Cleanup" }
                                    p { class: "text-gray-400", "Structures canonicalised text into semantic units for embedding, indexing, and retrieval. Includes chunking, segmentation, and boilerplate removal." }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Embeddings" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Defines embedding granularity (sentence / paragraph / chunk)." }
                                        li { "Ensures embeddings represent coherent semantic content." }
                                        li { "Reduces drift from inconsistent chunk boundaries." }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Graph" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Determines node boundaries." }
                                        li { "Influences edge creation via semantic adjacency." }
                                        li { "Cleaner, more interpretable graph structures." }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Clustering" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Controls cluster density and purity via chunk size." }
                                        li { "Clusters represent topics, not mixed content." }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Summarization" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Well-formed chunks improve summary quality." }
                                        li { "Each chunk contains a unified topic." }
                                        li { "Boilerplate removal reduces summarizer confusion." }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Retrieval" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Each chunk ≈ one semantic idea → better precision." }
                                        li { "Stable ranking from low-noise embedding vectors." }
                                        li { "More accurate context selection for generation." }
                                    }
                                    p { class: "italic text-gray-300 pt-1", "The semantic structuring layer of RAG." }
                                }
                            }

                            // ── Tab 3: Orchestration ──
                            if tip_tab() == 3 {
                                div { class: "space-y-2",
                                    h3 { class: "text-xs font-bold text-violet-400 uppercase tracking-wide", "3 · Orchestration" }
                                    p { class: "text-gray-400", "Coordinates the three prior layers into a deterministic, reproducible ingestion flow. Defines configuration, execution order, and output formats." }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Embeddings" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Deterministic embedding generation across runs." }
                                        li { "Enables caching and incremental indexing." }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Graph" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Stable graph topology across re-indexing cycles." }
                                        li { "Supports versioning and reproducible analytics." }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Clustering" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Consistent cluster assignments over time." }
                                        li { "Enables comparison of cluster evolution." }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Summarization" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Summaries stay stable when the pipeline is unchanged." }
                                        li { "Supports reproducible summary-first retrieval." }
                                    }
                                    h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide pt-1", "Retrieval" }
                                    ul { class: "ml-3 space-y-0.5 list-disc list-outside text-gray-400",
                                        li { "Consistent retrieval rankings." }
                                        li { "Quality changes attributable to data/model, not ingestion drift." }
                                        li { "Reliable evaluation and debugging of retrieval behaviour." }
                                    }
                                    p { class: "italic text-gray-300 pt-1", "The determinism layer of RAG." }
                                }
                            }

                            // ── Tab 4: Pipeline Flow ──
                            if tip_tab() == 4 {
                                div { class: "space-y-4",
                                div { class: "flex justify-end -mt-1 -mr-1",
                                    button { class: "text-gray-500 hover:text-gray-200 text-sm font-bold leading-none", onclick: move |_| show_tip_info.set(false), "✕" }
                                }
                                h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2", "Pipeline flow" }
                                pre { class: "text-xs font-mono leading-snug text-gray-400",
                                    span { class: "text-sky-400", "Parser" }
                                    "  →  raw text\n  │\n  ▼\n"
                                    span { class: "text-sky-300", "Format cleanup" }
                                    "    HTML tag stripping (HTML only)  Unicode/typography cleanup (PDF/DOCX/ODT/EPUB/PPTX/HTML)\n"
                                    "  │  unicode: curly quotes → straight  em/en-dash → \" - \"  ellipsis → \"...\"\n"
                                    "  │           PDF ligatures (ﬁ ﬂ ﬀ) → letter pairs\n"
                                    "  │\n  ▼\n"
                                    span { class: "text-amber-400", "normalize(Store)" }
                                    "   NFC + whitespace collapse\n"
                                    "  │  strips : U+00AD (soft-hyphen)  U+200B–D (zero-width)  U+2060  U+FEFF\n"
                                    "  │  maps   : CR+LF / lone CR → LF  FF (U+000C) / VT (U+000B) → LF\n"
                                    "  │           NBSP + all Unicode space variants → ASCII space\n"
                                    "  │  collapses space runs  preserves \\n\\n for chunker\n"
                                    "  ▼\n"
                                    "stored content  (user-visible, NFC — typographic chars preserved)\n"
                                    "  │\n  ▼\n"
                                    span { class: "text-emerald-400", "Typography & Tag Cleanup (chunker)" }
                                    "  →  chunks\n"
                                    "  │\n"
                                    "  ├──► "
                                    span { class: "text-amber-400", "normalize(Embed)" }
                                    "   NFKC + whitespace collapse  (same collapse as above)\n"
                                    "  │      NFKC decomposes: ligatures (ﬁ→fi  ﬂ→fl)  fullwidth ASCII\n"
                                    "  │                       superscripts  compatibility equivalences\n"
                                    "  │      no punct stripping — embedding model's tokenizer handles that\n"
                                    "  │      → embedder  →  HNSW vector store\n"
                                    "  │\n"
                                    "  └──► "
                                    span { class: "text-amber-400", "normalize(Index)" }
                                    "   NFKC + whitespace collapse + punct canonicalization\n"
                                    "         punct canon: smart quotes → ASCII  en-dash / em-dash → \" - \"\n"
                                    "                      U+2010 / U+2011 / U+2212 → \"-\"  ellipsis → \"...\"\n"
                                    "         → Tantivy BM25 index\n"
                                    "\n── QUERY ───────────────────────────────────────────────────────────────────\n\n"
                                    "raw query\n"
                                    "  ├──► "
                                    span { class: "text-amber-400", "normalize(Embed)" }
                                    "  →  embedder  →  vector search ──────────┐\n"
                                    "  ├──► "
                                    span { class: "text-amber-400", "normalize(Index)" }
                                    "  →  Tantivy BM25 search ─────────────────┤  RRF  →  top-k  →  LLM\n"
                                    "  └──► raw               →  NER  →  graph search ────────────────┘\n"
                                    "         NER receives raw query — has its own tokenizer"
                                }
                            // Summary table
                            div { class: "border-t border-gray-700 pt-3 mt-2",
                                table { class: "w-full text-xs border-collapse",
                                    thead {
                                        tr { class: "border-b border-gray-600",
                                            th { class: "text-left py-1.5 pr-4 text-gray-300 font-semibold", "Layer" }
                                            th { class: "text-left py-1.5 pr-4 text-gray-300 font-semibold", "Function" }
                                            th { class: "text-left py-1.5 text-gray-300 font-semibold", "RAG Impact" }
                                        }
                                    }
                                    tbody {
                                        tr { class: "border-b border-gray-700",
                                            td { class: "py-1.5 pr-4 font-medium text-gray-200", "0 · Parser" }
                                            td { class: "py-1.5 pr-4 text-gray-400", "Extract text from bytes" }
                                            td { class: "py-1.5 text-gray-400", "Empty parse = zero vectors, zero graph nodes, zero results" }
                                        }
                                        tr { class: "border-b border-gray-700",
                                            td { class: "py-1.5 pr-4 font-medium text-gray-200", "1 · Canonicalization" }
                                            td { class: "py-1.5 pr-4 text-gray-400", "Normalize text" }
                                            td { class: "py-1.5 text-gray-400", "Stable embeddings, high recall, no duplicate graph nodes" }
                                        }
                                        tr { class: "border-b border-gray-700",
                                            td { class: "py-1.5 pr-4 font-medium text-gray-200", "2 · Typography & Tag Cleanup" }
                                            td { class: "py-1.5 pr-4 text-gray-400", "Structure text" }
                                            td { class: "py-1.5 text-gray-400", "Coherent chunks, accurate embeddings, meaningful graph nodes" }
                                        }
                                        tr {
                                            td { class: "py-1.5 pr-4 font-medium text-gray-200", "3 · Orchestration" }
                                            td { class: "py-1.5 pr-4 text-gray-400", "Coordinate pipeline" }
                                            td { class: "py-1.5 text-gray-400", "Deterministic indexing, reproducible retrieval, stable summaries" }
                                        }
                                    }
                                }
                            }
                                }
                            }
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

            // ── Chunker info modal ──────────────────────────────────────────
            if show_chunker_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_chunker_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg w-[500px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between px-6 py-3 border-b border-gray-600 shrink-0",
                            h2 { class: "text-base font-semibold text-gray-100", "Chunker" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                onclick: move |_| show_chunker_info.set(false),
                                "✕"
                            }
                        }
                        div { class: "flex-1 overflow-y-auto min-h-0 px-6 py-4 text-xs text-gray-300 space-y-4",
                            p {
                                "The chunker splits the NFC-normalized stored text into "
                                strong { class: "text-gray-100", "chunks" }
                                " — the unit of indexing for both the vector store and the BM25 index. Each mode is a different answer to the same question: where should one chunk end and the next begin?"
                            }

                            // fixed
                            div { class: "p-3 rounded-lg space-y-1", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200", "fixed" }
                                p { class: "text-gray-400", "Splits on a hard token count. Every chunk is exactly max_size tokens (default 384) with no awareness of sentences, paragraphs, or meaning. The last chunk of a document may be shorter." }
                                p { class: "text-gray-400", "Overlap (default 32 tokens) is carried forward from the tail of the previous chunk so a sentence cut at a boundary can still be retrieved from either side." }
                                p { class: "text-gray-500", "Best for uniform corpora — logs, CSVs, code. Avoid for prose: sentences are frequently split mid-way, degrading retrieval quality." }
                            }

                            // lightweight
                            div { class: "p-3 rounded-lg space-y-1", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200", "lightweight" }
                                p { class: "text-gray-400", "Accumulates sentences until the chunk reaches the target token count, then flushes at the next sentence boundary. Sentence detection uses punctuation patterns (.!? followed by a capital letter) — no NLP model required." }
                                p { class: "text-gray-400", "If a single sentence would overflow the hard max, it is flushed immediately. The sentence_flushes counter in chunk stats shows how often the chunker waited for a boundary rather than cutting mid-sentence." }
                                p { class: "text-gray-500", "The default mode. Good for general prose — articles, PDFs, documentation. Faster than semantic with much more readable output than fixed." }
                            }

                            // semantic
                            div { class: "p-3 rounded-lg space-y-1", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200", "semantic" }
                                p { class: "text-gray-400", "Splits the document into natural units — paragraphs, headings, code blocks — then embeds each unit and compares consecutive embeddings. When cosine similarity between two adjacent units drops below a threshold (default 0.78), that gap is treated as a topic shift and a chunk is flushed." }
                                p { class: "text-gray-400", "Result: each chunk covers one coherent idea. A paragraph about database indexing won't share a chunk with one about UI styling even if both fit within the token limit. The semantic_flushes counter shows how many topic-shift boundaries were detected." }
                                p { class: "text-gray-400", "Threshold is tunable via SEMANTIC_SIMILARITY_THRESHOLD. Lower (e.g. 0.65) → larger chunks spanning more related content. Higher (e.g. 0.90) → tighter, more focused chunks." }
                                p { class: "text-gray-500", "Best for long, mixed-topic documents. Requires the embedder to be running. Expect 2–5× the ingestion time of lightweight." }
                            }

                            // shared mechanics
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "Shared mechanics" }
                            ul { class: "ml-4 space-y-1 list-disc list-outside text-gray-400",
                                li { "Min chunk: 128 tokens — smaller units are merged with the next rather than indexed alone." }
                                li { "Max chunk: 384 tokens — hard ceiling, always flushed regardless of boundaries." }
                                li { "Overlap: 32 tokens — tail of each chunk is prepended to the next for cross-boundary retrieval." }
                                li { "Each chunk becomes one embedding vector and one BM25 document." }
                                li { "Tunable via CHUNK_MIN_SIZE, CHUNK_MAX_SIZE, CHUNK_OVERLAP, CHUNKER_MODE. Re-index after changing." }
                            }
                        }
                        div { class: "px-6 py-3 border-t border-gray-600 shrink-0 flex justify-end bg-gray-800 rounded-b-lg",
                            button {
                                class: "px-5 py-1.5 text-sm font-medium rounded text-white hover:opacity-80",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;",
                                onclick: move |_| show_chunker_info.set(false),
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
                        class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),

                        // Header
                        div { class: "flex items-center justify-between px-6 py-3 border-b border-gray-600 shrink-0",
                            h2 { class: "text-base font-semibold text-gray-100", "Parser" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                onclick: move |_| show_parser_info.set(false),
                                "✕"
                            }
                        }

                        // Scrollable body
                        div { class: "flex-1 overflow-y-auto min-h-0",

                        // Two-column body
                        div { class: "grid grid-cols-2 divide-x divide-gray-700 px-0",

                            // Left: what the parser is
                            div { class: "px-6 py-4 text-xs text-gray-300 space-y-2",
                                p {
                                    "The parser is the first stage of the ingestion pipeline. It reads raw \
                                    input—files, URLs, or streams—and converts them into a "
                                    strong { class: "text-gray-100", "plain-text representation" }
                                    " that subsequent pipeline stages can process."
                                }
                                h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "Supported formats" }
                                ul { class: "ml-4 space-y-0.5 list-disc list-outside text-gray-400",
                                    li { "Plain text and Markdown" }
                                    li { "HTML and XML (tag-aware extraction)" }
                                    li { "PDF (text layer extraction)" }
                                    li { "Office formats: DOCX, XLSX, ODT, ODS, CSV" }
                                    li { "Source code files (language-aware)" }
                                    li { "JSON (structure-aware flattening)" }
                                }
                                h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "RAG impact" }
                                ul { class: "ml-4 space-y-0.5 list-disc list-outside text-gray-400",
                                    li { "Determines what text is available for chunking and embedding." }
                                    li { "Format-specific extraction preserves semantic structure." }
                                    li { "Poor parsing propagates noise into every downstream stage." }
                                }
                                p { class: "italic text-gray-400 pt-1",
                                    "The parser is the entry point of the entire ingestion pipeline."
                                }
                            }

                            // Right: what the monitor tells you
                            div { class: "px-6 py-4 text-xs text-gray-300 space-y-2",
                                p {
                                    "The Parser monitor tells you what's actually entering your RAG pipeline — \
                                    the "
                                    strong { class: "text-gray-100", "single most impactful variable in retrieval quality" }
                                    ", and the one most often invisible."
                                }
                                div { class: "space-y-2 pt-1",
                                    div {
                                        h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "1. Format distribution drift" }
                                        p { class: "text-gray-400 mt-0.5",
                                            "If the "
                                            code { class: "text-gray-200 bg-gray-700 px-0.5 rounded", "pdf" }
                                            " row suddenly dominates while "
                                            code { class: "text-gray-200 bg-gray-700 px-0.5 rounded", "docx" }
                                            " drops to zero, someone changed their upload workflow — shifting \
                                            text quality without anyone noticing."
                                        }
                                    }
                                    div {
                                        h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "2. Empty yield detection" }
                                        p { class: "text-gray-400 mt-0.5",
                                            "The "
                                            strong { class: "text-yellow-500", "empty" }
                                            " column is the most useful signal. High empty on "
                                            code { class: "text-gray-200 bg-gray-700 px-0.5 rounded", "pdf" }
                                            " means scanned PDFs are polluting the vector space with empty chunks."
                                        }
                                    }
                                    div {
                                        h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "3. OCR health" }
                                        p { class: "text-gray-400 mt-0.5",
                                            "If "
                                            strong { class: "text-gray-100", "attempted" }
                                            " stays at 0 for scanned PDFs, something upstream is misclassifying them. \
                                            If "
                                            strong { class: "text-gray-100", "ok" }
                                            " is far below attempted, DPI or language pack is wrong."
                                        }
                                    }
                                    div {
                                        h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "4. Chars as content density proxy" }
                                        p { class: "text-gray-400 mt-0.5",
                                            "If DOCX extracts 10× more chars than PDF, your PDF pipeline is lossy. \
                                            Also helps tune chunk size — if avg doc is 500 chars, a 512-token chunk \
                                            produces single-chunk documents, not semantic units."
                                        }
                                    }
                                    div {
                                        h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "5. Debugging specific uploads" }
                                        p { class: "text-gray-400 mt-0.5",
                                            "\"I uploaded this and can't find it\" → check parser stats first. \
                                            If format shows "
                                            strong { class: "text-yellow-500", "empty: 1" }
                                            ", the problem is extraction — not chunking, embedding, or retrieval."
                                        }
                                    }
                                    div {
                                        h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide", "6. Format support gaps" }
                                        p { class: "text-gray-400 mt-0.5",
                                            "Non-zero "
                                            code { class: "text-gray-200 bg-gray-700 px-0.5 rounded", "unknown" }
                                            " or "
                                            code { class: "text-gray-200 bg-gray-700 px-0.5 rounded", "binary" }
                                            " means files are being uploaded that the parser can't handle."
                                        }
                                    }
                                }
                                p { class: "italic text-gray-100 pt-1",
                                    "Bad parsing is invisible at search time but explains most retrieval failures."
                                }
                            }
                        }

                        // Pipeline flow
                        div { class: "border-t border-gray-700 px-6 py-3",
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2", "Pipeline" }
                            pre { class: "text-xs font-mono text-gray-400 leading-snug",
"── INGESTION ──────────────────────────────────────────────────────────────

bytes on disk
  │
  ▼
infer (magic bytes) + extension + heuristic  →  ContentType
  │
  ▼
┌─ PDF ──────► pdftotext -layout  →  pdf-extract  →  OCR (300 dpi)
├─ HTML ─────► remove script/style/head  →  strip tags  →  entity decode
├─ DOCX/ODT ─► unzip + strip XML
├─ XLSX/ODS ─► calamine → tab-separated rows
├─ EPUB ─────► unzip + strip XHTML
├─ PPTX ─────► unzip + strip slide XML
└─ text/code/JSON/MD ── detect_and_decode (BOM → chardetng → encoding_rs)
  │
  ▼
dedupe_pdf_noise  (PDF only — strips repeated headers/footers)
  │
  ▼
apply_text_preprocessing  (opt: clean_html, clean_unicode)
  │
  ▼
normalize(Store)   NFC + whitespace collapse    →  stored content
  │
  ▼
chunker
  │
  ├──► normalize(Embed)   NFKC + whitespace                →  embedder  →  HNSW vector store
  ├──► normalize(Index)   NFKC + whitespace + punct canon  →  Tantivy BM25 index
  └──► chunks (Embed)                                      →  Neo4j / petgraph

── QUERY ───────────────────────────────────────────────────────────────────

raw query
  │
  ├──► normalize(Embed)  →  embedder  →  vector search ──────────┐
  ├──► normalize(Index)  →  Tantivy BM25 search ─────────────────┤
  └──► NER (raw)         →  petgraph traversal ──────────────────┤
                                                                  ▼
                                                           RRF merge  →  top-k  →  LLM"
                            }
                        }

                        } // end scrollable body

                        // Footer
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

            // ── Typography & Tag Cleanup info modal ──────────────────────────
            if show_preprocessing_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_preprocessing_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg w-[90vw] max-w-2xl max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between px-6 py-3 border-b border-gray-600 shrink-0",
                            h2 { class: "text-base font-semibold text-gray-100", "Typography & Tag Cleanup" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                onclick: move |_| show_preprocessing_info.set(false),
                                "✕"
                            }
                        }
                        div { class: "flex-1 overflow-y-auto min-h-0 px-6 py-4 text-xs text-gray-300 space-y-4",
                            p {
                                "Runs immediately after the parser and before NFC canonicalization. \
                                Both passes are "
                                strong { class: "text-gray-100", "automatic — keyed to the detected file format" }
                                ". No settings to toggle."
                            }

                            // ── HTML tag stripping ──────────────────────────────
                            div { class: "rounded border border-gray-700 p-3 space-y-3",
                                p { class: "text-gray-100 font-semibold", "HTML tag stripping" }

                                div { class: "space-y-1",
                                    p { class: "text-gray-400 uppercase tracking-wide text-[10px] font-semibold", "Applied to" }
                                    p { "HTML files only. All other formats are passed through unchanged." }
                                }

                                div { class: "space-y-1",
                                    p { class: "text-gray-400 uppercase tracking-wide text-[10px] font-semibold", "What it does" }
                                    p {
                                        "Removes " code { class: "text-cyan-300", "<tag>" } " and " code { class: "text-cyan-300", "</tag>" }
                                        " patterns using a fast character-scan. Raw text content is preserved. \
                                        Without this, the embedder sees markup tokens ("
                                        code { class: "text-cyan-300", "div class= wrapper span id= ..." }
                                        ") that dilute semantic quality and inflate chunk size without carrying meaning."
                                    }
                                }

                                div { class: "space-y-1",
                                    p { class: "text-gray-400 uppercase tracking-wide text-[10px] font-semibold", "Caveats" }
                                    ul { class: "ml-4 space-y-1 list-disc list-outside text-gray-300",
                                        li {
                                            strong { class: "text-gray-200", "Semantic markup" }
                                            " — if " code { class: "text-cyan-300", "<em>" } " or " code { class: "text-cyan-300", "<strong>" }
                                            " carry meaning you want the embedder to see, stripping removes that signal."
                                        }
                                        li {
                                            strong { class: "text-gray-200", "Source code with HTML literals" }
                                            " — files that contain HTML as data (template files, code examples) \
                                            will have their literal content stripped."
                                        }
                                        li {
                                            strong { class: "text-gray-200", "Citation and re-render workflows" }
                                            " — retrieval returns chunks verbatim to the LLM. If the original \
                                            was structured HTML (tables, lists), the stripped text looks garbled \
                                            to a user or LLM reading it back as a source passage."
                                        }
                                        li {
                                            strong { class: "text-gray-200", "Re-indexing consistency" }
                                            " — if you later re-ingest the same document with a different format \
                                            detection result, the stored chunks differ even though the source \
                                            file didn't change. Diff-based deduplication breaks down."
                                        }
                                    }
                                }
                            }

                            // ── Unicode typography normalisation ────────────────
                            div { class: "rounded border border-gray-700 p-3 space-y-3",
                                p { class: "text-gray-100 font-semibold", "Unicode typography normalisation" }

                                div { class: "space-y-1",
                                    p { class: "text-gray-400 uppercase tracking-wide text-[10px] font-semibold", "Applied to" }
                                    p { "PDF, DOCX, ODT, EPUB, PPTX, HTML — any format produced by a publishing or rich-text tool." }
                                }

                                div { class: "space-y-1",
                                    p { class: "text-gray-400 uppercase tracking-wide text-[10px] font-semibold", "Why these formats" }
                                    p {
                                        "Any tool that considers itself a publishing surface inserts typographic \
                                        characters automatically: Word, InDesign, LaTeX (curly quotes and ligatures \
                                        survive PDF extraction), browser copy-paste (&nbsp;, smart quotes), \
                                        EPUB publishers, CMS exports (WordPress, Confluence, Notion), and Excel \
                                        cells with formatted text. Plain terminals, code editors, and hand-typed \
                                        Markdown almost never produce these — those formats are left untouched."
                                    }
                                }

                                div { class: "space-y-1",
                                    p { class: "text-gray-400 uppercase tracking-wide text-[10px] font-semibold", "What it does" }
                                    p { "Substitutes typographic characters with plain ASCII equivalents:" }
                                    ul { class: "ml-4 space-y-0.5 list-disc list-outside text-gray-300",
                                        li { code { class: "text-amber-300", "' '" } " curly quotes → " code { class: "text-amber-300", "'" } }
                                        li { code { class: "text-amber-300", "\" \"" } " curly double quotes → " code { class: "text-amber-300", "\"" } }
                                        li { code { class: "text-amber-300", "– —" } " en/em dash → " code { class: "text-amber-300", " - " } }
                                        li { code { class: "text-amber-300", "‑" } " non-breaking hyphen → " code { class: "text-amber-300", "-" } }
                                        li { code { class: "text-amber-300", "…" } " ellipsis → " code { class: "text-amber-300", "..." } }
                                        li { "PDF ligatures ﬁ ﬂ ﬀ ﬃ ﬄ ﬆ → fi fl ff ffi ffl st" }
                                    }
                                    p { class: "pt-1",
                                        "Without this, " code { class: "text-amber-300", "\u{2018}word\u{2019}" }
                                        " and " code { class: "text-amber-300", "'word'" }
                                        " tokenise differently despite being the same word, \
                                        and a user who searches with a plain keyboard misses chunks \
                                        that contain the typographic equivalent."
                                    }
                                }

                                div { class: "space-y-1",
                                    p { class: "text-gray-400 uppercase tracking-wide text-[10px] font-semibold", "Caveats" }
                                    ul { class: "ml-4 space-y-1 list-disc list-outside text-gray-300",
                                        li {
                                            strong { class: "text-gray-200", "Multilingual corpora" }
                                            " — Arabic diacritics, CJK punctuation, and accented Latin \
                                            are left untouched by the substitution table. However, if \
                                            your corpus mixes scripts heavily, verify that no legitimate \
                                            characters collide with the substitution set."
                                        }
                                        li {
                                            strong { class: "text-gray-200", "Faithful quotation" }
                                            " — replacing smart quotes with straight quotes changes the \
                                            stored chunk. If users are likely to search for an exact \
                                            typographic string, normalisation closes that gap in your favour; \
                                            if you need to reproduce the original punctuation verbatim, it works against you."
                                        }
                                    }
                                }
                            }

                            // ── Plain text / Markdown / Code ────────────────────
                            div { class: "rounded border border-gray-700 p-3 space-y-1",
                                p { class: "text-gray-100 font-semibold", "Plain text / Markdown / Code" }
                                p {
                                    "Neither pass runs. These formats use plain ASCII and should be \
                                    preserved exactly as parsed — stripping or substituting would \
                                    corrupt code literals and hand-typed content."
                                }
                            }

                            p { class: "text-gray-300 italic",
                                "The two steps are complementary: this stage removes format artefacts \
                                by substitution; NFC canonicalization (the next stage) resolves \
                                Unicode combining-character equivalences. They solve different problems."
                            }
                        }
                    }
                }
            }

            // ── Canonicalize NFC info modal ──────────────────────────────────
            if show_nfc_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_nfc_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg w-[500px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between px-6 py-3 border-b border-gray-600 shrink-0",
                            h2 { class: "text-base font-semibold text-gray-100", "Canonicalize NFC" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                onclick: move |_| show_nfc_info.set(false),
                                "✕"
                            }
                        }
                        div { class: "flex-1 overflow-y-auto min-h-0 px-6 py-4 text-xs text-gray-300 space-y-3",
                            p { "Applied once per document immediately after text extraction, before chunking. The normalized text is what gets persisted to disk and shown to users in search results." }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "What NFC does" }
                            p { class: "text-gray-400", "NFC (Canonical Decomposition followed by Canonical Composition) unifies different byte-level encodings of the same character into a single canonical form. It only touches canonical equivalences — characters that are genuinely the same glyph stored differently." }
                            p { class: "text-gray-400 font-mono text-[0.7rem]",
                                "é (U+00E9)  ←  e (U+0065) + combining accent (U+0301)"
                            }
                            p { class: "text-gray-400", "Both representations are valid UTF-8 for the same character. NFC rewrites the two-codepoint form to the single-codepoint form. Without this step, the same word can produce different byte sequences depending on the source application, causing missed matches in exact-string comparisons." }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "What NFC does NOT touch" }
                            ul { class: "ml-4 space-y-0.5 list-disc list-outside text-gray-400",
                                li { "Curly quotes and straight quotes remain distinct." }
                                li { "Em-dashes, en-dashes, and hyphens are left as-is." }
                                li { "Ligatures (ﬁ, ﬂ) are preserved — NFC is typography-safe." }
                                li { "Fullwidth ASCII characters keep their fullwidth form." }
                            }
                            p { class: "text-gray-500", "This is intentional: stored text should look exactly as the author intended. Compatibility folding (ﬁ → fi) only happens at the Embed and Index stages where it is safe to discard visual distinctions." }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "RAG impact" }
                            ul { class: "ml-4 space-y-0.5 list-disc list-outside text-gray-400",
                                li { "Eliminates encoding drift when the same document is re-ingested from different sources." }
                                li { "Ensures BM25 exact-match and phrase queries work correctly for accented characters." }
                                li { "Prevents the same passage appearing as duplicate graph nodes due to byte-level differences." }
                            }
                        }
                        div { class: "px-6 py-3 border-t border-gray-600 shrink-0 flex justify-end bg-gray-800 rounded-b-lg",
                            button {
                                class: "px-5 py-1.5 text-sm font-medium rounded text-white hover:opacity-80",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;",
                                onclick: move |_| show_nfc_info.set(false),
                                "Got it"
                            }
                        }
                    }
                }
            }

            // ── Canonicalize NFKC info modal ─────────────────────────────────
            if show_nfkc_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_nfkc_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg w-[500px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between px-6 py-3 border-b border-gray-600 shrink-0",
                            h2 { class: "text-base font-semibold text-gray-100", "Canonicalize NFKC" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                onclick: move |_| show_nfkc_info.set(false),
                                "✕"
                            }
                        }
                        div { class: "flex-1 overflow-y-auto min-h-0 px-6 py-4 text-xs text-gray-300 space-y-3",
                            p { "Applied to each chunk before the embedding model, and to each search query before embedding. The same normalization is used on both sides so document and query vectors are directly comparable." }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "What NFKC does" }
                            p { class: "text-gray-400", "NFKC (Compatibility Decomposition followed by Canonical Composition) is a strict superset of NFC. It applies all the canonical equivalences NFC applies, and then additionally folds compatibility variants — characters that look different but carry the same meaning for text processing." }
                            p { class: "text-gray-400 font-mono text-[0.7rem]",
                                "ﬁ → fi  ·  ① → 1  ·  Ⅳ → IV  ·  ² → 2  ·  ａｂｃ → abc"
                            }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "Why this matters for embeddings" }
                            p { class: "text-gray-400", "Embedding models tokenize on byte sequences. If \"ﬁnance\" and \"finance\" produce different byte sequences, they produce different tokens — and therefore slightly different embedding vectors — even though they mean the same thing. NFKC collapses these variants before the text reaches the tokenizer, so semantically identical content maps to identical vectors." }
                            p { class: "text-gray-400", "This is why the query side must use the exact same normalization as the ingestion side: if a user types \"ﬁnance\" in the search box and the indexed chunks were normalized to \"finance\", the vectors still match because the query is also normalized before embedding." }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "What NFKC does NOT do" }
                            ul { class: "ml-4 space-y-0.5 list-disc list-outside text-gray-400",
                                li { "No punctuation canonicalization — smart quotes and dashes are left as-is." }
                                li { "No case folding — \"AI\" and \"ai\" remain distinct." }
                                li { "Punctuation is left to the embedding model's own tokenizer." }
                            }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "RAG impact" }
                            ul { class: "ml-4 space-y-0.5 list-disc list-outside text-gray-400",
                                li { "Prevents duplicate vectors for visually identical content encoded differently." }
                                li { "Improves nearest-neighbour recall — more character variants hit the same cluster." }
                                li { "Symmetric ingest/query normalization keeps the embedding space consistent." }
                            }
                        }
                        div { class: "px-6 py-3 border-t border-gray-600 shrink-0 flex justify-end bg-gray-800 rounded-b-lg",
                            button {
                                class: "px-5 py-1.5 text-sm font-medium rounded text-white hover:opacity-80",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;",
                                onclick: move |_| show_nfkc_info.set(false),
                                "Got it"
                            }
                        }
                    }
                }
            }

            // ── Canonicalize NFKC+punct info modal ───────────────────────────
            if show_nfkc_punct_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_nfkc_punct_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg w-[500px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between px-6 py-3 border-b border-gray-600 shrink-0",
                            h2 { class: "text-base font-semibold text-gray-100", "Canonicalize NFKC + punct" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                onclick: move |_| show_nfkc_punct_info.set(false),
                                "✕"
                            }
                        }
                        div { class: "flex-1 overflow-y-auto min-h-0 px-6 py-4 text-xs text-gray-300 space-y-3",
                            p { "Applied to each chunk for BM25 (Tantivy) indexing, and to each search query at search time. This is the most aggressive normalization level — it is a strict superset of NFKC." }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "What it adds on top of NFKC" }
                            p { class: "text-gray-400", "In addition to all NFKC folding, punctuation canonicalization rewrites typographic punctuation to its ASCII equivalent:" }
                            p { class: "text-gray-400 font-mono text-[0.7rem]",
                                "\u{201C}word\u{201D} → \"word\"  ·  \u{2018}it\u{2019}s\u{2019} → 'it's'  ·  don\u{2019}t → don't"
                            }
                            p { class: "text-gray-400 font-mono text-[0.7rem]",
                                "em-dash (\u{2014}) → \" - \"  ·  en-dash (\u{2013}) → \" - \"  ·  ellipsis (\u{2026}) → \"...\""
                            }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "Why BM25 needs punct canonicalization" }
                            p { class: "text-gray-400", "BM25 is a term-frequency model: it compares exact tokens. If a document contains \u{201C}don\u{2019}t\u{201D} (curly apostrophe) and a user searches for \"don't\" (straight apostrophe), the tokens are different — BM25 returns zero matches even though the intent is identical." }
                            p { class: "text-gray-400", "Applying the same punct canonicalization to both the indexed text and the incoming query guarantees that what is stored and what is searched are token-identical. The Tantivy analyzer then sees the same token stream regardless of which punctuation variant was in the original source." }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "Why embeddings don't need this" }
                            p { class: "text-gray-400", "Embedding models use subword tokenizers (BPE/SentencePiece) that treat \u{201C}don\u{2019}t\u{201D} and \"don't\" as the same or very similar token sequences anyway. Punct canonicalization at the embedding stage would be redundant and could discard nuance the model was trained to use." }
                            h4 { class: "text-xs font-semibold text-gray-400 uppercase tracking-wide pt-1", "RAG impact" }
                            ul { class: "ml-4 space-y-0.5 list-disc list-outside text-gray-400",
                                li { "Prevents BM25 misses caused by typographic punctuation variants." }
                                li { "Symmetric ingest/query canonicalization keeps term matching consistent." }
                                li { "Removes one class of retrieval failures that are otherwise very hard to debug." }
                            }
                        }
                        div { class: "px-6 py-3 border-t border-gray-600 shrink-0 flex justify-end bg-gray-800 rounded-b-lg",
                            button {
                                class: "px-5 py-1.5 text-sm font-medium rounded text-white hover:opacity-80",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;",
                                onclick: move |_| show_nfkc_punct_info.set(false),
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

#[derive(Props, Clone, PartialEq)]
struct StoreRecordsViewProps {
    records: Vec<StoreRecord>,
}

#[component]
fn StoreRecordsView(props: StoreRecordsViewProps) -> Element {
    rsx! {
        div { class: "overflow-y-auto", style: "max-height:220px;",
            if props.records.is_empty() {
                div { class: "flex flex-col items-center justify-center h-24 gap-1",
                    p { class: "text-xs text-gray-400", "Upload a file to see NFC stats" }
                    p { class: "text-xs text-gray-400", "Stats reset on service restart" }
                }
            } else {
                table { class: "w-full text-xs",
                    thead {
                        tr { class: "text-gray-300 border-b border-gray-500",
                            th { class: "text-left pb-1 pr-2 font-medium", "File" }
                            th { class: "text-right pb-1 pr-2 font-medium", "In" }
                            th { class: "text-right pb-1 pr-2 font-medium", "Out" }
                            th { class: "text-right pb-1 font-medium", "Δ" }
                        }
                    }
                    tbody {
                        for rec in &props.records {
                            tr { class: "border-b border-gray-500/50",
                                td { class: "py-0.5 pr-2 text-gray-200 font-mono truncate", style: "max-width:130px;", title: "{rec.file}", "{rec.file}" }
                                td { class: "py-0.5 pr-2 text-right tabular-nums text-gray-400", "{format_chars(rec.chars_in)}" }
                                td { class: "py-0.5 pr-2 text-right tabular-nums text-gray-400", "{format_chars(rec.chars_out)}" }
                                td { class: "py-0.5 text-right tabular-nums text-gray-400",
                                    {
                                        if rec.chars_in == 0 { "—".to_string() }
                                        else {
                                            let d = rec.chars_out as f64 / rec.chars_in as f64 * 100.0 - 100.0;
                                            if d >= 0.0 { format!("+{:.1}%", d) } else { format!("{:.1}%", d) }
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

#[derive(Props, Clone, PartialEq)]
struct ChunkerStatsViewProps {
    snapshots: Vec<ChunkingStatsSnapshot>,
}

#[component]
fn ChunkerStatsView(props: ChunkerStatsViewProps) -> Element {
    let mut seen = std::collections::HashSet::new();
    let deduped: Vec<&ChunkingStatsSnapshot> = props.snapshots
        .iter()
        .filter(|s| seen.insert(s.file.clone()))
        .collect();

    rsx! {
        div { class: "overflow-y-auto", style: "max-height:220px;",
            if props.snapshots.is_empty() {
                div { class: "flex flex-col items-center justify-center h-24 gap-1",
                    p { class: "text-xs text-gray-400", "Upload a file to see chunking stats" }
                    p { class: "text-xs text-gray-400", "Stats reset on service restart" }
                }
            } else {
                table { class: "w-full text-xs",
                    thead {
                        tr { class: "text-gray-300 border-b border-gray-500",
                            th { class: "text-left pb-1 pr-2 font-medium", "File" }
                            th { class: "text-left pb-1 pr-2 font-medium", "Mode" }
                            th { class: "text-right pb-1 pr-2 font-medium", "Chunks" }
                            th { class: "text-right pb-1 pr-2 font-medium", "Tokens" }
                            th { class: "text-right pb-1 font-medium", "ms" }
                        }
                    }
                    tbody {
                        for snap in &deduped {
                            tr { class: "border-b border-gray-500/50",
                                td { class: "py-0.5 pr-2 text-gray-200 font-mono truncate", style: "max-width:100px;", title: "{snap.file}", "{snap.file}" }
                                td { class: "py-0.5 pr-2 text-gray-400 font-mono", "{snap.chunker_mode}" }
                                td { class: "py-0.5 pr-2 text-right tabular-nums text-gray-300", "{snap.chunks}" }
                                td { class: "py-0.5 pr-2 text-right tabular-nums text-gray-400", "{snap.tokens}" }
                                td { class: "py-0.5 text-right tabular-nums text-gray-400", "{snap.duration_ms}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn delta_pct(site: &CallSiteStats) -> String {
    if site.chars_in == 0 {
        return "—".to_string();
    }
    let d = site.chars_out as f64 / site.chars_in as f64 * 100.0 - 100.0;
    if d >= 0.0 { format!("+{:.1}%", d) } else { format!("{:.1}%", d) }
}

#[derive(Clone, PartialEq)]
struct CanonMiniRow {
    label: &'static str,
    description: &'static str,
    site: CallSiteStats,
}

#[derive(Props, Clone, PartialEq)]
struct CanonMiniTableProps {
    rows: Vec<CanonMiniRow>,
}

#[component]
fn CanonMiniTable(props: CanonMiniTableProps) -> Element {
    rsx! {
        div { class: "overflow-y-auto", style: "max-height:220px;",
            table { class: "w-full text-xs",
                thead {
                    tr { class: "text-gray-300 border-b border-gray-500",
                        th { class: "text-left pb-1 pr-2 font-medium", style: "width:3rem;", "" }
                        th { class: "pb-1 pr-3" }
                        th { class: "text-right pb-1 pr-2 font-medium", "Calls" }
                        th { class: "text-right pb-1 pr-2 font-medium", "In" }
                        th { class: "text-right pb-1 pr-2 font-medium", "Out" }
                        th { class: "text-right pb-1 font-medium", "Δ" }
                    }
                }
                tbody {
                    for row in &props.rows {
                        CanonRow {
                            label: row.label,
                            description: row.description,
                            site: row.site.clone(),
                            delta: delta_pct(&row.site),
                        }
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct CanonRowProps {
    label: &'static str,
    description: &'static str,
    site: CallSiteStats,
    delta: String,
}

#[component]
fn CanonRow(props: CanonRowProps) -> Element {
    let dim = if props.site.calls == 0 { "text-gray-600" } else { "text-gray-300" };
    let mut show = use_signal(|| false);
    rsx! {
        tr { class: "border-b border-gray-700 {dim}",
            td { class: "py-1.5 pr-2", style: "width:3rem;", "{props.label}" }
            td { class: "py-1.5 pr-3",
                button {
                    class: PARAM_ICON_BUTTON_CLASS,
                    style: PARAM_ICON_BUTTON_STYLE,
                    onclick: move |_| show.set(true),
                    title: "About this stage",
                    InfoIcon {}
                }
                if show() {
                    div {
                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                        onclick: move |_| show.set(false),
                        div {
                            class: "bg-gray-800 border border-gray-600 rounded-lg w-80 shadow-xl",
                            onclick: move |e| e.stop_propagation(),
                            div { class: "flex items-center justify-between px-4 py-3 border-b border-gray-600",
                                h2 { class: "text-sm font-semibold text-gray-100", "{props.label}" }
                                button {
                                    class: "text-gray-400 hover:text-gray-200 text-xl font-bold leading-none",
                                    onclick: move |_| show.set(false),
                                    "✕"
                                }
                            }
                            div { class: "px-4 py-3 text-xs text-gray-300", "{props.description}" }
                            div { class: "px-4 py-2 border-t border-gray-600 flex justify-end",
                                button {
                                    class: "px-4 py-1 text-xs font-medium rounded text-white hover:opacity-80",
                                    style: "background-color:#7C2A02;border:1px solid #7C2A02;",
                                    onclick: move |_| show.set(false),
                                    "Got it"
                                }
                            }
                        }
                    }
                }
            }
            td { class: "py-1.5 pr-3 text-right tabular-nums", "{props.site.calls}" }
            td { class: "py-1.5 pr-3 text-right tabular-nums", "{format_chars(props.site.chars_in)}" }
            td { class: "py-1.5 pr-3 text-right tabular-nums", "{format_chars(props.site.chars_out)}" }
            td { class: "py-1.5 text-right tabular-nums text-gray-400", "{props.delta}" }
        }
    }
}
