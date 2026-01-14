use crate::{api, app::Route, components::monitor::*};
use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use std::rc::Rc;
use wasm_bindgen_futures::JsFuture;
use web_sys::console;

const REINDEX_SYNC_COMMAND: &str = "curl -X POST http://127.0.0.1:3010/reindex";
const REINDEX_ASYNC_COMMAND: &str = "curl -X POST http://127.0.0.1:3010/reindex/async";
const REINDEX_STATUS_COMMAND: &str = "curl http://127.0.0.1:3010/reindex/status/<job_id>";
const JOURNALCTL_COMMAND: &str = "journalctl -u ag.service -n 200 -f";
const TAIL_LOGS_COMMAND: &str = "tail -f logs/ag.log";
const STORAGE_PATHS: [(&str, &str); 4] = [
    ("Tantivy Index", "~/.local/share/ag/index"),
    ("Vectors Store", "~/.local/share/ag/data/vectors.json"),
    ("SQLite Metadata", "~/.local/share/ag/db/metadata.db"),
    ("Documents", "~/ag/documents"),
];

#[derive(Clone, Default)]
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
    let show_more_actions = use_signal(|| false);
    let chunk_info_open = use_signal(|| false);
    let reindex_info_open = use_signal(|| false);

    {
        let mut state = state.clone();
        let mut index_info = index_info.clone();
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
        let state = state.clone();
        let jobs = jobs.clone();
        use_future(move || async move {
            loop {
                refresh_job_statuses(jobs.clone(), state.clone(), false).await;

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

    let trigger_sync_reindex = {
        let state = state.clone();
        Rc::new(move |_| {
            let mut state = state.clone();
            spawn(async move {
                {
                    let mut snapshot = state.write();
                    snapshot.sync_running = true;
                    snapshot.status_message = Some("Triggering sync reindex…".into());
                }

                match api::reindex().await {
                    Ok(_) => {
                        let mut snapshot = state.write();
                        snapshot.status_message = Some("Sync reindex request accepted".into());
                    }
                    Err(err) => {
                        let mut snapshot = state.write();
                        snapshot.status_message = Some(format!("Sync reindex failed: {}", err));
                    }
                }

                state.write().sync_running = false;
            });
        })
    };

    let trigger_async_reindex = {
        let state = state.clone();
        let jobs = jobs.clone();
        Rc::new(move |_| {
            let mut state = state.clone();
            let mut jobs = jobs.clone();
            spawn(async move {
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

                        if let Err(err) =
                            refresh_single_job(resp.job_id, jobs.clone(), state.clone()).await
                        {
                            state.write().status_message =
                                Some(format!("Failed to fetch async status: {}", err));
                        }
                    }
                    Err(err) => {
                        let mut snapshot = state.write();
                        snapshot.status_message = Some(format!("Async reindex failed: {}", err));
                    }
                }

                state.write().async_running = false;
            });
        })
    };

    let snapshot = state.read().clone();
    let info_snapshot = index_info.read().clone();
    let job_rows = jobs.read().clone();
    let latest_job = job_rows.first().cloned();

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

            RowHeader {
                title: "Index Statistics".into(),
                description: Some("Live snapshot from /index/info".into()),
            }


            Panel { title: Some("Current Snapshot".into()), refresh: Some("10s".into()),
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
                                            // Logging toggle
                                            if snapshot.chunking_logging_enabled.is_some() {
                                                button {
                                                    class: "text-[11px] px-3 py-1 rounded border border-slate-500 text-slate-200 hover:bg-slate-600/20 disabled:opacity-40",
                                                    onclick: {
                                                        let state = state.clone();
                                                        move |_| {
                                                            let mut state = state.clone();
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
                                            // More info dropdown
                                            div { class: "relative",
                                                button {
                                                    class: "text-[11px] px-3 py-1 rounded border border-slate-500 text-slate-200 hover:bg-slate-600/20 disabled:opacity-40",
                                                    onclick: {
                                                        let mut show_more_actions = show_more_actions.clone();
                                                        move |_| show_more_actions.set(!show_more_actions())
                                                    },
                                                    if show_more_actions() { "Close" } else { "More info" }
                                                }
                                                if show_more_actions() {
                                                    Fragment {
                                                        div {
                                                            class: "fixed inset-0 z-10",
                                                            onclick: {
                                                                let mut show_more_actions = show_more_actions.clone();
                                                                let mut reindex_info_open = reindex_info_open.clone();
                                                                move |_| {
                                                                    show_more_actions.set(false);
                                                                    reindex_info_open.set(false);
                                                                }
                                                            },
                                                        }
                                                        div {
                                                            class: "absolute z-50 left-1/2 -translate-x-1/2 top-full mt-2 w-52 rounded border border-slate-700 bg-slate-900 text-[11px] text-slate-100 shadow-xl",
                                                            button {
                                                                class: "w-full px-3 py-2 text-left hover:bg-slate-800 border-b border-slate-700",
                                                                onclick: {
                                                                    let mut chunk_info_open = chunk_info_open.clone();
                                                                    let mut reindex_info_open = reindex_info_open.clone();
                                                                    let mut show_more_actions = show_more_actions.clone();
                                                                    move |_| {
                                                                        reindex_info_open.set(false);
                                                                        chunk_info_open.set(true);
                                                                        show_more_actions.set(false);
                                                                    }
                                                                },
                                                                span { class: "font-semibold", "Chunks" }
                                                                div { class: "text-[10px] text-slate-400", "Pipeline overview" }
                                                            }
                                                            button {
                                                                class: "w-full px-3 py-2 text-left hover:bg-slate-800 border-b border-slate-700",
                                                                onclick: {
                                                                    let mut chunk_info_open = chunk_info_open.clone();
                                                                    let mut reindex_info_open = reindex_info_open.clone();
                                                                    let mut show_more_actions = show_more_actions.clone();
                                                                    move |_| {
                                                                        chunk_info_open.set(false);
                                                                        reindex_info_open.set(true);
                                                                        show_more_actions.set(false);
                                                                    }
                                                                },
                                                                span { class: "font-semibold", "Reindexing" }
                                                                div { class: "text-[10px] text-slate-400", "Cost overview" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if chunk_info_open() {
                                        div {
                                            class: "fixed inset-0 z-40 bg-black/60 backdrop-blur-sm",
                                            onclick: {
                                                let mut chunk_info_open = chunk_info_open.clone();
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
                                                        let mut chunk_info_open = chunk_info_open.clone();
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

                                    if reindex_info_open() {
                                        div {
                                            class: "fixed inset-0 z-40 bg-black/60 backdrop-blur-sm",
                                            onclick: {
                                                let mut reindex_info_open = reindex_info_open.clone();
                                                move |_| reindex_info_open.set(false)
                                            }
                                        }
                                        div {
                                            class: "fixed z-50 top-24 left-1/2 -translate-x-1/2 w-[95%] max-w-2xl rounded-lg border border-slate-700 bg-slate-950/95 p-5 text-[11px] text-slate-100 shadow-2xl space-y-4",
                                            onclick: move |evt| evt.stop_propagation(),
                                            div { class: "flex items-start justify-between gap-4",
                                                div {
                                                    div { class: "text-[12px] font-semibold", "Why reindexing is costly" }
                                                    div { class: "text-[10px] text-slate-400", "Resource impact overview" }
                                                }
                                                button {
                                                    class: "text-slate-400 hover:text-red-400 text-xl leading-none",
                                                    onclick: {
                                                        let mut reindex_info_open = reindex_info_open.clone();
                                                        move |_| reindex_info_open.set(false)
                                                    },
                                                    "×"
                                                }
                                            }
                                            p { class: "text-slate-300", "Reindexing is definitely one of the most resource-intensive operations in this codebase. It has to:" }
                                            ol { class: "list-decimal ml-4 space-y-3 text-slate-200",
                                                li { "Re-scan the entire documents/ corpus (disk I/O)." }
                                                li { "Chunk, embed, and serialize every file (CPU + any external embedding provider calls)." }
                                                li { "Rebuild Tantivy’s indexes and vector stores (CPU + RAM)." }
                                            }
                                            p { class: "text-slate-300", "That pipeline touches almost every heavy subsystem, so it consumes the most CPU time and memory in a single run. Other workloads—like search, cache refresh, or rate-limit tracking—are comparatively cheap because they operate incrementally or over recent data only." }
                                            p { class: "text-slate-300", "So reindexing is treated as the “costly” operation, which is why we gate it behind manual/explicit triggers and provide Rust async job handling to keep the UI responsive while it churns." }
                                        }
                                    }
                                }
                            }

                            if let Some(warning) = info.warning.clone() {
                                div { class: "text-xs text-yellow-400", "{warning}" }
                            }
                        }
                    } else {
                        div { class: "text-sm text-gray-400", "No index info available" }
                    }
                }
            }
            RowHeader {
                title: "Reindex Control".into(),
                description: Some("Trigger sync/async runs and inspect status".into()),
            }

            Panel { title: Some("Reindex Status".into()), refresh: Some("5s".into()),
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
                                        button {
                                            class: "text-[11px] px-3 py-1 rounded border border-slate-500 text-slate-200 hover:bg-slate-600/20",
                                            onclick: {
                                                let mut reindex_info_open = reindex_info_open.clone();
                                                move |_| reindex_info_open.set(true)
                                            },
                                            "More info"
                                        }
                                    }
                                }
                            }
                            // Upload button outside the block, near More info
                            label {
                                class: "text-[11px] px-3 py-1 rounded border border-slate-500 text-slate-200 hover:bg-slate-600/20 cursor-pointer",
                                input {
                                    r#type: "file",
                                    class: "hidden",
                                    multiple: true,
                                    disabled: snapshot.upload_running,
                                    onchange: {
                                        let state = state.clone();
                                        move |evt: dioxus::prelude::Event<dioxus::prelude::FormData>| {
                                            let mut state = state.clone();
                                            spawn(async move {
                                                // Use Dioxus 0.7 file handling
                                                let files = evt.files();
                                                let total = files.len();

                                                if total == 0 {
                                                    return;
                                                }

                                                // Start upload
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
                                                    // Update current file
                                                    {
                                                        let mut s = state.write();
                                                        s.upload_current_file = Some(file_name.clone());
                                                        s.upload_message = Some(format!("Uploading: {}", file_name));
                                                    }

                                                    match file_data.read_bytes().await {
                                                        Ok(contents) => {
                                                            match api::upload_document(&file_name, &contents).await {
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
        let copied = copied.clone();
        move |_| {
            let text = text.clone();
            let mut copied = copied.clone();
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
        if let Err(err) = refresh_single_job(job_id.clone(), jobs.clone(), state.clone()).await {
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
