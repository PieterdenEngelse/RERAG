use crate::{
    api,
    app::{PageErrors, Route},
    components::monitor::*,
    pages::hardware::constants::{
        INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
        QUICK_ACTION_INFO_BUTTON_CLASS, QUICK_ACTION_INFO_ICON_CLASS,
    },
};
use dioxus::prelude::*;
use dioxus_router::hooks::use_navigator;
use gloo_timers::future::TimeoutFuture;

/// Format bytes into human-readable string
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[derive(Clone, Default)]
struct OverviewState {
    loading: bool,
    error: Option<String>,
    health_status: Option<String>,
    documents: Option<usize>,
    vectors: Option<usize>,
    request_rate_rps: Option<f64>,
    latency_p95_ms: Option<f64>,
    error_rate_percent: Option<f64>,
    // io_uring stats
    io_uring_backend: Option<String>,
    io_uring_bytes_read: Option<u64>,
    io_uring_errors: Option<u64>,
    // Neo4j status
    neo4j_enabled: Option<bool>,
    neo4j_connected: Option<bool>,
    // Redis status
    redis_enabled: Option<bool>,
    redis_connected: Option<bool>,
    // Docker status
    docker_available: Option<bool>,
    docker_containers_running: Option<usize>,
    docker_containers_total: Option<usize>,
}

