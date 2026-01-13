use crate::{
    api,
    app::Route,
    components::config_nav::{ConfigNav, ConfigTab},
    components::monitor::*,
};
use dioxus::prelude::*;

// CSS class constants for consistent styling (matching hardware page)
const PARAM_LABEL_CLASS: &str = "text-gray-300 font-medium";
const PARAM_NUMBER_INPUT_CLASS: &str =
    "input input-xs input-bordered bg-gray-700 text-gray-200 w-24";
const PARAM_ICON_BUTTON_CLASS: &str =
    "w-6 h-6 min-w-6 min-h-6 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80";
const PARAM_ICON_BUTTON_STYLE: &str = "background-color: #1D6B9A; border: 1px solid #1D6B9A;";

/// A small info icon (circled "i") used as a help button.
#[component]
fn InfoIcon() -> Element {
    rsx! {
        svg {
            class: "w-5 h-5 text-white",
            view_box: "0 0 20 20",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.5",
            circle { cx: "10", cy: "10", r: "9" }
            line { x1: "10", y1: "8", x2: "10", y2: "14" }
            circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
        }
    }
}

/// Renders a modal dialog with a title and multiple paragraphs of help text.
fn info_modal(title: &str, toggle: Signal<bool>, paragraphs: Vec<&str>) -> Element {
    let mut toggle = toggle;
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| toggle.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-[90vw] max-h-[95vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100", "{title}" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| toggle.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-3",
                    for paragraph in paragraphs {
                        p { "{paragraph}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn Config() -> Element {
    // Chunk configuration state
    let mut chunk_target_size = use_signal(|| 256usize);
    let mut chunk_min_size = use_signal(|| 128usize);
    let mut chunk_max_size = use_signal(|| 384usize);
    let mut chunk_overlap = use_signal(|| 32usize);
    let chunk_loading = use_signal(|| false);
    let chunk_error = use_signal(|| Option::<String>::None);

    let mut commit_status = use_signal(|| Option::<String>::None);
    let mut committing = use_signal(|| false);
    let mut last_job_id = use_signal(|| Option::<String>::None);

    // Info modal state
    let show_chunk_info = use_signal(|| false);
    let mut chunk_info_signal = show_chunk_info.clone();
    let show_strategy_info = use_signal(|| false);
    let mut strategy_info_signal = show_strategy_info.clone();

    // Load chunk config on mount
    {
        let mut chunk_target_size = chunk_target_size.clone();
        let mut chunk_min_size = chunk_min_size.clone();
        let mut chunk_max_size = chunk_max_size.clone();
        let mut chunk_overlap = chunk_overlap.clone();
        let mut chunk_loading = chunk_loading.clone();
        let mut chunk_error = chunk_error.clone();
        use_future(move || async move {
            chunk_loading.set(true);
            chunk_error.set(None);
            match api::fetch_chunk_config().await {
                Ok(resp) => {
                    chunk_target_size.set(resp.chunker_config.target_size);
                    chunk_min_size.set(resp.chunker_config.min_size);
                    chunk_max_size.set(resp.chunker_config.max_size);
                    chunk_overlap.set(resp.chunker_config.overlap);
                }
                Err(err) => {
                    chunk_error.set(Some(format!("Failed to load chunk config: {}", err)));
                }
            }
            chunk_loading.set(false);
        });
    }

    let on_commit = {
        let chunk_target_size = chunk_target_size.clone();
        let chunk_min_size = chunk_min_size.clone();
        let chunk_max_size = chunk_max_size.clone();
        let chunk_overlap = chunk_overlap.clone();
        move |_| {
            committing.set(true);
            commit_status.set(Some("Applying settings…".into()));
            let payload = api::ChunkCommitRequest {
                target_size: chunk_target_size(),
                min_size: chunk_min_size(),
                max_size: chunk_max_size(),
                overlap: chunk_overlap(),
                semantic_similarity_threshold: None,
            };
            spawn(async move {
                match api::commit_chunk_config(&payload).await {
                    Ok(resp) => {
                        if resp.reindex_job_id.is_some() {
                            last_job_id.set(resp.reindex_job_id.clone());
                        }
                        commit_status.set(Some(resp.message));
                    }
                    Err(err) => {
                        commit_status.set(Some(format!("Commit failed: {}", err)));
                    }
                }
                committing.set(false);
            });
        }
    };

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::Config {})),
                ],
            }

            ConfigNav { active: ConfigTab::Home }

            Panel { title: Some("Config sections".into()), refresh: None,
                div { class: "text-sm text-gray-300", "Use these tabs to open dedicated views for Sampling, Prompt, Hardware & performance, or Other settings while keeping this overview intact." }
            }

            RowHeader {
                title: "RAG".into(),
                description: Some("RAG subsystem status.".into()),
            }
            Panel { title: Some("RAG".into()), refresh: None,
                    div { class: "flex flex-col gap-4",
                        // Two boards side by side: Chunk Strategy (left) and Set Tokens RAG Chunking (right)
                        div { class: "flex flex-row gap-4 justify-between",
                            // Left board: Chunk Strategy (flex-1 to maximize size)
                            div { class: "flex-1 rounded p-4 bg-gray-800 border border-gray-700 flex flex-col gap-3",
                                div { class: "flex items-center gap-2",
                                    span { class: "text-base text-gray-200 font-semibold", "Chunk Strategy" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| strategy_info_signal.set(true),
                                        title: "Chunk Strategy help",
                                        InfoIcon {}
                                    }
                                }
                                span { class: "text-xs text-gray-400", "Select chunking approach for document processing." }
                            }
                            // Right board: Set Tokens RAG Chunking
                            div { class: "rounded p-4 bg-gray-800 border border-gray-700 flex flex-col gap-3",
                                div { class: "flex items-center gap-2",
                                    span { class: "text-base text-gray-200 font-semibold", "Set Tokens RAG Chunking" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| chunk_info_signal.set(true),
                                        title: "RAG Chunking help",
                                        InfoIcon {}
                                    }
                                }
                                span { class: "text-xs text-gray-400 mb-2", "Configure how documents are split into chunks for RAG retrieval." }
                                if chunk_loading() {
                                    div { class: "text-xs text-gray-400", "Loading chunk config…" }
                                } else if let Some(err) = chunk_error() {
                                    div { class: "text-xs text-red-400", "{err}" }
                                } else {
                                    // Horizontal layout for all input fields
                                    div { class: "flex flex-row flex-wrap gap-4 items-end",
                                        div { class: "flex items-center gap-2 text-xs text-gray-200",
                                            label { class: PARAM_LABEL_CLASS, "CHUNK_TARGET_SIZE" }
                                            input {
                                                r#type: "number",
                                                min: "64",
                                                max: "512",
                                                step: "32",
                                                class: PARAM_NUMBER_INPUT_CLASS,
                                                value: format!("{}", chunk_target_size()),
                                                onchange: move |evt| {
                                                    if let Ok(value) = evt.value().parse::<usize>() {
                                                        chunk_target_size.set(value.clamp(64, 512));
                                                    }
                                                }
                                            }
                                        }
                                        div { class: "flex items-center gap-2 text-xs text-gray-200",
                                            label { class: PARAM_LABEL_CLASS, "CHUNK_MIN_SIZE" }
                                            input {
                                                r#type: "number",
                                                min: "32",
                                                max: "256",
                                                step: "16",
                                                class: PARAM_NUMBER_INPUT_CLASS,
                                                value: format!("{}", chunk_min_size()),
                                                onchange: move |evt| {
                                                    if let Ok(value) = evt.value().parse::<usize>() {
                                                        chunk_min_size.set(value.clamp(32, 256));
                                                    }
                                                }
                                            }
                                        }
                                        div { class: "flex items-center gap-2 text-xs text-gray-200",
                                            label { class: PARAM_LABEL_CLASS, "CHUNK_MAX_SIZE" }
                                            input {
                                                r#type: "number",
                                                min: "128",
                                                max: "512",
                                                step: "32",
                                                class: PARAM_NUMBER_INPUT_CLASS,
                                                value: format!("{}", chunk_max_size()),
                                                onchange: move |evt| {
                                                    if let Ok(value) = evt.value().parse::<usize>() {
                                                        chunk_max_size.set(value.clamp(128, 512));
                                                    }
                                                }
                                            }
                                        }
                                        div { class: "flex items-center gap-2 text-xs text-gray-200",
                                            label { class: PARAM_LABEL_CLASS, "CHUNK_OVERLAP" }
                                            input {
                                                r#type: "number",
                                                min: "0",
                                                max: "128",
                                                step: "8",
                                                class: PARAM_NUMBER_INPUT_CLASS,
                                                value: format!("{}", chunk_overlap()),
                                                onchange: move |evt| {
                                                    if let Ok(value) = evt.value().parse::<usize>() {
                                                        chunk_overlap.set(value.clamp(0, 128));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                div { class: "flex items-center gap-3 pt-2",
                                    button {
                                        class: "btn btn-primary btn-sm",
                                        onclick: on_commit.clone(),
                                        disabled: committing(),
                                        if committing() { "Applying…" } else { "Commit" }
                                    }
                                    if let Some(status) = commit_status() {
                                        span { class: "text-xs text-gray-200", "{status}" }
                                    }
                                }
                                if let Some(job_id) = last_job_id() {
                                    div { class: "text-[0.7rem] text-gray-400",
                                        "Reindex job ID: {job_id} (monitor via /reindex/status/{job_id})"
                                    }
                                }
                            }
                        }
                        // 3 health cards horizontally under the RAG Chunking panel
                        div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                            HealthCard { name: "Chunk-Size Overlapping".into(), status: "Healthy".into(), detail: Some("Ready".into()) }
                            HealthCard { name: "Chunker".into(), status: "Ready".into(), detail: Some("384 tokens".into()) }
                            HealthCard { name: "Documents".into(), status: "--".into(), detail: Some("Uploaded".into()) }
                        }
                    }
                }

            RowHeader {
                title: "Agent".into(),
                description: Some("Agent runtime status.".into()),
            }
            Panel { title: Some("Agent".into()), refresh: None,
                    div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                        HealthCard { name: "Memory".into(), status: "Active".into(), detail: Some("SQLite".into()) }
                        HealthCard { name: "Tools".into(), status: "3".into(), detail: Some("Enabled".into()) }
                        HealthCard { name: "LLM".into(), status: "phi".into(), detail: Some("Local".into()) }
                        HealthCard { name: "Usage".into(), status: "--".into(), detail: Some("Recent".into()) }
                    }
                }

            // Info modal for RAG Chunking
            if show_chunk_info() {
                {info_modal(
                    "RAG Chunking Configuration",
                    show_chunk_info,
                    vec![
                        "CHUNK_TARGET_SIZE: Target number of tokens per chunk. Recommended ~50% of embedding model max (256 for BGE-small-en-v1.5).",
                        "CHUNK_MIN_SIZE: Minimum tokens per chunk. Chunks smaller than this will be merged with adjacent content.",
                        "CHUNK_MAX_SIZE: Maximum tokens per chunk. Stay under 512 for BGE-small-en-v1.5 embeddings.",
                        "CHUNK_OVERLAP: Number of tokens to overlap between consecutive chunks (~12% of target size recommended).",
                    ],
                )}
            }

            // Info modal for Chunk Strategy
            if show_strategy_info() {
                {info_modal(
                    "Chunk Strategy",
                    show_strategy_info,
                    vec![
                        "The system needs to do three things in sequence: figure out what kind of document it's looking at, choose the right chunking approach based on that, then execute the chunking.",
                        "Detection combines multiple signals. MIME type is the most authoritative when present—the source system is explicitly telling you what the content is.",
                        "Why MIME Type Detection is Better?",
                        "Extension-based: ❌ Can be spoofed/wrong (user renames .exe to .txt) | MIME Type: ✅ Inspects actual file content",
                        "Extension-based: ❌ Missing extension = unknown type | MIME Type: ✅ Works without extension",
                        "Extension-based: ❌ Ambiguous (.doc could be old Word or text) | MIME Type: ✅ Identifies actual format",
                        "Extension-based: ❌ Case sensitivity issues | MIME Type: ✅ Content-based, no case issues",
                        "File extension is a weaker signal but useful as fallback. Content sniffing (looking for patterns like markdown headers or HTML tags) fills in when metadata is missing or suspect. You weigh these signals and arrive at a format classification.",
                        "Dispatch maps that format to a chunking strategy. Markdown and HTML get document-aware parsing that respects their structural boundaries. Plain text gets recursive character splitting since there's no structure to exploit. Code gets AST-based chunking. PDFs typically fall back to character splitting because their extracted text rarely preserves meaningful structure.",
                        "Execution applies the selected strategy through a common interface. Every chunker takes content in and produces chunks out, regardless of internal approach. This uniformity means the rest of your pipeline—embedding, storage, retrieval—doesn't care which strategy ran.",
                        "The escape hatch matters: users can override auto-detection when they know better. Detection fails in predictable ways, and you shouldn't trap people when it does.",
                        "For observability, you log both the raw inputs (mime type, extension) and derived conclusions (detected format, chosen strategy) alongside outcome metrics. The gap between what you observed and what you concluded is where detection failures hide. When retrieval quality degrades, you trace back through these logs to find the mismatch.",
                    ],
                )}
            }
        }
    }
}
