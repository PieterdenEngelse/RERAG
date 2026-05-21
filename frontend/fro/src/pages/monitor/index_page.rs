use crate::{
    api,
    app::Route,
    components::monitor::*,
    pages::hardware::components::InfoIcon,
    pages::hardware::constants::{PARAM_ICON_BUTTON_STYLE, QUICK_ACTION_INFO_BUTTON_CLASS},
};
use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use std::rc::Rc;
use wasm_bindgen_futures::JsFuture;
use web_sys::console;

const REINDEX_SYNC_COMMAND: &str = "curl -X POST http://127.0.0.1:3011/reindex";
const REINDEX_ASYNC_COMMAND: &str = "curl -X POST http://127.0.0.1:3011/reindex/async";
const REINDEX_STATUS_COMMAND: &str = "curl http://127.0.0.1:3011/reindex/status/<job_id>";
const JOURNALCTL_COMMAND: &str = "journalctl -u ag.service -n 200 -f";
const TAIL_LOGS_COMMAND: &str = "tail -f logs/ag.log";
const STORAGE_PATHS: [(&str, &str); 4] = [
    ("Tantivy Index", "~/.local/share/ag/index"),
    ("Vectors Store", "~/.local/share/ag/data/vectors.json"),
    ("SQLite Metadata", "~/.local/share/ag/db/metadata.db"),
    ("Documents", "~/ag/documents"),
];

#[derive(Clone)]
struct IndexState {
    sync_running: bool,
    async_running: bool,
    status_message: Option<String>,
    chunking_logging_enabled: Option<bool>,
    chunking_logging_message: Option<String>,
    // Upload progress tracking
    upload_running: bool,
    upload_total_files: usize,
    upload_completed_files: usize,
    upload_failed_files: usize,
    upload_current_file: Option<String>,
    upload_message: Option<String>,
    // LoRA snapshot export state
    lora_loading: bool,
    lora_error: Option<String>,
    lora_status: Option<api::LoraExportStatus>,
    lora_config: Option<api::LoraExportConfig>,
    lora_message: Option<String>,
    lora_triggering: bool,
    lora_saving_config: bool,
    lora_saving_filter: bool,
    lora_config_dirty: bool,
    lora_filter_dirty: bool,
    lora_auto_enabled: bool,
    lora_debounce_input: String,
    lora_filter_input: String,
    // Synthetic Q&A generation state
    synthetic_qa_status: Option<api::SyntheticQaStatus>,
    synthetic_qa_triggering: bool,
    synthetic_qa_questions_per_chunk: u32,
    synthetic_qa_max_chunks: String,
    // Synthetic Q&A examples viewer
    synthetic_qa_examples: Option<api::SyntheticQaExamplesResponse>,
    synthetic_qa_examples_loading: bool,
    synthetic_qa_examples_offset: usize,
}

impl Default for IndexState {
    fn default() -> Self {
        Self {
            sync_running: false,
            async_running: false,
            status_message: None,
            chunking_logging_enabled: None,
            chunking_logging_message: None,
            upload_running: false,
            upload_total_files: 0,
            upload_completed_files: 0,
            upload_failed_files: 0,
            upload_current_file: None,
            upload_message: None,
            lora_loading: true,
            lora_error: None,
            lora_status: None,
            lora_config: None,
            lora_message: None,
            lora_triggering: false,
            lora_saving_config: false,
            lora_saving_filter: false,
            lora_config_dirty: false,
            lora_filter_dirty: false,
            lora_auto_enabled: true,
            lora_debounce_input: String::new(),
            lora_filter_input: String::new(),
            synthetic_qa_status: None,
            synthetic_qa_triggering: false,
            synthetic_qa_questions_per_chunk: 3,
            synthetic_qa_max_chunks: String::new(),
            synthetic_qa_examples: None,
            synthetic_qa_examples_loading: false,
            synthetic_qa_examples_offset: 0,
        }
    }
}

#[derive(Clone)]
struct IndexInfoSnapshot {
    loading: bool,
    error: Option<String>,
    data: Option<api::IndexInfoResponse>,
}

