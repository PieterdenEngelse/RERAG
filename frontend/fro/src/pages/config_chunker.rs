//! Dedicated Chunker configuration page — /config/chunker

use crate::pages::hardware::components::{info_modal, InfoIcon};
use crate::pages::hardware::constants::{PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE};
use crate::{
    api,
    app::{ActiveCorpus, Route},
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

    // ─── Corpus selector ──────────────────────────────────────
    let mut active_corpus = use_context::<Signal<ActiveCorpus>>();
    let mut corpora = use_signal(|| Vec::<api::CorpusEntry>::new());
    let mut show_corpus_dropdown = use_signal(|| false);
    let _corpus_res = use_resource(move || async move {
        let _ = active_corpus.read().slug().to_string(); // reactive dep
        if let Ok(list) = api::fetch_corpora().await {
            corpora.set(list);
        }
    });

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
    let mut show_ram_info = use_signal(|| false);
    let mut index_in_ram = use_signal(|| false);
    let mut ram_msg: Signal<Option<String>> = use_signal(|| None);
    let mut index_doc_count: Signal<usize> = use_signal(|| 0);
    let mut index_size_bytes: Signal<u64> = use_signal(|| 0);
    let mut index_size_human: Signal<String> = use_signal(|| "…".to_string());
    let mut memory_label: Signal<String> = use_signal(|| "Est. RAM if active".to_string());
    let mut restart_msg: Signal<Option<String>> = use_signal(|| None);
    let mut show_restart_confirm = use_signal(|| false);

    // Load global config on mount (used as defaults before per-corpus overrides are known).
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
            if let Ok(info) = api::fetch_index_info().await {
                index_in_ram.set(info.index_in_ram);
                index_doc_count.set(info.total_documents);
                index_size_bytes.set(info.index_size_bytes.unwrap_or(0));
                index_size_human.set(info.index_size_human.unwrap_or_else(|| "?".into()));
                memory_label.set(info.memory_label.unwrap_or_else(|| "Est. RAM if active".into()));
            }
        });
    });

    // Re-load whenever the active corpus changes: apply per-corpus overrides on top of globals.
    let _per_corpus_res = use_resource(move || async move {
        let slug = active_corpus.read().slug().to_string();
        // Re-fetch global base first so switching back to "default" resets correctly.
        let (global_mode, global_target, global_min, global_max, global_overlap,
             global_threshold, global_cp_en, global_cp_tok, global_stages) =
            if let Ok(resp) = api::fetch_chunk_config().await {
                let c = resp.chunker_config;
                let has_lw  = c.pipeline_stages.split(',').any(|s| s.trim() == "lw");
                let has_sem = c.pipeline_stages.split(',').any(|s| s.trim() == "sem");
                let preset = match (has_lw, has_sem) {
                    (true,  true)  => "lw_sent_sem".to_string(),
                    (false, true)  => "sent_sem".to_string(),
                    _              => "lw_sent".to_string(),
                };
                (c.mode, c.target_size, c.min_size, c.max_size, c.overlap,
                 c.semantic_similarity_threshold as f64,
                 c.context_prefix_enabled, c.context_prefix_tokens, preset)
            } else {
                return;
            };

        // Apply per-corpus overrides.
        if let Ok(s) = api::fetch_corpus_settings(&slug).await {
            let cs = s.settings;
            mode.set(cs.chunker_mode.unwrap_or(global_mode));
            target_size.set(cs.target_size.unwrap_or(global_target));
            min_size.set(cs.min_size.unwrap_or(global_min));
            max_size.set(cs.max_size.unwrap_or(global_max));
            overlap.set(cs.overlap.unwrap_or(global_overlap));
            semantic_threshold.set(cs.semantic_similarity_threshold.unwrap_or(global_threshold));
            context_prefix_enabled.set(cs.context_prefix_enabled.unwrap_or(global_cp_en));
            context_prefix_tokens.set(cs.context_prefix_tokens.unwrap_or(global_cp_tok));
            if let Some(stages_raw) = cs.pipeline_stages {
                let has_lw  = stages_raw.split(',').any(|s| s.trim() == "lw");
                let has_sem = stages_raw.split(',').any(|s| s.trim() == "sem");
                pipeline_preset.set(match (has_lw, has_sem) {
                    (true,  true)  => "lw_sent_sem".to_string(),
                    (false, true)  => "sent_sem".to_string(),
                    _              => "lw_sent".to_string(),
                });
            } else {
                pipeline_preset.set(global_stages);
            }
        }
    });

    // Save handler — writes per-corpus overrides; triggers automatic reindex.
    let save_config = move |_| {
        spawn(async move {
            saving.set(true);
            save_message.set(None);
            let slug = active_corpus.read().slug().to_string();
            let stages = match pipeline_preset().as_str() {
                "lw_sent"  => "lw,sent".to_string(),
                "sent_sem" => "sent,sem".to_string(),
                _          => "lw,sent,sem".to_string(),
            };
            let settings = api::CorpusSettings {
                chunker_mode: Some(mode()),
                target_size: Some(target_size()),
                min_size: Some(min_size()),
                max_size: Some(max_size()),
                overlap: Some(overlap()),
                semantic_similarity_threshold: Some(semantic_threshold()),
                context_prefix_enabled: Some(context_prefix_enabled()),
                context_prefix_tokens: Some(context_prefix_tokens()),
                pipeline_stages: Some(stages),
                ..Default::default()
            };
            match api::patch_corpus_settings(&slug, &settings).await {
                Ok(()) => save_message.set(Some(format!("Saved for corpus '{slug}'"))),
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

            // ─── CORPUS SELECTOR ───────────────────────────────────────
            Panel { title: None, refresh: None,
                div { class: "flex items-center gap-3 flex-wrap",
                    span { class: "text-xs text-gray-400 whitespace-nowrap", "Corpus" }
                    div { class: "relative",
                        button {
                            class: "flex items-center gap-1 px-2 py-1 rounded text-xs font-mono",
                            style: "background-color: rgba(124,42,2,0.35); border: 1px solid rgba(124,42,2,0.6); color: white;",
                            onclick: move |_| show_corpus_dropdown.set(!show_corpus_dropdown()),
                            "{active_corpus.read().slug()}"
                            span { class: "text-xs opacity-60 ml-1",
                                if show_corpus_dropdown() { "▲" } else { "▼" }
                            }
                        }
                        if show_corpus_dropdown() {
                            div {
                                class: "absolute left-0 mt-1 flex flex-col gap-0.5 z-20 p-1 rounded-lg",
                                style: "background-color: #1f2937; border: 1px solid rgba(255,255,255,0.12); min-width: 8rem;",
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
                    span { class: "text-xs text-gray-400", "· selection shared with home & monitor" }
                }
            }

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
                                class: "text-xs text-gray-400 hover:text-cyan-400",
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
                                    class: if is_semantic { PARAM_LABEL_CLASS } else { "text-gray-500 whitespace-nowrap" },
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
                                    span { class: "text-xs text-gray-400 italic",
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
                                    span { class: "text-xs text-gray-400 mt-1",
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

                    // ─── CONTEXT PREFIX ───────────────────────────────────
                    div { class: "rounded border border-gray-600 p-4 flex-none",
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

            // ─── INDEX IN RAM ─────────────────────────────────────────
            if show_restart_confirm() {
                div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_restart_confirm.set(false),
                    div {
                        class: "bg-gray-900 border border-gray-700 rounded-lg p-6 w-80 shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        h2 { class: "text-base font-bold text-gray-100 mb-2", "Restart app?" }
                        p { class: "text-sm text-gray-300 mb-4",
                            "The app will restart to apply the new index setting. Active requests will be dropped."
                        }
                        div { class: "flex gap-2",
                            button {
                                class: "btn btn-sm flex-1",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;color:white;",
                                onclick: move |_| {
                                    show_restart_confirm.set(false);
                                    spawn(async move {
                                        match api::restart_service().await {
                                            Ok(()) => restart_msg.set(Some("Restarting…".into())),
                                            Err(e) => restart_msg.set(Some(format!("Error: {}", e))),
                                        }
                                    });
                                },
                                "Yes, restart"
                            }
                            button {
                                class: "btn btn-sm flex-1 btn-ghost text-gray-300",
                                onclick: move |_| show_restart_confirm.set(false),
                                "Cancel"
                            }
                        }
                    }
                }
            }
            if show_ram_info() {
                { info_modal("Index in RAM", show_ram_info, vec![
                    "Heap-allocates Tantivy segments (RamDirectory). Lower search latency, higher memory use.",
                    "Re-indexes on every restart — leave SKIP_INITIAL_INDEXING=false.",
                    "Avoid when the index exceeds ~100 MB on disk.",
                ]) }
            }
            Panel { title: None, refresh: None,
                div { class: "flex items-center justify-between gap-4",
                    div {
                        div { class: "flex items-center gap-2 mb-0.5",
                            h3 { class: "text-sm font-medium text-gray-200", "Index in RAM" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_ram_info.set(true),
                                InfoIcon {}
                            }
                            input {
                                r#type: "checkbox",
                                class: "toggle toggle-sm",
                                checked: index_in_ram(),
                                onchange: move |evt: Event<FormData>| {
                                    let enabled = evt.value() == "true";
                                    let sz_bytes = index_size_bytes();
                                    let sz = index_size_human();
                                    spawn(async move {
                                        match api::set_index_in_ram(enabled).await {
                                            Ok(()) => {
                                                index_in_ram.set(enabled);
                                                if enabled && sz_bytes > 100_000_000 {
                                                    ram_msg.set(Some(format!(
                                                        "Saved — index is {} and will be fully heap-allocated. Restart to apply.",
                                                        sz
                                                    )));
                                                } else {
                                                    ram_msg.set(Some("Saved — restart to apply.".into()));
                                                }
                                            }
                                            Err(e) => {
                                                ram_msg.set(Some(format!("Error: {}", e)));
                                            }
                                        }
                                    });
                                },
                            }
                            if index_in_ram() {
                                span { class: "text-xs text-teal-400", "Active" }
                            } else {
                                span { class: "text-xs text-gray-400", "Inactive" }
                            }
                        }
                        div { class: "flex items-center gap-2 text-xs text-gray-400",
                            span { class: "text-gray-300", "{memory_label()}:" }
                            span { "{index_size_human()}" }
                            span { class: "text-gray-600", "·" }
                            span { "{index_doc_count()} chunks" }
                            span { class: "text-gray-600", "·" }
                            button {
                                class: "text-yellow-400 hover:text-yellow-200 underline underline-offset-2 cursor-pointer",
                                onclick: move |_| show_restart_confirm.set(true),
                                "Restart APP to apply"
                            }
                        }
                        if let Some(msg) = ram_msg() {
                            div { class: "text-xs text-teal-400 mt-1", "{msg}" }
                        }
                        if let Some(msg) = restart_msg() {
                            div { class: "text-xs text-yellow-300 mt-1", "{msg}" }
                        }
                    }
                }
            }

            // ─── ENV VAR REFERENCE ────────────────────────────────────
            Panel { title: None, refresh: None,
                div { class: "flex flex-col gap-2",
                    span { class: "text-sm text-gray-300 font-semibold", "Current .env values" }
                    span { class: "text-xs text-gray-400 italic mb-1",
                        "Read-only reference — edit .env and restart to set startup defaults."
                    }
                    div { class: "text-xs font-mono text-gray-400 space-y-1 bg-gray-900 rounded p-3 border border-gray-700",
                        div { class: "text-gray-400", "# Index" }
                        div { "INDEX_IN_RAM={index_in_ram()}" }
                        div { class: "text-gray-400 mt-1", "# Mode & size" }
                        div { "CHUNKER_MODE={mode()}" }
                        div { "CHUNK_TARGET_SIZE={target_size()}" }
                        div { "CHUNK_MIN_SIZE={min_size()}" }
                        div { "CHUNK_MAX_SIZE={max_size()}" }
                        div { "CHUNK_OVERLAP={overlap()}" }
                        div { class: "text-gray-400 mt-1", "# Semantic" }
                        div { "SEMANTIC_SIMILARITY_THRESHOLD={semantic_threshold()}" }
                        div { class: "text-gray-400 mt-1", "# Context prefix" }
                        div { "CHUNK_CONTEXT_PREFIX={context_prefix_enabled()}" }
                        div { "CHUNK_CONTEXT_PREFIX_TOKENS={context_prefix_tokens()}" }
                        if mode() == "pipeline" {
                            div { class: "text-gray-400 mt-1", "# Pipeline" }
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
                            div { class: "grid grid-cols-2 gap-3 text-xs mt-2 mb-3",
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
                            div { class: "overflow-x-auto",
                                table { class: "w-full text-xs border-collapse",
                                    thead {
                                        tr { class: "border-b border-gray-600",
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Corpus type" }
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Properties" }
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Optimal use case" }
                                            th { class: "text-left py-1 text-gray-400 font-semibold", "Failure mode" }
                                        }
                                    }
                                    tbody {
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Log files" }
                                            td { class: "py-1 pr-3 text-gray-400", "Line-based, atomic records" }
                                            td { class: "py-1 pr-3 text-gray-400", "Fast ingestion; perfect for logs" }
                                            td { class: "py-1 text-orange-400", "No semantic coherence" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "CSV files" }
                                            td { class: "py-1 pr-3 text-gray-400", "Structured rows" }
                                            td { class: "py-1 pr-3 text-gray-400", "Each row is a chunk" }
                                            td { class: "py-1 text-orange-400", "Mixed cells if merged" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "TSV / NDJSON" }
                                            td { class: "py-1 pr-3 text-gray-400", "Record-per-line" }
                                            td { class: "py-1 pr-3 text-gray-400", "Ideal for analytics" }
                                            td { class: "py-1 text-orange-400", "No context across lines" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Code (single-line)" }
                                            td { class: "py-1 pr-3 text-gray-400", "Short statements" }
                                            td { class: "py-1 pr-3 text-gray-400", "Precise atomic units" }
                                            td { class: "py-1 text-orange-400", "Breaks multi-line functions" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Configuration files" }
                                            td { class: "py-1 pr-3 text-gray-400", "Key-value lines" }
                                            td { class: "py-1 pr-3 text-gray-400", "Stable retrieval" }
                                            td { class: "py-1 text-orange-400", "Comments mixed with logic" }
                                        }
                                        tr {
                                            td { class: "py-1 pr-3 text-gray-200", "System metrics" }
                                            td { class: "py-1 pr-3 text-gray-400", "One metric per line" }
                                            td { class: "py-1 pr-3 text-gray-400", "Perfect atomicity" }
                                            td { class: "py-1 text-orange-400", "No grouping of related metrics" }
                                        }
                                    }
                                }
                            }
                            p { class: "text-xs text-gray-400 mt-2 italic",
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
                            div { class: "grid grid-cols-2 gap-3 text-xs mt-2 mb-3",
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
                            div { class: "overflow-x-auto",
                                table { class: "w-full text-xs border-collapse",
                                    thead {
                                        tr { class: "border-b border-gray-600",
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Corpus type" }
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Properties" }
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Optimal use case" }
                                            th { class: "text-left py-1 text-gray-400 font-semibold", "Failure mode" }
                                        }
                                    }
                                    tbody {
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Documentation" }
                                            td { class: "py-1 pr-3 text-gray-400", "Paragraphs + headings" }
                                            td { class: "py-1 pr-3 text-gray-400", "Preserves structure" }
                                            td { class: "py-1 text-orange-400", "Paragraphs vary too much" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Articles" }
                                            td { class: "py-1 pr-3 text-gray-400", "Well-formed prose" }
                                            td { class: "py-1 pr-3 text-gray-400", "Good default for docs" }
                                            td { class: "py-1 text-orange-400", "No semantic awareness" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Reports" }
                                            td { class: "py-1 pr-3 text-gray-400", "Headings + sections" }
                                            td { class: "py-1 pr-3 text-gray-400", "Stable chunk sizes" }
                                            td { class: "py-1 text-orange-400", "Long paragraphs overflow" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Blog posts" }
                                            td { class: "py-1 pr-3 text-gray-400", "Paragraph-based" }
                                            td { class: "py-1 pr-3 text-gray-400", "Good readability" }
                                            td { class: "py-1 text-orange-400", "Topic shifts undetected" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Markdown docs" }
                                            td { class: "py-1 pr-3 text-gray-400", "Headings + lists" }
                                            td { class: "py-1 pr-3 text-gray-400", "Respects layout" }
                                            td { class: "py-1 text-orange-400", "No sentence-level control" }
                                        }
                                        tr {
                                            td { class: "py-1 pr-3 text-gray-200", "Technical notes" }
                                            td { class: "py-1 pr-3 text-gray-400", "Short paragraphs" }
                                            td { class: "py-1 pr-3 text-gray-400", "Good balance" }
                                            td { class: "py-1 text-orange-400", "No topic detection" }
                                        }
                                    }
                                }
                            }
                            p { class: "text-xs text-gray-400 mt-2 italic",
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
                            div { class: "grid grid-cols-2 gap-3 text-xs mt-2 mb-3",
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
                            div { class: "overflow-x-auto",
                                table { class: "w-full text-xs border-collapse",
                                    thead {
                                        tr { class: "border-b border-gray-600",
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Corpus type" }
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Properties" }
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Optimal use case" }
                                            th { class: "text-left py-1 text-gray-400 font-semibold", "Failure mode" }
                                        }
                                    }
                                    tbody {
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Narrative text" }
                                            td { class: "py-1 pr-3 text-gray-400", "Long-range meaning" }
                                            td { class: "py-1 pr-3 text-gray-400", "Smooth coherence" }
                                            td { class: "py-1 text-orange-400", "Blind to topic shifts" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Essays" }
                                            td { class: "py-1 pr-3 text-gray-400", "Explanatory flow" }
                                            td { class: "py-1 pr-3 text-gray-400", "Good for reasoning" }
                                            td { class: "py-1 text-orange-400", "Overlaps may be noisy" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Story-like docs" }
                                            td { class: "py-1 pr-3 text-gray-400", "Sequential meaning" }
                                            td { class: "py-1 pr-3 text-gray-400", "High recall" }
                                            td { class: "py-1 text-orange-400", "Expensive vs lightweight" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Explanations" }
                                            td { class: "py-1 pr-3 text-gray-400", "Tutorial-like text" }
                                            td { class: "py-1 pr-3 text-gray-400", "Stable semantic units" }
                                            td { class: "py-1 text-orange-400", "Sentence boundaries imperfect" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Conversational prose" }
                                            td { class: "py-1 pr-3 text-gray-400", "Long sentences" }
                                            td { class: "py-1 pr-3 text-gray-400", "Better than paragraph-based" }
                                            td { class: "py-1 text-orange-400", "Still no topic detection" }
                                        }
                                        tr {
                                            td { class: "py-1 pr-3 text-gray-200", "Knowledge articles" }
                                            td { class: "py-1 pr-3 text-gray-400", "Mid-length prose" }
                                            td { class: "py-1 pr-3 text-gray-400", "Good for QA" }
                                            td { class: "py-1 text-orange-400", "Weak on multi-topic docs" }
                                        }
                                    }
                                }
                            }
                            p { class: "text-xs text-gray-400 mt-2 italic",
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
                            div { class: "grid grid-cols-2 gap-3 text-xs mt-2 mb-3",
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
                            div { class: "overflow-x-auto",
                                table { class: "w-full text-xs border-collapse",
                                    thead {
                                        tr { class: "border-b border-gray-600",
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Corpus type" }
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Properties" }
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Optimal use case" }
                                            th { class: "text-left py-1 text-gray-400 font-semibold", "Failure mode" }
                                        }
                                    }
                                    tbody {
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Long reports" }
                                            td { class: "py-1 pr-3 text-gray-400", "Multiple topics" }
                                            td { class: "py-1 pr-3 text-gray-400", "Topic-coherent chunks" }
                                            td { class: "py-1 text-orange-400", "Slow; uneven sizes" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Books" }
                                            td { class: "py-1 pr-3 text-gray-400", "Complex structure" }
                                            td { class: "py-1 pr-3 text-gray-400", "Best retrieval quality" }
                                            td { class: "py-1 text-orange-400", "Threshold tuning required" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Research notes" }
                                            td { class: "py-1 pr-3 text-gray-400", "Topic clusters" }
                                            td { class: "py-1 pr-3 text-gray-400", "High semantic purity" }
                                            td { class: "py-1 text-orange-400", "Over-splitting possible" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Whitepapers" }
                                            td { class: "py-1 pr-3 text-gray-400", "Long conceptual arcs" }
                                            td { class: "py-1 pr-3 text-gray-400", "Ideal for reasoning" }
                                            td { class: "py-1 text-orange-400", "Embedding cost high" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Multi-topic articles" }
                                            td { class: "py-1 pr-3 text-gray-400", "Shifting themes" }
                                            td { class: "py-1 pr-3 text-gray-400", "Detects topic drift" }
                                            td { class: "py-1 text-orange-400", "Chunk sizes unpredictable" }
                                        }
                                        tr {
                                            td { class: "py-1 pr-3 text-gray-200", "Enterprise docs" }
                                            td { class: "py-1 pr-3 text-gray-400", "Heterogeneous content" }
                                            td { class: "py-1 pr-3 text-gray-400", "Best accuracy" }
                                            td { class: "py-1 text-orange-400", "Slowest mode" }
                                        }
                                    }
                                }
                            }
                            p { class: "text-xs text-gray-400 mt-2 italic",
                                "Mental model: chunk where the meaning changes, not where the text layout changes."
                            }
                        }

                        // ── Pipeline ───────────────────────────────────────────────
                        div { class: "rounded border border-gray-700 p-4",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "text-xs font-mono font-semibold text-cyan-300 bg-cyan-900/30 border border-cyan-700 rounded px-2 py-0.5", "Pipeline" }
                                span { class: "text-xs text-gray-400", "— multi-stage cascade" }
                            }
                            p { class: "text-gray-300 mb-2",
                                "Runs two or three stages in sequence (configurable below). Each stage refines the output of the previous one. Highest quality; most embedding calls."
                            }
                            div { class: "overflow-x-auto",
                                table { class: "w-full text-xs border-collapse",
                                    thead {
                                        tr { class: "border-b border-gray-600",
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Corpus type" }
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Properties" }
                                            th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Optimal use case" }
                                            th { class: "text-left py-1 text-gray-400 font-semibold", "Failure mode" }
                                        }
                                    }
                                    tbody {
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Mixed content" }
                                            td { class: "py-1 pr-3 text-gray-400", "Prose + tables + code" }
                                            td { class: "py-1 pr-3 text-gray-400", "Highest quality" }
                                            td { class: "py-1 text-orange-400", "Most expensive" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Technical manuals" }
                                            td { class: "py-1 pr-3 text-gray-400", "Steps + prose + diagrams" }
                                            td { class: "py-1 pr-3 text-gray-400", "Multi-stage refinement" }
                                            td { class: "py-1 text-orange-400", "Overkill for simple docs" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "API documentation" }
                                            td { class: "py-1 pr-3 text-gray-400", "Code + prose" }
                                            td { class: "py-1 pr-3 text-gray-400", "Perfect separation" }
                                            td { class: "py-1 text-orange-400", "Requires config tuning" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Legal + commentary" }
                                            td { class: "py-1 pr-3 text-gray-400", "Statutes + analysis" }
                                            td { class: "py-1 pr-3 text-gray-400", "Structure + semantics" }
                                            td { class: "py-1 text-orange-400", "Slow indexing" }
                                        }
                                        tr { class: "border-b border-gray-700/50",
                                            td { class: "py-1 pr-3 text-gray-200", "Scientific papers" }
                                            td { class: "py-1 pr-3 text-gray-400", "Sections + figures" }
                                            td { class: "py-1 pr-3 text-gray-400", "Combines structure + meaning" }
                                            td { class: "py-1 text-orange-400", "Complex pipeline" }
                                        }
                                        tr {
                                            td { class: "py-1 pr-3 text-gray-200", "Enterprise knowledge bases" }
                                            td { class: "py-1 pr-3 text-gray-400", "Highly varied formats" }
                                            td { class: "py-1 pr-3 text-gray-400", "Best overall retrieval" }
                                            td { class: "py-1 text-orange-400", "High compute cost" }
                                        }
                                    }
                                }
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
                    div { class: "mt-6 pt-4 border-t border-gray-700",
                        button {
                            class: "btn btn-sm w-full text-white",
                            style: "background-color: #7C2A02; border-color: #7C2A02;",
                            onclick: move |_| show_mode_info.set(false),
                            "Got it"
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

                        p { class: "text-xs text-gray-400", "Default: 0.75." }
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

                        div { class: "text-xs text-gray-400 pt-2 border-t border-gray-700",
                            "The semantic similarity threshold (SEMANTIC_SIMILARITY_THRESHOLD) controls flush sensitivity in both "
                            "sent → sem and lw → sent → sem. It has no effect in lw → sent."
                        }
                    }
                }
            }
        }
    }
}