#[component]
pub fn MonitorOverview() -> Element {
    let mut state = use_signal(OverviewState::default);
    let mut page_errors = use_context::<Signal<PageErrors>>();
    let navigator = use_navigator();

    // Action button states
    let mut reindex_loading = use_signal(|| false);
    let mut reindex_result = use_signal(|| Option::<String>::None);
    let mut cache_loading = use_signal(|| false);
    let mut cache_result = use_signal(|| Option::<String>::None);

    // Info modal states
    let mut show_reindex_info = use_signal(|| false);
    let mut show_cache_info = use_signal(|| false);
    let mut show_grafana_info = use_signal(|| false);

    // Main data fetch loop
    use_future(move || async move {
        loop {
            let health = api::health_check().await;
            let requests = api::fetch_requests_snapshot().await;
            let io_uring = api::fetch_io_uring_stats().await;
            let neo4j = api::fetch_neo4j_config().await;
            let docker = api::fetch_docker_status().await;
            let cache = api::fetch_cache_info().await;

            match (health, requests) {
                (Ok(h), Ok(r)) => {
                    let (io_backend, io_bytes, io_errors) = match &io_uring {
                        Ok(io) => (
                            Some(io.io_uring.backend.clone()),
                            Some(io.io_uring.stats.bytes_read),
                            Some(io.io_uring.stats.total_errors),
                        ),
                        Err(_) => (None, None, None),
                    };

                    let (neo4j_enabled, neo4j_connected) = match &neo4j {
                        Ok(n) => (Some(n.enabled), Some(n.connected)),
                        Err(_) => (None, None),
                    };

                    let (docker_available, docker_running, docker_total) = match &docker {
                        Ok(d) => {
                            let running =
                                d.containers.iter().filter(|c| c.state == "running").count();
                            (
                                Some(d.docker_available),
                                Some(running),
                                Some(d.containers.len()),
                            )
                        }
                        Err(_) => (None, None, None),
                    };

                    let (redis_enabled, redis_connected) = match &cache {
                        Ok(c) => (Some(c.redis.enabled), Some(c.redis.connected)),
                        Err(_) => (None, None),
                    };

                    state.set(OverviewState {
                        loading: false,
                        error: None,
                        health_status: Some(h.status),
                        documents: h.documents,
                        vectors: h.vectors,
                        request_rate_rps: Some(r.request_rate_rps),
                        latency_p95_ms: Some(r.latency_p95_ms),
                        error_rate_percent: Some(r.error_rate_percent),
                        io_uring_backend: io_backend,
                        io_uring_bytes_read: io_bytes,
                        io_uring_errors: io_errors,
                        neo4j_enabled,
                        neo4j_connected,
                        redis_enabled,
                        redis_connected,
                        docker_available,
                        docker_containers_running: docker_running,
                        docker_containers_total: docker_total,
                    });
                    page_errors.with_mut(|e| e.clear_error("overview"));
                }
                (Ok(h), Err(req_err)) => {
                    let err = format!("Failed to load request stats: {}", req_err);
                    let previous = state.read().clone();
                    state.set(OverviewState {
                        loading: false,
                        error: Some(err.clone()),
                        health_status: Some(h.status),
                        documents: h.documents,
                        vectors: h.vectors,
                        ..previous
                    });
                    page_errors.with_mut(|errs| errs.set_error("overview", &err));
                }
                (Err(err), _) => {
                    let previous = state.read().clone();
                    state.set(OverviewState {
                        loading: false,
                        error: Some(err.clone()),
                        ..previous
                    });
                    page_errors.with_mut(|errs| errs.set_error("overview", &err));
                }
            }

            TimeoutFuture::new(5_000).await;
        }
    });

    let snapshot = state.read().clone();

    // Reindex handler
    let on_reindex = move |_| {
        reindex_loading.set(true);
        reindex_result.set(None);
        spawn(async move {
            match api::reindex_async().await {
                Ok(resp) => {
                    reindex_result.set(Some(format!("✓ Started job {}", resp.job_id)));
                }
                Err(e) => {
                    reindex_result.set(Some(format!("✗ {}", e)));
                }
            }
            reindex_loading.set(false);
        });
    };

    // Clear cache handler
    let on_clear_cache = move |_| {
        cache_loading.set(true);
        cache_result.set(None);
        spawn(async move {
            match api::clear_cache().await {
                Ok(_) => {
                    cache_result.set(Some("✓ Cache cleared".to_string()));
                }
                Err(e) => {
                    cache_result.set(Some(format!("✗ {}", e)));
                }
            }
            cache_loading.set(false);
        });
    };

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                ],
            }

            NavTabs { active: Route::MonitorOverview {} }

            // System Health Panel
            Panel { title: Some("System Health".into()), refresh: Some("5s".into()),
                if snapshot.loading {
                    div { class: "text-gray-400 text-sm", "Loading health status…" }
                } else if let Some(err) = snapshot.error.clone() {
                    div { class: "text-red-400 text-sm", "Failed to load health: {err}" }
                } else {
                    div { class: "grid grid-cols-2 md:grid-cols-4 lg:grid-cols-7 gap-4",
                        // API Health
                        HealthCard {
                            name: "API".into(),
                            status: snapshot.health_status.clone().unwrap_or_else(|| "unknown".into()).into(),
                            detail: Some("Actix".into()),
                            info: Some("The Actix Web backend server. Shows 'Healthy' when responding to health checks, 'Busy' when processing heavy workloads, 'Degraded' when some services are unavailable.".into()),
                        }
                        // Documents
                        HealthCard {
                            name: "Documents".into(),
                            status: snapshot.documents.map(|d| d.to_string()).unwrap_or_else(|| "--".into()).into(),
                            detail: Some("Indexed".into()),
                            info: Some("Total number of documents indexed in Tantivy. Each uploaded file becomes one or more documents after chunking. Use 'Trigger Reindex' to rebuild the index.".into()),
                        }
                        // Vectors
                        HealthCard {
                            name: "Vectors".into(),
                            status: snapshot.vectors.map(|v| v.to_string()).unwrap_or_else(|| "--".into()).into(),
                            detail: Some("Embeddings".into()),
                            info: Some("Number of embedding vectors stored. Each document chunk gets converted to a vector for semantic search. Vectors enable similarity-based retrieval.".into()),
                        }
                        // File I/O
                        HealthCard {
                            name: "File I/O".into(),
                            status: if snapshot.io_uring_backend.is_none() {
                                "Unknown".into()
                            } else if snapshot.io_uring_errors.unwrap_or(0) > 0 {
                                "Unhealthy".into()
                            } else if snapshot.io_uring_backend.as_deref() == Some("io_uring") {
                                "Healthy".into()
                            } else {
                                "Degraded".into()
                            },
                            detail: Some({
                                if snapshot.io_uring_backend.is_none() {
                                    "API unreachable".to_string()
                                } else {
                                    format!(
                                        "{} | {}",
                                        snapshot.io_uring_backend.clone().unwrap_or_else(|| "--".into()),
                                        format_bytes(snapshot.io_uring_bytes_read.unwrap_or(0))
                                    )
                                }
                            }.into()),
                            info: Some("Async file I/O backend. 'io_uring' (Linux 5.1+) provides 2-3x faster reads. Falls back to 'tokio::fs' on older systems. Configure in Settings → io_uring.".into()),
                        }
                        // Neo4j
                        HealthCard {
                            name: "Neo4j".into(),
                            status: match (snapshot.neo4j_enabled, snapshot.neo4j_connected) {
                                (Some(true), Some(true)) => "Healthy".into(),
                                (Some(true), Some(false)) => "Unhealthy".into(),
                                (Some(false), _) => "Disabled".into(),
                                _ => "Unknown".into(),
                            },
                            detail: Some(match (snapshot.neo4j_enabled, snapshot.neo4j_connected) {
                                (Some(true), Some(true)) => "Connected".to_string(),
                                (Some(true), Some(false)) => "Not connected".to_string(),
                                (Some(false), _) => "Not enabled".to_string(),
                                _ => "Checking...".to_string(),
                            }.into()),
                            info: Some("Neo4j knowledge graph database for GraphRAG. Stores entities and relationships extracted from documents. Enable in Settings → Neo4j. Runs on port 7474 (browser) and 7687 (bolt).".into()),
                        }
                        // Redis
                        HealthCard {
                            name: "Redis".into(),
                            status: match (snapshot.redis_enabled, snapshot.redis_connected) {
                                (Some(false), _) => "Disabled".into(),
                                (Some(true), Some(true)) => "Healthy".into(),
                                (Some(true), Some(false)) => "Unhealthy".into(),
                                _ => "Unknown".into(),
                            },
                            detail: Some(match (snapshot.redis_enabled, snapshot.redis_connected) {
                                (Some(false), _) => "Not configured".to_string(),
                                (Some(true), Some(true)) => "L3 Cache".to_string(),
                                (Some(true), Some(false)) => "Disconnected".to_string(),
                                _ => "Checking...".to_string(),
                            }.into()),
                            info: Some("Redis serves as L3 cache for search results. L1 (in-memory) and L2 (disk) caches are checked first. Redis provides shared caching across restarts. Runs on port 6379. Set REDIS_ENABLED=true to enable.".into()),
                        }
                        // Docker
                        HealthCard {
                            name: "Docker".into(),
                            status: match (snapshot.docker_available, snapshot.docker_containers_running, snapshot.docker_containers_total) {
                                (Some(true), Some(r), Some(t)) if r == t && t > 0 => "Healthy".into(),
                                (Some(true), Some(r), Some(t)) if r < t => "Degraded".into(),
                                (Some(true), Some(0), _) => "Unhealthy".into(),
                                (Some(false), _, _) => "Unavailable".into(),
                                _ => "Unknown".into(),
                            },
                            detail: Some(match (snapshot.docker_available, snapshot.docker_containers_running, snapshot.docker_containers_total) {
                                (Some(true), Some(r), Some(t)) => format!("{}/{} running", r, t),
                                (Some(false), _, _) => "Not accessible".to_string(),
                                _ => "Checking...".to_string(),
                            }.into()),
                            info: Some("Docker containers running ag infrastructure: Neo4j, Redis, Prometheus, Grafana, Loki, Tempo, OTel Collector. Start with 'docker compose up -d'. See Monitor → Docker for details.".into()),
                        }
                    }
                }
            }

            // Key Metrics
            RowHeader {
                title: "Key Metrics".into(),
                description: Some("Live request stats refreshed every 5s. Click a card to see details.".into()),
            }
            div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                // Clickable stat cards that navigate to detail pages
                div {
                    class: "cursor-pointer hover:opacity-80 transition-opacity",
                    onclick: move |_| { navigator.push(Route::MonitorRequests {}); },
                    StatCard {
                        title: "Requests/sec".into(),
                        value: snapshot
                            .request_rate_rps
                            .map(|v| format!("{:.2}", v))
                            .unwrap_or_else(|| "--".into())
                            .into(),
                        unit: Some("req/s".into()),
                        info_tooltip: Some("Number of HTTP requests per second hitting the backend. Includes all endpoints: search, upload, health checks, etc. Click to see detailed request breakdown.".into()),
                    }
                }
                div {
                    class: "cursor-pointer hover:opacity-80 transition-opacity",
                    onclick: move |_| { navigator.push(Route::MonitorRequests {}); },
                    StatCard {
                        title: "p95 Latency".into(),
                        value: snapshot
                            .latency_p95_ms
                            .map(|v| format!("{:.1}", v))
                            .unwrap_or_else(|| "--".into())
                            .into(),
                        unit: Some("ms".into()),
                        info_tooltip: Some("95th percentile response time. 95% of requests complete faster than this. High values indicate slow queries or system load. Target: <100ms for search, <500ms for uploads.".into()),
                    }
                }
                div {
                    class: "cursor-pointer hover:opacity-80 transition-opacity",
                    onclick: move |_| { navigator.push(Route::MonitorLogs {}); },
                    StatCard {
                        title: "Error Rate".into(),
                        value: snapshot
                            .error_rate_percent
                            .map(|v| format!("{:.2}", v))
                            .unwrap_or_else(|| "--".into())
                            .into(),
                        unit: Some("%".into()),
                        info_tooltip: Some("Percentage of requests returning 4xx/5xx errors. Should be <1% in normal operation. Click to see error logs. Common causes: rate limiting, invalid queries, backend issues.".into()),
                    }
                }
            }

            // Quick Actions
            RowHeader {
                title: "Quick Actions".into(),
                description: None,
            }
            div { class: "flex flex-wrap gap-3 items-center",
                // Reindex button with info
                div { class: "flex items-center gap-2",
                    button {
                        class: "px-4 py-2 rounded text-white transition-colors",
                        style: "background-color: #1D6B9A;",
                        disabled: reindex_loading(),
                        onclick: on_reindex,
                        if reindex_loading() { "Reindexing..." } else { "Trigger Reindex" }
                    }
                    button {
                        class: QUICK_ACTION_INFO_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        onclick: move |_| show_reindex_info.set(true),
                        InfoIcon {}
                    }
                }
                if let Some(result) = reindex_result() {
                    span {
                        class: if result.starts_with("✓") { "text-green-400 text-sm" } else { "text-red-400 text-sm" },
                        "{result}"
                    }
                }

                // Clear Cache button with info
                div { class: "flex items-center gap-2",
                    button {
                        class: "px-4 py-2 rounded bg-gray-700 text-gray-200 hover:bg-gray-600 transition-colors",
                        disabled: cache_loading(),
                        onclick: on_clear_cache,
                        if cache_loading() { "Clearing..." } else { "Clear Cache" }
                    }
                    button {
                        class: QUICK_ACTION_INFO_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        onclick: move |_| show_cache_info.set(true),
                        InfoIcon {}
                    }
                }
                if let Some(result) = cache_result() {
                    span {
                        class: if result.starts_with("✓") { "text-green-400 text-sm" } else { "text-red-400 text-sm" },
                        "{result}"
                    }
                }

                // View Grafana button with info
                div { class: "flex items-center gap-2",
                    a {
                        href: "http://localhost:3001",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        class: "px-4 py-2 rounded border border-teal-400 text-teal-400 hover:bg-teal-500/10 transition-colors inline-block",
                        "View Grafana ↗"
                    }
                    button {
                        class: QUICK_ACTION_INFO_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        onclick: move |_| show_grafana_info.set(true),
                        InfoIcon {}
                    }
                }
            }

            // Info modals for buttons
            if show_reindex_info() {
                InfoModal {
                    title: "Trigger Reindex",
                    content: "Starts an asynchronous reindexing job that:\n\n• Scans the documents directory for new/changed files\n• Chunks documents using the configured chunker (fixed/semantic)\n• Generates embedding vectors for each chunk\n• Updates the Tantivy full-text index\n• Rebuilds the vector store\n\nReindexing runs in the background. You can continue using the app while it runs. Check Monitor → Index for progress.",
                    on_close: move || show_reindex_info.set(false),
                }
            }

            if show_cache_info() {
                InfoModal {
                    title: "Clear Cache",
                    content: "Clears all search result caches:\n\n• L1 Cache: In-memory cache (fastest, lost on restart)\n• L2 Cache: Disk-based cache (persists across restarts)\n\nNote: Redis (L3) cache is not cleared by this action.\n\nUse this when:\n• Search results seem stale after document updates\n• Testing cache performance\n• Debugging cache-related issues\n\nCache will rebuild automatically as new searches are performed.",
                    on_close: move || show_cache_info.set(false),
                }
            }

            if show_grafana_info() {
                InfoModal {
                    title: "View Grafana",
                    content: "Opens Grafana dashboards at http://localhost:3001\n\nGrafana provides:\n• Time-series charts for latency, throughput, errors\n• Log aggregation via Loki\n• Distributed tracing via Tempo\n• Custom alerting rules\n\nDefault credentials: admin / admin\n\nPre-built dashboards:\n• AG – Latency & Rate\n• AG Logs (Loki)\n• Trace Alerting\n\nNote: Grafana runs on port 3001 (not 3000) to avoid conflict with Qodo.",
                    on_close: move || show_grafana_info.set(false),
                }
            }
        }
    }
}

/// Standard info icon from AGENTS.md
#[component]
fn InfoIcon() -> Element {
    rsx! {
        svg {
            class: QUICK_ACTION_INFO_ICON_CLASS,
            view_box: "0 0 20 20",
            fill: "none",
            stroke: "currentColor",
            circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
            line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
            circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
        }
    }
}

/// Info modal component
#[component]
fn InfoModal(title: &'static str, content: &'static str, on_close: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| on_close.call(()),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[90vw] max-w-lg max-h-[90vh] overflow-y-auto shadow-xl",
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
                    class: "text-sm text-gray-300 whitespace-pre-line leading-relaxed",
                    "{content}"
                }
            }
        }
    }
}
