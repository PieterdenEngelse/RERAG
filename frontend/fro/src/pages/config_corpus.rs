use crate::{
    api,
    app::Route,
    components::config_nav::{ConfigNav, ConfigTab},
    components::monitor::*,
};
use dioxus::prelude::*;

const BTN_CLASS: &str =
    "w-6 h-6 min-w-6 min-h-6 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80";
const BTN_STYLE: &str = "background-color:#7C2A02;border:1px solid #7C2A02;";
const ICO_CLASS: &str = "w-5 h-5 text-white";

fn info_icon() -> Element {
    rsx! {
        svg {
            class: ICO_CLASS,
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

#[component]
pub fn ConfigCorpus() -> Element {
    let mut corpora = use_signal(Vec::<api::CorpusEntry>::new);
    let mut selected = use_signal(|| "default".to_string());

    // Per-corpus settings (editable)
    let mut top_k_str = use_signal(String::new);
    let mut chunker_mode = use_signal(String::new);
    let mut metric = use_signal(String::new);
    let mut ef_construction_str = use_signal(String::new);
    let mut ef_search_str = use_signal(String::new);
    let mut pq_str = use_signal(String::new);
    // Native PDF tri-state: "" = inherit global, "true" = on, "false" = off.
    let mut native_pdf_str = use_signal(String::new);
    let mut saving = use_signal(|| false);
    let mut save_msg = use_signal(|| Option::<String>::None);

    // Watched directory — per-corpus override of the file watcher path.
    // Restart-required; saved through a separate endpoint.
    let mut watch_dir_str = use_signal(String::new);
    let mut watch_dir_saving = use_signal(|| false);
    let mut watch_dir_msg = use_signal(|| Option::<String>::None);
    let mut show_watch_dir = use_signal(|| false);

    // Create form state
    let mut new_slug = use_signal(String::new);
    let mut create_error = use_signal(|| None::<String>);
    let mut creating = use_signal(|| false);

    // Info toggles
    let mut show_corpus_info = use_signal(|| false);
    let mut show_slug_info = use_signal(|| false);
    let mut show_settings_info = use_signal(|| false);
    let mut show_top_k = use_signal(|| false);
    let mut show_chunker = use_signal(|| false);
    let mut show_metric = use_signal(|| false);
    let mut show_ef_c = use_signal(|| false);
    let mut show_ef_s = use_signal(|| false);
    let mut show_pq = use_signal(|| false);
    let mut show_native_pdf = use_signal(|| false);

    // Build metadata for drift detection
    let mut build_meta = use_signal(|| Option::<api::CorpusBuildMeta>::None);

    // Global panels
    let mut chunk_cfg = use_signal(|| Option::<api::ChunkerConfigSnapshot>::None);
    let mut embed_cfg = use_signal(|| Option::<api::EmbeddingConfigResponse>::None);
    let mut index_cfg = use_signal(|| Option::<api::IndexInfoResponse>::None);

    use_future(move || async move {
        if let Ok(list) = api::fetch_corpora().await {
            // Seed the watch_dir input with the selected corpus' current value.
            if let Some(c) = list.iter().find(|c| c.slug == selected()) {
                watch_dir_str.set(c.watch_dir.clone().unwrap_or_default());
            }
            corpora.set(list);
        }
        if let Ok(r) = api::fetch_corpus_settings("default").await {
            let s = r.settings;
            top_k_str.set(s.search_top_k.map(|v| v.to_string()).unwrap_or_default());
            chunker_mode.set(s.chunker_mode.unwrap_or_default());
            metric.set(s.distance_metric.unwrap_or_default());
            ef_construction_str.set(
                s.hnsw_ef_construction
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            );
            ef_search_str.set(s.hnsw_ef_search.map(|v| v.to_string()).unwrap_or_default());
            pq_str.set(s.pq_subvectors.map(|v| v.to_string()).unwrap_or_default());
            native_pdf_str.set(
                s.native_pdf_enabled
                    .map(|b| b.to_string())
                    .unwrap_or_default(),
            );
            build_meta.set(Some(r.build_meta));
        }
        if let Ok(r) = api::fetch_chunk_config().await {
            chunk_cfg.set(Some(r.chunker_config));
        }
        if let Ok(r) = api::fetch_embedding_config().await {
            embed_cfg.set(Some(r));
        }
        if let Ok(r) = api::fetch_index_info().await {
            index_cfg.set(Some(r));
        }
    });

    let mut on_corpus_change = move |slug: String| {
        selected.set(slug.clone());
        top_k_str.set(String::new());
        chunker_mode.set(String::new());
        metric.set(String::new());
        ef_construction_str.set(String::new());
        ef_search_str.set(String::new());
        pq_str.set(String::new());
        native_pdf_str.set(String::new());
        save_msg.set(None);
        build_meta.set(None);
        // Seed watch_dir from the already-fetched corpora list (cheap).
        watch_dir_str.set(
            corpora
                .read()
                .iter()
                .find(|c| c.slug == slug)
                .and_then(|c| c.watch_dir.clone())
                .unwrap_or_default(),
        );
        watch_dir_msg.set(None);
        spawn(async move {
            if let Ok(r) = api::fetch_corpus_settings(&slug).await {
                let s = r.settings;
                top_k_str.set(s.search_top_k.map(|v| v.to_string()).unwrap_or_default());
                chunker_mode.set(s.chunker_mode.unwrap_or_default());
                metric.set(s.distance_metric.unwrap_or_default());
                ef_construction_str.set(
                    s.hnsw_ef_construction
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                );
                ef_search_str.set(s.hnsw_ef_search.map(|v| v.to_string()).unwrap_or_default());
                pq_str.set(s.pq_subvectors.map(|v| v.to_string()).unwrap_or_default());
                native_pdf_str.set(
                    s.native_pdf_enabled
                        .map(|b| b.to_string())
                        .unwrap_or_default(),
                );
                build_meta.set(Some(r.build_meta));
            }
        });
    };

    let save = move |_| {
        let slug = selected();
        saving.set(true);
        save_msg.set(None);
        let top_k = top_k_str().parse::<usize>().ok();
        let mode = chunker_mode();
        let met = metric();
        let ef_c = ef_construction_str().parse::<usize>().ok();
        let ef_s = ef_search_str().parse::<usize>().ok();
        let pq = pq_str().parse::<usize>().ok();
        let native_pdf = match native_pdf_str().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        };
        spawn(async move {
            let settings = api::CorpusSettings {
                search_top_k: top_k,
                chunker_mode: if mode.is_empty() { None } else { Some(mode) },
                distance_metric: if met.is_empty() { None } else { Some(met) },
                hnsw_ef_construction: ef_c,
                hnsw_ef_search: ef_s,
                pq_subvectors: pq,
                native_pdf_enabled: native_pdf,
                ..Default::default()
            };
            match api::patch_corpus_settings(&slug, &settings).await {
                Ok(()) => save_msg.set(Some("Saved".into())),
                Err(e) => save_msg.set(Some(format!("Error: {}", e))),
            }
            saving.set(false);
        });
    };

    // Drift detection — compute before rsx!
    let drift = build_meta().and_then(|meta| {
        meta.built_at.as_ref()?;
        let current_metric = if metric().is_empty() {
            "cosine".to_string()
        } else {
            metric()
        };
        let current_ef_c = ef_construction_str().parse::<usize>().unwrap_or(100);
        let current_ef_s = ef_search_str().parse::<usize>().unwrap_or(100);
        let current_pq = pq_str().parse::<usize>().unwrap_or(48);
        let needs_reindex = current_metric
            != meta
                .distance_metric
                .clone()
                .unwrap_or_else(|| "cosine".to_string())
            || current_ef_c != meta.hnsw_ef_construction.unwrap_or(100)
            || current_pq != meta.pq_subvectors.unwrap_or(48);
        let ef_s_changed = current_ef_s != meta.hnsw_ef_search.unwrap_or(100);
        if needs_reindex || ef_s_changed {
            Some((needs_reindex, ef_s_changed))
        } else {
            None
        }
    });

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::ConfigRuntime {})),
                    BreadcrumbItem::new("Corpus", Some(Route::ConfigCorpus {})),
                ],
            }

            ConfigNav { active: ConfigTab::Corpus }

            // Corpus selector + New corpus side by side
            Panel { title: None, refresh: None,
                div { class: "flex flex-col gap-2",

                    // Two columns: labels + controls on same row
                    div { class: "flex items-center gap-4 flex-wrap",
                        // Left: Corpus label + info + selector
                        div { class: "flex flex-col gap-1",
                            div { class: "flex items-center gap-2",
                                label { class: "text-sm text-white shrink-0", "Corpus" }
                                button {
                                    class: BTN_CLASS, style: BTN_STYLE,
                                    onclick: move |_| show_corpus_info.set(!show_corpus_info()),
                                    {info_icon()}
                                }
                            }
                            select {
                                class: "select select-sm select-bordered bg-gray-700 text-gray-200 w-32",
                                value: selected(),
                                onchange: move |evt| on_corpus_change(evt.value()),
                                for corpus in corpora.read().clone() {
                                    option {
                                        value: "{corpus.slug}",
                                        selected: corpus.slug == selected(),
                                        if corpus.doc_count > 0 {
                                            "{corpus.slug} ({corpus.doc_count} docs)"
                                        } else {
                                            "{corpus.slug}"
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "w-px bg-gray-700 self-stretch" }
                        // Right: New Corpus label + info + input + button
                        div { class: "flex flex-col gap-1",
                            div { class: "flex items-center gap-2",
                                label { class: "text-sm text-gray-400 shrink-0", "New Corpus (Slug)" }
                                button {
                                    class: BTN_CLASS, style: BTN_STYLE,
                                    onclick: move |_| show_slug_info.set(!show_slug_info()),
                                    {info_icon()}
                                }
                            }
                            div { class: "flex items-center gap-2",
                                input {
                                    class: "input input-sm input-bordered bg-gray-700 text-gray-200 w-48 font-mono",
                                    placeholder: "my-corpus",
                                    value: "{new_slug}",
                                    oninput: move |evt| new_slug.set(evt.value().clone()),
                                }
                                button {
                                    class: "btn btn-sm",
                                    style: "background-color:#7C2A02;border-color:#7C2A02;color:white;",
                                    disabled: creating(),
                                    onclick: move |_| {
                                        let slug = new_slug.read().clone();
                                        spawn(async move {
                                            creating.set(true);
                                            create_error.set(None);
                                            match api::create_corpus(&slug, &slug, "").await {
                                                Ok(_) => {
                                                    new_slug.set(String::new());
                                                    if let Ok(list) = api::fetch_corpora().await {
                                                        corpora.set(list);
                                                    }
                                                }
                                                Err(e) => create_error.set(Some(e)),
                                            }
                                            creating.set(false);
                                        });
                                    },
                                    if creating() { "Creating…" } else { "Create" }
                                }
                            }
                            if let Some(e) = create_error() {
                                p { class: "text-red-400 text-xs", "{e}" }
                            }
                        }
                    }

                    // Info panels (full width, below labels)
                    if show_corpus_info() {
                        div { class: "rounded bg-gray-800 border border-gray-600 p-3 text-xs text-gray-300 space-y-2",
                            p { "A corpus is a named, isolated collection of documents. Each corpus has its own Tantivy index, upload directory, and vector store — so documents in one corpus never pollute search results in another." }
                            p { span { class: "text-gray-100 font-semibold", "Active corpus — " } "The active corpus is the one used by the chat window on the home page for retrieval. It is highlighted with an orange border. To switch, create a second corpus and click \"Use\" on the one you want to activate. The selection persists in your browser session." }
                            p { span { class: "text-gray-100 font-semibold", "default — " } "The default corpus always exists and cannot be deleted. It maps to the same Tantivy index and upload dir that existed before corpora were introduced, so existing documents are automatically in it." }
                            p { span { class: "text-gray-100 font-semibold", "Slug rules — " } "Slugs are 1–64 characters, lowercase alphanumeric and hyphens, starting and ending with an alphanumeric character. The slug is permanent — rename only changes the display name." }
                        }
                    }
                    if show_slug_info() {
                        div { class: "rounded bg-gray-800 border border-gray-600 p-3 text-xs text-gray-300 space-y-2",
                            p { span { class: "font-mono text-gray-100", "slug" } " = machine-friendly identifier. Lowercase, URL-safe, filesystem-safe." }
                            p { span { class: "text-gray-100 font-semibold", "Formatting rules — " } "no spaces (use " span { class: "font-mono", "-" } "), no punctuation, lowercase ASCII only." }
                            p { span { class: "text-gray-100 font-semibold", "URL-safe — " } "must survive browsers, routers, servers, and percent-encoding rules without mangling." }
                            p { span { class: "text-gray-100 font-semibold", "Filesystem-safe — " } "Windows forbids " span { class: "font-mono", "<>:\"/\\|?*" } ", macOS normalizes Unicode, Linux is case-sensitive. Lowercase ASCII with hyphens is the only universal subset." }
                            p { class: "text-gray-300", "The slug is permanent — rename only changes the display name." }
                        }
                    }

                }
            }

            // Per-corpus settings
            RowHeader {
                title: "Per-corpus settings".into(),
                leading: rsx! {
                    button {
                        class: BTN_CLASS, style: BTN_STYLE,
                        onclick: move |_| show_settings_info.set(!show_settings_info()),
                        {info_icon()}
                    }
                },
            }
            if show_settings_info() {
                div { class: "rounded bg-gray-800 border border-gray-600 p-3 text-xs text-gray-300 space-y-2 -mt-2 mb-1",
                    p { "Override the global defaults for this corpus only. Each setting falls back to the global value when left blank." }
                    p { class: "text-gray-200 font-semibold mt-1", "Global defaults" }
                    p { span { class: "text-gray-100 font-medium", "Search top-k — " } "10. How many chunks are returned per query and passed to the LLM as context." }
                    p { span { class: "text-gray-100 font-medium", "Chunker mode — " } "set by CHUNKER_MODE env var (fixed / lightweight / semantic). Controls how documents are split at upload and reindex time." }
                    p { span { class: "text-gray-100 font-medium", "Distance metric — " } "cosine. The similarity function used when comparing query embeddings to stored chunk vectors." }
                    p { span { class: "text-gray-100 font-medium", "HNSW ef_construction — " } "100. Build-time graph density. Higher = better recall, slower index build." }
                    p { span { class: "text-gray-100 font-medium", "HNSW ef_search — " } "100. Query-time candidate pool. Higher = better recall, slower queries." }
                    p { span { class: "text-gray-100 font-medium", "PQ subvectors — " } "48. Product quantization compression segments. Higher = better recall, less compression." }
                    p { span { class: "text-gray-100 font-medium", "Native PDF — " } "set by LAYOUT_ML_ENABLED on /config/onnx. Controls whether PDFs in this corpus go through the layout-aware extractor or plain pdftotext. Per-corpus override takes effect on the next upload — no restart, no reindex." }
                }
            }
            Panel { title: None, refresh: None,
                div { class: "flex flex-col gap-3",

                    // ── Watched directory (its own row — restart-required) ─
                    div { class: "flex items-end gap-2 w-full flex-wrap",
                        div { class: "flex flex-col gap-1 flex-1 min-w-[24rem]",
                            div { class: "flex items-center gap-1",
                                label { class: "text-xs text-gray-400 shrink-0", "Watched directory" }
                                button {
                                    class: BTN_CLASS, style: BTN_STYLE,
                                    onclick: move |_| show_watch_dir.set(!show_watch_dir()),
                                    {info_icon()}
                                }
                            }
                            input {
                                r#type: "text",
                                class: "input input-sm input-bordered bg-gray-700 text-gray-200 w-full font-mono",
                                placeholder: "(default — leave blank to use the PathManager-derived folder)",
                                value: watch_dir_str(),
                                oninput: move |evt| watch_dir_str.set(evt.value()),
                            }
                        }
                        button {
                            class: "btn btn-sm",
                            style: "background-color:#7C2A02;border-color:#7C2A02;color:white;",
                            disabled: watch_dir_saving(),
                            onclick: move |_| {
                                let slug = selected();
                                let raw = watch_dir_str();
                                let trimmed = raw.trim().to_string();
                                watch_dir_saving.set(true);
                                watch_dir_msg.set(None);
                                spawn(async move {
                                    let arg = if trimmed.is_empty() { None } else { Some(trimmed.as_str()) };
                                    match api::update_corpus_watch_dir(&slug, arg).await {
                                        Ok(()) => {
                                            watch_dir_msg.set(Some(
                                                "Saved — restart required for the watcher to pick up the new path.".into(),
                                            ));
                                            if let Ok(list) = api::fetch_corpora().await {
                                                corpora.set(list);
                                            }
                                        }
                                        Err(e) => watch_dir_msg.set(Some(format!("Error: {e}"))),
                                    }
                                    watch_dir_saving.set(false);
                                });
                            },
                            if watch_dir_saving() { "Saving…" } else { "Save path" }
                        }
                    }
                    if show_watch_dir() {
                        div { class: "rounded bg-gray-800 border border-gray-600 p-3 text-xs text-gray-300 space-y-1",
                            p { "Absolute path of the directory the file watcher monitors for this corpus. Drop a file into this folder and the app will parse, chunk, embed, and index it automatically." }
                            p { class: "text-gray-400", "Leave blank to use the default: " span { class: "font-mono text-gray-200", "~/.local/share/ag/data/corpora/{selected()}/documents/" } }
                            p { class: "text-gray-200 font-semibold mt-1", "Precedence (default corpus only)" }
                            p { "1. " span { class: "font-mono text-gray-100", "FILE_WATCHER_DIR" } " env/runtime override" }
                            p { "2. This corpus' watched-directory setting" }
                            p { "3. The PathManager-derived default" }
                            p { class: "text-yellow-600 mt-1", "Restart required. The running watcher is not respawned on save — restart ag for the new path to take effect." }
                            p { class: "text-gray-400 mt-1",
                                "Background: "
                                a { href: "/docu/index/file-watcher", class: "text-blue-400 hover:text-blue-300 underline", "File Watcher" }
                                " — how the underlying notify-based watcher works, what events it forwards, and why the debounce window matters."
                            }
                        }
                    }
                    if let Some(msg) = watch_dir_msg() {
                        p {
                            class: if msg.starts_with("Error") { "text-xs text-red-400" } else { "text-xs text-yellow-300" },
                            "{msg}"
                        }
                    }

                    // ── All 6 fields on one row, labels above controls ────
                    div { class: "flex items-end gap-4 w-full flex-wrap",

                        div { class: "flex flex-col gap-1",
                            div { class: "flex items-center gap-1",
                                label { class: "text-xs text-gray-400 shrink-0", "Search top-k" }
                                button { class: BTN_CLASS, style: BTN_STYLE, onclick: move |_| show_top_k.set(!show_top_k()), {info_icon()} }
                            }
                            input {
                                r#type: "number", min: "1", max: "200",
                                class: "input input-sm input-bordered bg-gray-700 text-gray-200 w-20",
                                placeholder: "10",
                                value: top_k_str(),
                                oninput: move |evt| top_k_str.set(evt.value()),
                            }
                        }

                        div { class: "flex flex-col gap-1",
                            div { class: "flex items-center gap-1",
                                label { class: "text-xs text-gray-400 shrink-0", "Chunker" }
                                button { class: BTN_CLASS, style: BTN_STYLE, onclick: move |_| show_chunker.set(!show_chunker()), {info_icon()} }
                            }
                            select {
                                class: "select select-sm select-bordered bg-gray-700 text-gray-200",
                                value: chunker_mode(),
                                onchange: move |evt| chunker_mode.set(evt.value()),
                                option { value: "",
                                    {
                                        let label = chunk_cfg().map(|c| format!("— global ({}) —", c.mode)).unwrap_or_else(|| "— global —".into());
                                        label
                                    }
                                }
                                option { value: "fixed", "fixed" }
                                option { value: "lightweight", "lightweight" }
                                option { value: "semantic", "semantic" }
                            }
                        }

                        div { class: "flex flex-col gap-1 w-fit",
                            div { class: "flex items-center gap-1",
                                label { class: "text-xs text-gray-400 shrink-0", "Distance metric" }
                                button { class: BTN_CLASS, style: BTN_STYLE, onclick: move |_| show_metric.set(!show_metric()), {info_icon()} }
                            }
                            select {
                                class: "select select-sm select-bordered bg-gray-700 text-gray-200",
                                value: metric(),
                                onchange: move |evt| metric.set(evt.value()),
                                option { value: "", "— global (cosine) —" }
                                option { value: "cosine", "cosine" }
                                option { value: "dotproduct", "dot product" }
                                option { value: "euclidean", "euclidean" }
                            }
                        }

                        div { class: "flex flex-col gap-1",
                            div { class: "flex items-center gap-1",
                                label { class: "text-xs text-gray-400 shrink-0", "ef_construction" }
                                button { class: BTN_CLASS, style: BTN_STYLE, onclick: move |_| show_ef_c.set(!show_ef_c()), {info_icon()} }
                            }
                            input {
                                r#type: "number", min: "10", max: "2000",
                                class: "input input-sm input-bordered bg-gray-700 text-gray-200 w-20",
                                placeholder: "100",
                                value: ef_construction_str(),
                                oninput: move |evt| ef_construction_str.set(evt.value()),
                            }
                        }

                        div { class: "flex flex-col gap-1",
                            div { class: "flex items-center gap-1",
                                label { class: "text-xs text-gray-400 shrink-0", "ef_search" }
                                button { class: BTN_CLASS, style: BTN_STYLE, onclick: move |_| show_ef_s.set(!show_ef_s()), {info_icon()} }
                            }
                            input {
                                r#type: "number", min: "10", max: "2000",
                                class: "input input-sm input-bordered bg-gray-700 text-gray-200 w-20",
                                placeholder: "100",
                                value: ef_search_str(),
                                oninput: move |evt| ef_search_str.set(evt.value()),
                            }
                        }

                        div { class: "flex flex-col gap-1",
                            div { class: "flex items-center gap-1",
                                label { class: "text-xs text-gray-400 shrink-0", "PQ subvectors" }
                                button { class: BTN_CLASS, style: BTN_STYLE, onclick: move |_| show_pq.set(!show_pq()), {info_icon()} }
                            }
                            input {
                                r#type: "number", min: "1", max: "512",
                                class: "input input-sm input-bordered bg-gray-700 text-gray-200 w-20",
                                placeholder: "48",
                                value: pq_str(),
                                oninput: move |evt| pq_str.set(evt.value()),
                            }
                        }

                        div { class: "flex flex-col gap-1",
                            div { class: "flex items-center gap-1",
                                label { class: "text-xs text-gray-400 shrink-0", "Native PDF" }
                                button { class: BTN_CLASS, style: BTN_STYLE, onclick: move |_| show_native_pdf.set(!show_native_pdf()), {info_icon()} }
                            }
                            select {
                                class: "select select-sm select-bordered bg-gray-700 text-gray-200",
                                value: native_pdf_str(),
                                onchange: move |evt| native_pdf_str.set(evt.value()),
                                option { value: "", "— global —" }
                                option { value: "true",  "on" }
                                option { value: "false", "off" }
                            }
                        }
                    }

                    // Info panels expand below
                    if show_top_k() {
                        div { class: "rounded bg-gray-800 border border-gray-600 p-3 text-xs text-gray-300 space-y-1",
                            p { "How many chunks the retriever returns per query. The RAG agent ranks and picks from these candidates before sending context to the LLM." }
                            p { "Lower values (3–5) keep token cost down. Higher values (15–30) give the agent more candidates — useful when documents are long or varied." }
                            p { class: "text-gray-300", "Global default: 10. Leave blank to inherit." }
                        }
                    }
                    if show_chunker() {
                        div { class: "rounded bg-gray-800 border border-gray-600 p-3 text-xs text-gray-300 space-y-1",
                            p { "Controls how documents are split into chunks at upload and reindex time." }
                            p { span { class: "text-gray-200 font-medium", "fixed — " } "fixed token count with overlap. Fast, predictable. Best for structured data." }
                            p { span { class: "text-gray-200 font-medium", "lightweight — " } "sentence-aware splits. Balanced speed and coherence." }
                            p { span { class: "text-gray-200 font-medium", "semantic — " } "embedding-similarity boundaries. Highest quality, slowest. Best for narrative prose." }
                            p { class: "text-gray-300", "Takes effect on next upload or reindex. Leave blank to inherit global." }
                        }
                    }
                    if show_metric() {
                        div { class: "rounded bg-gray-800 border border-gray-600 p-3 text-xs text-gray-300 space-y-1",
                            p { "Similarity function used to compare query embeddings against stored chunk embeddings." }
                            p { span { class: "text-gray-200 font-medium", "cosine — " } "angle between vectors, ignoring magnitude. Most common, works with any embedding model." }
                            p { span { class: "text-gray-200 font-medium", "dot product — " } "raw inner product. Faster but sensitive to norms. Best with pre-normalized embeddings." }
                            p { span { class: "text-gray-200 font-medium", "euclidean — " } "straight-line distance. Intuitive but can underperform in high dimensions." }
                            p { class: "text-gray-400 mt-1", "Independent of chunker mode — the metric is a vector index property, not a text-splitting property." }
                            p { class: "text-yellow-700", "Must match the embedding model's training metric. BGE models expect cosine; switching to dot product or euclidean degrades recall." }
                            p { class: "text-yellow-600", "Requires reindex — the HNSW graph is built using the metric." }
                        }
                    }
                    if show_ef_c() {
                        div { class: "rounded bg-gray-800 border border-gray-600 p-3 text-xs text-gray-300 space-y-1",
                            p { span { class: "text-gray-200 font-medium", "ef = exploration factor. " } "Sometimes also called \"entry factor\" or \"expansion factor\" depending on the paper — the meaning is the same. It controls how many candidate nodes the algorithm keeps in its priority queue while exploring the graph." }
                            p { "Size of the dynamic candidate list when inserting each node into the HNSW graph at build time. Higher = denser graph, better recall, slower build." }
                            p { "Typical range: 50–400. Default 100 is a good balance. High-precision corpora may benefit from 200–400." }
                            p { class: "text-yellow-600", "Takes effect only after reindex." }
                        }
                    }
                    if show_ef_s() {
                        div { class: "rounded bg-gray-800 border border-gray-600 p-3 text-xs text-gray-300 space-y-1",
                            p { span { class: "text-gray-200 font-medium", "ef = exploration factor. " } "Sometimes also called \"entry factor\" or \"expansion factor\" depending on the paper — the meaning is the same. It controls how many candidate nodes the algorithm keeps in its priority queue while exploring the graph." }
                            p { "Priority queue size during HNSW graph traversal at query time. Higher = more candidates examined = better recall, slower queries." }
                            p { "Must be ≥ top-k. Typical range: 50–400. A value of 2–4× top-k is a good starting point." }
                            p { class: "text-yellow-600", "Baked into the Hnsw struct at build time — requires reindex to change." }
                        }
                    }
                    if show_pq() {
                        div { class: "rounded bg-gray-800 border border-gray-600 p-3 text-xs text-gray-300 space-y-1",
                            p { "Product Quantization compresses vectors by splitting each into N sub-vectors and quantizing independently. Fewer subvectors = more compression, lower recall. More = better recall, less compression." }
                            p { "Must divide the embedding dimension evenly. For dim 384: valid values include 1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 96, 128, 192, 384. Default 48 ≈ 8× compression." }
                            p { class: "text-yellow-600", "Takes effect only after the PQ index is rebuilt (reindex)." }
                        }
                    }
                    if show_native_pdf() {
                        div { class: "rounded bg-gray-800 border border-gray-600 p-3 text-xs text-gray-300 space-y-1",
                            p { "Per-corpus override for the Native PDF Extraction pipeline (Stage 0: lopdf word bboxes → layout classifier → table detection → DocIR with block-type tags)." }
                            p { span { class: "text-gray-200 font-medium", "on — " } "Use Native PDF for this corpus. Slower but extracts headers, tables, captions, and reading order. Best for papers, manuals, and PDFs with real layout." }
                            p { span { class: "text-gray-200 font-medium", "off — " } "Skip the Native pipeline. PDFs go through plain pdftotext and arrive as one flat text block. Fast and side-effect-free; best for scratch corpora and bulk text dumps." }
                            p { span { class: "text-gray-200 font-medium", "global — " } "Inherit the install-wide default (the " span { class: "font-mono text-gray-100", "LAYOUT_ML_ENABLED" } " setting on /config/onnx)." }
                            p { class: "text-gray-400", "Takes effect on the next upload — no reindex of existing documents is triggered. Existing docs keep whatever extractor produced them; re-upload or reindex to refresh." }
                        }
                    }

                    // Drift warning — shown when saved settings differ from last build
                    if let Some((needs_reindex, ef_s_changed)) = drift {
                        div { class: "rounded bg-yellow-900 border border-yellow-700 px-3 py-2 text-xs text-yellow-200 space-y-1",
                            if needs_reindex {
                                p { "⚠ Saved settings differ from the last index build. Reindex required for distance metric, ef_construction, or PQ subvectors to take effect." }
                            }
                            if ef_s_changed {
                                p { "ℹ ef_search changed — this takes effect immediately on next search, no reindex needed." }
                            }
                        }
                    }

                    // Save
                    div { class: "flex items-center gap-3 pt-1",
                        button {
                            class: "btn btn-sm",
                            style: "background-color:#7C2A02; border-color:#7C2A02; color:white;",
                            disabled: saving(),
                            onclick: save,
                            if saving() { "Saving…" } else { "Save" }
                        }
                        if let Some(msg) = save_msg() {
                            span {
                                class: if msg.starts_with("Error") { "text-xs text-red-400" } else { "text-xs text-green-400" },
                                "{msg}"
                            }
                        }
                    }
                }
            }

            // Global defaults (read-only context)
            RowHeader {
                title: "Global defaults".into(),
                description: Some("Shared settings — shown for reference, not per-corpus.".into()),
            }

            div { class: "grid grid-cols-1 lg:grid-cols-3 gap-4",
                Panel { title: Some("Chunking".into()), refresh: None,
                    if let Some(cfg) = chunk_cfg() {
                        div { class: "grid grid-cols-2 gap-2",
                            HealthCard { name: "Mode".into(), status: cfg.mode.into(), detail: Some("Global".into()) }
                            HealthCard { name: "Target size".into(), status: format!("{} tok", cfg.target_size).into(), detail: Some("Tokens".into()) }
                            HealthCard { name: "Overlap".into(), status: format!("{} tok", cfg.overlap).into(), detail: Some("Tokens".into()) }
                            HealthCard { name: "Sem. thr.".into(), status: format!("{:.2}", cfg.semantic_similarity_threshold).into(), detail: Some("Cosine".into()) }
                        }
                    } else {
                        p { class: "text-sm text-gray-300", "Loading…" }
                    }
                }

                Panel { title: Some("Embedding".into()), refresh: None,
                    if let Some(cfg) = embed_cfg() {
                        div { class: "grid grid-cols-2 gap-2",
                            HealthCard { name: "Model".into(), status: cfg.model.into(), detail: Some("Active".into()) }
                            HealthCard { name: "Dimension".into(), status: format!("{}", cfg.dimension).into(), detail: Some("Output dim".into()) }
                            HealthCard { name: "Provider".into(), status: cfg.provider.into(), detail: Some("Backend".into()) }
                        }
                    } else {
                        p { class: "text-sm text-gray-300", "Loading…" }
                    }
                }

                Panel { title: Some("Index".into()), refresh: None,
                    if let Some(cfg) = index_cfg() {
                        div { class: "grid grid-cols-2 gap-2",
                            HealthCard { name: "Mode".into(), status: cfg.mode.into(), detail: Some("Index type".into()) }
                            HealthCard { name: "Documents".into(), status: format!("{}", cfg.total_documents).into(), detail: Some("Indexed".into()) }
                            HealthCard { name: "Vectors".into(), status: format!("{}", cfg.total_vectors).into(), detail: Some("Stored".into()) }
                        }
                    } else {
                        p { class: "text-sm text-gray-300", "Loading…" }
                    }
                }
            }
        }
    }
}
