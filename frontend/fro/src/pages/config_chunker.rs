//! Dedicated Chunker configuration page — /config/chunker

use crate::pages::hardware::components::{info_modal, InfoIcon};
use crate::pages::hardware::constants::{PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE};
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
const PARAM_NUMBER_INPUT_CLASS: &str =
    "input input-xs input-bordered bg-gray-700 text-gray-200 !w-24";
const PARAM_CHECKBOX_CLASS: &str = "checkbox checkbox-xs onnx-checkbox";
const PARAM_SELECT_CLASS: &str = "select select-xs select-bordered bg-gray-700 text-gray-200 w-36";

#[component]
pub fn ConfigChunker() -> Element {
    // ─── Mode & Size ──────────────────────────────────────────
    let mut mode = use_signal(|| "lightweight".to_string());
    let mut target_size = use_signal(|| 256usize);
    let mut min_size = use_signal(|| 128usize);
    let mut max_size = use_signal(|| 384usize);
    let mut overlap = use_signal(|| 32usize);

    // ─── Semantic ─────────────────────────────────────────────
    let mut semantic_threshold = use_signal(|| 0.75f64);

    // ─── Context Prefix ───────────────────────────────────────
    let mut context_prefix_enabled = use_signal(|| false);
    let mut context_prefix_tokens = use_signal(|| 32usize);

    // ─── Pipeline preset ──────────────────────────────────────
    // "lw_sent" | "sent_sem" | "lw_sent_sem"
    let mut pipeline_preset = use_signal(|| "lw_sent_sem".to_string());

    // ─── Embedding model ──────────────────────────────────────
    // ─── Embedding model (display-only reference) ────────────
    let mut embed_model_label = use_signal(|| "bge-small-en-v1.5".to_string());

    // ─── Save ─────────────────────────────────────────────────
    let mut saving = use_signal(|| false);
    let mut save_message = use_signal(|| Option::<String>::None);

    // ─── Info modal signals ───────────────────────────────────
    let mut show_mode_info = use_signal(|| false);
    let mut show_target_info = use_signal(|| false);
    let mut show_min_info = use_signal(|| false);
    let mut show_max_info = use_signal(|| false);
    let mut show_overlap_info = use_signal(|| false);
    let mut show_semantic_info = use_signal(|| false);
    let mut show_prefix_enabled_info = use_signal(|| false);
    let mut show_prefix_tokens_info = use_signal(|| false);
    let mut show_centroid_info = use_signal(|| false);
    let mut show_pipeline_stages_info = use_signal(|| false);

    // Load config on mount
    use_effect(move || {
        spawn(async move {
            if let Ok(resp) = api::fetch_chunk_config().await {
                let c = resp.chunker_config;
                mode.set(c.mode);
                target_size.set(c.target_size);
                min_size.set(c.min_size);
                max_size.set(c.max_size);
                overlap.set(c.overlap);
                semantic_threshold.set(c.semantic_similarity_threshold as f64);
                context_prefix_enabled.set(c.context_prefix_enabled);
                context_prefix_tokens.set(c.context_prefix_tokens);
                let has_lw  = c.pipeline_stages.split(',').any(|s| s.trim() == "lw");
                let has_sem = c.pipeline_stages.split(',').any(|s| s.trim() == "sem");
                pipeline_preset.set(match (has_lw, has_sem) {
                    (true,  true)  => "lw_sent_sem".to_string(),
                    (false, true)  => "sent_sem".to_string(),
                    _              => "lw_sent".to_string(),
                });
            }
            if let Ok(emb) = api::fetch_embedding_config().await {
                embed_model_label.set(emb.model);
            }
        });
    });

    // Save handler
    let save_config = move |_| {
        spawn(async move {
            saving.set(true);
            save_message.set(None);
            let stages = match pipeline_preset().as_str() {
                "lw_sent"    => "lw,sent".to_string(),
                "sent_sem"   => "sent,sem".to_string(),
                _            => "lw,sent,sem".to_string(),
            };
            let payload = api::ChunkCommitRequest {
                target_size: target_size(),
                min_size: min_size(),
                max_size: max_size(),
                overlap: overlap(),
                semantic_similarity_threshold: Some(semantic_threshold() as f32),
                mode: Some(mode()),
                clean_html: None,
                clean_unicode: None,
                context_prefix_enabled: Some(context_prefix_enabled()),
                context_prefix_tokens: Some(context_prefix_tokens()),
                pipeline_stages: Some(stages),
            };
            match api::commit_chunk_config(&payload).await {
                Ok(resp) => save_message.set(Some(resp.message)),
                Err(e) => save_message.set(Some(format!("Error: {e}"))),
            }
            saving.set(false);
        });
    };

    let is_semantic = mode() == "semantic"
        || (mode() == "pipeline" && pipeline_preset() != "lw_sent");

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::Config {})),
                    BreadcrumbItem::new("Chunker", Some(Route::ConfigChunker {})),
                ],
            }

            ConfigNav { active: ConfigTab::Chunker }

            // ─── HEADER TILE ───────────────────────────────────────────
            Panel { title: None, refresh: None,
                div { class: "flex items-center gap-4 flex-wrap",
                    span { class: "text-base text-gray-100 font-semibold", "Chunker Configuration" }
                    span { class: "text-xs text-cyan-400", "Save applies immediately — reindex starts automatically. No restart needed." }
                    button {
                        class: "btn btn-xs text-white",
                        style: "background-color: #7C2A02; border-color: #7C2A02;",
                        disabled: saving(),
                        onclick: save_config,
                        if saving() { "Saving…" } else { "Save" }
                    }
                    if let Some(msg) = save_message() {
                        span { class: "text-xs text-gray-400", "{msg}" }
                    }
                }
            }

            // ─── MODE, MODEL & SIZE ────────────────────────────────────
            Panel { title: None, refresh: None,
                div { class: "flex flex-wrap gap-8",

                    // ── Embedding Model board ──────────────────────────
                    div { class: "rounded border border-gray-600 p-4 flex-none",
                        span { class: "text-xs text-gray-400 block mb-1", "Embedding Model" }
                        div { class: "flex items-center gap-2",
                            span { class: "text-xs font-mono text-cyan-300 bg-gray-800 px-2 py-0.5 rounded",
                                "{embed_model_label()}"
                            }
                            Link {
                                to: Route::ConfigEmbedding {},
                                class: "text-xs text-gray-500 hover:text-cyan-400",
                                "configure →"
                            }
                        }
                    }

                    // Mode
                    div { class: "rounded border border-gray-600 p-4 flex-1 min-w-52",
                        span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Mode" }
                        div { class: PARAM_COLUMN_CLASS,
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "CHUNKER_MODE" }
                                div { class: "flex items-center gap-2",
                                    select {
                                        class: PARAM_SELECT_CLASS,
                                        value: "{mode()}",
                                        onchange: move |e| mode.set(e.value()),
                                        option { value: "fixed", "Fixed" }
                                        option { value: "lightweight", "Lightweight (recommended)" }
                                        option { value: "semantic", "Semantic" }
                                        option { value: "sentence", "Sentence" }
                                        option { value: "pipeline", "Pipeline" }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_mode_info.set(true),
                                        title: "Chunker mode help",
                                        InfoIcon {}
                                    }
                                }
                            }

                            // Semantic threshold — active in semantic / pipeline modes
                            div { class: PARAM_BLOCK_CLASS,
                                label {
                                    class: if is_semantic { PARAM_LABEL_CLASS } else { "text-gray-600 whitespace-nowrap" },
                                    "SEMANTIC_SIMILARITY_THRESHOLD"
                                }
                                div { class: "flex items-center gap-2",
                                    input {
                                        r#type: "number",
                                        min: "0",
                                        max: "1",
                                        step: "0.05",
                                        class: if is_semantic {
                                            PARAM_NUMBER_INPUT_CLASS
                                        } else {
                                            "input input-xs input-bordered bg-gray-800 text-gray-600 !w-24 cursor-not-allowed"
                                        },
                                        disabled: !is_semantic,
                                        value: "{semantic_threshold()}",
                                        oninput: move |e| {
                                            if let Ok(v) = e.value().parse::<f64>() {
                                                semantic_threshold.set(v.clamp(0.0, 1.0));
                                            }
                                        },
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_semantic_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                                if !is_semantic {
                                    span { class: "text-xs text-gray-600 italic",
                                        "Only active in Semantic and Pipeline modes"
                                    }
                                }
                            }

                            // Pipeline stages — active in pipeline mode only
                            if mode() == "pipeline" {
                                div { class: "mt-3 flex flex-col gap-2",
                                    div { class: "flex items-center gap-2",
                                        label { class: PARAM_LABEL_CLASS, "PIPELINE_STAGES" }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_pipeline_stages_info.set(true),
                                            title: "Pipeline stages help",
                                            InfoIcon {}
                                        }
                                    }
                                    select {
                                        class: "select select-xs select-bordered bg-gray-700 text-gray-200 w-full mt-1",
                                        value: "{pipeline_preset()}",
                                        onchange: move |e| pipeline_preset.set(e.value()),
                                        option {
                                            value: "lw_sent",
                                            "lw → sent — Prose, no embedding cost"
                                        }
                                        option {
                                            value: "sent_sem",
                                            "sent → sem — Narrative / flat prose"
                                        }
                                        option {
                                            value: "lw_sent_sem",
                                            "lw → sent → sem — Best quality, highest cost"
                                        }
                                    }
                                    span { class: "text-xs text-gray-500 mt-1",
                                        match pipeline_preset().as_str() {
                                            "lw_sent"  => "Structural paragraph splits → sentence boundary refinement. No embedding calls.",
                                            "sent_sem" => "Sentence boundaries → semantic topic merging. Good for content without clear headings.",
                                            _          => "Full cascade: structure → sentences → topic coherence. Most embedding calls.",
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Size
                    div { class: "rounded border border-gray-600 p-4 flex-1 min-w-52",
                        span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Token Sizes" }
                        div { class: "flex flex-wrap gap-8",
                            div { class: PARAM_COLUMN_CLASS,

                                // target_size
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "CHUNK_TARGET_SIZE" }
                                    div { class: "flex items-center gap-2",
                                        input {
                                            r#type: "number",
                                            min: "64",
                                            max: "2048",
                                            step: "32",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: "{target_size()}",
                                            oninput: move |e| {
                                                if let Ok(v) = e.value().parse::<usize>() {
                                                    target_size.set(v.max(1));
                                                }
                                            },
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_target_info.set(true),
                                            title: "Target chunk size",
                                            InfoIcon {}
                                        }
                                    }
                                }

                                // min_size
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "CHUNK_MIN_SIZE" }
                                    div { class: "flex items-center gap-2",
                                        input {
                                            r#type: "number",
                                            min: "1",
                                            max: "1024",
                                            step: "16",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: "{min_size()}",
                                            oninput: move |e| {
                                                if let Ok(v) = e.value().parse::<usize>() {
                                                    min_size.set(v.max(1));
                                                }
                                            },
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_min_info.set(true),
                                            title: "Minimum chunk size",
                                            InfoIcon {}
                                        }
                                    }
                                }

                                // max_size
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "CHUNK_MAX_SIZE" }
                                    div { class: "flex items-center gap-2",
                                        input {
                                            r#type: "number",
                                            min: "64",
                                            max: "4096",
                                            step: "64",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: "{max_size()}",
                                            oninput: move |e| {
                                                if let Ok(v) = e.value().parse::<usize>() {
                                                    max_size.set(v.max(1));
                                                }
                                            },
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_max_info.set(true),
                                            title: "Maximum chunk size",
                                            InfoIcon {}
                                        }
                                    }
                                }

                                // overlap
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "CHUNK_OVERLAP" }
                                    div { class: "flex items-center gap-2",
                                        input {
                                            r#type: "number",
                                            min: "0",
                                            max: "512",
                                            step: "8",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: "{overlap()}",
                                            oninput: move |e| {
                                                if let Ok(v) = e.value().parse::<usize>() {
                                                    overlap.set(v);
                                                }
                                            },
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_overlap_info.set(true),
                                            title: "Overlap tokens",
                                            InfoIcon {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ─── CONTEXT PREFIX ───────────────────────────────────────
            Panel { title: None, refresh: None,
                div { class: "flex flex-wrap gap-8",

                    // ─── CONTEXT PREFIX ───────────────────────────────────
                    div { class: "rounded border border-gray-600 p-4 flex-1 min-w-64",
                        span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Context Prefix" }
                        div { class: PARAM_COLUMN_CLASS,

                            // context_prefix_enabled
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center gap-3",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: context_prefix_enabled(),
                                        onchange: move |e| context_prefix_enabled.set(e.checked()),
                                    }
                                    label { class: PARAM_LABEL_CLASS, "CHUNK_CONTEXT_PREFIX" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_prefix_enabled_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                            }

                            // context_prefix_tokens
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "CHUNK_CONTEXT_PREFIX_TOKENS" }
                                div { class: "flex items-center gap-2",
                                    input {
                                        r#type: "number",
                                        min: "8",
                                        max: "128",
                                        step: "8",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: "{context_prefix_tokens()}",
                                        oninput: move |e| {
                                            if let Ok(v) = e.value().parse::<usize>() {
                                                context_prefix_tokens.set(v.max(1));
                                            }
                                        },
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_prefix_tokens_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ─── ENV VAR REFERENCE ────────────────────────────────────
            Panel { title: None, refresh: None,
                div { class: "flex flex-col gap-2",
                    span { class: "text-sm text-gray-300 font-semibold", "Current .env values" }
                    span { class: "text-xs text-gray-500 italic mb-1",
                        "Read-only reference — edit .env and restart to set startup defaults."
                    }
                    div { class: "text-xs font-mono text-gray-400 space-y-1 bg-gray-900 rounded p-3 border border-gray-700",
                        div { class: "text-gray-500", "# Mode & size" }
                        div { "CHUNKER_MODE={mode()}" }
                        div { "CHUNK_TARGET_SIZE={target_size()}" }
                        div { "CHUNK_MIN_SIZE={min_size()}" }
                        div { "CHUNK_MAX_SIZE={max_size()}" }
                        div { "CHUNK_OVERLAP={overlap()}" }
                        div { class: "text-gray-500 mt-1", "# Semantic" }
                        div { "SEMANTIC_SIMILARITY_THRESHOLD={semantic_threshold()}" }
                        div { class: "text-gray-500 mt-1", "# Context prefix" }
                        div { "CHUNK_CONTEXT_PREFIX={context_prefix_enabled()}" }
                        div { "CHUNK_CONTEXT_PREFIX_TOKENS={context_prefix_tokens()}" }
                        if mode() == "pipeline" {
                            div { class: "text-gray-500 mt-1", "# Pipeline" }
                            div {
                                {
                                    let stages = match pipeline_preset().as_str() {
                                        "lw_sent"  => "lw,sent",
                                        "sent_sem" => "sent,sem",
                                        _          => "lw,sent,sem",
                                    };
                                    format!("PIPELINE_STAGES={stages}")
                                }
                            }
                        }
                    }
                }
            }
        }

        // ─── INFO MODALS ──────────────────────────────────────────────
        if show_mode_info() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_mode_info.set(false),
                div {
                    class: "bg-gray-900 border border-gray-700 rounded-lg p-6 w-[90vw] max-w-[90vw] max-h-[95vh] overflow-y-auto shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-xl font-bold text-gray-100", "CHUNKER_MODE" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_mode_info.set(false),
                            "×"
                        }
                    }
                    div { class: "text-sm text-gray-300 leading-relaxed space-y-5",
                        p { "Selects the chunking algorithm applied to every ingested document." }

                        // ── Fixed ──────────────────────────────────────────────────
                        div { class: "rounded border border-gray-700 p-4",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "text-xs font-mono font-semibold text-cyan-300 bg-cyan-900/30 border border-cyan-700 rounded px-2 py-0.5", "Fixed" }
                                span { class: "text-xs text-gray-400", "— one line → one chunk" }
                            }
                            p { class: "text-gray-300 mb-2",
                                "Splits strictly on line boundaries. No merging, no overlap, no size control. Every line becomes exactly one chunk."
                            }
                            div { class: "grid grid-cols-2 gap-3 text-xs mt-2",
                                div {
                                    p { class: "text-green-400 font-semibold mb-1", "Strengths" }
                                    ul { class: "space-y-0.5 text-gray-400",
                                        li { "Fastest — zero computation" }
                                        li { "Perfect for logs, CSVs, tables, code" }
                                    }
                                }
                                div {
                                    p { class: "text-red-400 font-semibold mb-1", "Weaknesses" }
                                    ul { class: "space-y-0.5 text-gray-400",
                                        li { "Terrible for prose" }
                                        li { "No semantic coherence whatsoever" }
                                    }
                                }
                            }
                            p { class: "text-xs text-gray-500 mt-2 italic",
                                "Mental model: treat every line as an atomic record."
                            }
                        }

                        // ── Lightweight ────────────────────────────────────────────
                        div { class: "rounded border border-gray-700 p-4",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "text-xs font-mono font-semibold text-cyan-300 bg-cyan-900/30 border border-cyan-700 rounded px-2 py-0.5", "Lightweight" }
                                span { class: "text-xs text-gray-400", "— paragraph-aware heuristics" }
                                span { class: "text-xs text-green-400 font-semibold ml-auto", "recommended default" }
                            }
                            p { class: "text-gray-300 mb-2",
                                "Splits on double newlines (paragraphs) and heading lines (lines starting with "
                                span { class: "font-mono text-gray-200", "#" }
                                " or ending with "
                                span { class: "font-mono text-gray-200", ":" }
                                "). Then enforces min/target/max chunk sizes."
                            }
                            div { class: "grid grid-cols-2 gap-3 text-xs mt-2",
                                div {
                                    p { class: "text-green-400 font-semibold mb-1", "Strengths" }
                                    ul { class: "space-y-0.5 text-gray-400",
                                        li { "Good default for docs, articles, reports" }
                                        li { "Preserves natural paragraph structure" }
                                        li { "Cheap to compute — no embeddings" }
                                    }
                                }
                                div {
                                    p { class: "text-red-400 font-semibold mb-1", "Weaknesses" }
                                    ul { class: "space-y-0.5 text-gray-400",
                                        li { "Paragraphs can be too long or short" }
                                        li { "No sentence-level awareness" }
                                        li { "No semantic awareness" }
                                    }
                                }
                            }
                            p { class: "text-xs text-gray-500 mt-2 italic",
                                "Mental model: paragraph-aware chunking with simple heuristics."
                            }
                        }

                        // ── Sentence ───────────────────────────────────────────────
                        div { class: "rounded border border-gray-700 p-4",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "text-xs font-mono font-semibold text-cyan-300 bg-cyan-900/30 border border-cyan-700 rounded px-2 py-0.5", "Sentence" }
                                span { class: "text-xs text-gray-400", "— size-controlled with overlap" }
                            }
                            p { class: "text-gray-300 mb-2",
                                "Splits on sentence boundaries (. ! ?), accumulates until target_size, hard-flushes at max_size, then carries overlap sentences into the next chunk."
                            }
                            div { class: "grid grid-cols-2 gap-3 text-xs mt-2",
                                div {
                                    p { class: "text-green-400 font-semibold mb-1", "Strengths" }
                                    ul { class: "space-y-0.5 text-gray-400",
                                        li { "Very coherent for narrative text" }
                                        li { "Sentences are natural embedding units" }
                                        li { "Overlap improves retrieval recall" }
                                    }
                                }
                                div {
                                    p { class: "text-red-400 font-semibold mb-1", "Weaknesses" }
                                    ul { class: "space-y-0.5 text-gray-400",
                                        li { "More expensive than Lightweight" }
                                        li { "Still blind to topic shifts" }
                                        li { "Noisy boundaries in technical docs" }
                                    }
                                }
                            }
                            p { class: "text-xs text-gray-500 mt-2 italic",
                                "Mental model: sentence-first chunking with size control and overlap."
                            }
                        }

                        // ── Semantic ───────────────────────────────────────────────
                        div { class: "rounded border border-gray-700 p-4",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "text-xs font-mono font-semibold text-cyan-300 bg-cyan-900/30 border border-cyan-700 rounded px-2 py-0.5", "Semantic" }
                                span { class: "text-xs text-gray-400", "— topic-shift detection via embeddings" }
                            }
                            p { class: "text-gray-300 mb-2",
                                "Embeds text progressively and detects topic shifts via cosine similarity against the running chunk centroid. Splits when similarity drops below the threshold. Produces variable-length, topic-coherent chunks."
                            }
                            div { class: "grid grid-cols-2 gap-3 text-xs mt-2",
                                div {
                                    p { class: "text-green-400 font-semibold mb-1", "Strengths" }
                                    ul { class: "space-y-0.5 text-gray-400",
                                        li { "Most coherent chunks" }
                                        li { "Best retrieval quality" }
                                        li { "Ideal for long, complex documents" }
                                    }
                                }
                                div {
                                    p { class: "text-red-400 font-semibold mb-1", "Weaknesses" }
                                    ul { class: "space-y-0.5 text-gray-400",
                                        li { "Slowest — one embedding per segment" }
                                        li { "Requires threshold tuning" }
                                        li { "Can produce uneven chunk sizes" }
                                    }
                                }
                            }
                            p { class: "text-xs text-gray-500 mt-2 italic",
                                "Mental model: chunk where the meaning changes, not where the text layout changes."
                            }
                        }

                        // ── Pipeline ───────────────────────────────────────────────
                        div { class: "rounded border border-gray-700 p-4",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "text-xs font-mono font-semibold text-cyan-300 bg-cyan-900/30 border border-cyan-700 rounded px-2 py-0.5", "Pipeline" }
                                span { class: "text-xs text-gray-400", "— multi-stage cascade" }
                            }
                            p { class: "text-gray-300",
                                "Runs two or three stages in sequence (configurable below). Each stage refines the output of the previous one. Highest quality; most embedding calls."
                            }
                        }

                        // ── Summary table ──────────────────────────────────────────
                        h4 { class: "text-sm font-semibold text-green-300 pt-1", "Summary" }
                        div { class: "overflow-x-auto",
                            table { class: "w-full text-xs border-collapse",
                                thead {
                                    tr { class: "border-b border-gray-600",
                                        th { class: "text-left py-1 pr-4 text-gray-400 font-semibold", "Mode" }
                                        th { class: "text-left py-1 pr-4 text-gray-400 font-semibold", "Splits on" }
                                        th { class: "text-left py-1 pr-4 text-gray-400 font-semibold", "Cost" }
                                        th { class: "text-left py-1 text-gray-400 font-semibold", "Best for" }
                                    }
                                }
                                tbody {
                                    tr { class: "border-b border-gray-700/50",
                                        td { class: "py-1 pr-4 text-cyan-300 font-mono", "Fixed" }
                                        td { class: "py-1 pr-4 text-gray-300", "Lines" }
                                        td { class: "py-1 pr-4 text-gray-300", "Lowest" }
                                        td { class: "py-1 text-gray-400", "Logs, CSV, code" }
                                    }
                                    tr { class: "border-b border-gray-700/50",
                                        td { class: "py-1 pr-4 text-cyan-300 font-mono", "Lightweight" }
                                        td { class: "py-1 pr-4 text-gray-300", "Paragraphs / headings" }
                                        td { class: "py-1 pr-4 text-gray-300", "Low" }
                                        td { class: "py-1 text-gray-400", "Docs, articles, reports" }
                                    }
                                    tr { class: "border-b border-gray-700/50",
                                        td { class: "py-1 pr-4 text-cyan-300 font-mono", "Sentence" }
                                        td { class: "py-1 pr-4 text-gray-300", "Sentence boundaries" }
                                        td { class: "py-1 pr-4 text-gray-300", "Medium" }
                                        td { class: "py-1 text-gray-400", "Narrative, essays" }
                                    }
                                    tr { class: "border-b border-gray-700/50",
                                        td { class: "py-1 pr-4 text-cyan-300 font-mono", "Semantic" }
                                        td { class: "py-1 pr-4 text-gray-300", "Topic shifts (embeddings)" }
                                        td { class: "py-1 pr-4 text-gray-300", "High" }
                                        td { class: "py-1 text-gray-400", "Long reports, books" }
                                    }
                                    tr {
                                        td { class: "py-1 pr-4 text-cyan-300 font-mono", "Pipeline" }
                                        td { class: "py-1 pr-4 text-gray-300", "Staged cascade" }
                                        td { class: "py-1 pr-4 text-gray-300", "Highest" }
                                        td { class: "py-1 text-gray-400", "Long mixed content" }
                                    }
                                }
                            }
                        }

                        // ── Decision guide ─────────────────────────────────────────
                        h4 { class: "text-sm font-semibold text-green-300 pt-1", "How to choose" }
                        ul { class: "text-xs space-y-1 text-gray-300",
                            li { span { class: "text-gray-400", "Structured data (logs, CSV, code) → " } span { class: "text-cyan-300 font-mono", "Fixed" } }
                            li { span { class: "text-gray-400", "Prose with headings (docs, articles) → " } span { class: "text-cyan-300 font-mono", "Lightweight" } span { class: "text-green-400 text-xs ml-1", "(recommended)" } }
                            li { span { class: "text-gray-400", "Story-like or explanatory text → " } span { class: "text-cyan-300 font-mono", "Sentence" } }
                            li { span { class: "text-gray-400", "Long, complex, multi-topic documents → " } span { class: "text-cyan-300 font-mono", "Semantic" } }
                            li { span { class: "text-gray-400", "Mixed content needing highest quality → " } span { class: "text-cyan-300 font-mono", "Pipeline" } }
                        }
                    }
                }
            }
        }
        if show_target_info() {
            { info_modal("CHUNK_TARGET_SIZE", show_target_info, vec![
                "Soft target size in tokens. The chunker accumulates segments until it reaches this threshold, then flushes.",
                "For BGE-small-en-v1.5 (max 512 tokens), 256 is a safe default that leaves headroom for the context prefix and overlap.",
                "Raise for denser retrieval hits; lower for more granular recall.",
                "Default: 256 tokens.",
            ]) }
        }
        if show_min_info() {
            { info_modal("CHUNK_MIN_SIZE", show_min_info, vec![
                "Minimum token threshold before a heading or semantic boundary can trigger a flush.",
                "Prevents single-sentence chunks that would create noisy embeddings.",
                "If a segment would flush before this threshold, it is merged with the next segment instead.",
                "Default: 128 tokens.",
            ]) }
        }
        if show_max_info() {
            { info_modal("CHUNK_MAX_SIZE", show_max_info, vec![
                "Hard maximum chunk size in tokens. A flush is forced regardless of boundaries when this is exceeded.",
                "Must be ≤ your embedding model's context window (512 for BGE-small-en-v1.5).",
                "Setting this too high produces chunks that overflow the embedder and get silently truncated.",
                "Default: 384 tokens.",
            ]) }
        }
        if show_overlap_info() {
            { info_modal("CHUNK_OVERLAP", show_overlap_info, vec![
                "Number of tokens (or sentences) carried forward into the next chunk after a flush.",
                "Overlap ensures that context at a chunk boundary is not lost — a fact split across two chunks appears in both.",
                "For Sentence mode, overlap removes sentences from the front of the carry buffer until the budget is consumed.",
                "Recommended: ~12% of target_size. At 256 target, 32 overlap is a good starting point.",
                "Default: 32 tokens.",
            ]) }
        }
        if show_semantic_info() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_semantic_info.set(false),
                div {
                    class: "bg-gray-900 border border-gray-700 rounded-lg p-6 w-[90vw] max-w-[90vw] max-h-[95vh] overflow-y-auto shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-xl font-bold text-gray-100", "SEMANTIC_SIMILARITY_THRESHOLD" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_semantic_info.set(false),
                            "×"
                        }
                    }
                    div { class: "text-sm text-gray-300 leading-relaxed space-y-4",
                        p {
                            "Cosine similarity threshold (0–1) below which a topic shift is detected and a new chunk is started. "
                            "Each new segment's embedding is compared to the running "
                            button {
                                class: "text-cyan-400 underline hover:text-cyan-300 cursor-pointer bg-transparent border-none p-0 font-inherit text-inherit",
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    show_semantic_info.set(false);
                                    show_centroid_info.set(true);
                                },
                                "centroid"
                            }
                            " of the current chunk. Only active in Semantic and Pipeline (sent→sem, lw→sent→sem) modes."
                        }

                        // ── What it controls ──────────────────────────────────────
                        div { class: "rounded border border-gray-700 p-4",
                            p { class: "text-gray-200 font-semibold mb-2", "What the threshold controls" }
                            ul { class: "text-xs space-y-1 text-gray-300",
                                li {
                                    span { class: "text-orange-300 font-semibold", "Higher (0.85–0.95) " }
                                    "→ stricter gate → more chunks, tighter topics"
                                }
                                li {
                                    span { class: "text-blue-300 font-semibold", "Lower (0.60–0.75) " }
                                    "→ looser gate → fewer chunks, broader topics"
                                }
                            }
                        }

                        // ── Starting points ───────────────────────────────────────
                        div { class: "rounded border border-gray-700 p-4",
                            p { class: "text-gray-200 font-semibold mb-2", "Starting points by content type" }
                            table { class: "w-full text-xs border-collapse",
                                thead {
                                    tr { class: "border-b border-gray-600",
                                        th { class: "text-left py-1 pr-4 text-gray-400 font-semibold", "Content" }
                                        th { class: "text-left py-1 text-gray-400 font-semibold", "Range" }
                                    }
                                }
                                tbody {
                                    tr { class: "border-b border-gray-700/50",
                                        td { class: "py-1 pr-4 text-gray-300", "General text" }
                                        td { class: "py-1 text-cyan-300 font-mono", "0.80 – 0.85" }
                                    }
                                    tr { class: "border-b border-gray-700/50",
                                        td { class: "py-1 pr-4 text-gray-300", "Narrative text" }
                                        td { class: "py-1 text-cyan-300 font-mono", "0.75" }
                                    }
                                    tr {
                                        td { class: "py-1 pr-4 text-gray-300", "Technical / tightly structured" }
                                        td { class: "py-1 text-cyan-300 font-mono", "0.90" }
                                    }
                                }
                            }
                        }

                        // ── By embedding model ────────────────────────────────────
                        div { class: "rounded border border-gray-700 p-4",
                            p { class: "text-gray-200 font-semibold mb-2", "By embedding model" }
                            p { class: "text-xs text-gray-400 mb-2",
                                "Different models produce different similarity distributions — smaller models are noisier and need a lower threshold."
                            }
                            table { class: "w-full text-xs border-collapse",
                                thead {
                                    tr { class: "border-b border-gray-600",
                                        th { class: "text-left py-1 pr-4 text-gray-400 font-semibold", "Model" }
                                        th { class: "text-left py-1 text-gray-400 font-semibold", "Range" }
                                    }
                                }
                                tbody {
                                    tr { class: "border-b border-gray-700/50",
                                        td { class: "py-1 pr-4 text-gray-300", "OpenAI text-embedding-3-large" }
                                        td { class: "py-1 text-cyan-300 font-mono", "0.82 – 0.88" }
                                    }
                                    tr { class: "border-b border-gray-700/50",
                                        td { class: "py-1 pr-4 text-gray-300", "BGE-large / E5-large" }
                                        td { class: "py-1 text-cyan-300 font-mono", "0.78 – 0.85" }
                                    }
                                    tr {
                                        td { class: "py-1 pr-4 text-gray-300", "MiniLM / small models" }
                                        td { class: "py-1 text-cyan-300 font-mono", "0.70 – 0.80" }
                                    }
                                }
                            }
                        }

                        // ── By retrieval goal ─────────────────────────────────────
                        div { class: "rounded border border-gray-700 p-4",
                            p { class: "text-gray-200 font-semibold mb-2", "By retrieval goal" }
                            ul { class: "text-xs space-y-1.5 text-gray-300",
                                li {
                                    span { class: "text-gray-100", "Tight topical coherence " }
                                    span { class: "text-gray-400", "(GraphRAG, knowledge graphs, summarisation) → " }
                                    span { class: "text-cyan-300 font-mono", "0.85 – 0.92" }
                                }
                                li {
                                    span { class: "text-gray-100", "Classic RAG / QA over long docs " }
                                    span { class: "text-gray-400", "→ " }
                                    span { class: "text-cyan-300 font-mono", "0.75 – 0.82" }
                                }
                                li {
                                    span { class: "text-gray-100", "Highly narrative text " }
                                    span { class: "text-gray-400", "→ " }
                                    span { class: "text-cyan-300 font-mono", "0.70 – 0.78" }
                                }
                            }
                        }

                        // ── Empirical tuning ──────────────────────────────────────
                        div { class: "rounded border border-gray-700 p-4",
                            p { class: "text-gray-200 font-semibold mb-2", "Empirical tuning (best method)" }
                            p { class: "text-xs text-gray-400 mb-3",
                                "Embedding distributions vary between models and corpora — the only reliable method is to inspect real output."
                            }
                            ol { class: "text-xs space-y-3 text-gray-300 list-decimal list-inside",
                                li {
                                    "Pick one document that is representative of your hardest case — ideally one that is long, covers multiple topics, or mixes narrative and technical content. Short, single-topic documents will not reveal threshold sensitivity."
                                }
                                li {
                                    "Copy a few paragraphs from it (an excerpt — a contiguous block of text copied directly from the document, not a summary) — enough to span two or three natural topic transitions — and paste them into the preview box in "
                                    Link {
                                        to: Route::MonitorChunks {},
                                        class: "text-cyan-400 underline hover:text-cyan-300",
                                        onclick: move |_| show_semantic_info.set(false),
                                        "/monitor/chunks"
                                    }
                                    ". Save the threshold here between runs, then preview at "
                                    span { class: "text-cyan-300 font-mono", "0.70, 0.80, 0.85, 0.90" }
                                    " in turn."
                                }
                                li {
                                    span { class: "text-gray-200", "Read each chunk and apply this test: " }
                                    "\"Could I answer a factual question using only this chunk, without the surrounding ones?\" A good chunk is self-contained. A bad chunk either needs its neighbour to make sense, or contains two unrelated ideas."
                                }
                                li {
                                    span { class: "text-gray-200", "Diagnose by symptom:" }
                                    ul { class: "list-disc list-inside ml-3 mt-1 space-y-1 text-gray-400",
                                        li {
                                            span { class: "text-orange-300", "Chunks mix topics " }
                                            "→ threshold too low. Raise it by 0.05."
                                        }
                                        li {
                                            span { class: "text-orange-300", "Chunks are tiny / single-sentence " }
                                            "→ threshold too high. Lower it by 0.05."
                                        }
                                        li {
                                            span { class: "text-orange-300", "Adjacent chunks repeat the same idea " }
                                            "→ threshold too low. Raise it."
                                        }
                                        li {
                                            span { class: "text-orange-300", "One topic is scattered across 4+ chunks " }
                                            "→ threshold too high. Lower it."
                                        }
                                    }
                                }
                                li {
                                    "Converge by halving the step: if 0.80 mixes topics and 0.85 over-splits, try "
                                    span { class: "text-cyan-300 font-mono", "0.82" }
                                    ". Stop when most chunks pass the self-contained test."
                                }
                            }
                        }

                        p { class: "text-xs text-gray-500", "Default: 0.75." }
                    }
                }
            }
        }
        if show_prefix_enabled_info() {
            { info_modal("CHUNK_CONTEXT_PREFIX", show_prefix_enabled_info, vec![
                "Prepend [Source: filename] to each chunk before embedding.",
                "Implements the Anthropic contextual retrieval technique: including the source filename in each chunk's embedding improves retrieval precision when multiple documents cover similar topics.",
                "Example: '[Source: quarterly_report_2024.pdf] Revenue increased by 12%...'",
                "The prefix token budget is controlled by CHUNK_CONTEXT_PREFIX_TOKENS. Chunks with the prefix are slightly shorter in content to stay within max_size.",
                "Default: false.",
            ]) }
        }
        if show_prefix_tokens_info() {
            { info_modal("CHUNK_CONTEXT_PREFIX_TOKENS", show_prefix_tokens_info, vec![
                "Maximum number of tokens to reserve for the [Source: filename] prefix.",
                "The prefix '[Source: filename]' typically uses 8–16 tokens for short filenames and up to 32 for longer paths.",
                "This value is informational — it does not currently trim the prefix. It is stored in the config for future implementations that cap prefix length.",
                "Default: 32 tokens.",
            ]) }
        }
        if show_centroid_info() {
            { info_modal("Centroid", show_centroid_info, vec![
                "In this context, the centroid is the running average embedding vector of the current chunk — a single vector that represents the semantic center of everything collected in that chunk so far.",
                "",
                "🧠 What centroid means in Semantic Chunking",
                "When chunking with semantic similarity, each new segment is compared to a centroid — not to the previous segment:",
                "• The mean of all embedding vectors in the current chunk",
                "• Updated every time a new segment is added",
                "• A stable representation of the chunk's overall topic",
                "",
                "Mathematically, if your chunk has embeddings e₁, e₂, …, eₙ:",
                "c = (1/n) × (e₁ + e₂ + … + eₙ)",
                "",
                "When a new segment arrives with embedding e_new:",
                "• Compute cosine similarity between e_new and the centroid c",
                "• If similarity < threshold → topic shift detected → start new chunk",
                "• Otherwise → add segment to chunk and update centroid",
                "",
                "🔍 Why use a centroid instead of the last embedding?",
                "The centroid:",
                "• Smooths out noise from individual sentences",
                "• Represents the whole chunk, not just the last sentence",
                "• Makes topic-shift detection more stable",
                "• Prevents accidental splits from local deviations (e.g. a metaphor or one-off example)",
                "This is especially important in long documents where the topic evolves gradually.",
                "",
                "🧩 Example (intuitive)",
                "Chunk so far contains sentences about machine learning:",
                "• \"Neural networks are powerful models.\"",
                "• \"Training requires large datasets.\"",
                "• \"Backpropagation computes gradients.\"",
                "Their embeddings average into a centroid representing machine learning.",
                "",
                "Now a new segment arrives: \"The Eiffel Tower is in Paris.\"",
                "Its embedding is far from the centroid → cosine similarity drops below threshold → new chunk starts.",
                "",
                "🛠️ In GraphRAG / RAG pipelines",
                "Centroid-based semantic chunking is used to:",
                "• Keep chunks topically coherent",
                "• Improve retrieval quality (relevant docs score higher against queries)",
                "• Reduce hallucinations by keeping context tight",
                "It is a simple but powerful trick.",
            ]) }
        }
        if show_pipeline_stages_info() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_pipeline_stages_info.set(false),
                div {
                    class: "bg-gray-900 border border-gray-700 rounded-lg p-6 w-[90vw] max-w-[90vw] max-h-[95vh] overflow-y-auto shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-xl font-bold text-gray-100", "PIPELINE_STAGES" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_pipeline_stages_info.set(false),
                            "×"
                        }
                    }
                    div { class: "text-sm text-gray-300 leading-relaxed space-y-4",
                        p {
                            "Pipeline mode runs two or three chunkers in sequence. Each stage refines the output of the previous one. "
                            "The order is always fixed: Lightweight → Sentence → Semantic."
                        }

                        // lw → sent
                        div { class: "rounded border border-gray-700 p-4",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "text-xs font-mono font-semibold text-cyan-300 bg-cyan-900/30 border border-cyan-700 rounded px-2 py-0.5",
                                    "lw → sent"
                                }
                                span { class: "text-xs text-gray-400", "Prose — no embedding cost" }
                            }
                            p { class: "text-gray-300 mb-2",
                                "Lightweight splits on headings and blank lines to give macro paragraph structure. "
                                "Sentence then refines each paragraph to clean sentence boundaries and carries overlap forward."
                            }
                            p { class: "text-gray-400 text-xs",
                                "No embeddings are generated during chunking — fast and deterministic. "
                                "Best for: blog posts, documentation, reports where headings provide natural structure."
                            }
                        }

                        // sent → sem
                        div { class: "rounded border border-gray-700 p-4",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "text-xs font-mono font-semibold text-cyan-300 bg-cyan-900/30 border border-cyan-700 rounded px-2 py-0.5",
                                    "sent → sem"
                                }
                                span { class: "text-xs text-gray-400", "Narrative / flat prose" }
                            }
                            p { class: "text-gray-300 mb-2",
                                "Sentence handles all boundary splitting first, then Semantic compares each sentence embedding to the running "
                                "chunk centroid and flushes when the topic drifts below the similarity threshold."
                            }
                            p { class: "text-gray-400 text-xs",
                                "Skips Lightweight intentionally — useful for content with no clear structural markers (transcripts, novels, flat HTML). "
                                "Embedding cost is proportional to the number of sentences."
                            }
                        }

                        // lw → sent → sem
                        div { class: "rounded border border-gray-700 p-4",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "text-xs font-mono font-semibold text-cyan-300 bg-cyan-900/30 border border-cyan-700 rounded px-2 py-0.5",
                                    "lw → sent → sem"
                                }
                                span { class: "text-xs text-gray-400", "Best quality — highest cost" }
                            }
                            p { class: "text-gray-300 mb-2",
                                "Full three-stage cascade. Lightweight gives macro structure, Sentence refines to clean boundaries, "
                                "Semantic ensures each chunk is topically coherent by detecting embedding-level topic shifts."
                            }
                            p { class: "text-gray-400 text-xs",
                                "Most embedding calls per document — one per sentence in each Lightweight paragraph. "
                                "Best for: long-form mixed content, web-scraped pages, technical PDFs with shifting topics. "
                                "Pair with Clean HTML and context prefix for highest retrieval quality."
                            }
                        }

                        div { class: "text-xs text-gray-500 pt-2 border-t border-gray-700",
                            "The semantic similarity threshold (SEMANTIC_SIMILARITY_THRESHOLD) controls flush sensitivity in both "
                            "sent → sem and lw → sent → sem. It has no effect in lw → sent."
                        }
                    }
                }
            }
        }
    }
}
