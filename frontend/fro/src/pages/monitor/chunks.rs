use crate::{
    api,
    app::Route,
    components::monitor::*,
    pages::hardware::constants::{
        INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
    },
};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

const PREVIEW_TEXTAREA_CLASS: &str =
    "textarea textarea-sm bg-gray-700 text-gray-200 w-full font-mono text-xs min-h-24 resize-y";

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
    let mut canon_stats = use_signal(|| None::<api::CanonStats>);
    let mut golden = use_signal(|| None::<api::GoldenSampleResponse>);
    let mut loading = use_signal(|| true);
    let mut show_info = use_signal(|| false);
    let mut show_shimmytok = use_signal(|| false);
    let mut show_canon_info = use_signal(|| false);
    let mut show_file_norm_info = use_signal(|| false);
    let mut show_mode_info = use_signal(|| false);
    let mut show_strategy_info = use_signal(|| false);
    let mut show_golden_info = use_signal(|| false);
    let mut show_seed_info = use_signal(|| false);
    let mut show_recapture_info = use_signal(|| false);
    let mut recapture_msg = use_signal(|| None::<String>);
    let mut error = use_signal(|| None::<String>);

    // Tokenizer compare/swap state
    let mut candidate_kind = use_signal(|| "path"); // "path" or "ollama"
    let mut candidate_input = use_signal(String::new);
    let mut diff_loading = use_signal(|| false);
    let mut diff_report = use_signal(|| None::<api::TokenizerDiffReport>);
    let mut diff_error = use_signal(|| None::<String>);
    let mut expanded_entry = use_signal(|| None::<i64>);
    let mut swap_loading = use_signal(|| false);
    let mut swap_msg = use_signal(|| None::<String>);
    let mut show_compare_info = use_signal(|| false);
    let mut show_picker_info = use_signal(|| false);
    let mut show_swap_info = use_signal(|| false);

    // Golden sample browse state
    let mut show_golden_entries = use_signal(|| false);
    let mut expanded_golden_entry = use_signal(|| None::<i64>);
    let mut show_golden_read_info = use_signal(|| false);

    // Corpus filter
    let mut corpus_filter = use_signal(String::new);
    let mut corpora: Signal<Vec<api::CorpusEntry>> = use_signal(Vec::new);

    // Chunk preview state
    let mut preview_text = use_signal(String::new);
    let mut preview_filename = use_signal(String::new);
    let mut preview_loading = use_signal(|| false);
    let mut preview_result = use_signal(|| None::<api::ChunkPreviewResponse>);
    let mut preview_error = use_signal(|| None::<String>);

    // Load corpus list once on mount.
    use_future(move || async move {
        if let Ok(list) = api::fetch_corpora().await {
            corpora.set(list);
        }
    });

    // Main polling loop — restarts when corpus_filter changes.
    use_future(move || {
        let corpus_val = corpus_filter();
        async move {
            stats.set(None);
            canon_stats.set(None);
            loading.set(true);
            let c = if corpus_val.is_empty() {
                None
            } else {
                Some(corpus_val.as_str())
            };
            loop {
                let (tok_res, stats_res, canon_res, golden_res) = futures_util::join!(
                    api::fetch_tokenizer_info(),
                    api::fetch_chunking_stats(20, c),
                    api::fetch_canon_stats(c),
                    api::fetch_golden_sample(20),
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
                if let Ok(cs) = canon_res {
                    canon_stats.set(Some(cs));
                }
                if let Ok(gs) = golden_res {
                    golden.set(Some(gs));
                }
                loading.set(false);
                TimeoutFuture::new(10_000).await;
            }
        }
    });

    let tok = tokenizer();
    let tok_model = tok.as_ref().map(|t| t.model.clone()).unwrap_or_default();
    let tok_exact = tok.as_ref().map(|t| t.is_exact).unwrap_or(false);
    let tok_vocab = tok.as_ref().map(|t| t.vocab_size).unwrap_or(0);
    let tok_fallback_reason = tok
        .as_ref()
        .and_then(|t| t.fallback_reason.clone())
        .unwrap_or_default();
    let tok_fallback_detail = tok
        .as_ref()
        .and_then(|t| t.fallback_detail.clone())
        .unwrap_or_default();
    let tok_attempted_path = tok
        .as_ref()
        .and_then(|t| t.attempted_path.clone())
        .unwrap_or_default();
    let (fallback_label, fallback_blurb, fallback_is_unexpected) =
        match tok_fallback_reason.as_str() {
            "cloud_backend" => (
                "Cloud backend",
                "No local GGUF available — heuristic counting is the intended mode for cloud LLMs.",
                false,
            ),
            "no_model_configured" => (
                "No model configured",
                "Backend selected but no model name set — set a model to enable exact counting.",
                true,
            ),
            "path_not_found" => (
                "GGUF path not found",
                "Could not locate the active model's GGUF blob. Token counts are approximate until this is resolved.",
                true,
            ),
            "load_failed" => (
                "GGUF load failed",
                "The GGUF file was found but could not be parsed. Token counts are approximate until this is resolved.",
                true,
            ),
            "not_attempted" => (
                "Not attempted",
                "Tokenizer load has not run yet for the current backend.",
                false,
            ),
            _ => ("", "", false),
        };
    let show_fallback_banner = !tok_exact && !fallback_label.is_empty();

    // Pre-compute tokenizer mismatch outside RSX — collect both stale models and
    // which corpora contain them so the warning can point at the right corpus.
    let (mismatch_models, mismatch_corpora): (String, String) = stats()
        .as_ref()
        .map(|snaps| {
            let mut models = std::collections::BTreeSet::new();
            let mut corpora = std::collections::BTreeSet::new();
            for s in snaps {
                if let Some(ref m) = s.tokenizer_model {
                    if !tok_model.is_empty() && m != &tok_model {
                        models.insert(m.clone());
                        let c = if s.corpus.is_empty() {
                            "default"
                        } else {
                            s.corpus.as_str()
                        };
                        corpora.insert(c.to_string());
                    }
                }
            }
            (
                models.into_iter().collect::<Vec<_>>().join(", "),
                corpora.into_iter().collect::<Vec<_>>().join(", "),
            )
        })
        .unwrap_or_default();
    let has_mismatch = !mismatch_models.is_empty();
    let corpus_count = mismatch_corpora
        .split(", ")
        .filter(|s| !s.is_empty())
        .count();
    let corpus_label = if corpus_count == 1 {
        "Corpus"
    } else {
        "Corpora"
    };
    let was_were = if corpus_count == 1 { "was" } else { "were" };
    let that_corpus = if corpus_count == 1 {
        "that corpus"
    } else {
        "those corpora"
    };
    let the_affected = if corpus_count == 1 {
        "the affected corpus"
    } else {
        "each affected corpus"
    };

    // Detection mismatch: extension implies a strategy that doesn't match chosen_strategy.
    // Only flagged when the extension is unambiguous; heuristic-only detections are skipped.
    fn expected_strategy(ext: &str) -> Option<&'static str> {
        match ext.to_lowercase().trim_start_matches('.') {
            "pdf" => Some("character_split"),
            "txt" => Some("paragraph_split"),
            "md" | "markdown" => Some("header_aware"),
            "html" | "htm" => Some("tag_aware"),
            "xml" => Some("tag_aware"),
            "json" => Some("structure_aware"),
            "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "c" | "cpp" | "cc" | "h" | "hpp" | "go"
            | "java" | "cs" | "rb" | "php" | "swift" | "kt" | "scala" => Some("ast_based"),
            "docx" => Some("paragraph_split"),
            "odt" => Some("paragraph_split"),
            "xlsx" => Some("row_split"),
            "ods" => Some("row_split"),
            "csv" => Some("row_split"),
            "epub" => Some("chapter_split"),
            "pptx" => Some("slide_split"),
            _ => None,
        }
    }
    // Collect mismatched files for banner + per-row flagging.
    let detection_mismatches: Vec<(String, String, String)> = stats()
        .as_ref()
        .map(|snaps| {
            snaps
                .iter()
                .filter_map(|s| {
                    let det = s.detection.as_ref()?;
                    let ext = det
                        .extension
                        .as_deref()
                        .or_else(|| s.file.rsplit('.').next())?;
                    let expected = expected_strategy(ext)?;
                    if det.chosen_strategy != expected {
                        let file_short = s.file.rsplit('/').next().unwrap_or(&s.file).to_string();
                        Some((
                            file_short,
                            expected.to_string(),
                            det.chosen_strategy.clone(),
                        ))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let has_detection_mismatch = !detection_mismatches.is_empty();
    let detection_mismatch_count = detection_mismatches.len();
    let detection_mismatch_plural = if detection_mismatch_count == 1 {
        "file"
    } else {
        "files"
    };

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

            // Corpus filter
            div { class: "flex flex-col gap-0.5",
                div {
                    class: "flex items-center px-2 py-0.5 rounded cursor-pointer",
                    style: if corpus_filter().is_empty() {
                        "background-color: rgba(124,42,2,0.35); border: 1px solid rgba(124,42,2,0.6);"
                    } else {
                        "background-color: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.08);"
                    },
                    onclick: move |_| corpus_filter.set(String::new()),
                    span {
                        class: "text-xs font-mono",
                        style: if corpus_filter().is_empty() { "color: white;" } else { "color: #d1d5db;" },
                        "All corpora"
                    }
                }
                for entry in corpora() {
                    {
                        let slug = entry.slug.clone();
                        let slug2 = slug.clone();
                        let active = corpus_filter() == slug;
                        rsx! {
                            div {
                                class: "flex items-center px-2 py-0.5 rounded cursor-pointer",
                                style: if active {
                                    "background-color: rgba(124,42,2,0.35); border: 1px solid rgba(124,42,2,0.6);"
                                } else {
                                    "background-color: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.08);"
                                },
                                onclick: move |_| corpus_filter.set(slug.clone()),
                                span {
                                    class: "text-xs font-mono",
                                    style: if active { "color: white;" } else { "color: #d1d5db;" },
                                    "{slug2}"
                                }
                            }
                        }
                    }
                }
            }

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

            // GGUF fallback status banner
            if show_fallback_banner {
                {
                    let (bg, border, icon_color, text_color) = if fallback_is_unexpected {
                        (
                            "background-color: rgba(234,179,8,0.1); border: 1px solid rgba(234,179,8,0.3);",
                            "",
                            "text-yellow-400",
                            "text-yellow-300",
                        )
                    } else {
                        (
                            "background-color: rgba(96,165,250,0.08); border: 1px solid rgba(96,165,250,0.25);",
                            "",
                            "text-blue-400",
                            "text-blue-300",
                        )
                    };
                    let _ = border;
                    rsx! {
                        Panel { title: None, refresh: None,
                            div { class: "flex items-start gap-3 p-3 rounded-lg",
                                style: "{bg}",
                                span { class: "text-lg {icon_color}",
                                    if fallback_is_unexpected { "⚠" } else { "ℹ" }
                                }
                                div { class: "text-sm {text_color} flex-1",
                                    p { class: "font-medium mb-1", "Tokenizer fallback: {fallback_label}" }
                                    p { class: "{text_color}/80", "{fallback_blurb}" }
                                    if !tok_fallback_detail.is_empty() {
                                        p { class: "mt-2 text-xs font-mono opacity-70", "Detail: {tok_fallback_detail}" }
                                    }
                                    if !tok_attempted_path.is_empty() {
                                        p { class: "mt-1 text-xs font-mono opacity-70", "Path: {tok_attempted_path}" }
                                    }
                                }
                            }
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
                                "{corpus_label} {mismatch_corpora} {was_were} indexed with {mismatch_models} but the active tokenizer is {tok_model}. Token counts in {that_corpus} may be inaccurate."
                            }
                            p { class: "text-yellow-400/80 mt-1",
                                "Re-index {the_affected} from Config → Corpus to fix this — documents in other corpora are unaffected."
                            }
                        }
                    }
                }
            }

            // Detection strategy mismatch warning
            if has_detection_mismatch {
                Panel { title: None, refresh: None,
                    div { class: "flex items-start gap-3 p-3 rounded-lg",
                        style: "background-color: rgba(234,179,8,0.1); border: 1px solid rgba(234,179,8,0.3);",
                        span { class: "text-yellow-400 text-lg", "⚠" }
                        div { class: "text-sm text-yellow-300",
                            p { class: "font-medium mb-1",
                                "Format detection mismatch — {detection_mismatch_count} {detection_mismatch_plural} may have been misidentified"
                            }
                            p { class: "text-yellow-400/80 mb-2",
                                "The file extension implies a different chunking strategy than what was used. This usually means the file had no extension, the wrong extension, or an unsupported MIME type. Check the Strategy column below for details."
                            }
                            div { class: "flex flex-col gap-0.5",
                                for (file, expected, actual) in &detection_mismatches {
                                    p { class: "text-xs font-mono text-yellow-400/70",
                                        "{file} — expected {expected}, got {actual}"
                                    }
                                }
                            }
                            p { class: "text-yellow-400/80 mt-2",
                                "Fix: rename the file with the correct extension and re-index it."
                            }
                        }
                    }
                }
            }

            // Golden corpus sample
            {
                let gs = golden();
                let st = gs.as_ref().and_then(|g| g.status.as_ref());
                let cap = st.map(|s| s.capacity).unwrap_or(0);
                let cur = st.map(|s| s.current_size).unwrap_or(0);
                let seen = st.map(|s| s.chunks_seen).unwrap_or(0);
                let captured = st.and_then(|s| s.captured_at.clone()).unwrap_or_default();
                let model = st.and_then(|s| s.tokenizer_model.clone()).unwrap_or_default();
                let seed = st.map(|s| s.seed).unwrap_or(0);
                let pct = if cap > 0 { (cur * 100) / cap } else { 0 };
                let bar_color = if cur == 0 {
                    "bg-yellow-500"
                } else if cur < cap {
                    "bg-blue-500"
                } else {
                    "bg-green-500"
                };
                let last_msg = recapture_msg();
                rsx! {
                    Panel { title: None, refresh: None,
                        div { class: "flex items-center gap-2 mb-3",
                            h3 { class: "text-sm font-semibold text-gray-200", "Golden Corpus Sample" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_golden_info.set(true),
                                title: "About the golden corpus sample",
                                InfoIcon {}
                            }
                            if cur > 0 {
                                button {
                                    class: "btn btn-xs bg-gray-700 hover:bg-gray-600 text-gray-200 border-gray-600",
                                    onclick: move |_| {
                                        show_golden_entries.set(!show_golden_entries());
                                        expanded_golden_entry.set(None);
                                    },
                                    if show_golden_entries() { "▲ Hide chunks" } else { "▼ Browse chunks" }
                                }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| show_golden_read_info.set(true),
                                    title: "How to read and evaluate the sample",
                                    InfoIcon {}
                                }
                            }
                        }
                        div { class: "flex flex-wrap gap-6 text-sm mb-3",
                            div { class: "flex flex-col gap-1",
                                span { class: "text-gray-400 text-xs", "Sample size" }
                                span { class: "text-gray-200 font-medium", "{cur} / {cap}" }
                            }
                            div { class: "flex flex-col gap-1",
                                span { class: "text-gray-400 text-xs", "Chunks offered" }
                                span { class: "text-gray-200 font-medium", "{seen}" }
                            }
                            div { class: "flex flex-col gap-1",
                                span { class: "text-gray-400 text-xs", "Tokenizer at capture" }
                                span { class: "text-gray-200 font-medium",
                                    if model.is_empty() { "—" } else { "{model}" }
                                }
                            }
                            div { class: "flex flex-col gap-1",
                                span { class: "text-gray-400 text-xs", "Last update" }
                                span { class: "text-gray-200 font-medium",
                                    if captured.is_empty() { "never" } else { "{captured}" }
                                }
                            }
                            div { class: "flex flex-col gap-1",
                                span { class: "text-gray-400 text-xs", "Seed" }
                                span { class: "text-gray-200 font-mono text-xs", "{seed}" }
                            }
                        }
                        div { class: "w-full bg-gray-700 rounded h-2 mb-3 overflow-hidden",
                            div {
                                class: "h-full {bar_color}",
                                style: "width: {pct}%;",
                            }
                        }
                        div { class: "flex items-center gap-2 flex-wrap",
                            button {
                                class: "btn btn-sm bg-gray-700 hover:bg-gray-600 text-gray-200 border-gray-600",
                                onclick: move |_| async move {
                                    recapture_msg.set(Some("Clearing sample…".into()));
                                    match api::recapture_golden_sample(true).await {
                                        Ok(_) => recapture_msg.set(Some(
                                            "Sample cleared. Ingest a document to repopulate.".into(),
                                        )),
                                        Err(e) => recapture_msg.set(Some(format!("Failed: {}", e))),
                                    }
                                },
                                "Re-capture (clear sample)"
                            }
                            button {
                                class: "w-5 h-5 min-w-5 min-h-5 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80",
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_recapture_info.set(true),
                                title: "What re-capture does",
                                svg {
                                    class: "w-4 h-4 text-white",
                                    view_box: "0 0 20 20",
                                    fill: "none",
                                    stroke: "currentColor",
                                    circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                    line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                    circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                }
                            }
                            if let Some(msg) = last_msg {
                                span { class: "text-xs text-gray-400 ml-2", "{msg}" }
                            }
                        }

                        // Browseable entries table
                        if show_golden_entries() {
                            if let Some(entries) = gs.as_ref().map(|g| &g.entries) {
                                if entries.is_empty() {
                                    p { class: "text-xs text-gray-400 mt-3", "No entries returned — try refreshing." }
                                } else {
                                    div { class: "mt-3 overflow-x-auto",
                                        table { class: "w-full text-xs text-left border-collapse",
                                            thead {
                                                tr { class: "border-b border-gray-600",
                                                    th { class: "py-1 pr-3 text-gray-400 font-medium", "Pos" }
                                                    th { class: "py-1 pr-3 text-gray-400 font-medium", "Tokens" }
                                                    th { class: "py-1 pr-3 text-gray-400 font-medium", "IDs?" }
                                                    th { class: "py-1 pr-3 text-gray-400 font-medium", "Captured" }
                                                    th { class: "py-1 text-gray-400 font-medium", "Preview" }
                                                }
                                            }
                                            tbody {
                                                for entry in entries.iter() {
                                                    {
                                                        let eid = entry.id;
                                                        let is_expanded = expanded_golden_entry() == Some(eid);
                                                        let preview: String = entry.chunk_text.chars().take(80).collect();
                                                        let preview = if entry.chunk_text.chars().count() > 80 {
                                                            format!("{}…", preview)
                                                        } else {
                                                            preview
                                                        };
                                                        let has_ids = entry.baseline_token_ids.is_some();
                                                        let captured = entry.captured_at.get(..16).unwrap_or(&entry.captured_at).to_string();
                                                        let chunk_text = entry.chunk_text.clone();
                                                        let token_ids = entry.baseline_token_ids.clone();
                                                        rsx! {
                                                            tr {
                                                                class: "border-b border-gray-700 hover:bg-gray-700/40 cursor-pointer",
                                                                onclick: move |_| {
                                                                    if is_expanded {
                                                                        expanded_golden_entry.set(None);
                                                                    } else {
                                                                        expanded_golden_entry.set(Some(eid));
                                                                    }
                                                                },
                                                                td { class: "py-1 pr-3 text-gray-300 font-mono", "{entry.position_in_corpus}" }
                                                                td { class: "py-1 pr-3 text-gray-200 font-medium", "{entry.baseline_token_count}" }
                                                                td { class: "py-1 pr-3",
                                                                    if has_ids {
                                                                        span { class: "text-green-400", "✓" }
                                                                    } else {
                                                                        span { class: "text-gray-500", "—" }
                                                                    }
                                                                }
                                                                td { class: "py-1 pr-3 text-gray-400 font-mono", "{captured}" }
                                                                td { class: "py-1 text-gray-300", "{preview}" }
                                                            }
                                                            if is_expanded {
                                                                tr { class: "bg-gray-800/60",
                                                                    td { colspan: "5", class: "py-2 px-1",
                                                                        div { class: "flex flex-col gap-2",
                                                                            div { class: "font-mono text-xs text-gray-200 whitespace-pre-wrap bg-gray-900 rounded p-2 max-h-48 overflow-y-auto",
                                                                                "{chunk_text}"
                                                                            }
                                                                            if let Some(ids) = token_ids {
                                                                                div { class: "flex flex-col gap-0.5",
                                                                                    span { class: "text-gray-400 text-xs", "Token IDs ({ids.len()} tokens):" }
                                                                                    div { class: "font-mono text-xs text-gray-300 bg-gray-900 rounded p-2 max-h-20 overflow-y-auto break-all",
                                                                                        {ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(" ")}
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
                                }
                            }
                        }
                    }
                }
            }

            // Compare & Swap Tokenizer
            {
                let report = diff_report();
                let report_summary = report.as_ref().map(|r| r.summary.clone());
                let report_entries = report.as_ref().map(|r| r.entries.clone()).unwrap_or_default();
                let candidate_model_name = report.as_ref().map(|r| r.candidate_model_name.clone()).unwrap_or_default();
                let candidate_vocab = report.as_ref().map(|r| r.candidate_vocab_size).unwrap_or(0);
                let candidate_path_resolved = report.as_ref().map(|r| r.candidate_path.clone()).unwrap_or_default();
                let baseline_model = report.as_ref().and_then(|r| r.baseline_tokenizer_model.clone()).unwrap_or_else(|| "—".into());
                let golden_size = golden().as_ref().and_then(|g| g.status.as_ref()).map(|s| s.current_size).unwrap_or(0);
                let kind = candidate_kind();
                let input_text = candidate_input();
                let is_loading = diff_loading();
                let err = diff_error();
                let swap_in_flight = swap_loading();
                let swap_message = swap_msg();
                let exp_id = expanded_entry();
                rsx! {
                    Panel { title: None, refresh: None,
                        div { class: "flex items-center gap-2 mb-3",
                            h3 { class: "text-sm font-semibold text-gray-200", "Compare & Swap Tokenizer" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_compare_info.set(true),
                                title: "About comparing tokenizers",
                                InfoIcon {}
                            }
                        }
                        div { class: "text-xs text-gray-400 mb-3",
                            "Run a candidate tokenizer against the {golden_size}-chunk golden baseline. Read-only until you click Accept swap."
                        }

                        // Picker
                        div { class: "p-3 rounded bg-gray-900/40 border border-gray-700 mb-3",
                            div { class: "flex items-center gap-2 mb-3",
                                h4 { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide", "Candidate" }
                                button {
                                    class: "w-5 h-5 min-w-5 min-h-5 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80",
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| show_picker_info.set(true),
                                    title: "How to specify a candidate",
                                    svg {
                                        class: "w-4 h-4 text-white",
                                        view_box: "0 0 20 20",
                                        fill: "none",
                                        stroke: "currentColor",
                                        circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                        line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                    }
                                }
                            }

                            // Runtime candidates
                            p { class: "text-xs text-gray-500 uppercase tracking-wide mb-2", "Runtime" }
                            div { class: "flex flex-col gap-2 mb-4",
                                // Ollama
                                div { class: "flex items-stretch gap-2",
                                    label { class: "flex items-center gap-2 w-24 shrink-0",
                                        input {
                                            r#type: "radio",
                                            name: "candidate-kind",
                                            checked: kind == "ollama",
                                            onchange: move |_| candidate_kind.set("ollama"),
                                        }
                                        span { class: "text-sm text-gray-300", "Ollama" }
                                    }
                                    input {
                                        r#type: "text",
                                        class: "input input-sm bg-gray-800 text-gray-200 border-gray-600 flex-1 font-mono text-xs",
                                        disabled: kind != "ollama",
                                        placeholder: "phi:latest",
                                        value: if kind == "ollama" { "{input_text}" } else { "" },
                                        oninput: move |e| candidate_input.set(e.value()),
                                    }
                                    button {
                                        class: "btn btn-sm bg-blue-700 hover:bg-blue-600 text-white border-blue-600",
                                        disabled: is_loading || kind != "ollama" || input_text.trim().is_empty() || golden_size == 0,
                                        onclick: move |_| async move {
                                            let trimmed = candidate_input().trim().to_string();
                                            if trimmed.is_empty() { return; }
                                            diff_loading.set(true);
                                            diff_error.set(None);
                                            diff_report.set(None);
                                            expanded_entry.set(None);
                                            match api::compute_tokenizer_diff(None, Some(trimmed), false, Some(50)).await {
                                                Ok(r) => diff_report.set(Some(r)),
                                                Err(e) => diff_error.set(Some(e)),
                                            }
                                            diff_loading.set(false);
                                        },
                                        if is_loading && kind == "ollama" { "Running…" } else { "Run diff" }
                                    }
                                }
                                // llama.cpp
                                div { class: "flex items-center gap-2",
                                    label { class: "flex items-center gap-2 w-24 shrink-0",
                                        input {
                                            r#type: "radio",
                                            name: "candidate-kind",
                                            checked: kind == "llama_cpp",
                                            onchange: move |_| {
                                                candidate_kind.set("llama_cpp");
                                                candidate_input.set(String::new());
                                            },
                                        }
                                        span { class: "text-sm text-gray-300", "llama.cpp" }
                                    }
                                    span { class: "text-xs text-gray-500 flex-1", "Resolved from ~/.config/ag/llama-server.env" }
                                    button {
                                        class: "btn btn-sm bg-blue-700 hover:bg-blue-600 text-white border-blue-600",
                                        disabled: is_loading || kind != "llama_cpp" || golden_size == 0,
                                        onclick: move |_| async move {
                                            diff_loading.set(true);
                                            diff_error.set(None);
                                            diff_report.set(None);
                                            expanded_entry.set(None);
                                            match api::compute_tokenizer_diff(None, None, true, Some(50)).await {
                                                Ok(r) => diff_report.set(Some(r)),
                                                Err(e) => diff_error.set(Some(e)),
                                            }
                                            diff_loading.set(false);
                                        },
                                        if is_loading && kind == "llama_cpp" { "Running…" } else { "Run diff" }
                                    }
                                }
                            }

                            // GGUF file — advanced
                            p { class: "text-xs text-gray-500 uppercase tracking-wide mb-2", "GGUF file" }
                            div { class: "flex items-stretch gap-2",
                                label { class: "flex items-center gap-2 w-24 shrink-0",
                                    input {
                                        r#type: "radio",
                                        name: "candidate-kind",
                                        checked: kind == "path",
                                        onchange: move |_| candidate_kind.set("path"),
                                    }
                                    span { class: "text-sm text-gray-300", "Path" }
                                }
                                input {
                                    r#type: "text",
                                    class: "input input-sm bg-gray-800 text-gray-200 border-gray-600 flex-1 font-mono text-xs",
                                    disabled: kind != "path",
                                    placeholder: "/absolute/path/to/model.gguf",
                                    value: if kind == "path" { "{input_text}" } else { "" },
                                    oninput: move |e| candidate_input.set(e.value()),
                                }
                                button {
                                    class: "btn btn-sm bg-blue-700 hover:bg-blue-600 text-white border-blue-600",
                                    disabled: is_loading || kind != "path" || input_text.trim().is_empty() || golden_size == 0,
                                    onclick: move |_| async move {
                                        let trimmed = candidate_input().trim().to_string();
                                        if trimmed.is_empty() { return; }
                                        diff_loading.set(true);
                                        diff_error.set(None);
                                        diff_report.set(None);
                                        expanded_entry.set(None);
                                        match api::compute_tokenizer_diff(Some(trimmed), None, false, Some(50)).await {
                                            Ok(r) => diff_report.set(Some(r)),
                                            Err(e) => diff_error.set(Some(e)),
                                        }
                                        diff_loading.set(false);
                                    },
                                    if is_loading && kind == "path" { "Running…" } else { "Run diff" }
                                }
                            }

                            if golden_size == 0 {
                                p { class: "text-xs text-yellow-300 mt-3",
                                    "The golden sample is empty. Ingest a document first so there's a baseline to diff against."
                                }
                            }
                        }

                        if let Some(e) = err {
                            div { class: "p-3 rounded bg-red-900/30 border border-red-700 text-sm text-red-300 mb-3",
                                "Diff failed: {e}"
                            }
                        }

                        // Diff results
                        if let Some(s) = report_summary {
                            {
                                let total_pct = s.total_delta_pct.map(|p| format!("{:+.2}%", p)).unwrap_or_else(|| "—".into());
                                let mean_signed = format!("{:+.2}", s.mean_count_delta);
                                let mean_abs = format!("{:.2}", s.mean_count_delta_abs);
                                rsx! {
                                    div { class: "p-3 rounded bg-gray-900/40 border border-gray-700 mb-3",
                                        div { class: "flex items-center justify-between mb-2",
                                            div { class: "text-xs text-gray-400",
                                                "Candidate: "
                                                span { class: "text-gray-200 font-medium", "{candidate_model_name}" }
                                                " (vocab {candidate_vocab})"
                                            }
                                            div { class: "text-xs text-gray-400",
                                                "Baseline: "
                                                span { class: "text-gray-200 font-medium", "{baseline_model}" }
                                            }
                                        }
                                        div { class: "text-xs text-gray-500 font-mono mb-3 truncate", title: "{candidate_path_resolved}", "{candidate_path_resolved}" }
                                        div { class: "grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-3 text-sm",
                                            div { class: "flex flex-col gap-1",
                                                span { class: "text-gray-400 text-xs", "Diffed" }
                                                span { class: "text-gray-200 font-medium", "{s.entries_total}" }
                                                if s.entries_skipped > 0 {
                                                    span { class: "text-xs text-gray-500", "({s.entries_skipped} skipped)" }
                                                }
                                            }
                                            div { class: "flex flex-col gap-1",
                                                span { class: "text-gray-400 text-xs", "Identical" }
                                                span { class: "text-green-300 font-medium", "{s.entries_identical}" }
                                            }
                                            div { class: "flex flex-col gap-1",
                                                span { class: "text-gray-400 text-xs", "IDs changed" }
                                                span { class: "text-yellow-300 font-medium", "{s.entries_ids_changed}" }
                                            }
                                            div { class: "flex flex-col gap-1",
                                                span { class: "text-gray-400 text-xs", "Count changed" }
                                                span { class: "text-yellow-300 font-medium", "{s.entries_count_changed}" }
                                            }
                                            div { class: "flex flex-col gap-1",
                                                span { class: "text-gray-400 text-xs", "Mean Δ tokens" }
                                                span { class: "text-gray-200 font-medium", "{mean_signed}" }
                                                span { class: "text-xs text-gray-500", "|Δ|: {mean_abs}" }
                                            }
                                            div { class: "flex flex-col gap-1",
                                                span { class: "text-gray-400 text-xs", "Total token Δ" }
                                                span { class: "text-gray-200 font-medium", "{total_pct}" }
                                                span { class: "text-xs text-gray-500", "{s.total_baseline_tokens} → {s.total_candidate_tokens}" }
                                            }
                                        }
                                        div { class: "text-xs text-gray-500 mt-2",
                                            "Max |Δ| in any single chunk: {s.max_count_delta_abs}"
                                        }
                                    }
                                }
                            }

                            // Per-entry table
                            div { class: "overflow-x-auto mb-3",
                                table { class: "table table-xs w-full text-gray-300",
                                    thead {
                                        tr {
                                            th { class: "text-gray-400", "#" }
                                            th { class: "text-gray-400 text-right", "Baseline" }
                                            th { class: "text-gray-400 text-right", "Candidate" }
                                            th { class: "text-gray-400 text-right", "Δ" }
                                            th { class: "text-gray-400 text-center", "IDs" }
                                            th { class: "text-gray-400", "Diverges" }
                                            th { class: "text-gray-400", "Preview" }
                                            th { class: "text-gray-400", "" }
                                        }
                                    }
                                    tbody {
                                        for entry in report_entries.iter() {
                                            {
                                                let entry_id = entry.id;
                                                let pos = entry.position_in_corpus;
                                                let bc = entry.baseline_count;
                                                let cc = entry.candidate_count;
                                                let delta = entry.count_delta;
                                                let delta_class = if delta == 0 { "text-gray-400" } else if delta > 0 { "text-yellow-300" } else { "text-blue-300" };
                                                let delta_str = if delta == 0 { "0".to_string() } else { format!("{:+}", delta) };
                                                let ids_match = entry.ids_match;
                                                let prefix = entry.common_prefix_len;
                                                let suffix = entry.common_suffix_len;
                                                let mid_b = bc.saturating_sub(prefix + suffix);
                                                let mid_c = cc.saturating_sub(prefix + suffix);
                                                let diverges = if ids_match {
                                                    "—".to_string()
                                                } else {
                                                    format!("prefix {} · mid {}↔{} · suffix {}", prefix, mid_b, mid_c, suffix)
                                                };
                                                let preview: String = entry.chunk_text.chars().take(80).collect();
                                                let preview = if entry.chunk_text.chars().count() > 80 {
                                                    format!("{}…", preview)
                                                } else {
                                                    preview
                                                };
                                                let is_expanded = exp_id == Some(entry_id);
                                                let baseline_ids = entry.baseline_token_ids.clone();
                                                let candidate_ids = entry.candidate_token_ids.clone();
                                                rsx! {
                                                    tr { key: "{entry_id}", class: "hover:bg-gray-800/40",
                                                        td { class: "text-gray-400 text-xs", "{pos}" }
                                                        td { class: "text-right text-gray-300", "{bc}" }
                                                        td { class: "text-right text-gray-300", "{cc}" }
                                                        td { class: "text-right {delta_class} font-medium", "{delta_str}" }
                                                        td { class: "text-center",
                                                            if ids_match {
                                                                span { class: "text-green-400", "✓" }
                                                            } else {
                                                                span { class: "text-yellow-400", "≠" }
                                                            }
                                                        }
                                                        td { class: "text-xs text-gray-400 font-mono", "{diverges}" }
                                                        td { class: "text-xs text-gray-400 max-w-md truncate", title: "{entry.chunk_text}", "{preview}" }
                                                        td {
                                                            button {
                                                                class: "btn btn-xs bg-gray-700 hover:bg-gray-600 text-gray-200 border-gray-600",
                                                                onclick: move |_| {
                                                                    if exp_id == Some(entry_id) {
                                                                        expanded_entry.set(None);
                                                                    } else {
                                                                        expanded_entry.set(Some(entry_id));
                                                                    }
                                                                },
                                                                if is_expanded { "Hide" } else { "IDs" }
                                                            }
                                                        }
                                                    }
                                                    if is_expanded {
                                                        tr { key: "{entry_id}-exp",
                                                            td { colspan: "8",
                                                                div { class: "p-3 bg-gray-900/60 rounded grid grid-cols-1 md:grid-cols-2 gap-3",
                                                                    div {
                                                                        div { class: "text-xs text-gray-400 mb-1", "Baseline IDs ({baseline_ids.len()})" }
                                                                        div { class: "text-xs text-gray-200 font-mono break-all max-h-40 overflow-y-auto",
                                                                            "{baseline_ids:?}"
                                                                        }
                                                                    }
                                                                    div {
                                                                        div { class: "text-xs text-gray-400 mb-1", "Candidate IDs ({candidate_ids.len()})" }
                                                                        div { class: "text-xs text-gray-200 font-mono break-all max-h-40 overflow-y-auto",
                                                                            "{candidate_ids:?}"
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

                            // Accept-swap controls
                            div { class: "flex items-center gap-2 pt-2 border-t border-gray-700",
                                button {
                                    class: "btn btn-sm bg-amber-700 hover:bg-amber-600 text-white border-amber-600",
                                    disabled: swap_in_flight,
                                    onclick: move |_| async move {
                                        swap_loading.set(true);
                                        swap_msg.set(Some("Swapping tokenizer…".into()));
                                        let trimmed = candidate_input().trim().to_string();
                                        let kind_now = candidate_kind();
                                        let (path, ollama, llama_cpp) = if kind_now == "path" {
                                            (Some(trimmed), None, false)
                                        } else if kind_now == "ollama" {
                                            (None, Some(trimmed), false)
                                        } else {
                                            (None, None, true)
                                        };
                                        match api::swap_tokenizer(path, ollama, llama_cpp).await {
                                            Ok(_) => swap_msg.set(Some(
                                                "Swap accepted. The live tokenizer is now the candidate. Re-capture the golden sample so the new baseline reflects this tokenizer.".into(),
                                            )),
                                            Err(e) => swap_msg.set(Some(format!("Swap failed: {}", e))),
                                        }
                                        swap_loading.set(false);
                                    },
                                    if swap_in_flight { "Swapping…" } else { "Accept swap" }
                                }
                                button {
                                    class: "w-5 h-5 min-w-5 min-h-5 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80",
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| show_swap_info.set(true),
                                    title: "What accept-swap does",
                                    svg {
                                        class: "w-4 h-4 text-white",
                                        view_box: "0 0 20 20",
                                        fill: "none",
                                        stroke: "currentColor",
                                        circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                        line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                    }
                                }
                                if let Some(m) = swap_message {
                                    span { class: "text-xs text-gray-300 ml-2", "{m}" }
                                }
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
                                        th { class: "text-gray-400",
                                            div { class: "flex items-center gap-1",
                                                span { "Mode" }
                                                button {
                                                    class: "w-4 h-4 min-w-4 min-h-4 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80",
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_mode_info.set(true),
                                                    title: "About chunker modes",
                                                    svg {
                                                        class: "w-3 h-3 text-white",
                                                        view_box: "0 0 20 20",
                                                        fill: "none",
                                                        stroke: "currentColor",
                                                        circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                        line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                                    }
                                                }
                                            }
                                        }
                                        th { class: "text-gray-400 text-right", "Chunks" }
                                        th { class: "text-gray-400 text-right", "Tokens" }
                                        th { class: "text-gray-400 text-right", "Duration" }
                                        th { class: "text-gray-400", "Format" }
                                        th { class: "text-gray-400",
                                            div { class: "flex items-center gap-1",
                                                span { "Strategy" }
                                                button {
                                                    class: "w-4 h-4 min-w-4 min-h-4 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80",
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_strategy_info.set(true),
                                                    title: "About chunking strategies",
                                                    svg {
                                                        class: "w-3 h-3 text-white",
                                                        view_box: "0 0 20 20",
                                                        fill: "none",
                                                        stroke: "currentColor",
                                                        circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                        line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                                    }
                                                }
                                            }
                                        }
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
                                            let strategy_expected = snap.detection.as_ref()
                                                .and_then(|d| d.extension.as_deref()
                                                    .or_else(|| snap.file.rsplit('.').next()))
                                                .and_then(expected_strategy);
                                            let strategy_mismatch = strategy_expected
                                                .map(|e| e != strategy.as_str())
                                                .unwrap_or(false);
                                            rsx! {
                                                tr { class: "hover:bg-gray-800/50",
                                                    td { class: "font-mono text-xs", "{time_short}" }
                                                    td { class: "max-w-48 truncate", title: "{snap.file}", "{file_short}" }
                                                    td { "{snap.chunker_mode}" }
                                                    td { class: "text-right", "{snap.chunks}" }
                                                    td { class: "text-right", "{snap.tokens}" }
                                                    td { class: "text-right", "{snap.duration_ms}ms" }
                                                    td { class: "text-xs", "{detected_fmt}" }
                                                    td {
                                                        class: if strategy_mismatch { "text-xs text-yellow-400" } else { "text-xs" },
                                                        title: if strategy_mismatch {
                                                            format!("Expected {} for this extension", strategy_expected.unwrap_or("unknown"))
                                                        } else {
                                                            String::new()
                                                        },
                                                        if strategy_mismatch { "⚠ " } else { "" }
                                                        "{strategy}"
                                                    }
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
                                                        let icon = if snap_tok == "unknown" || matches_active {
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

            // Chunk Preview panel
            Panel { title: Some("Chunk Preview".into()), refresh: None,
                div { class: "flex flex-col gap-3",
                    span { class: "text-xs text-gray-400",
                        "Paste sample text to preview how it will be chunked with the current configuration. No documents are indexed."
                    }
                    div { class: "flex flex-col gap-2",
                        textarea {
                            class: PREVIEW_TEXTAREA_CLASS,
                            placeholder: "Paste text here…",
                            value: "{preview_text()}",
                            oninput: move |e| preview_text.set(e.value()),
                        }
                        div { class: "flex items-center gap-3",
                            input {
                                class: "input input-xs input-bordered bg-gray-700 text-gray-200 w-48",
                                placeholder: "filename (optional)",
                                value: "{preview_filename()}",
                                oninput: move |e| preview_filename.set(e.value()),
                            }
                            button {
                                class: "btn btn-xs text-white",
                                style: "background-color: #7C2A02; border-color: #7C2A02;",
                                disabled: preview_loading() || preview_text().is_empty(),
                                onclick: move |_| {
                                    let text = preview_text();
                                    let filename = preview_filename();
                                    spawn(async move {
                                        preview_loading.set(true);
                                        preview_error.set(None);
                                        let req = api::ChunkPreviewRequest {
                                            text,
                                            filename: if filename.is_empty() { None } else { Some(filename) },
                                        };
                                        match api::chunk_preview(&req).await {
                                            Ok(resp) => preview_result.set(Some(resp)),
                                            Err(e) => preview_error.set(Some(e)),
                                        }
                                        preview_loading.set(false);
                                    });
                                },
                                if preview_loading() { "Previewing…" } else { "Preview" }
                            }
                            if let Some(res) = preview_result() {
                                span { class: "text-xs text-gray-400",
                                    "{res.chunk_count} chunks — mode: {res.mode}"
                                }
                            }
                        }
                    }

                    if let Some(err) = preview_error() {
                        div { class: "text-xs text-red-400", "{err}" }
                    }

                    if let Some(res) = preview_result() {
                        // Stats summary
                        if let Some(s) = &res.stats {
                            div { class: "flex flex-wrap gap-4 text-xs text-gray-300 bg-gray-900 rounded p-3 border border-gray-700",
                                div { class: "flex flex-col gap-1",
                                    span { class: "text-gray-500", "avg tokens" }
                                    span { "{s.avg_chunk_tokens}" }
                                }
                                div { class: "flex flex-col gap-1",
                                    span { class: "text-gray-500", "min tokens" }
                                    span { "{s.min_chunk_tokens}" }
                                }
                                div { class: "flex flex-col gap-1",
                                    span { class: "text-gray-500", "max tokens" }
                                    span { "{s.max_chunk_tokens}" }
                                }
                                div { class: "flex flex-col gap-1",
                                    span { class: "text-gray-500", "size flushes" }
                                    span { "{s.size_flushes}" }
                                }
                                div { class: "flex flex-col gap-1",
                                    span { class: "text-gray-500", "sentence flushes" }
                                    span { "{s.sentence_flushes}" }
                                }
                                div { class: "flex flex-col gap-1",
                                    span { class: "text-gray-500", "semantic flushes" }
                                    span { "{s.semantic_flushes}" }
                                }
                                div { class: "flex flex-col gap-1",
                                    span { class: "text-gray-500", "heading flushes" }
                                    span { "{s.heading_flushes}" }
                                }
                                if s.html_tags_stripped > 0 {
                                    div { class: "flex flex-col gap-1",
                                        span { class: "text-gray-500", "html stripped" }
                                        span { class: "text-yellow-400", "{s.html_tags_stripped}" }
                                    }
                                }
                                if s.unicode_chars_normalized > 0 {
                                    div { class: "flex flex-col gap-1",
                                        span { class: "text-gray-500", "unicode norm." }
                                        span { class: "text-yellow-400", "{s.unicode_chars_normalized}" }
                                    }
                                }
                            }
                        }

                        // Chunk list
                        div { class: "flex flex-col gap-2 max-h-96 overflow-y-auto",
                            for (idx, chunk) in res.chunks.iter().enumerate() {
                                {
                                    let tok_approx = chunk.split_whitespace().count() * 4 / 3;
                                    let min_size = 128usize; // rough visual threshold
                                    let max_size = 384usize;
                                    let color = if tok_approx < min_size {
                                        "border-yellow-600/40 bg-yellow-900/10"
                                    } else if tok_approx > max_size {
                                        "border-red-600/40 bg-red-900/10"
                                    } else {
                                        "border-green-700/30 bg-green-900/10"
                                    };
                                    rsx! {
                                        div {
                                            class: "rounded border p-2 text-xs text-gray-300 font-mono whitespace-pre-wrap {color}",
                                            div { class: "text-gray-500 mb-1 text-[0.65rem]", "#{idx + 1} · ~{tok_approx} tokens" }
                                            "{chunk}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Text Normalization panel
            if let Some(cs) = canon_stats() {
                Panel { title: None, refresh: None,
                    div { class: "flex items-center gap-2 mb-3",
                        h3 { class: "text-sm font-semibold text-gray-200", "Text Normalization" }
                        button {
                            class: PARAM_ICON_BUTTON_CLASS,
                            style: PARAM_ICON_BUTTON_STYLE,
                            onclick: move |_| show_canon_info.set(true),
                            title: "About text normalization",
                            InfoIcon {}
                        }
                    }
                    // Call-site stat rows
                    div { class: "overflow-x-auto",
                        table { class: "table table-xs w-full text-gray-300",
                            thead {
                                tr {
                                    th { class: "text-gray-400", "Step" }
                                    th { class: "text-gray-400 text-right", "Times run" }
                                    th { class: "text-gray-400 text-right", "Chars in" }
                                    th { class: "text-gray-400 text-right", "Chars out" }
                                    th { class: "text-gray-400 text-right", "Δ%" }
                                }
                            }
                            tbody {
                                {
                                    let rows: &[(&str, &api::CallSiteStats)] = &[
                                        ("save · upload",            &cs.store_ingestion),
                                        ("vectorise · upload",       &cs.embed_ingestion),
                                        ("keyword search · upload",  &cs.index_ingestion),
                                        ("vectorise · search",       &cs.embed_query),
                                        ("keyword search · search",  &cs.index_query),
                                    ];
                                    rsx! {
                                        for (label, site) in rows.iter() {
                                            {
                                                let delta_pct = if site.chars_in > 0 {
                                                    let diff = site.chars_in as i64 - site.chars_out as i64;
                                                    let pct = diff as f64 / site.chars_in as f64 * 100.0;
                                                    Some(pct)
                                                } else {
                                                    None
                                                };
                                                let (delta_str, delta_cls) = match delta_pct {
                                                    None => ("—".to_string(), "text-gray-500"),
                                                    Some(p) if p.abs() < 0.1 => (format!("{:+.1}%", p), "text-gray-400"),
                                                    Some(p) if p > 0.0 => (format!("{:+.1}%", p), "text-yellow-400"),
                                                    Some(p) => (format!("{:+.1}%", p), "text-green-400"),
                                                };
                                                rsx! {
                                                    tr { class: "hover:bg-gray-800/50",
                                                        td { class: "font-mono text-xs text-gray-400", "{label}" }
                                                        td { class: "text-right", "{site.calls}" }
                                                        td { class: "text-right font-mono text-xs", "{site.chars_in}" }
                                                        td { class: "text-right font-mono text-xs", "{site.chars_out}" }
                                                        td { class: "text-right font-mono text-xs {delta_cls}", "{delta_str}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Per-file normalization records
                    if !cs.store_records.is_empty() {
                        {
                            let delta = |i: u64, o: u64| -> f64 {
                                if i > 0 { (i as i64 - o as i64) as f64 / i as f64 * 100.0 } else { 0.0 }
                            };
                            let delta_fmt = |pct: f64| -> (&'static str, String) {
                                if pct.abs() < 0.1 { ("text-gray-400", format!("{:+.1}%", pct)) }
                                else if pct > 0.0   { ("text-yellow-400", format!("{:+.1}%", pct)) }
                                else                { ("text-green-400",  format!("{:+.1}%", pct)) }
                            };
                            // Show embed/index columns only when divergence from store exceeds 0.5% for any file.
                            let show_detail = cs.store_records.iter().any(|r| {
                                let s = delta(r.chars_in, r.chars_out);
                                let e = delta(r.embed_chars_in, r.embed_chars_out);
                                let ix = delta(r.index_chars_in, r.index_chars_out);
                                (e - s).abs() > 0.5 || (ix - s).abs() > 0.5
                            });
                            rsx! {
                                div { class: "mt-4",
                                    div { class: "flex items-center gap-2 mb-2",
                                        p { class: "text-xs text-gray-500",
                                            if show_detail { "Recent files — save / vectorise / keyword-search" }
                                            else { "Recent files" }
                                        }
                                        button {
                                            class: "w-4 h-4 min-w-4 min-h-4 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80",
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_file_norm_info.set(true),
                                            title: "About per-file normalization columns",
                                            svg {
                                                class: "w-3 h-3 text-white",
                                                view_box: "0 0 20 20",
                                                fill: "none",
                                                stroke: "currentColor",
                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }
                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                                circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                            }
                                        }
                                    }
                                    div { class: "overflow-x-auto",
                                        table { class: "table table-xs w-full text-gray-300",
                                            thead {
                                                tr {
                                                    th { class: "text-gray-400", "File" }
                                                    if show_detail {
                                                        th { class: "text-gray-400 text-right", "Save Δ%" }
                                                        th { class: "text-gray-400 text-right", "Vectorise Δ%" }
                                                        th { class: "text-gray-400 text-right", "Keyword Δ%" }
                                                    } else {
                                                        th { class: "text-gray-400 text-right", "Chars in" }
                                                        th { class: "text-gray-400 text-right", "Chars out" }
                                                        th { class: "text-gray-400 text-right", "Δ%" }
                                                    }
                                                }
                                            }
                                            tbody {
                                                for rec in cs.store_records.iter() {
                                                    {
                                                        let file_short = rec.file.rsplit('/').next().unwrap_or(&rec.file).to_string();
                                                        let file_full = rec.file.clone();
                                                        let s_pct = delta(rec.chars_in, rec.chars_out);
                                                        let e_pct = delta(rec.embed_chars_in, rec.embed_chars_out);
                                                        let ix_pct = delta(rec.index_chars_in, rec.index_chars_out);
                                                        let (s_cls, s_str) = delta_fmt(s_pct);
                                                        let (e_cls, e_str) = delta_fmt(e_pct);
                                                        let (ix_cls, ix_str) = delta_fmt(ix_pct);
                                                        let chars_in = rec.chars_in;
                                                        let chars_out = rec.chars_out;
                                                        rsx! {
                                                            tr { class: "hover:bg-gray-800/50",
                                                                td { class: "max-w-48 truncate text-xs", title: "{file_full}", "{file_short}" }
                                                                if show_detail {
                                                                    td { class: "text-right font-mono text-xs {s_cls}", "{s_str}" }
                                                                    td { class: "text-right font-mono text-xs {e_cls}", "{e_str}" }
                                                                    td { class: "text-right font-mono text-xs {ix_cls}", "{ix_str}" }
                                                                } else {
                                                                    td { class: "text-right font-mono text-xs", "{chars_in}" }
                                                                    td { class: "text-right font-mono text-xs", "{chars_out}" }
                                                                    td { class: "text-right font-mono text-xs {s_cls}", "{s_str}" }
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
            }

            // Chunker mode info modal
            if show_mode_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_mode_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-lg max-h-[90vh] overflow-y-auto shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4",
                            h2 { class: "text-lg font-semibold text-gray-100", "Chunker Mode" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_mode_info.set(false),
                                "x"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-4",
                            p { class: "text-gray-400", "The chunker mode controls how a document is split into pieces before indexing. Each mode is a different answer to the same question: where should one chunk end and the next begin?" }

                            // fixed
                            div { class: "p-3 rounded-lg space-y-2", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200", "fixed" }
                                p { class: "text-gray-400 text-xs", "Splits on a hard token count. Every chunk is exactly max_size tokens (default 384), with no awareness of sentences, paragraphs, or meaning. The last chunk of a document may be shorter." }
                                p { class: "text-gray-400 text-xs", "Overlap (default 32 tokens) is carried forward from the tail of the previous chunk so that a sentence cut at a boundary can still be retrieved from either side." }
                                p { class: "text-gray-500 text-xs", "Best for: structured, uniform corpora (logs, CSVs, code) where sentence coherence is irrelevant. Avoid for prose — a sentence will frequently be split mid-way, degrading retrieval quality." }
                            }

                            // lightweight
                            div { class: "p-3 rounded-lg space-y-2", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200", "lightweight" }
                                p { class: "text-gray-400 text-xs", "Accumulates sentences until the chunk reaches a target token count (default 384), then flushes at the next sentence boundary. If a single sentence would overflow the hard max, it is flushed immediately regardless of boundary." }
                                p { class: "text-gray-400 text-xs", "Sentence detection uses punctuation patterns (.!? followed by a capital letter), so it works without any NLP model. The sentence_flushes counter in the preview stats shows how many times the chunker waited for a boundary rather than cutting mid-sentence." }
                                p { class: "text-gray-500 text-xs", "Best for: general prose — articles, PDFs, documentation. The default mode. Faster than semantic and produces readable, retrievable passages." }
                            }

                            // semantic
                            div { class: "p-3 rounded-lg space-y-2", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200", "semantic" }
                                p { class: "text-gray-400 text-xs", "First splits the document into natural units — paragraphs, headings, code blocks — then embeds each unit and compares consecutive embeddings. When the cosine similarity between two adjacent units falls below a threshold (default 0.78), it treats that gap as a topic shift and flushes a chunk." }
                                p { class: "text-gray-400 text-xs", "The result: each chunk covers one coherent idea. A paragraph about database indexing won't share a chunk with one about UI styling, even if both fit within the token limit. The semantic_flushes counter shows how many times a topic-shift boundary was detected." }
                                p { class: "text-gray-400 text-xs", "The similarity threshold is tunable via SEMANTIC_SIMILARITY_THRESHOLD. Lower values (e.g. 0.65) produce larger chunks spanning more related content; higher values (e.g. 0.90) produce smaller, tightly-scoped chunks." }
                                p { class: "text-gray-500 text-xs", "Best for: long, mixed-topic documents where retrieval precision matters. Requires the embedding model to be running. Slowest mode — expect 2–5× the ingestion time of lightweight." }
                            }

                            // shared mechanics
                            div { class: "p-3 rounded-lg space-y-1", style: "background-color: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.06);",
                                p { class: "text-xs text-gray-400 font-medium mb-1", "Shared mechanics (all modes)" }
                                p { class: "text-xs text-gray-500", "Min chunk size: 128 tokens — chunks smaller than this are merged with the next unit rather than indexed alone." }
                                p { class: "text-xs text-gray-500", "Max chunk size: 384 tokens — hard ceiling; a chunk that would exceed this is always flushed regardless of boundaries." }
                                p { class: "text-xs text-gray-500", "Overlap: 32 tokens by default — the tail of each chunk is prepended to the next, so context that straddles a boundary is retrievable from either chunk." }
                                p { class: "text-xs text-gray-500", "All three values are tunable via CHUNK_MIN_SIZE, CHUNK_MAX_SIZE, and CHUNK_OVERLAP. The active mode is set via CHUNKER_MODE. Change any of these and re-index for them to take effect." }
                            }
                        }
                    }
                }
            }

            // Chunking strategy info modal
            if show_strategy_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_strategy_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[98vw] shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-3",
                            h2 { class: "text-base font-semibold text-gray-100", "Chunking Strategy" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_strategy_info.set(false),
                                "✕"
                            }
                        }
                        p { class: "text-xs text-gray-400 mb-3", "A descriptive label derived from the file's detected format. It tells you what kind of natural boundaries exist in this document type — it does not select a different code path at chunking time. All three chunker modes (fixed, lightweight, semantic) process every format through the same pipeline." }
                        p { class: "text-xs text-gray-400 mb-3", "The actual boundary-honoring happens in two places that run the same for all modes: the DocIR extraction pipeline (which parses documents into typed blocks — headings, paragraphs, tables, code), and the IR chunker which always flushes on atomic blocks (tables, code, formulas) and heading boundaries regardless of mode. The mode then applies its token budget on top of that." }
                        div { class: "grid grid-cols-2 gap-2 mb-3",
                            div { class: "p-2 rounded space-y-0.5", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200 text-xs", "paragraph_split" }
                                p { class: "text-gray-500 text-xs", "Natural boundary: blank lines between paragraphs. Plain text, .docx, .odt." }
                            }
                            div { class: "p-2 rounded space-y-0.5", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200 text-xs", "character_split" }
                                p { class: "text-gray-500 text-xs", "Natural boundary: character count. PDF — paragraph structure is lost after text extraction." }
                            }
                            div { class: "p-2 rounded space-y-0.5", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200 text-xs", "header_aware" }
                                p { class: "text-gray-500 text-xs", "Natural boundary: Markdown headings (#, ##, ###). Headings become strong-boundary blocks in DocIR and always flush the accumulation." }
                            }
                            div { class: "p-2 rounded space-y-0.5", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200 text-xs", "tag_aware" }
                                p { class: "text-gray-500 text-xs", "Natural boundary: block-level HTML/XML tags. Markup is stripped before indexing; tag structure guides the IR block split." }
                            }
                            div { class: "p-2 rounded space-y-0.5", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200 text-xs", "structure_aware" }
                                p { class: "text-gray-500 text-xs", "Natural boundary: JSON keys / array elements. Objects are kept whole — partial records can't be meaningfully retrieved." }
                            }
                            div { class: "p-2 rounded space-y-0.5", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200 text-xs", "ast_based" }
                                p { class: "text-gray-500 text-xs", "Natural boundary: top-level syntax units — functions, classes, declarations. Source code. Each unit becomes an atomic block in DocIR." }
                            }
                            div { class: "p-2 rounded space-y-0.5", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200 text-xs", "row_split" }
                                p { class: "text-gray-500 text-xs", "Natural boundary: rows. CSV, .xlsx, .ods. Rows group up to the token budget; column headers are preserved." }
                            }
                            div { class: "p-2 rounded space-y-0.5", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200 text-xs", "slide_split" }
                                p { class: "text-gray-500 text-xs", "Natural boundary: slide breaks. .pptx. Each slide is an atomic block — title, bullets, and notes stay together." }
                            }
                            div { class: "p-2 rounded space-y-0.5", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200 text-xs", "chapter_split" }
                                p { class: "text-gray-500 text-xs", "Natural boundary: chapter/section markers. EPUB. Each chapter is a strong-boundary block." }
                            }
                            div { class: "p-2 rounded space-y-0.5", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200 text-xs", "fallback_paragraph" }
                                p { class: "text-gray-500 text-xs", "Best-effort paragraph split for unrecognised formats. Same as paragraph_split but signals format detection fell back to a heuristic." }
                            }
                            div { class: "p-2 rounded space-y-0.5", style: "background-color: rgba(255,255,255,0.04); border-left: 2px solid #6b7280;",
                                p { class: "font-semibold text-gray-200 text-xs", "skip" }
                                p { class: "text-gray-500 text-xs", "Binary file — not indexed. No chunks produced." }
                            }
                        }
                        div { class: "p-2 rounded mb-3", style: "background-color: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.06);",
                            p { class: "text-xs text-gray-400 font-medium", "Not configurable" }
                            p { class: "text-xs text-gray-500 mt-0.5", "Strategy is set by format detection — you can't change it. What you control is Mode (fixed / lightweight / semantic) via CHUNKER_MODE." }
                        }
                        button {
                            class: "btn btn-sm w-full",
                            style: "background-color:#7C2A02;border-color:#7C2A02;color:white;",
                            onclick: move |_| show_strategy_info.set(false),
                            "Got it"
                        }
                    }
                }
            }

            // Per-file normalization columns info modal
            if show_file_norm_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_file_norm_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-[580px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4 shrink-0",
                            h2 { class: "text-lg font-semibold text-gray-100", "Per-file Normalization" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_file_norm_info.set(false),
                                "x"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-3 overflow-y-auto",
                            p { "Each uploaded document is cleaned up at three stages before it enters the search engine. This table tracks how much each stage shrinks the text." }
                            p { class: "font-semibold text-gray-200", "The three stages" }
                            p {
                                span { class: "font-mono text-gray-100", "save " }
                                "— Applied once to the full extracted text before it is split into chunks. Preserves the text as-is for display. Near-zero Δ% is normal."
                            }
                            p {
                                span { class: "font-mono text-gray-100", "vectorise " }
                                "— Applied to each chunk before converting it to a vector for semantic search. Collapses compatibility characters (e.g. the ligature "
                                span { class: "font-mono text-gray-100", "ﬁ → fi" }
                                ", fullwidth letters). A higher Δ% than save means the document contains such characters."
                            }
                            p {
                                span { class: "font-mono text-gray-100", "keyword search " }
                                "— Applied to each chunk before adding it to the keyword search index. Also normalises punctuation. Usually slightly higher Δ% than vectorise."
                            }
                            p { class: "font-semibold text-gray-200", "When the extra columns appear" }
                            p {
                                "The vectorise and keyword-search columns are hidden when all files show less than "
                                span { class: "font-mono text-gray-100", "0.5%" }
                                " difference between the save Δ% and the other two. For clean documents that threshold is never crossed — the extra columns would just be noise. For scanned PDFs, documents with typographic ligatures, or East Asian text, the difference is real and the columns expand automatically."
                            }
                            p { class: "text-xs text-gray-500 mt-2",
                                "A positive Δ% means characters were collapsed (text got shorter). A negative Δ% means the text got longer — rare, but possible with some character forms."
                            }
                        }
                    }
                }
            }

            // Canon stats info modal
            if show_canon_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_canon_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-lg shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4",
                            h2 { class: "text-lg font-semibold text-gray-100", "Text Normalization" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_canon_info.set(false),
                                "x"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-3",
                            p { "AG cleans up text differently depending on what it will be used for. The table shows how much each cleaning step shrinks the text." }
                            div { class: "overflow-x-auto",
                                table { class: "table table-xs w-full text-gray-300 mt-2",
                                    thead {
                                        tr {
                                            th { class: "text-gray-400", "Step" }
                                            th { class: "text-gray-400", "Unicode" }
                                            th { class: "text-gray-400", "When" }
                                        }
                                    }
                                    tbody {
                                        tr {
                                            td { class: "font-mono", "save" }
                                            td { "NFC" }
                                            td { class: "text-gray-400 text-xs", "Saving text for display — preserves typography" }
                                        }
                                        tr {
                                            td { class: "font-mono", "vectorise" }
                                            td { "NFKC" }
                                            td { class: "text-gray-400 text-xs", "Converting to vectors for semantic search — strips compatibility variants" }
                                        }
                                        tr {
                                            td { class: "font-mono", "keyword search" }
                                            td { "NFKC + punct" }
                                            td { class: "text-gray-400 text-xs", "Building the keyword search index — also normalises punctuation" }
                                        }
                                    }
                                }
                            }
                            p { "The Δ% column shows how much the cleaning step shrinks the text. A positive Δ means some characters were collapsed (e.g. the ligature \"ﬁ\" → \"fi\"). Near-zero Δ is normal for clean documents." }
                            p { "The file table below shows the last 50 uploaded files so you can spot documents with unusual encoding." }
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
                            p { "When you switch models, the token counter automatically reloads with the new model's vocabulary. Chunks indexed under the old model keep their original token counts. The mismatch warning tells you when this has happened — token counts shown may not match the active model's tokenization." }
                            p { "To fix a mismatch, re-index the affected corpus from the Config → Corpus page. Only that corpus needs re-indexing — documents in other corpora are unaffected." }
                            p { class: "font-semibold text-gray-200 mt-3", "Why a fallback banner appears" }
                            p { "When exact (GGUF) counting cannot be set up, AG falls back to heuristic counting and shows a banner with the reason. The reasons:" }
                            p { class: "ml-2",
                                span { class: "text-blue-400 font-medium", "Cloud backend " }
                                "— The active LLM runs on a remote API (Anthropic, OpenAI, etc.). There is no local GGUF to read, so heuristic is the intended mode. Not an error."
                            }
                            p { class: "ml-2",
                                span { class: "text-yellow-400 font-medium", "No model configured " }
                                "— The backend is set to Ollama or llama.cpp but no model name is filled in. Set a model in the hardware config to enable exact counting."
                            }
                            p { class: "ml-2",
                                span { class: "text-yellow-400 font-medium", "GGUF path not found " }
                                "— A model is configured but its GGUF blob could not be located on disk (Ollama manifest missing, MODEL_PATH env not set, file moved). The detail line shows what was searched."
                            }
                            p { class: "ml-2",
                                span { class: "text-yellow-400 font-medium", "GGUF load failed " }
                                "— The file was found but shimmytok could not parse it (corrupt download, unsupported quantization, vocab format AG does not yet recognize). The detail line shows the parser error."
                            }
                            p { class: "text-xs text-gray-400 mt-2",
                                "Operational consequence: while in fallback, all token counts shown in the UI and used for chunk-size decisions are approximations within roughly ±20%. Retrieval still works, but chunk boundaries may drift from what the model actually sees."
                            }
                        }
                    }
                }
            }

            // How to read & evaluate the sample modal
            if show_golden_read_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_golden_read_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-[640px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4 shrink-0",
                            h2 { class: "text-lg font-semibold text-gray-100", "How to read and evaluate the sample" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_golden_read_info.set(false),
                                "✕"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-3 overflow-y-auto",
                            p { class: "font-semibold text-gray-200", "What you're looking at" }
                            p { "Each row is one chunk from your corpus — the exact text the embedder sees after NFKC normalisation. These are a random cross-section of everything you've ingested, selected to be statistically representative." }

                            p { class: "font-semibold text-gray-200 mt-1", "Column guide" }
                            p { class: "ml-2",
                                span { class: "font-semibold text-gray-100", "Pos " }
                                "— position in the corpus stream when this chunk was offered to the reservoir. Lower numbers came from early ingests; higher numbers from recent ones."
                            }
                            p { class: "ml-2",
                                span { class: "font-semibold text-gray-100", "Tokens " }
                                "— how many tokens the active tokenizer at capture time produced for this chunk. This is the baseline you're comparing against."
                            }
                            p { class: "ml-2",
                                span { class: "font-semibold text-green-400", "✓ " }
                                span { class: "text-gray-300", "in IDs — the full token ID sequence was saved (requires a GGUF-loaded tokenizer). The diff engine can do a deep sequence comparison." }
                            }
                            p { class: "ml-2",
                                span { class: "text-gray-500", "— " }
                                span { class: "text-gray-300", "in IDs — only the count was saved (heuristic fallback). The diff engine will skip per-token ID comparison for this entry." }
                            }
                            p { class: "ml-2",
                                span { class: "font-semibold text-gray-100", "Captured " }
                                "— when this chunk was last written to the sample (inserts and reservoir replacements both update this)."
                            }
                            p { class: "ml-2",
                                span { class: "font-semibold text-gray-100", "Preview " }
                                "— first 80 chars of the chunk text. Click the row to expand the full text and token IDs."
                            }

                            p { class: "font-semibold text-gray-200 mt-1", "What to look for" }
                            p { "Scan a few rows and ask: does this look like a representative cross-section of my documents? You want a mix of topics, lengths, and formats — not 80 rows from the same PDF page." }
                            p { "Check the token counts. Chunks with very high counts (> 500) may be oversized and could cause embedding quality issues. Very low counts (< 20) may be headings or noise." }
                            p { "If most IDs show " span { class: "text-gray-500 font-mono", "—" } " (no IDs stored), you were running on a heuristic tokenizer at capture time. Re-capture after loading a GGUF model to get exact ID sequences — the diff engine is much more informative with them." }

                            p { class: "font-semibold text-gray-200 mt-1", "What to do next" }
                            p {
                                "Once you have a representative, GGUF-backed sample, scroll down to "
                                span { class: "font-semibold text-gray-100", "Compare & Swap Tokenizer" }
                                " and point it at a candidate model file. It will re-tokenize every chunk in this table and report the delta — how many tokens changed, by how much, and which chunks diverged most."
                            }
                            p { "Use that diff to decide whether the new tokenizer is safe to swap in. A mean delta near zero and mostly ✓ IDs means the two tokenizers are practically equivalent. Large deltas or many ≠ rows mean the swap will meaningfully change how your corpus is split — worth understanding before committing." }

                            button {
                                class: "btn btn-sm w-full mt-2",
                                style: "background-color:#7C2A02;",
                                onclick: move |_| show_golden_read_info.set(false),
                                "Got it"
                            }
                        }
                    }
                }
            }

            // Golden sample panel info modal
            if show_golden_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_golden_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-[640px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4 shrink-0",
                            h2 { class: "text-lg font-semibold text-gray-100", "Golden Corpus Sample" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_golden_info.set(false),
                                "x"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-3 overflow-y-auto",
                            p {
                                "The golden sample is a stable, "
                                span {
                                    class: "text-blue-400 underline cursor-pointer hover:text-blue-300",
                                    onclick: move |_| show_seed_info.set(true),
                                    "seeded"
                                }
                                " (a fixed initial value for the random generator, so the selection is repeatable) random subset of your actual corpus chunks. It serves as the baseline for comparing tokenizers — when you want to evaluate a candidate tokenizer, AG re-tokenizes these exact chunks with it and reports how the output drifts from this baseline."
                            }
                            p { class: "font-semibold text-gray-200", "How chunks are selected" }
                            p { "Reservoir sampling: every chunk produced by an ingest is offered to the reservoir, which keeps the first N (capacity) and then probabilistically replaces older entries as new ones arrive. The result is a uniform random sample over all chunks the system has ever seen, without needing to know the corpus size in advance." }
                            p { "The seed is stored alongside the sample so the selection is reproducible. The seed rotates on explicit re-capture (so a re-capture doesn't deterministically reproduce the prior selection)." }
                            p { class: "font-semibold text-gray-200", "What is stored per chunk" }
                            p { class: "ml-2", "- The chunk's embed-normalized text (NFKC), exactly what the embedder sees" }
                            p { class: "ml-2", "- The baseline token count under the tokenizer active at capture time" }
                            p { class: "ml-2", "- The baseline token IDs (a JSON array of u32), if the tokenizer was exact (GGUF). For heuristic fallback, IDs are omitted and the diff engine will refuse to diff against this entry." }
                            p { class: "ml-2", "- The position in the corpus stream when the chunk was offered" }
                            p { class: "font-semibold text-gray-200", "Sample size" }
                            p { "Default capacity is 100 chunks (overridable via " span { class: "font-mono", "GOLDEN_SAMPLE_SIZE" } "). 100 is enough for the diff engine to produce a stable signal on boundary drift and token-count delta without making each diff run slow." }
                            p { class: "font-semibold text-gray-200", "When the sample fills" }
                            p { "The sample fills opportunistically as you ingest. On a fresh install with no ingests, the sample is empty. On an existing corpus that has been quiet for a while, the sample reflects whatever was ingested up to that point — re-capture won't refill it until you ingest again." }
                            p { class: "text-xs text-gray-400 mt-2",
                                "Operational consequence: a small or empty sample means the diff engine has less data to compare against. The diff is still valid, just noisier. Aim for at least 50 chunks before trusting a tokenizer comparison."
                            }
                        }
                    }
                }
            }

            // Seed explanation modal
            if show_seed_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_seed_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-[620px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4 shrink-0",
                            h2 { class: "text-lg font-semibold text-gray-100", "Why a fixed seed?" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_seed_info.set(false),
                                "x"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-3 overflow-y-auto",
                            p { "Without a seed, every time you restart the server and re-ingest the same documents, the reservoir could pick a completely different 100 chunks as the \"golden sample\" — making comparisons meaningless because you'd be comparing two different baselines." }
                            p { "With a fixed seed, the same chunk at the same position in the corpus always gets the same keep/replace decision. The sample is stable across restarts." }
                            p { class: "font-semibold text-gray-200", "How the seed is determined" }
                            p {
                                "It's a hardcoded constant: "
                                span { class: "font-mono text-gray-100", "DEFAULT_SEED = 0xA60D5A_4_AEu64" }
                                ", written directly into the source. There's no derivation — it's just a fixed hex literal baked at compile time."
                            }
                            p {
                                "That value is inserted into the "
                                span { class: "font-mono text-gray-100", "golden_sample_meta" }
                                " table on first init ("
                                span { class: "font-mono text-gray-100", "INSERT OR IGNORE" }
                                "), so it persists in SQLite across restarts. But the constant itself was chosen once and never changes unless you edit the source or override via a future env var (there's no "
                                span { class: "font-mono text-gray-100", "GOLDEN_SAMPLE_SEED" }
                                " env var — only "
                                span { class: "font-mono text-gray-100", "GOLDEN_SAMPLE_SIZE" }
                                " is configurable)."
                            }
                            p { class: "font-semibold text-gray-200", "The seed's narrow role" }
                            p {
                                "At position n, the per-chunk RNG state is "
                                span { class: "font-mono text-gray-100", "seed.wrapping_add(n).wrapping_add(1)" }
                                ", making the reservoir decision for each chunk position reproducible across restarts without persisting RNG state in the DB."
                            }
                        }
                    }
                }
            }

            // Re-capture button info modal
            if show_recapture_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_recapture_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-[560px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4 shrink-0",
                            h2 { class: "text-lg font-semibold text-gray-100", "Re-capture the Golden Sample" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_recapture_info.set(false),
                                "x"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-3 overflow-y-auto",
                            p { "Re-capture clears the current golden sample and resets the chunks-seen counter. The reservoir starts empty and will repopulate from the next ingest." }
                            p { class: "font-semibold text-gray-200", "When to use it" }
                            p { class: "ml-2", "- After accepting a tokenizer swap (Step 4) — the live tokenizer changed, so the baseline must be re-captured under the new one." }
                            p { class: "ml-2", "- When your corpus has shifted significantly (new domain, new languages) and you want the sample to reflect the current data." }
                            p { class: "ml-2", "- For experimentation: rotating the seed produces a different uniform sample, useful for cross-checking that diff results aren't artifacts of one specific selection." }
                            p { class: "font-semibold text-yellow-300", "Warning" }
                            p { "Re-capture erases the prior baseline. There is no undo. If you re-capture and then realize you wanted to compare against the old baseline, the only way to get it back is to revert the corpus and re-ingest — which is rarely practical. Re-capture only when you're sure." }
                            p { class: "font-semibold text-gray-200", "What does NOT change" }
                            p { class: "ml-2", "- Your indexed corpus (chunks, embeddings, vector index) is untouched." }
                            p { class: "ml-2", "- The active tokenizer is untouched. Re-capture is a baseline reset, not a tokenizer swap." }
                            p { class: "text-xs text-gray-400 mt-2",
                                "If you just want to refresh stale capture metadata without losing the baseline, do nothing — keep the current sample and let it age. The captured-at timestamp is a hint, not a freshness gate."
                            }
                        }
                    }
                }
            }

            // Compare/Swap panel info modal
            if show_compare_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_compare_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-[680px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4 shrink-0",
                            h2 { class: "text-lg font-semibold text-gray-100", "Compare & Swap Tokenizer" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_compare_info.set(false),
                                "x"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-3 overflow-y-auto",
                            p { "This panel evaluates a candidate tokenizer against the golden corpus baseline. The diff is read-only — nothing changes in retrieval or indexing until you click Accept swap." }
                            p { class: "font-semibold text-gray-200", "What the diff measures" }
                            p { class: "ml-2", "- Per chunk: baseline vs candidate token count, token-id sequence equality, and where the sequences diverge (longest common prefix and suffix, with the suffix capped so it can't overlap the prefix)." }
                            p { class: "ml-2", "- Aggregate: how many entries are identical, how many had ID changes, the mean signed delta, the mean absolute delta, the max absolute delta, and the total %-change in token volume across the sample." }
                            p { class: "font-semibold text-gray-200", "How to read the per-entry table" }
                            p { class: "ml-2", "- Sorted by |Δ| descending — biggest disruptions appear first." }
                            p { class: "ml-2", "- " span { class: "text-green-400", "✓" } " in IDs means the full sequence matches; " span { class: "text-yellow-400", "≠" } " means at least one token differs (count may still be unchanged if the candidate split a token differently but produced the same total)." }
                            p { class: "ml-2", "- The Diverges column shows " span { class: "font-mono", "prefix N · mid B↔C · suffix M" } " — N tokens at the start match, then B baseline tokens differ from C candidate tokens, then M tokens at the end match. Click " span { class: "font-mono", "IDs" } " to expand the actual ID sequences side-by-side." }
                            p { class: "font-semibold text-gray-200", "When to trust the result" }
                            p { "The diff is statistically meaningful once the golden sample has at least ~50 chunks. With fewer entries, treat the percentages and means as suggestive rather than authoritative." }
                            p { class: "font-semibold text-gray-200", "What changes after Accept swap" }
                            p { "Only the live in-memory tokenizer changes. Already-indexed chunks were tokenized under the old tokenizer; their stored token counts won't update until you re-index. After swapping, also re-capture the golden sample so its baseline reflects the new tokenizer." }
                            p { class: "text-xs text-gray-400 mt-2",
                                "Operationally: this is the safe way to validate a tokenizer change. Diff first, look at the rows where |Δ| is largest, decide whether the divergence is acceptable, then swap."
                            }
                        }
                    }
                }
            }

            // Picker info modal
            if show_picker_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_picker_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-[600px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4 shrink-0",
                            h2 { class: "text-lg font-semibold text-gray-100", "Picking a Candidate" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_picker_info.set(false),
                                "x"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-3 overflow-y-auto",
                            p { "You can specify a candidate tokenizer two ways:" }
                            p { class: "font-semibold text-gray-200", "GGUF path" }
                            p { "An absolute filesystem path to a " span { class: "font-mono", ".gguf" } " file. The diff engine loads the tokenizer vocabulary embedded in that file. Use this when you have a GGUF you downloaded manually or built yourself." }
                            p { class: "font-mono text-xs text-gray-400 ml-2", "Example: /home/you/llama.cpp/models/qwen2.5-7b-q4_k_m.gguf" }
                            p { class: "font-semibold text-gray-200", "Ollama model" }
                            p { "An Ollama model tag — AG resolves it to the GGUF blob in " span { class: "font-mono", "~/.ollama/models/blobs/" } " by reading the manifest. Convenient when you've already pulled the model with " span { class: "font-mono", "ollama pull" } "." }
                            p { class: "font-mono text-xs text-gray-400 ml-2", "Examples: phi:latest · llama3.2:3b · qwen2.5:7b" }
                            p { class: "font-semibold text-gray-200", "What's compared" }
                            p { "Only the tokenizer's vocab + merge rules — not the model weights. Two GGUFs with the same tokenizer family (e.g. all Llama 3 derivatives) will likely produce identical results; switching tokenizer families (Llama → Qwen, Phi → Mistral) is where you'll see meaningful drift." }
                            p { class: "text-xs text-gray-400 mt-2",
                                "Tip: comparing the currently active tokenizer against itself is a useful sanity check — every entry should be identical with Δ = 0. If it's not, something has gone wrong in capture or in the diff path."
                            }
                        }
                    }
                }
            }

            // Accept-swap info modal
            if show_swap_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_swap_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-[600px] max-h-[92vh] flex flex-col shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4 shrink-0",
                            h2 { class: "text-lg font-semibold text-gray-100", "Accept the Swap" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_swap_info.set(false),
                                "x"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-3 overflow-y-auto",
                            p { "Accept swap loads the candidate GGUF into the live token counter, replacing the currently active tokenizer in memory." }
                            p { class: "font-semibold text-gray-200", "What changes immediately" }
                            p { class: "ml-2", "- Future token counts (search-time, ingest-time, monitoring panels) use the candidate tokenizer." }
                            p { class: "ml-2", "- The fallback banner clears (or updates to reflect the new tokenizer's mode)." }
                            p { class: "font-semibold text-gray-200", "What does NOT change" }
                            p { class: "ml-2", "- Already-indexed chunks: their stored token counts were computed under the previous tokenizer and stay that way until you re-index." }
                            p { class: "ml-2", "- The chunk text itself, the embeddings, and the vector index — all unaffected." }
                            p { class: "ml-2", "- The configured backend (Ollama / llama.cpp). The swap is purely a tokenizer override; AG keeps talking to the same LLM service." }
                            p { class: "font-semibold text-yellow-300", "Persistence" }
                            p { "The swap is in-memory only. On restart, AG resolves the tokenizer from the configured backend's GGUF again. To make the swap permanent, change the backend's model in Settings (which also rolls forward the LLM, not just the tokenizer)." }
                            p { class: "font-semibold text-gray-200", "Recommended follow-up" }
                            p { class: "ml-2", "- Re-capture the golden sample (button in the panel above). The baseline must reflect the new tokenizer or future diffs will measure noise." }
                            p { class: "ml-2", "- If the diff showed large drift on most entries, consider re-indexing — the stored counts are now misleading." }
                            p { class: "text-xs text-gray-400 mt-2",
                                "Use accept-swap as an A/B testing tool: try a candidate, see how the system behaves under it, and either commit (by changing the backend model in Settings) or revert by restarting."
                            }
                        }
                    }
                }
            }
        }
    }
}