impl Default for IndexInfoSnapshot {
    fn default() -> Self {
        Self {
            loading: true,
            error: None,
            data: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ReindexJobRow {
    job_id: String,
    status: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    vectors_indexed: Option<usize>,
    mappings_indexed: Option<usize>,
    error: Option<String>,
}

impl ReindexJobRow {
    fn from_status(response: api::ReindexStatusResponse) -> Self {
        Self {
            job_id: response.job_id,
            status: response.status,
            started_at: response.started_at,
            completed_at: response.completed_at,
            vectors_indexed: response.vectors_indexed,
            mappings_indexed: response.mappings_indexed,
            error: response.error,
        }
    }

    fn placeholder(async_response: &api::ReindexAsyncResponse) -> Self {
        Self {
            job_id: async_response.job_id.clone(),
            status: async_response.status.clone(),
            started_at: None,
            completed_at: None,
            vectors_indexed: None,
            mappings_indexed: None,
            error: None,
        }
    }

    fn is_terminal(&self) -> bool {
        matches!(self.status.as_str(), "completed" | "failed" | "not_found")
    }
}

#[component]
pub fn MonitorIndex() -> Element {
    let state = use_signal(IndexState::default);
    let index_info = use_signal(IndexInfoSnapshot::default);
    let jobs = use_signal(Vec::<ReindexJobRow>::new);
    let _show_more_actions = use_signal(|| false);
    let chunk_info_open = use_signal(|| false);
    let mut reindex_control_info_open = use_signal(|| false);
    let show_lora_info = use_signal(|| false);
    let mut show_snapshot_info = use_signal(|| false);
    let mut show_chunking_logging_info = use_signal(|| false);
    let mem_info = use_signal(|| Option::<api::MemoryInfo>::None);
    let show_synthetic_qa_examples = use_signal(|| false);
    let mut show_synthetic_qa_info = use_signal(|| false);
    let selected_corpus: Signal<Option<String>> = use_signal(|| None);
    let corpora: Signal<Vec<api::CorpusEntry>> = use_signal(Vec::new);

    {
        let mut corpora = corpora;
        use_future(move || async move {
            if let Ok(list) = api::fetch_corpora().await {
                corpora.set(list);
            }
        });
    }

    {
        let mut state = state;
        let mut index_info = index_info;
        use_future(move || async move {
            loop {
                match api::fetch_index_info().await {
                    Ok(info) => {
                        index_info.set(IndexInfoSnapshot {
                            loading: false,
                            error: None,
                            data: Some(info),
                        });
                    }
                    Err(err) => {
                        let previous = index_info.read().data.clone();
                        index_info.set(IndexInfoSnapshot {
                            loading: false,
                            error: Some(err),
                            data: previous,
                        });
                    }
                }

                match api::get_chunking_logging().await {
                    Ok(resp) => {
                        let mut snapshot = state.write();
                        snapshot.chunking_logging_enabled = Some(resp.logging_enabled);
                        snapshot.chunking_logging_message = Some(format!(
                            "Logging {}",
                            if resp.logging_enabled {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        ));
                    }
                    Err(err) => {
                        let mut snapshot = state.write();
                        snapshot.chunking_logging_enabled = None;
                        snapshot.chunking_logging_message =
                            Some(format!("Failed to load logging status: {}", err));
                    }
                }

                TimeoutFuture::new(10_000).await;
            }
        });
    }

    {
        use_future(move || async move {
            loop {
                refresh_job_statuses(jobs, state, false).await;

                // Poll faster (2s) when there's an active job, slower (5s) when idle
                let has_active_job = {
                    let guard = jobs.read();
                    guard.iter().any(|job| !job.is_terminal())
                };
                let poll_interval = if has_active_job { 2_000 } else { 5_000 };
                TimeoutFuture::new(poll_interval).await;
            }
        });
    }

    {
        let mut state = state;
        use_future(move || async move {
            loop {
                let status_result = api::fetch_export_snapshot_status().await;
                let config_result = api::fetch_export_snapshot_config().await;

                {
                    let mut snapshot = state.write();
                    if snapshot.lora_loading {
                        snapshot.lora_loading = false;
                    }

                    let mut combined_error: Option<String> = None;

                    match status_result {
                        Ok(status) => {
                            snapshot.lora_status = Some(status);
                        }
                        Err(err) => {
                            combined_error = Some(format!("Status: {}", err));
                        }
                    }

                    match config_result {
                        Ok(config) => {
                            snapshot.lora_config = Some(config.clone());
                            if !snapshot.lora_config_dirty && !snapshot.lora_saving_config {
                                snapshot.lora_auto_enabled = config.auto_export_enabled;
                                snapshot.lora_debounce_input =
                                    config.default_debounce_ms.to_string();
                            }
                            if !snapshot.lora_filter_dirty && !snapshot.lora_saving_filter {
                                snapshot.lora_filter_input =
                                    config.export_filter.clone().unwrap_or_default();
                            }
                            if combined_error.is_some() {
                                // keep existing error text
                            }
                        }
                        Err(err) => {
                            combined_error = Some(match combined_error {
                                Some(prev) => format!("{}; Config: {}", prev, err),
                                None => format!("Config: {}", err),
                            });
                        }
                    }

                    snapshot.lora_error = combined_error;
                }

                // Also fetch synthetic QA status
                if let Ok(qa_status) = api::fetch_synthetic_qa_status().await {
                    state.write().synthetic_qa_status = Some(qa_status);
                }

                // Fetch memory usage for the auto-export indicator
                if let Ok(mem) = api::fetch_memory().await {
                    mem_info.clone().set(Some(mem));
                }

                TimeoutFuture::new(5_000).await;
            }
        });
    }

    let trigger_lora_export = {
        Rc::new(move |_| {
            let mut state = state;
            spawn(async move {
                {
                    let mut snapshot = state.write();
                    snapshot.lora_triggering = true;
                    snapshot.lora_message = Some("Starting LoRA export…".into());
                }

                match api::trigger_export_snapshot().await {
                    Ok(()) => {
                        let mut snapshot = state.write();
                        snapshot.lora_message =
                            Some("LoRA export job started. Check status below.".into());
                    }
                    Err(err) => {
                        let mut snapshot = state.write();
                        snapshot.lora_message = Some(format!("LoRA export failed: {}", err));
                    }
                }

                state.write().lora_triggering = false;
            });
        })
    };

    let save_lora_config = {
        Rc::new(move |_| {
            let mut state = state;
            spawn(async move {
                let (auto_enabled, debounce_str) = {
                    let snapshot = state.read();
                    (
                        snapshot.lora_auto_enabled,
                        snapshot.lora_debounce_input.clone(),
                    )
                };

                let debounce_trimmed = debounce_str.trim();
                let debounce = match debounce_trimmed.parse::<u64>() {
                    Ok(value) => value,
                    Err(_) => {
                        state.write().lora_message =
                            Some("Debounce must be a non-negative number of milliseconds".into());
                        return;
                    }
                };

                {
                    let mut snapshot = state.write();
                    snapshot.lora_saving_config = true;
                    snapshot.lora_message = Some("Saving auto-export settings…".into());
                }

                match api::save_export_snapshot_config(auto_enabled, debounce).await {
                    Ok(config) => {
                        let mut snapshot = state.write();
                        snapshot.lora_config = Some(config.clone());
                        snapshot.lora_auto_enabled = config.auto_export_enabled;
                        snapshot.lora_debounce_input = config.default_debounce_ms.to_string();
                        snapshot.lora_config_dirty = false;
                        snapshot.lora_message = Some("Auto-export settings saved".into());
                    }
                    Err(err) => {
                        state.write().lora_message =
                            Some(format!("Failed to save auto-export settings: {}", err));
                    }
                }

                state.write().lora_saving_config = false;
            });
        })
    };

    let save_lora_filter = {
        Rc::new(move |_| {
            let mut state = state;
            spawn(async move {
                let filter_value = {
                    let snapshot = state.read();
                    snapshot.lora_filter_input.trim().to_string()
                };
                let payload = if filter_value.is_empty() {
                    None
                } else {
                    Some(filter_value.clone())
                };

                {
                    let mut snapshot = state.write();
                    snapshot.lora_saving_filter = true;
                    snapshot.lora_message = Some("Saving filter override…".into());
                }

                match api::save_export_snapshot_filter(payload).await {
                    Ok(config) => {
                        let mut snapshot = state.write();
                        snapshot.lora_config = Some(config.clone());
                        snapshot.lora_filter_input =
                            config.export_filter.clone().unwrap_or_default();
                        snapshot.lora_filter_dirty = false;
                        snapshot.lora_message = Some("Filter override saved".into());
                    }
                    Err(err) => {
                        state.write().lora_message =
                            Some(format!("Failed to save filter override: {}", err));
                    }
                }

                state.write().lora_saving_filter = false;
            });
        })
    };

    let trigger_synthetic_qa = {
        Rc::new(move |_| {
            let mut state = state;
            spawn(async move {
                let (questions_per_chunk, max_chunks) = {
                    let snapshot = state.read();
                    let max = snapshot.synthetic_qa_max_chunks.parse::<usize>().ok();
                    (Some(snapshot.synthetic_qa_questions_per_chunk), max)
                };

                {
                    let mut snapshot = state.write();
                    snapshot.synthetic_qa_triggering = true;
                    snapshot.lora_message = Some("Starting synthetic Q&A generation…".into());
                }

                match api::trigger_synthetic_qa(questions_per_chunk, max_chunks).await {
                    Ok(()) => {
                        let mut snapshot = state.write();
                        snapshot.lora_message =
                            Some("Synthetic Q&A generation started. This may take a while.".into());
                    }
                    Err(err) => {
                        let mut snapshot = state.write();
                        snapshot.lora_message = Some(format!("Synthetic Q&A failed: {}", err));
                    }
                }

                state.write().synthetic_qa_triggering = false;
            });
        })
    };

    let load_synthetic_qa_examples = {
        Rc::new(move |offset: usize| {
            let mut state = state;
            spawn(async move {
                state.write().synthetic_qa_examples_loading = true;
                state.write().synthetic_qa_examples_offset = offset;

                match api::fetch_synthetic_qa_examples(Some(10), Some(offset)).await {
                    Ok(response) => {
                        state.write().synthetic_qa_examples = Some(response);
                    }
                    Err(_err) => {
                        // Failed to load examples, leave as None
                    }
                }

                state.write().synthetic_qa_examples_loading = false;
            });
        })
    };

    let trigger_sync_reindex = {
        Rc::new(move |_| {
            let mut state = state;
            let selected_corpus = selected_corpus;
            spawn(async move {
                let slug = selected_corpus.read().clone();
                {
                    let mut snapshot = state.write();
                    snapshot.sync_running = true;
                    snapshot.status_message = Some(if let Some(ref s) = slug {
                        format!("Reindexing corpus '{}'…", s)
                    } else {
                        "Triggering sync reindex…".into()
                    });
                }

                let result = if let Some(ref slug) = slug {
                    api::reindex_corpus(slug).await
                } else {
                    api::reindex().await
                };

                match result {
                    Ok(_) => {
                        let mut snapshot = state.write();
                        snapshot.status_message = Some(if let Some(ref s) = slug {
                            format!("Corpus '{}' reindexed", s)
                        } else {
                            "Sync reindex request accepted".into()
                        });
                    }
                    Err(err) => {
                        let mut snapshot = state.write();
                        snapshot.status_message = Some(format!("Reindex failed: {}", err));
                    }
                }

                state.write().sync_running = false;
            });
        })
    };

    let trigger_async_reindex = {
        Rc::new(move |_| {
            let mut state = state;
            let mut jobs = jobs;
            let selected_corpus = selected_corpus;
            spawn(async move {
                let slug = selected_corpus.read().clone();

                if let Some(ref slug) = slug {
                    {
                        let mut snapshot = state.write();
                        snapshot.async_running = true;
                        snapshot.status_message = Some(format!("Reindexing corpus '{}'…", slug));
                    }
                    match api::reindex_corpus(slug).await {
                        Ok(_) => {
                            state.write().status_message =
                                Some(format!("Corpus '{}' reindexed", slug));
                        }
                        Err(err) => {
                            state.write().status_message =
                                Some(format!("Corpus reindex failed: {}", err));
                        }
                    }
                    state.write().async_running = false;
                } else {
                    {
                        let mut snapshot = state.write();
                        snapshot.async_running = true;
                        snapshot.status_message = Some("Submitting async reindex…".into());
                    }

                    match api::reindex_async().await {
                        Ok(resp) => {
                            {
                                let mut rows = jobs.write();
                                rows.retain(|row| row.job_id != resp.job_id);
                                rows.insert(0, ReindexJobRow::placeholder(&resp));
                            }

                            state.write().status_message =
                                Some(format!("Async job {} accepted", resp.job_id));

                            if let Err(err) = refresh_single_job(resp.job_id, jobs, state).await {
                                state.write().status_message =
                                    Some(format!("Failed to fetch async status: {}", err));
                            }
                        }
                        Err(err) => {
                            let mut snapshot = state.write();
                            snapshot.status_message =
                                Some(format!("Async reindex failed: {}", err));
                        }
                    }

                    state.write().async_running = false;
                }
            });
        })
    };

    let snapshot = state.read().clone();
    let info_snapshot = index_info.read().clone();
    let job_rows = jobs.read().clone();
    let latest_job = job_rows.first().cloned();

    let mem_bar = mem_info().map(|mem| {
        let pct = mem.usage_percent.min(100.0);
        let bar_color = if pct >= 85.0 {
            "background-color:#ef4444;"
        } else if pct >= 70.0 {
            "background-color:#eab308;"
        } else {
            "background-color:#22c55e;"
        };
        let used_gb = mem.used_memory_bytes as f64 / 1_073_741_824.0;
        (pct, bar_color, used_gb, mem.total_memory_gb)
    });

    let lora_status = snapshot.lora_status.clone();
    let lora_running = lora_status.as_ref().map(|s| s.running).unwrap_or(false);
    let lora_last_error = lora_status
        .as_ref()
        .and_then(|s| s.last_error.clone())
        .filter(|value| !value.trim().is_empty());

    let job_table_rows: Vec<Vec<String>> = job_rows
        .iter()
        .map(|row| {
            vec![
                row.job_id.clone(),
                pretty_status(&row.status),
                format_timestamp(row.started_at.as_ref()),
                format_timestamp(row.completed_at.as_ref()),
                row.vectors_indexed
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                row.mappings_indexed
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "—".into()),
                row.error.clone().unwrap_or_else(|| "—".into()),
            ]
        })
        .collect();

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Index", None),
                ],
            }

            NavTabs { active: Route::MonitorIndex {} }

            // Corpus selector — scopes upload and reindex to a named corpus
            if !corpora.read().is_empty() {
                div { class: "flex items-center gap-3 px-1",
                    span { class: "text-xs text-gray-400 shrink-0", "Corpus" }
                    select {
                        class: "select select-sm select-bordered bg-gray-700 text-gray-200",
                        onchange: {
                            let mut selected_corpus = selected_corpus;
                            move |evt: Event<FormData>| {
                                let val = evt.value();
                                if val == "__default__" {
                                    selected_corpus.set(None);
                                } else {
                                    selected_corpus.set(Some(val));
                                }
                            }
                        },
                        option { value: "__default__", "Default (global)" }
                        for corpus in corpora.read().iter() {
                            {
                                let slug = corpus.slug.clone();
                                let name = corpus.name.clone();
                                let count = corpus.doc_count;
                                rsx! {
                                    option { value: "{slug}", "{name} ({count} docs)" }
                                }
                            }
                        }
                    }
                    if let Some(slug) = selected_corpus.read().as_ref() {
                        span { class: "text-xs text-teal-400", "Active: {slug}" }
                    }
                }
            }

            RowHeader {
                title: "Index Statistics".into(),
                description: Some("Live snapshot from /index/info".into()),
            }


            Panel { title: Some("Current Snapshot".into()), refresh: Some("10s".into()),
                div { class: "flex items-center gap-2 mb-2 mx-4",
                    h3 { class: "text-sm font-semibold text-gray-200", "Index state" }
                    button {
                        class: QUICK_ACTION_INFO_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        onclick: move |_| show_snapshot_info.set(true),
                        title: "What does this snapshot show?",
                        InfoIcon {}
                    }
                }
                div { class: "relative rounded border border-slate-700 bg-slate-900/40 p-4 mx-4",
                    if info_snapshot.loading {
                        div { class: "text-sm text-gray-400", "Loading index info…" }
                    } else if let Some(err) = info_snapshot.error.clone() {
                        div { class: "text-sm text-red-400", "Failed to load index info: {err}" }
                    } else if let Some(info) = info_snapshot.data.clone() {
                        div { class: "space-y-4",
                            div { class: "relative grid grid-cols-1 md:grid-cols-4 gap-4",
                                div { class: "relative rounded p-4 bg-gray-800",
                                    div { class: "flex items-center justify-between gap-4",
                                        div { class: "flex items-center gap-3",
                                            div {
                                                div { class: "text-xs text-gray-400", "Document Chunks" }
                                                div { class: "text-2xl font-bold text-gray-100", "{info.total_documents}" }
                                            }
                                            if let Some(msg) = snapshot.chunking_logging_message.clone() {
                                                div { class: "text-[11px] text-slate-300 whitespace-nowrap", "{msg}" }
                                            }
                                        }
                                        div { class: "flex items-center gap-2",
                                            if snapshot.chunking_logging_enabled.is_some() {
                                                button {
                                                    class: "text-[11px] px-3 py-1 rounded border border-slate-500 text-slate-200 hover:bg-slate-600/20 disabled:opacity-40",
                                                    onclick: {
                                                        move |_| {
                                                            let mut state = state;
                                                            spawn(async move {
                                                                let current = state.read().chunking_logging_enabled;
                                                                if let Some(current) = current {
                                                                    state.write().chunking_logging_message = Some("Updating logging…".into());
                                                                    match api::set_chunking_logging(!current).await {
                                                                        Ok(resp) => {
                                                                            let mut snapshot = state.write();
                                                                            snapshot.chunking_logging_enabled = Some(resp.logging_enabled);
                                                                            snapshot.chunking_logging_message = Some(format!(
                                                                                "Logging {}",
                                                                                if resp.logging_enabled { "enabled" } else { "disabled" }
                                                                            ));
                                                                        }
                                                                        Err(err) => {
                                                                            state.write().chunking_logging_message = Some(format!(
                                                                                "Failed to update logging: {}",
                                                                                err
                                                                            ));
                                                                        }
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    },
                                                    if snapshot.chunking_logging_enabled.unwrap_or(true) { "Disable logging" } else { "Enable logging" }
                                                }
                                            }
                                            // Info button for chunking logging
                                            button {
                                                class: QUICK_ACTION_INFO_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_chunking_logging_info.set(true),
                                                title: "What is chunking snapshot logging?",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                            }

                            if chunk_info_open() {
                                div {
                                    class: "fixed inset-0 z-40 bg-black/60 backdrop-blur-sm",
                                    onclick: {
                                        let mut chunk_info_open = chunk_info_open;
                                        move |_| chunk_info_open.set(false)
                                    }
                                }
                                div {
                                    class: "fixed z-50 top-24 left-1/2 -translate-x-1/2 w-[95%] max-w-2xl rounded-lg border border-slate-700 bg-slate-950/95 p-5 text-[11px] text-slate-100 shadow-2xl space-y-4",
                                    onclick: move |evt| evt.stop_propagation(),
                                    div { class: "flex items-start justify-between gap-4",
                                        div {
                                            div { class: "text-[12px] font-semibold", "Chunking pipeline" }
                                            div { class: "text-[10px] text-slate-400", "From src/memory/chunker.rs" }
                                        }
                                        button {
                                            class: "text-slate-400 hover:text-red-400 text-xl leading-none",
                                            onclick: {
                                                let mut chunk_info_open = chunk_info_open;
                                                move |_| chunk_info_open.set(false)
                                            },
                                            "×"
                                        }
                                    }
                                    div { class: "text-[11px] text-slate-300",
                                        "Documents are chunked by the semantic chunker in "
                                        code { "src/memory/chunker.rs" }
                                        ". The pipeline is:"
                                    }
                                    ol { class: "list-decimal ml-4 space-y-3 text-slate-200",
                                        li {
                                            span { class: "font-semibold", "Token-friendly parsing" }
                                            span { class: "text-slate-300", " – The SemanticChunker first splits a document into “semantic units” (paragraphs, headings, etc.)." }
                                        }
                                        li {
                                            span { class: "font-semibold", "Chunk assembly" }
                                            span { class: "text-slate-300", " – It groups those units while respecting ChunkerConfig:" }
                                            ul { class: "list-disc ml-5 space-y-1 mt-1 text-slate-300",
                                                li {
                                                    span { class: "font-semibold", "max_size" }
                                                    span { " (default 512 tokens) caps each chunk’s length." }
                                                }
                                                li {
                                                    span { class: "font-semibold", "overlap" }
                                                    span { " (default 48 tokens) keeps adjacent chunks sharing context." }
                                                }
                                                li { "If a unit would overflow the chunk, it starts a new chunk with the configured overlap." }
                                            }
                                        }
                                        li {
                                            span { class: "font-semibold", "Metadata" }
                                            span { class: "text-slate-300", " – Each chunk carries ChunkMetadata (document ID, chunk index, start/end offsets, source type)." }
                                        }
                                        li {
                                            span { class: "font-semibold", "Embedding & index" }
                                            span { class: "text-slate-300", " – During reindex, index.rs calls chunk_text/SemanticChunker, embeds each chunk, and writes vectors + text into Tantivy and the vector store." }
                                        }
                                    }
                                    p { class: "text-slate-300", "So chunks are contiguous slices (~512 tokens) with a small overlap, ready for embedding and retrieval." }
                                }
                            }
                        }
                    } else {
                        div { class: "text-sm text-gray-400", "No index info available" }
                    }
                }
            }

            if reindex_control_info_open() {
                InfoModal {
                    title: "Reindex Control",
                    content: "This board lets you manage the full Tantivy reindex pipeline.\n\nButtons:\n• Now – Run a synchronous reindex, blocking until the job finishes. Useful when you want to watch progress in this page or CLI.\n• Background – Submit an async job so the server keeps working while you monitor the queue below.\n• Upload – Feed additional documents; each upload automatically reuses the status/progress UI in this panel.\n\nTips:\n• Use 'Now' during maintenance windows; it locks the writer until completion.\n• Use 'Background' during normal operations so the system stays responsive.\n• Refresh cadence is 5s; you can also hit /monitor/index via curl for JSON stats.\n• The Async Jobs table below stores history so you can correlate job IDs with logs.\n\nSee the Runbook section for ready-to-copy curl commands when you need to trigger jobs from the CLI.",
                    on_close: move || reindex_control_info_open.set(false),
                }
            }

            RowHeader {
                title: "LoRA Snapshot Export".into(),
                description: Some("Auto-export dataset builder and filter overrides".into()),
            }

            Panel {
                div { class: "flex items-center justify-between mb-3",
                    div { class: "flex items-center gap-3",
                        h3 { class: "text-sm font-semibold text-gray-200", "LoRA Export Controls" }
                        button {
                            class: QUICK_ACTION_INFO_BUTTON_CLASS,
                            style: PARAM_ICON_BUTTON_STYLE,
                            onclick: {
                                let mut show_lora_info = show_lora_info;
                                move |_| show_lora_info.set(true)
                            },
                            InfoIcon {}
                        }
                    }
                    span { class: "text-xs text-white", "5s" }
                }
                if snapshot.lora_loading {
                    div { class: "text-sm text-gray-400", "Loading LoRA export status…" }
                } else {
                    div { class: "space-y-4",
                        if let Some(err) = snapshot.lora_error.clone() {
                            div { class: "text-xs text-red-400", "{err}" }
                        }
                        if let Some(msg) = snapshot.lora_message.clone() {
                            div { class: "text-xs text-indigo-300", "{msg}" }
                        }
                        div { class: "flex flex-wrap items-center gap-4",
                            div {
                                div { class: "text-xs text-gray-400", "Current Status" }
                                if let Some(status) = lora_status.clone() {
                                    div {
                                        class: if status.running {
                                            "text-lg font-bold text-amber-300"
                                        } else if lora_last_error.is_some() {
                                            "text-lg font-bold text-red-400"
                                        } else {
                                            "text-lg font-bold text-emerald-300"
                                        },
                                        if status.running {
                                            "Running"
                                        } else if lora_last_error.is_some() {
                                            "Error"
                                        } else {
                                            "Idle"
                                        }
                                    }
                                } else {
                                    div { class: "text-lg font-bold text-gray-500", "Unknown" }
                                }
                            }
                            button {
                                class: "text-[11px] px-3 py-1 rounded bg-cyan-600 text-white disabled:opacity-40",
                                disabled: snapshot.lora_triggering || lora_running,
                                onclick: {
                                    let trigger_lora_export = trigger_lora_export.clone();
                                    move |evt| (trigger_lora_export)(evt)
                                },
                                if snapshot.lora_triggering {
                                    "Starting…"
                                } else if lora_running {
                                    "Running"
                                } else {
                                    "Run Export"
                                }
                            }
                        }
                        div { class: "grid grid-cols-1 md:grid-cols-3 gap-4 text-xs text-gray-300",
                            div {
                                span { class: "font-semibold text-gray-200", "Last Started" }
                                br {}
                                span { class: "text-gray-400",
                                    {format_timestamp(lora_status.as_ref().and_then(|s| s.last_started.as_ref()))}
                                }
                            }
                            div {
                                span { class: "font-semibold text-gray-200", "Last Finished" }
                                br {}
                                span { class: "text-gray-400",
                                    {format_timestamp(lora_status.as_ref().and_then(|s| s.last_finished.as_ref()))}
                                }
                            }
                            div {
                                span { class: "font-semibold text-gray-200", "Last Error" }
                                br {}
                                span {
                                    class: if lora_last_error.is_some() {
                                        "text-red-400"
                                    } else {
                                        "text-gray-500"
                                    },
                                    if let Some(err) = lora_last_error.clone() {
                                        "{err}"
                                    } else {
                                        "—"
                                    }
                                }
                            }
                        }
                        div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                            div { class: "space-y-2 p-3 rounded border border-slate-700 bg-slate-900/40",
                                div { class: "text-sm font-semibold text-gray-200", "Auto-export after upload" }
                                div { class: "flex items-center gap-2",
                                    input {
                                        r#type: "checkbox",
                                        checked: snapshot.lora_auto_enabled,
                                        onchange: {
                                            let mut state = state;
                                            move |evt: Event<FormData>| {
                                                let value = evt.checked();
                                                let mut snapshot = state.write();
                                                snapshot.lora_auto_enabled = value;
                                                snapshot.lora_config_dirty = true;
                                            }
                                        },
                                    }
                                    span { class: "text-xs text-gray-300", "Enable automatic export when uploads complete" }
                                }
                                // Memory usage indicator — shown only when auto-export is on
                                if snapshot.lora_auto_enabled {
                                    if let Some((pct, bar_color, used_gb, total_gb)) = mem_bar {
                                        div { class: "space-y-0.5",
                                            div { class: "flex justify-between text-[10px] text-gray-400",
                                                span { "RAM" }
                                                span { "{used_gb:.1} / {total_gb:.1} GB  ({pct:.0}%)" }
                                            }
                                            div { class: "w-full h-1.5 rounded bg-gray-700",
                                                div {
                                                    class: "h-1.5 rounded",
                                                    style: "{bar_color}width:{pct:.1}%",
                                                }
                                            }
                                        }
                                    }
                                }

                                div { class: "space-y-1",
                                    label { class: "text-[11px] text-gray-400", "Debounce (ms)" }
                                    input {
                                        class: "w-full rounded border border-slate-700 bg-slate-800 px-2 py-1 text-xs text-gray-100",
                                        r#type: "number",
                                        min: "0",
                                        value: "{snapshot.lora_debounce_input}",
                                        oninput: {
                                            let mut state = state;
                                            move |evt: Event<FormData>| {
                                                let value = evt.value();
                                                let mut snapshot = state.write();
                                                snapshot.lora_debounce_input = value;
                                                snapshot.lora_config_dirty = true;
                                            }
                                        },
                                    }
                                }
                                div { class: "flex items-center gap-2",
                                    button {
                                        class: "text-[11px] px-3 py-1 rounded bg-emerald-600 text-white disabled:opacity-40",
                                        disabled: snapshot.lora_saving_config || !snapshot.lora_config_dirty,
                                        onclick: {
                                            let save_lora_config = save_lora_config.clone();
                                            move |evt| (save_lora_config)(evt)
                                        },
                                        if snapshot.lora_saving_config { "Saving…" } else { "Save" }
                                    }
                                    button {
                                        class: "text-[11px] px-3 py-1 rounded border border-slate-500 text-slate-200 disabled:opacity-40",
                                        disabled: snapshot.lora_saving_config,
                                        onclick: {
                                            move |_| reset_lora_config_inputs(state)
                                        },
                                        "Reset"
                                    }
                                }
                            }
                            div { class: "space-y-2 p-3 rounded border border-slate-700 bg-slate-900/40",
                                div { class: "text-sm font-semibold text-gray-200", "LORA_EXPORT_ONLY filter" }
                                textarea {
                                    class: "w-full rounded border border-slate-700 bg-slate-800 px-2 py-1 text-xs text-gray-100 h-24",
                                    value: "{snapshot.lora_filter_input}",
                                    placeholder: "Comma-separated file names (docs/example.md,notes/todo.md)…",
                                    oninput: {
                                        let mut state = state;
                                        move |evt: Event<FormData>| {
                                            let value = evt.value();
                                            let mut snapshot = state.write();
                                            snapshot.lora_filter_input = value;
                                            snapshot.lora_filter_dirty = true;
                                        }
                                    },
                                }
                                div { class: "flex items-center gap-2",
                                    button {
                                        class: "text-[11px] px-3 py-1 rounded bg-blue-600 text-white disabled:opacity-40",
                                        disabled: snapshot.lora_saving_filter || !snapshot.lora_filter_dirty,
                                        onclick: {
                                            let save_lora_filter = save_lora_filter.clone();
                                            move |evt| (save_lora_filter)(evt)
                                        },
                                        if snapshot.lora_saving_filter { "Saving…" } else { "Apply" }
                                    }
                                    button {
                                        class: "text-[11px] px-3 py-1 rounded border border-slate-500 text-slate-200 disabled:opacity-40",
                                        disabled: snapshot.lora_saving_filter,
                                        onclick: {
                                            move |_| reset_lora_filter_input(state)
                                        },
                                        "Reset"
                                    }
                                }
                                div { class: "text-[11px] text-gray-400",
                                    "Leave blank to export all documents. When populated, backend sets LORA_EXPORT_ONLY during export scripts."
                                }
                            }
                        }

                        // Synthetic Q&A Generation Section
                        div { class: "mt-4 p-3 rounded border border-cyan-700/50 bg-cyan-900/20",
                            div { class: "flex items-center justify-between mb-3",
                                div {
                                    div { class: "flex items-center gap-2",
                                        span { class: "text-sm font-semibold text-cyan-200", "Synthetic Q&A Generation" }
                                        button {
                                            class: QUICK_ACTION_INFO_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_synthetic_qa_info.set(true),
                                            title: "What is Synthetic Q&A Generation?",
                                            InfoIcon {}
                                        }
                                    }
                                    div { class: "text-[11px] text-cyan-400/70", "Auto-generate training data from your documents" }
                                }
                                {
                                    let qa_status = snapshot.synthetic_qa_status.as_ref();
                                    let qa_running = qa_status.map(|s| s.running).unwrap_or(false);
                                    rsx! {
                                        button {
                                            class: "text-[11px] px-3 py-1 rounded bg-cyan-600 text-white disabled:opacity-40",
                                            disabled: snapshot.synthetic_qa_triggering || qa_running || snapshot.lora_triggering,
                                            onclick: {
                                                let trigger_synthetic_qa = trigger_synthetic_qa.clone();
                                                move |evt| (trigger_synthetic_qa)(evt)
                                            },
                                            if snapshot.synthetic_qa_triggering {
                                                "Starting…"
                                            } else if qa_running {
                                                "Generating…"
                                            } else {
                                                "Generate Q&A"
                                            }
                                        }
                                    }
                                }
                            }
                            div { class: "grid grid-cols-1 md:grid-cols-2 gap-4 mb-3",
                                div { class: "space-y-1",
                                    label { class: "text-[11px] text-cyan-300", "Questions per chunk" }
                                    select {
                                        class: "w-full rounded border border-cyan-700/50 bg-slate-800 px-2 py-1 text-xs text-gray-100",
                                        value: "{snapshot.synthetic_qa_questions_per_chunk}",
                                        onchange: {
                                            let mut state = state;
                                            move |evt: Event<FormData>| {
                                                if let Ok(val) = evt.value().parse::<u32>() {
                                                    state.write().synthetic_qa_questions_per_chunk = val;
                                                }
                                            }
                                        },
                                        option { value: "1", "1" }
                                        option { value: "2", "2" }
                                        option { value: "3", "3 (default)" }
                                        option { value: "5", "5" }
                                        option { value: "10", "10" }
                                    }
                                }
                                div { class: "space-y-1",
                                    label { class: "text-[11px] text-cyan-300", "Max documents (blank = all)" }
                                    input {
                                        class: "w-full rounded border border-cyan-700/50 bg-slate-800 px-2 py-1 text-xs text-gray-100",
                                        r#type: "number",
                                        min: "1",
                                        placeholder: "All documents",
                                        value: "{snapshot.synthetic_qa_max_chunks}",
                                        oninput: {
                                            let mut state = state;
                                            move |evt: Event<FormData>| {
                                                state.write().synthetic_qa_max_chunks = evt.value();
                                            }
                                        },
                                    }
                                }
                            }
                            if let Some(status) = snapshot.synthetic_qa_status.as_ref() {
                                div { class: "grid grid-cols-2 md:grid-cols-4 gap-2 text-[11px]",
                                    div {
                                        span { class: "text-cyan-300", "Status: " }
                                        span {
                                            class: if status.running { "text-yellow-400" } else { "text-gray-400" },
                                            if status.running { "Running" } else { "Idle" }
                                        }
                                    }
                                    div {
                                        span { class: "text-cyan-300", "Examples: " }
                                        span { class: "text-gray-300",
                                            if let Some(count) = status.examples_generated {
                                                "{count}"
                                            } else {
                                                "—"
                                            }
                                        }
                                    }
                                    div {
                                        span { class: "text-cyan-300", "Last run: " }
                                        span { class: "text-gray-400",
                                            {format_timestamp(status.last_finished.as_ref())}
                                        }
                                    }
                                    if let Some(ref err) = status.last_error {
                                        div {
                                            span { class: "text-red-400", "Error: {err}" }
                                        }
                                    }
                                }
                            }
                            div { class: "flex items-center justify-between mt-2",
                                div { class: "text-[10px] text-cyan-400/60",
                                    "Uses Ollama to generate questions from your documents, then creates grounded answers via RAG."
                                }
                                button {
                                    class: "text-[11px] px-2 py-1 rounded border border-cyan-600/50 text-cyan-300 hover:bg-cyan-600/20",
                                    onclick: {
                                        let load_synthetic_qa_examples = load_synthetic_qa_examples.clone();
                                        let mut show_synthetic_qa_examples = show_synthetic_qa_examples;
                                        move |_| {
                                            (load_synthetic_qa_examples)(0);
                                            show_synthetic_qa_examples.set(true);
                                        }
                                    },
                                    "View Examples"
                                }
                            }
                        }
                    }

                    // Synthetic Q&A Examples Modal
                    if show_synthetic_qa_examples() {
                        div {
                            class: "fixed inset-0 z-40 bg-black/70 backdrop-blur-sm",
                            onclick: {
                                let mut show_synthetic_qa_examples = show_synthetic_qa_examples;
                                move |_| show_synthetic_qa_examples.set(false)
                            }
                        }
                        div {
                            class: "fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[95%] max-w-4xl rounded-xl border border-cyan-700/50 bg-slate-950/95 p-6 shadow-2xl text-sm text-slate-200 overflow-y-auto max-h-[85vh]",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-start justify-between gap-4 mb-4",
                                div {
                                    div { class: "text-base font-semibold text-cyan-200", "Generated Q&A Examples" }
                                    if let Some(ref examples) = snapshot.synthetic_qa_examples {
                                        div { class: "text-xs text-cyan-400/70",
                                            "Showing {examples.offset + 1}-{(examples.offset + examples.examples.len()).min(examples.total)} of {examples.total} examples"
                                        }
                                    }
                                }
                                button {
                                    class: "text-slate-400 hover:text-red-400 text-xl leading-none",
                                    onclick: {
                                        let mut show_synthetic_qa_examples = show_synthetic_qa_examples;
                                        move |_| show_synthetic_qa_examples.set(false)
                                    },
                                    "×"
                                }
                            }
                            if snapshot.synthetic_qa_examples_loading {
                                div { class: "text-center py-8 text-cyan-400", "Loading examples…" }
                            } else if let Some(ref examples) = snapshot.synthetic_qa_examples {
                                if examples.examples.is_empty() {
                                    div { class: "text-center py-8 text-gray-400",
                                        "No examples generated yet. Click \"Generate Q&A\" to create training data."
                                    }
                                } else {
                                    div { class: "space-y-4",
                                        for (idx, example) in examples.examples.iter().enumerate() {
                                            div { class: "p-3 rounded border border-slate-700 bg-slate-900/50",
                                                div { class: "flex items-start justify-between gap-2 mb-2",
                                                    div { class: "text-xs text-cyan-400 font-semibold",
                                                        "#{examples.offset + idx + 1}"
                                                    }
                                                    if let Some(ref source) = example.source {
                                                        div { class: "text-[10px] text-gray-500", "{source}" }
                                                    }
                                                }
                                                div { class: "mb-2",
                                                    div { class: "text-[10px] text-gray-400 uppercase tracking-wide mb-1", "Question" }
                                                    div { class: "text-sm text-gray-100", "{example.instruction}" }
                                                }
                                                div { class: "mb-2",
                                                    div { class: "text-[10px] text-gray-400 uppercase tracking-wide mb-1", "Answer" }
                                                    div { class: "text-sm text-gray-300 whitespace-pre-wrap",
                                                        {if example.response.len() > 500 {
                                                            format!("{}...", &example.response[..500])
                                                        } else {
                                                            example.response.clone()
                                                        }}
                                                    }
                                                }
                                                details { class: "text-xs",
                                                    summary { class: "text-gray-500 cursor-pointer hover:text-gray-300", "Show context" }
                                                    div { class: "mt-2 p-2 rounded bg-slate-800 text-gray-400 text-[11px] max-h-32 overflow-y-auto whitespace-pre-wrap",
                                                        {if example.context.len() > 1000 {
                                                            format!("{}...", &example.context[..1000])
                                                        } else {
                                                            example.context.clone()
                                                        }}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    // Pagination
                                    div { class: "flex items-center justify-between mt-4 pt-4 border-t border-slate-700",
                                        button {
                                            class: "text-[11px] px-3 py-1 rounded border border-slate-600 text-slate-300 disabled:opacity-40",
                                            disabled: examples.offset == 0,
                                            onclick: {
                                                let load_synthetic_qa_examples = load_synthetic_qa_examples.clone();
                                                let offset = examples.offset.saturating_sub(10);
                                                move |_| (load_synthetic_qa_examples)(offset)
                                            },
                                            "← Previous"
                                        }
                                        span { class: "text-xs text-gray-400",
                                            "Page {(examples.offset / 10) + 1} of {examples.total.div_ceil(10)}"
                                        }
                                        button {
                                            class: "text-[11px] px-3 py-1 rounded border border-slate-600 text-slate-300 disabled:opacity-40",
                                            disabled: examples.offset + examples.limit >= examples.total,
                                            onclick: {
                                                let load_synthetic_qa_examples = load_synthetic_qa_examples.clone();
                                                let offset = examples.offset + 10;
                                                move |_| (load_synthetic_qa_examples)(offset)
                                            },
                                            "Next →"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Synthetic Q&A Info Modal
                    if show_synthetic_qa_info() {
                        div {
                            class: "fixed inset-0 z-40 bg-black/70 backdrop-blur-sm",
                            onclick: {
                                let mut show_synthetic_qa_info = show_synthetic_qa_info;
                                move |_| show_synthetic_qa_info.set(false)
                            }
                        }
                        div {
                            class: "fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[95%] max-w-3xl rounded-xl border border-cyan-700/50 bg-slate-950/95 p-6 shadow-2xl text-sm text-slate-200 space-y-4 overflow-y-auto max-h-[80vh]",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-start justify-between gap-4",
                                div {
                                    div { class: "text-base font-semibold text-cyan-200", "Synthetic Q&A Generation" }
                                    div { class: "text-xs text-cyan-400/70", "Automated training data creation" }
                                }
                                button {
                                    class: "text-slate-400 hover:text-red-400 text-xl leading-none",
                                    onclick: {
                                        let mut show_synthetic_qa_info = show_synthetic_qa_info;
                                        move |_| show_synthetic_qa_info.set(false)
                                    },
                                    "×"
                                }
                            }
                            div { class: "space-y-4 text-[13px] text-slate-300",
                                div { class: "space-y-2",
                                    p { class: "font-semibold text-cyan-300", "What is it?" }
                                    p {
                                        "Synthetic Q&A Generation automatically creates training data for LoRA fine-tuning by generating question-answer pairs from your documents."
                                    }
                                }
                                div { class: "space-y-2",
                                    p { class: "font-semibold text-cyan-300", "How does it work?" }
                                    ol { class: "list-decimal ml-5 space-y-1",
                                        li { "Exports your documents to JSONL format" }
                                        li { "Uses Ollama to generate realistic questions about each document chunk" }
                                        li { "Retrieves context via your RAG API (simulating real usage)" }
                                        li { "Generates grounded answers using the retrieved context" }
                                        li { "Outputs training-ready JSONL with instruction/context/response format" }
                                    }
                                }
                                div { class: "space-y-2",
                                    p { class: "font-semibold text-cyan-300", "Why use it?" }
                                    ul { class: "list-disc ml-5 space-y-1",
                                        li { "Quickly reach the 500+ examples needed for LoRA training" }
                                        li { "Generate diverse questions covering your entire document corpus" }
                                        li { "Create training data that teaches the model how to use RAG context" }
                                        li { "Supplement user feedback data when traffic is low" }
                                    }
                                }
                                div { class: "space-y-2",
                                    p { class: "font-semibold text-cyan-300", "Configuration" }
                                    ul { class: "list-disc ml-5 space-y-1",
                                        li {
                                            strong { "Questions per chunk: " }
                                            "How many questions to generate for each document section (1-10)"
                                        }
                                        li {
                                            strong { "Max documents: " }
                                            "Limit processing to a subset of documents (blank = all)"
                                        }
                                    }
                                }
                                div { class: "space-y-2",
                                    p { class: "font-semibold text-cyan-300", "Requirements" }
                                    ul { class: "list-disc ml-5 space-y-1",
                                        li { "Ollama must be running (" code { class: "text-cyan-400", "ollama serve" } ")" }
                                        li { "RAG backend must be running" }
                                        li { "Documents must be indexed" }
                                    }
                                }
                                div { class: "text-xs text-slate-400 pt-2 border-t border-slate-700",
                                    p { "Output is saved to " code { "tools/lora_training/data/synthetic_qa.jsonl" } }
                                    p { "Click \"View Examples\" to browse generated Q&A pairs." }
                                }
                            }
                        }
                    }

                    if show_lora_info() {
                        div {
                            class: "fixed inset-0 z-40 bg-black/70 backdrop-blur-sm",
                            onclick: {
                                let mut show_lora_info = show_lora_info;
                                move |_| show_lora_info.set(false)
                            }
                        }
                        div {
                            class: "fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[95%] max-w-3xl rounded-xl border border-slate-700 bg-slate-950/95 p-6 shadow-2xl text-sm text-slate-200 space-y-4 overflow-y-auto max-h-[80vh]",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-start justify-between gap-4",
                                div {
                                    div { class: "text-base font-semibold", "How LoRA Export Controls Work" }
                                    div { class: "text-xs text-slate-400", "Applies to tools/lora_training scripts" }
                                }
                                button {
                                    class: "text-slate-400 hover:text-red-400 text-xl leading-none",
                                    onclick: {
                                        let mut show_lora_info = show_lora_info;
                                        move |_| show_lora_info.set(false)
                                    },
                                    "×"
                                }
                            }
                            div { class: "space-y-4 text-[13px] text-slate-300",
                                div { class: "space-y-2",
                                    p {
                                        span { class: "text-white font-semibold", "LoRA (Low-Rank Adaptation) " }
                                        "is a fine-tuning technique. Fine-tuning means updating a language model's weights so it learns from your data. Full fine-tuning updates all parameters — expensive. LoRA freezes the original model and inserts small trainable "
                                        span { class: "text-white font-semibold", "adapter matrices" }
                                        " at key layers. Only those adapters are trained. They are \"low-rank\" — two small matrices whose product approximates the full weight update — so they are fast to train and small to store."
                                    }
                                    p {
                                        "In ag, when documents are uploaded they go through the ingestion pipeline and become searchable (RAG). LoRA export is the parallel "
                                        span { class: "text-white font-semibold", "training path" }
                                        ": it takes those indexed chunks, generates synthetic question-answer pairs from each one, and packages them as JSONL training examples. A fine-tuning job then trains LoRA adapters on those examples. The result is a model that has "
                                        span { class: "text-white font-semibold", "internalized" }
                                        " your corpus as weights — it knows your domain without retrieving at inference time."
                                    }
                                }
                                div { class: "border-t border-slate-700 pt-3 space-y-1",
                                    p {
                                        "This board controls the entire LoRA snapshot pipeline. It talks to "
                                        code { "/training/export_snapshot" }
                                        ", the same endpoints that power the CLI scripts under "
                                        code { "tools/lora_training/" }
                                        "."
                                    }
                                    p {
                                        "Use it when you need a fresh JSONL dataset for fine-tuning or when you want uploads to trigger exports automatically without touching env files."
                                    }
                                }
                                div {
                                    class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                                    div {
                                        class: "space-y-1",
                                        strong { "Status card" }
                                        p {
                                            "Shows the live job state reported by the backend (running, idle, or last error) plus timestamps from the last run. It's read-only, but it confirms whether a manual or auto run actually started."
                                        }
                                    }
                                    div {
                                        class: "space-y-1",
                                        strong { "Run Export" }
                                        p {
                                            "Immediately launches "
                                            code { "export_docs.py" }
                                            " followed by "
                                            code { "normalize_dataset.py" }
                                            ". It respects whatever filter is configured below so you can export a narrow slice when testing."
                                        }
                                    }
                                    div {
                                        class: "space-y-1",
                                        strong { "Auto-export after upload" }
                                        p {
                                            "When enabled, every successful document upload batch schedules a LoRA export after the debounce window. Set the debounce (ms) to wait for multiple uploads before firing a single job."
                                        }
                                    }
                                    div {
                                        class: "space-y-1",
                                        strong { "Filter override" }
                                        p {
                                            "Writes to "
                                            code { "LORA_EXPORT_ONLY" }
                                            " in-memory before the scripts run. Provide comma-separated paths relative to "
                                            code { "documents/" }
                                            ". Leave blank to export everything."
                                        }
                                    }
                                }
                                div { class: "space-y-1",
                                    strong { "How it differs from the Reindex board" }
                                    p {
                                        "Reindex rebuilds the Tantivy search indexes so the RAG engine stays accurate. The LoRA board only curates datasets for model fine-tuning. It's normal to run LoRA exports more frequently than full reindexes when you are training adapters."
                                    }
                                }
                                div { class: "space-y-1 text-xs text-slate-400",
                                    p { "Whenever you press \"Save\"/\"Apply\", the backend stores the override in memory and responds with the latest config. The panel polls every 5 seconds so you'll see changes made from other clients." }
                                    p { "Direct API equivalents:" }
                                    ul { class: "list-disc ml-5 space-y-1",
                                        li { code { "POST /training/export_snapshot" } }
                                        li { code { "GET/POST /training/export_snapshot/config" } }
                                        li { code { "POST /training/export_snapshot/filter" } }
                                    }
                                }
                            }
                        }
                    }

                    // Current Snapshot Info Modal
                    if show_snapshot_info() {
                        div {
                            class: "fixed inset-0 z-40 bg-black/70 backdrop-blur-sm",
                            onclick: move |_| show_snapshot_info.set(false)
                        }
                        div {
                            class: "fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[95%] max-w-2xl rounded-xl border border-slate-700 bg-slate-950/95 p-6 shadow-2xl text-sm text-slate-200 space-y-4 max-h-[85vh] overflow-y-auto",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-start justify-between gap-4",
                                div {
                                    div { class: "text-base font-semibold text-white", "Current Snapshot" }
                                    div { class: "text-xs text-slate-400", "from GET /index/info · refreshes every 10s" }
                                }
                                button {
                                    class: "text-slate-400 hover:text-red-400 text-xl leading-none",
                                    onclick: move |_| show_snapshot_info.set(false),
                                    "×"
                                }
                            }
                            div { class: "space-y-3 text-[13px] text-slate-300",
                                p {
                                    "This panel shows a point-in-time read of the two storage layers the search engine uses: the "
                                    span { class: "text-white font-semibold", "Tantivy full-text index" }
                                    " and the "
                                    span { class: "text-white font-semibold", "vector store" }
                                    ". It tells you what the backend currently has indexed — the numbers here are what will be searched when a query arrives."
                                }
                                div { class: "grid grid-cols-1 md:grid-cols-2 gap-3",
                                    div { class: "bg-slate-900 rounded p-3 space-y-1",
                                        div { class: "text-white font-semibold text-xs", "Document Chunks" }
                                        p { "Number of text chunks stored in the Tantivy index. A single uploaded file becomes multiple chunks depending on the active chunker and chunk size. This number grows with each upload or reindex." }
                                    }
                                    div { class: "bg-slate-900 rounded p-3 space-y-1",
                                        div { class: "text-white font-semibold text-xs", "Vectors" }
                                        p {
                                            "Number of embedding vectors in the vector store. Each chunk produces one vector. In a healthy index, this number equals Document Chunks. A mismatch means some chunks are missing embeddings — a reindex will fix it."
                                        }
                                    }
                                    div { class: "bg-slate-900 rounded p-3 space-y-1",
                                        div { class: "text-white font-semibold text-xs", "Mode" }
                                        p { "The chunker that was active when documents were last indexed. "
                                            code { class: "text-green-300", "fixed" }
                                            " splits by token budget, "
                                            code { class: "text-green-300", "lightweight" }
                                            " splits by line/paragraph, "
                                            code { class: "text-green-300", "semantic" }
                                            " splits at topic boundaries. Changing the mode requires a full reindex to take effect."
                                        }
                                    }
                                    div { class: "bg-slate-900 rounded p-3 space-y-1",
                                        div { class: "text-white font-semibold text-xs", "Index in RAM" }
                                        p { "Whether the Tantivy reader has the index segments memory-mapped. When true, search hits are served from RAM — lower latency. When false, reads go through the OS page cache or disk." }
                                    }
                                }
                                p { class: "text-xs text-slate-400",
                                    "If the panel shows a warning, it usually means vectors and document counts are out of sync. Run a reindex from the Reindex Control section below to reconcile them."
                                }
                            }
                            button {
                                class: "btn btn-sm w-full mt-2",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;color:white;",
                                onclick: move |_| show_snapshot_info.set(false),
                                "Got it"
                            }
                        }
                    }

                    // Chunking Logging Info Modal
                    if show_chunking_logging_info() {
                        div {
                            class: "fixed inset-0 z-40 bg-black/70 backdrop-blur-sm",
                            onclick: {
                                let mut show_chunking_logging_info = show_chunking_logging_info;
                                move |_| show_chunking_logging_info.set(false)
                            }
                        }
                        div {
                            class: "fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[95%] max-w-3xl rounded-xl border border-slate-700 bg-slate-950/95 p-6 shadow-2xl text-sm text-slate-200 space-y-4 overflow-y-auto max-h-[80vh]",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-start justify-between gap-4",
                                div {
                                    div { class: "text-base font-semibold", "Chunking Snapshot Logging" }
                                    div { class: "text-xs text-slate-400", "Per-file indexing telemetry" }
                                }
                                button {
                                    class: "text-slate-400 hover:text-red-400 text-xl leading-none",
                                    onclick: {
                                        let mut show_chunking_logging_info = show_chunking_logging_info;
                                        move |_| show_chunking_logging_info.set(false)
                                    },
                                    "×"
                                }
                            }
                            div { class: "space-y-4 text-[13px] text-slate-300",
                                div { class: "space-y-2",
                                    p {
                                        "When enabled, every time a document is chunked (split into pieces for indexing), a detailed snapshot is recorded containing:"
                                    }
                                    div { class: "bg-slate-800/50 rounded p-3 text-xs font-mono",
                                        ul { class: "space-y-1",
                                            li { span { class: "text-blue-400", "file" } ": The filename being processed" }
                                            li { span { class: "text-blue-400", "chunker_mode" } ": Which strategy was used (Fixed, Lightweight, Semantic)" }
                                            li { span { class: "text-blue-400", "chunks" } ": Number of chunks created" }
                                            li { span { class: "text-blue-400", "tokens" } ": Total token count" }
                                            li { span { class: "text-blue-400", "duration_ms" } ": How long chunking took" }
                                            li { span { class: "text-blue-400", "detection" } ": How the file type was detected (MIME, extension, heuristic)" }
                                        }
                                    }
                                }
                                div {
                                    class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                                    div {
                                        class: "space-y-1",
                                        strong { "🔍 Debugging Chunking Issues" }
                                        p {
                                            "See why a document produced unexpected chunk counts, identify files that take too long to process, or verify the correct chunker mode is being used."
                                        }
                                    }
                                    div {
                                        class: "space-y-1",
                                        strong { "📊 Performance Monitoring" }
                                        p {
                                            "Track chunking duration over time, identify slow files or patterns, and monitor throughput during bulk indexing."
                                        }
                                    }
                                    div {
                                        class: "space-y-1",
                                        strong { "📈 Observability & Auditing" }
                                        p {
                                            "Logs are sent to your observability stack (Loki/Grafana via Vector). Query historical chunking operations and correlate indexing issues with specific files."
                                        }
                                    }
                                    div {
                                        class: "space-y-1",
                                        strong { "🔎 Format Detection Debugging" }
                                        p {
                                            "See what MIME type was detected, verify file extension handling, and debug why a file was chunked with the wrong strategy."
                                        }
                                    }
                                }
                                div { class: "space-y-1",
                                    strong { "When to disable" }
                                    p {
                                        "If you're doing high-volume indexing and don't need per-file logging, disable it to reduce log volume. The in-memory history is still kept for the UI."
                                    }
                                }
                                div { class: "space-y-1 text-xs text-slate-400",
                                    p { "Toggle methods:" }
                                    ul { class: "list-disc ml-5 space-y-1",
                                        li { "UI: Click the toggle button above" }
                                        li { code { "GET /monitoring/chunking/logging?enabled=true|false" } }
                                        li { "Env: Set " code { "CHUNKING_SNAPSHOT_LOGGING=true|false" } " in .env" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            RowHeader {
                title: "Reindex Control".into(),
                description: Some("Trigger sync/async runs and inspect status".into()),
                leading: Some(rsx! {
                    button {
                        class: QUICK_ACTION_INFO_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        onclick: {
                            let mut reindex_control_info_open = reindex_control_info_open;
                            move |_| reindex_control_info_open.set(true)
                        },
                        InfoIcon {}
                    }
                }),
            }

            Panel {
                div { class: "flex items-center justify-between mb-3",
                    div { class: "flex items-center gap-3",
                        h3 { class: "text-sm font-semibold text-gray-200", "Reindex Status" }
                    }
                    span { class: "text-xs text-white", "5s" }
                }
                div { class: "relative",
                    div { class: "relative rounded border border-slate-700 bg-slate-900/40 p-4 mx-4 space-y-4",
                        div { class: "flex items-center gap-2",
                            // Block: light grey card with status and action buttons
                            div { class: "inline-block rounded p-4 bg-gray-800",
                                div { class: "flex items-center gap-4",
                                    div {
                                        div { class: "text-xs text-gray-400", "Reindex Status" }
                                        if let Some(job) = latest_job.clone() {
                                            div { class: "text-lg font-bold text-gray-100", "{job.status}" }
                                        } else {
                                            div { class: "text-lg font-bold text-gray-500", "Ready" }
                                        }
                                    }
                                    div { class: "flex items-center gap-2",
                                        button {
                                            class: "text-[11px] px-3 py-1 rounded bg-indigo-600 text-white disabled:opacity-40",
                                            disabled: snapshot.sync_running,
                                            onclick: {
                                                let trigger_sync_reindex = trigger_sync_reindex.clone();
                                                move |evt| (trigger_sync_reindex)(evt)
                                            },
                                            if snapshot.sync_running { "Reindexing…" } else { "Now" }
                                        }
                                        button {
                                            class: "text-[11px] px-3 py-1 rounded bg-teal-600 text-white disabled:opacity-40",
                                            disabled: snapshot.async_running,
                                            onclick: {
                                                let trigger_async_reindex = trigger_async_reindex.clone();
                                                move |evt| (trigger_async_reindex)(evt)
                                            },
                                            if snapshot.async_running { "Submitting…" } else { "Background" }
                                        }
                                    }
                                }
                            }
                            // Upload button outside the block
                            label {
                                class: "text-[11px] px-3 py-1 rounded border border-slate-500 text-slate-200 hover:bg-slate-600/20 cursor-pointer",
                                input {
                                    r#type: "file",
                                    class: "hidden",
                                    multiple: true,
                                    disabled: snapshot.upload_running,
                                    onchange: {
                                        move |evt: Event<FormData>| {
                                            let mut state = state;
                                            let selected_corpus = selected_corpus;
                                            spawn(async move {
                                                let files = evt.files();
                                                let total = files.len();

                                                if total == 0 {
                                                    return;
                                                }

                                                let slug = selected_corpus.read().clone();

                                                {
                                                    let mut s = state.write();
                                                    s.upload_running = true;
                                                    s.upload_total_files = total;
                                                    s.upload_completed_files = 0;
                                                    s.upload_failed_files = 0;
                                                    s.upload_current_file = None;
                                                    s.upload_message = Some(format!("Starting upload of {} file(s)...", total));
                                                }

                                                for file_data in &files {
                                                    let file_name = file_data.name();
                                                    {
                                                        let mut s = state.write();
                                                        s.upload_current_file = Some(file_name.clone());
                                                        s.upload_message = Some(format!("Uploading: {}", file_name));
                                                    }

                                                    match file_data.read_bytes().await {
                                                        Ok(contents) => {
                                                            let upload_result = if let Some(ref s) = slug {
                                                                api::upload_document_to_corpus(s, &file_name, &contents).await
                                                            } else {
                                                                api::upload_document(&file_name, &contents).await
                                                            };
                                                            match upload_result {
                                                                Ok(resp) if !resp.index_errors.is_empty() => {
                                                                    let mut s = state.write();
                                                                    s.upload_failed_files += 1;
                                                                    let err_text = resp.index_errors.iter()
                                                                        .map(|e| e.error.as_str())
                                                                        .collect::<Vec<_>>()
                                                                        .join("; ");
                                                                    s.upload_message = Some(format!("{}: {}", file_name, err_text));
                                                                }
                                                                Ok(_) => {
                                                                    let mut s = state.write();
                                                                    s.upload_completed_files += 1;
                                                                }
                                                                Err(err) => {
                                                                    let mut s = state.write();
                                                                    s.upload_failed_files += 1;
                                                                    s.upload_message = Some(format!("Failed: {} - {}", file_name, err));
                                                                }
                                                            }
                                                        }
                                                        Err(err) => {
                                                            let mut s = state.write();
                                                            s.upload_failed_files += 1;
                                                            s.upload_message = Some(format!("Failed to read: {} - {}", file_name, err));
                                                        }
                                                    }
                                                }

                                                // Complete
                                                {
                                                    let mut s = state.write();
                                                    let completed = s.upload_completed_files;
                                                    let failed = s.upload_failed_files;
                                                    s.upload_running = false;
                                                    s.upload_current_file = None;
                                                    s.upload_message = Some(format!(
                                                        "Upload complete: {} succeeded, {} failed",
                                                        completed, failed
                                                    ));
                                                }
                                            });
                                        }
                                    },
                                }
                                if snapshot.upload_running { "Uploading..." } else { "Upload" }
                            }
                        }

                        // Upload progress monitor
                        if snapshot.upload_running || snapshot.upload_message.is_some() {
                            div { class: "space-y-2",
                                // Progress bar
                                if snapshot.upload_total_files > 0 {
                                    div { class: "relative h-2 bg-gray-700 rounded-full overflow-hidden",
                                        div {
                                            class: {
                                                if snapshot.upload_running {
                                                    "h-full bg-indigo-500 transition-all duration-300"
                                                } else if snapshot.upload_failed_files > 0 {
                                                    "h-full bg-yellow-500 transition-all duration-300"
                                                } else {
                                                    "h-full bg-teal-500 transition-all duration-300"
                                                }
                                            },
                                            style: "width: {((snapshot.upload_completed_files + snapshot.upload_failed_files) * 100) / snapshot.upload_total_files.max(1)}%",
                                        }
                                    }
                                }
                                // Status text
                                div { class: "flex justify-between text-[10px]",
                                    span {
                                        class: {
                                            if snapshot.upload_running {
                                                "text-indigo-300"
                                            } else if snapshot.upload_failed_files > 0 {
                                                "text-yellow-400"
                                            } else {
                                                "text-teal-300"
                                            }
                                        },
                                        if let Some(msg) = snapshot.upload_message.clone() {
                                            "{msg}"
                                        }
                                    }
                                    if snapshot.upload_total_files > 0 {
                                        span { class: "text-white",
                                            "{snapshot.upload_completed_files + snapshot.upload_failed_files} / {snapshot.upload_total_files} files"
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(job) = latest_job.clone() {
                            div { class: "space-y-3 text-sm text-gray-300",
                                // Combined Step + Progress Bar Monitor
                                {render_progress_monitor(&job)}

                                div { class: "grid grid-cols-1 md:grid-cols-3 gap-3 text-xs",
                                    div {
                                        span { class: "font-semibold text-gray-200", "Started" }
                                        br {}
                                        span { class: "text-gray-400", "{format_timestamp(job.started_at.as_ref())}" }
                                    }
                                    div {
                                        span { class: "font-semibold text-gray-200", "Completed" }
                                        br {}
                                        span { class: "text-gray-400", "{format_timestamp(job.completed_at.as_ref())}" }
                                    }
                                    div {
                                        span { class: "font-semibold text-gray-200", "Vectors / Docs" }
                                        br {}
                                        span { class: "text-gray-400", "{format_vectors_docs(job.vectors_indexed, job.mappings_indexed)}" }
                                    }
                                }
                                if let Some(err) = job.error.clone() {
                                    div { class: "text-xs text-red-400", "Error: {err}" }
                                }
                                if let Some(message) = snapshot.status_message.clone() {
                                    div { class: "text-xs text-indigo-300", "{message}" }
                                }
                            }
                        }
                    }
                }
            }

            Panel { title: Some("Async Jobs (Background)".into()), refresh: Some("5s".into()),
                if job_table_rows.is_empty() {
                    div { class: "text-sm text-gray-500", "No tracked jobs yet." }
                } else {
                    DataTable {
                        headers: vec![
                            "Job ID".into(),
                            "Status".into(),
                            "Started".into(),
                            "Completed".into(),
                            "Vectors".into(),
                            "Documents".into(),
                            "Error".into(),
                        ],
                        rows: job_table_rows,
                    }
                }
            }

            RowHeader {
                title: "Paths & Runbook".into(),
                description: Some("Copy CLI helpers or storage locations".into()),
            }

            Panel { title: Some("Operations".into()), refresh: None,
                div { class: "space-y-3 text-xs text-gray-300",
                    {command_block("Sync reindex", REINDEX_SYNC_COMMAND)}
                    {command_block("Async reindex", REINDEX_ASYNC_COMMAND)}
                    {command_block("Watch job status", REINDEX_STATUS_COMMAND)}
                    {command_block("Journalctl tail", JOURNALCTL_COMMAND)}
                    {command_block("Tail logs/ag.log", TAIL_LOGS_COMMAND)}
                }
            }

            Panel { title: Some("Storage Paths".into()), refresh: None,
                div { class: "space-y-3",
                    for (label, path) in STORAGE_PATHS.iter() {
                        div { class: "flex items-start justify-between gap-2 bg-gray-900/60 border border-gray-800 rounded px-3 py-2",
                            div {
                                div { class: "text-xs text-gray-200 font-semibold", {(*label).to_string()} }
                                div { class: "text-[11px] text-gray-400 break-all", {(*path).to_string()} }
                            }
                            CopyButton {
                                text: (*path).to_string(),
                                button_class: "text-[11px] px-2 py-1 rounded bg-slate-800 border border-slate-600 text-slate-200 hover:text-white hover:border-slate-400".to_string(),
                            }
                        }
                    }
                }
            }
        }
    }
}

fn command_block(label: &str, command: &str) -> Element {
    let command_owned = command.to_string();
    rsx! {
        div { class: "bg-slate-900/60 border border-slate-700 rounded px-3 py-2",
            div { class: "flex items-center gap-2 text-slate-200 text-[11px] font-semibold", {label} }
            div { class: "flex items-center gap-3 mt-1",
                code { class: "text-[11px] text-slate-300 break-all", {command} }
                CopyButton {
                    text: command_owned.clone(),
                    button_class: "text-[11px] px-2 py-1 rounded bg-slate-800 border border-slate-600 text-slate-200 hover:text-white hover:border-slate-400".to_string(),
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct CopyButtonProps {
    text: String,
    button_class: String,
}

#[component]
fn CopyButton(props: CopyButtonProps) -> Element {
    let copied = use_signal(|| false);

    let handle_click = {
        let text = props.text.clone();
        move |_| {
            let text = text.clone();
            let mut copied = copied;
            spawn(async move {
                copy_text_to_clipboard(&text);
                copied.set(true);
                TimeoutFuture::new(1_500).await;
                copied.set(false);
            });
        }
    };

    rsx! {
        button {
            class: props.button_class.clone(),
            onclick: handle_click,
            if copied() { "Copied" } else { "Copy" }
        }
    }
}

/// Renders a combined step indicator + progress bar for reindex jobs
fn render_progress_monitor(job: &ReindexJobRow) -> Element {
    // Determine current phase based on status
    let (phase, progress_percent) = match job.status.as_str() {
        "accepted" => (0, 5),    // Just started - Scanning
        "running" => (1, 50),    // In progress - Processing
        "completed" => (3, 100), // Done
        "failed" => (3, 100),    // Failed (show full bar in red)
        _ => (0, 0),
    };

    let is_failed = job.status == "failed";
    let is_running = matches!(job.status.as_str(), "accepted" | "running");

    // Phase labels
    let phases = ["Scan", "Process", "Index", "Done"];

    rsx! {
        div { class: "space-y-2",
            // Step indicators
            div { class: "flex items-center justify-between text-[10px]",
                for (i, label) in phases.iter().enumerate() {
                    div { class: "flex items-center gap-1",
                        // Step icon
                        span {
                            class: {
                                if is_failed && i == 3 {
                                    "w-4 h-4 rounded-full flex items-center justify-center bg-red-500 text-white text-[8px]"
                                } else if i < phase {
                                    "w-4 h-4 rounded-full flex items-center justify-center bg-teal-500 text-white text-[8px]"
                                } else if i == phase && is_running {
                                    "w-4 h-4 rounded-full flex items-center justify-center bg-indigo-500 text-white text-[8px] animate-pulse"
                                } else if i == phase {
                                    "w-4 h-4 rounded-full flex items-center justify-center bg-teal-500 text-white text-[8px]"
                                } else {
                                    "w-4 h-4 rounded-full flex items-center justify-center bg-gray-700 text-gray-500 text-[8px]"
                                }
                            },
                            if i < phase || (i == phase && !is_running) {
                                "✓"
                            } else if i == phase && is_running {
                                "●"
                            } else {
                                "○"
                            }
                        }
                        // Label
                        span {
                            class: {
                                if i <= phase {
                                    "text-gray-200"
                                } else {
                                    "text-gray-500"
                                }
                            },
                            "{label}"
                        }
                    }
                    // Connector line (except after last)
                    if i < phases.len() - 1 {
                        div {
                            class: {
                                if i < phase {
                                    "flex-1 h-px bg-teal-500 mx-2"
                                } else {
                                    "flex-1 h-px bg-gray-700 mx-2"
                                }
                            }
                        }
                    }
                }
            }

            // Progress bar
            div { class: "relative h-2 bg-gray-700 rounded-full overflow-hidden",
                div {
                    class: {
                        if is_failed {
                            "h-full bg-red-500 transition-all duration-500"
                        } else if is_running {
                            "h-full bg-indigo-500 transition-all duration-500"
                        } else {
                            "h-full bg-teal-500 transition-all duration-500"
                        }
                    },
                    style: "width: {progress_percent}%",
                }
            }

            // Status text
            div { class: "flex justify-between text-[10px]",
                span {
                    class: {
                        if is_failed {
                            "text-red-400"
                        } else if is_running {
                            "text-indigo-300"
                        } else {
                            "text-teal-300"
                        }
                    },
                    {pretty_status(&job.status)}
                }
                if let (Some(v), Some(m)) = (job.vectors_indexed, job.mappings_indexed) {
                    span { class: "text-gray-400", "{v} vectors / {m} docs" }
                }
            }
        }
    }
}

fn reset_lora_config_inputs(mut state: Signal<IndexState>) {
    let config = { state.read().lora_config.clone() };
    if let Some(config) = config {
        let mut snapshot = state.write();
        snapshot.lora_auto_enabled = config.auto_export_enabled;
        snapshot.lora_debounce_input = config.default_debounce_ms.to_string();
        snapshot.lora_config_dirty = false;
        snapshot.lora_message = Some("Reverted auto-export changes".into());
    }
}

fn reset_lora_filter_input(mut state: Signal<IndexState>) {
    let config = { state.read().lora_config.clone() };
    if let Some(config) = config {
        let mut snapshot = state.write();
        snapshot.lora_filter_input = config.export_filter.clone().unwrap_or_default();
        snapshot.lora_filter_dirty = false;
        snapshot.lora_message = Some("Reverted filter changes".into());
    }
}

fn pretty_status(status: &str) -> String {
    match status {
        "completed" => "✓ Completed".into(),
        "failed" => "⚠ Failed".into(),
        "running" => "● Running".into(),
        "accepted" => "● Accepted".into(),
        "not_found" => "? Not Found".into(),
        other => other.into(),
    }
}

fn format_timestamp(value: Option<&String>) -> String {
    if let Some(raw) = value {
        if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
            return dt
                .with_timezone(&Utc)
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string();
        }
    }
    "—".into()
}

fn format_vectors_docs(vectors: Option<usize>, mappings: Option<usize>) -> String {
    match (vectors, mappings) {
        (Some(v), Some(m)) => format!("{} vectors / {} docs", v, m),
        (Some(v), None) => format!("{} vectors", v),
        (None, Some(m)) => format!("{} docs", m),
        _ => "—".into(),
    }
}

fn copy_text_to_clipboard(text: &str) {
    if let Some(window) = web_sys::window() {
        let navigator = window.navigator();
        let clipboard = navigator.clipboard();
        let payload = text.to_string();
        spawn(async move {
            let promise = clipboard.write_text(&payload);
            if let Err(err) = JsFuture::from(promise).await {
                console::warn_1(&err);
            }
        });
    } else {
        console::warn_1(&"window unavailable for clipboard copy".into());
    }
}

async fn refresh_job_statuses(
    jobs: Signal<Vec<ReindexJobRow>>,
    mut state: Signal<IndexState>,
    include_terminal: bool,
) {
    let job_ids: Vec<String> = {
        let guard = jobs.read();
        guard
            .iter()
            .filter(|job| include_terminal || !job.is_terminal())
            .map(|job| job.job_id.clone())
            .collect()
    };

    for job_id in job_ids {
        if let Err(err) = refresh_single_job(job_id.clone(), jobs, state).await {
            state.write().status_message = Some(format!("Failed to refresh {}: {}", job_id, err));
        }
    }
}

async fn refresh_single_job(
    job_id: String,
    mut jobs: Signal<Vec<ReindexJobRow>>,
    mut state: Signal<IndexState>,
) -> Result<(), String> {
    match api::fetch_reindex_status(&job_id).await {
        Ok(resp) => {
            let status_label = resp.status.clone();
            {
                let mut rows = jobs.write();
                rows.retain(|row| row.job_id != resp.job_id);
                rows.insert(0, ReindexJobRow::from_status(resp));
            }

            if matches!(status_label.as_str(), "completed" | "failed") {
                let mut snapshot = state.write();
                snapshot.status_message =
                    Some(format!("Job {} {}", job_id, pretty_status(&status_label)));
            }

            Ok(())
        }
        Err(err) => Err(err),
    }
}

#[component]
fn InfoModal(title: &'static str, content: &'static str, on_close: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| on_close.call(()),
            div {
                class: "bg-gray-900 border border-slate-700 rounded-lg p-5 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-2xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-3",
                    h2 { class: "text-base font-semibold text-gray-100", "{title}" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                div {
                    class: "text-[12px] text-slate-200 whitespace-pre-line leading-relaxed",
                    "{content}"
                }
            }
        }
    }
}
