use crate::{
    api,
    app::{PageErrors, PendingChatQuery, Route},
    components::monitor::*,
    pages::hardware::constants::{INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE},
};
use dioxus::prelude::*;
use dioxus_router::hooks::use_navigator;
use gloo_timers::future::TimeoutFuture;

#[derive(Clone, Default)]
struct RateLimitState {
    loading: bool,
    error: Option<String>,
    data: Option<api::RateLimitInfoResponse>,
    toggling: bool,
}

#[derive(Clone, PartialEq)]
enum ServerFilter {
    All,
    Search,
    Upload,
}

impl ServerFilter {
    fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Search => "Search :3010",
            Self::Upload => "Upload :3011",
        }
    }

    fn key(&self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Search => Some("search"),
            Self::Upload => Some("upload"),
        }
    }
}

#[component]
pub fn MonitorRateLimits() -> Element {
    let state = use_signal(|| RateLimitState {
        loading: true,
        ..Default::default()
    });
    let limiter_info_open = use_signal(|| false);
    let mut show_edit_thresholds = use_signal(|| false);
    let mut edit_search_qps = use_signal(|| String::new());
    let mut edit_search_burst = use_signal(|| String::new());
    let mut edit_upload_qps = use_signal(|| String::new());
    let mut edit_upload_burst = use_signal(|| String::new());
    let mut thresholds_saving = use_signal(|| false);
    let mut thresholds_error = use_signal(|| Option::<String>::None);
    let mut show_thresholds_info = use_signal(|| false);
    let mut pending_query = use_context::<Signal<PendingChatQuery>>();
    let navigator = use_navigator();

    {
        let mut state = state.clone();
        let mut page_errors = use_context::<Signal<PageErrors>>();
        use_future(move || async move {
            loop {
                match api::fetch_rate_limit_info().await {
                    Ok(resp) => {
                        // Skip while a toggle is in-flight to avoid overwriting the optimistic state.
                        if !state.read().toggling {
                            state.set(RateLimitState {
                                loading: false,
                                error: None,
                                data: Some(resp),
                                toggling: false,
                            });
                        }
                        page_errors.with_mut(|e| e.clear_error("rate_limits"));
                    }
                    Err(err) => {
                        let previous = state.read().data.clone();
                        state.set(RateLimitState {
                            loading: false,
                            error: Some(err.clone()),
                            data: previous,
                            toggling: false,
                        });
                        page_errors.with_mut(|errs| errs.set_error("rate_limits", &err));
                        let _ = api::log_frontend_error("rate_limits", &err).await;
                    }
                }
                TimeoutFuture::new(5_000).await;
            }
        });
    }

    let server_filter = use_signal(|| ServerFilter::All);

    let snapshot = state.read().clone();
    let drop_rows = snapshot.data.as_ref().map(|d| build_drop_rows(d, server_filter.read().key()));
    let drop_counts = snapshot.data.as_ref().map(|d| build_drop_counts(d, server_filter.read().key()));

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Rate Limits", None),
                ],
            }

            NavTabs { active: Route::MonitorRateLimits {} }

            // Server filter
            div { class: "flex gap-2",
                for filter in [ServerFilter::All, ServerFilter::Search, ServerFilter::Upload] {
                    {
                        let is_active = *server_filter.read() == filter;
                        let label = filter.label();
                        let mut sf = server_filter.clone();
                        let color = if is_active {
                            "bg-teal-700 text-white border-teal-500"
                        } else {
                            "bg-gray-800 text-gray-300 border-gray-600 hover:border-gray-400"
                        };
                        rsx! {
                            button {
                                class: "px-3 py-1 rounded border text-xs font-medium transition-colors {color}",
                                onclick: move |_| sf.set(filter.clone()),
                                "{label}"
                            }
                        }
                    }
                }
            }

            Panel { title: Some("Summary".into()), refresh: Some("5s".into()),
                if snapshot.loading {
                    div { class: "text-gray-400 text-sm", "Loading rate-limit stats…" }
                } else if let Some(err) = snapshot.error {
                    div { class: "text-red-400 text-sm", "Failed to load stats: {err}" }
                } else if let Some(data) = snapshot.data.clone() {
                    div { class: "flex flex-wrap gap-4",
                        // Limiter first
                        div { class: "rounded p-4 bg-gray-800 border border-gray-700",
                            div { class: "text-xs text-gray-400 mb-2", "Limiter" }
                            div { class: "flex items-center gap-3",
                                label { class: "relative inline-flex items-center cursor-pointer",
                                    input {
                                        r#type: "checkbox",
                                        class: "sr-only peer",
                                        checked: state.read().data.as_ref().map(|d| d.config.enabled).unwrap_or(false),
                                        disabled: state.read().toggling,
                                        onchange: {
                                            let state = state.clone();
                                            move |_| {
                                                let mut state = state.clone();
                                                let current_enabled = state.read().data.as_ref().map(|d| d.config.enabled).unwrap_or(false);
                                                let new_enabled = !current_enabled;

                                                // Optimistic update - change UI immediately
                                                {
                                                    let mut s = state.write();
                                                    s.toggling = true;
                                                    if let Some(ref mut d) = s.data {
                                                        d.config.enabled = new_enabled;
                                                        d.limiter_state.enabled = new_enabled;
                                                    }
                                                }

                                                spawn(async move {
                                                    match api::set_rate_limit_enabled(new_enabled).await {
                                                        Ok(resp) => {
                                                            // Confirm with server response
                                                            if let Some(ref mut d) = state.write().data {
                                                                d.config.enabled = resp.enabled;
                                                                d.limiter_state.enabled = resp.enabled;
                                                            }
                                                        }
                                                        Err(err) => {
                                                            // Revert on error
                                                            web_sys::console::error_1(&format!("Failed to toggle: {}", err).into());
                                                            if let Some(ref mut d) = state.write().data {
                                                                d.config.enabled = current_enabled;
                                                                d.limiter_state.enabled = current_enabled;
                                                            }
                                                        }
                                                    }
                                                    state.write().toggling = false;
                                                });
                                            }
                                        },
                                    }
                                    div {
                                        class: "w-11 h-6 bg-gray-600 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-teal-500",
                                    }
                                }
                                span { class: "text-lg font-bold text-gray-100",
                                    if state.read().data.as_ref().map(|d| d.config.enabled).unwrap_or(false) { "On" } else { "Off" }
                                }
                                if state.read().toggling {
                                    span { class: "text-xs text-gray-400", "Updating..." }
                                }
                                button {
                                    class: "text-[11px] px-2 py-1 rounded border border-slate-500 text-slate-200 hover:bg-slate-600/20 ml-2",
                                    onclick: {
                                        let mut limiter_info_open = limiter_info_open.clone();
                                        move |_| limiter_info_open.set(!limiter_info_open())
                                    },
                                    if limiter_info_open() { "Close" } else { "More info" }
                                }
                            }
                            if limiter_info_open() {
                                div { class: "mt-3 p-3 bg-gray-700/50 rounded text-[11px] text-slate-200 leading-relaxed space-y-2",
                                    p { class: "font-semibold text-slate-100", "Rate Limiter Toggle" }
                                    p { class: "text-slate-300",
                                        strong { "When ON: " }
                                        "The rate limiter actively monitors incoming requests. Clients that exceed the configured QPS (queries per second) or burst limits get their requests rejected with HTTP 429 (Too Many Requests)."
                                    }
                                    p { class: "text-slate-300",
                                        strong { "When OFF: " }
                                        "All rate limiting is disabled. Requests are allowed through without any throttling."
                                    }
                                    p { class: "text-slate-300",
                                        strong { "Use cases: " }
                                        "Turn OFF during development/testing, or temporarily during trusted high-traffic events. Keep ON in production to protect against abuse and ensure fair resource distribution."
                                    }
                                }
                            }
                        }
                        StatCard {
                            title: "Total Drops".into(),
                            value: data.total_drops.to_string().into(),
                            unit: None,
                            description: Some("When the rate limiter is active and a client exceeds
the allowed requests per second (QPS) or burst limit,
those excess requests are 'dropped' (rejected with HTTP 429).".into()),
                        }
                        StatCard {
                            title: "Active Keys".into(),
                            value: data.limiter_state.active_keys.to_string().into(),
                            unit: Some(format!("/{}", data.limiter_state.capacity).into()),
                            description: Some("The number of unique client IP addresses currently
being tracked by the rate limiter. Each IP gets its own
token bucket. When full, the least recent IP is removed.".into()),
                        }
                        // Drops by Route
                        div { class: "rounded p-4 bg-gray-800 border border-gray-700",
                            div { class: "flex items-center gap-3 mb-2",
                                div { class: "text-sm font-semibold text-gray-200", "Drops by Route" }
                                span { class: "text-xs text-white", "5s" }
                            }
                            div { class: "text-[10px] text-gray-400 mb-2", "Shows a breakdown of which API endpoints have had the most rate-limited (rejected) requests." }
                            if drop_rows
                                .as_ref()
                                .map(|rows| rows.is_empty())
                                .unwrap_or(true)
                            {
                                div { class: "text-gray-500 text-sm", "No drops recorded yet." }
                            } else {
                                DataTable {
                                    headers: vec!["Route".into(), "Drops".into()],
                                    rows: drop_rows.clone().unwrap_or_default(),
                                }
                            }
                        }
                    }

                    if let Some(values) = drop_counts.clone() {
                        if !values.is_empty() {
                            Panel { title: Some("Drop Trend".into()), refresh: Some("5s".into()),
                                ChartPlaceholder {
                                    values,
                                    label: "Drops per route (top 5)".to_string(),
                                    unit: " drops".to_string(),
                                }
                            }
                        }
                    }

                    Panel { title: Some("Configuration".into()), refresh: None,
                        div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                            div {
                                div { class: "flex items-center justify-between mb-2",
                                    div { class: "flex items-center gap-2",
                                        span { class: "text-sm font-semibold text-gray-200", "Thresholds" }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_thresholds_info.set(!show_thresholds_info()),
                                            svg {
                                                class: INFO_ICON_SVG_CLASS,
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
                                    button {
                                        class: "text-xs px-2 py-0.5 rounded border border-gray-600 text-gray-300 hover:bg-gray-700",
                                        onclick: {
                                            let sq = data.config.search_qps;
                                            let sb = data.config.search_burst;
                                            let uq = data.config.upload_qps;
                                            let ub = data.config.upload_burst;
                                            move |_| {
                                                edit_search_qps.set(format!("{:.2}", sq));
                                                edit_search_burst.set(format!("{:.0}", sb));
                                                edit_upload_qps.set(format!("{:.2}", uq));
                                                edit_upload_burst.set(format!("{:.0}", ub));
                                                thresholds_error.set(None);
                                                show_edit_thresholds.set(!show_edit_thresholds());
                                            }
                                        },
                                        if show_edit_thresholds() { "Cancel" } else { "Edit" }
                                    }
                                }
                                if show_thresholds_info() {
                                    div { class: "mb-3 p-3 bg-gray-700/50 rounded text-[11px] text-slate-200 leading-relaxed space-y-2",
                                        div { class: "flex items-start justify-between gap-2",
                                            p { class: "font-semibold text-slate-100", "Rate Limit Thresholds" }
                                            button {
                                                class: "text-gray-400 hover:text-white text-xs shrink-0",
                                                onclick: move |_| show_thresholds_info.set(false),
                                                "✕"
                                            }
                                        }
                                        p { class: "text-slate-300",
                                            strong { "QPS (queries per second): " }
                                            "Each IP gets its own "
                                            a {
                                                href: "https://en.wikipedia.org/wiki/Token_bucket",
                                                target: "_blank",
                                                rel: "noopener noreferrer",
                                                class: "text-teal-400 underline hover:text-teal-300",
                                                "token bucket"
                                            }
                                            ". Tokens refill continuously at the QPS rate — for example, QPS 2.0 means one new token every 500 ms."
                                        }
                                        p { class: "text-slate-300",
                                            strong { "Burst: " }
                                            "The maximum tokens a bucket can accumulate while idle. A burst of 10 lets a client send 10 requests back-to-back before the QPS ceiling kicks in."
                                        }
                                        p { class: "text-slate-300",
                                            strong { "Live edits: " }
                                            "Changes apply instantly to all in-flight requests — no restart required. The new values are also written to "
                                            code { class: "bg-gray-600 px-1 rounded", ".env.rate_limits" }
                                            " so they survive a backend restart."
                                        }
                                        div { class: "overflow-x-auto",
                                            table { class: "w-full text-[10px] border-collapse",
                                                thead {
                                                    tr { class: "border-b border-gray-500",
                                                        th { class: "text-left pb-1 pr-3 text-slate-100 font-semibold whitespace-nowrap", "Bucket" }
                                                        th { class: "text-left pb-1 pr-3 text-slate-100 font-semibold whitespace-nowrap", "Type of routes" }
                                                        th { class: "text-left pb-1 pr-3 text-slate-100 font-semibold whitespace-nowrap", "Examples" }
                                                        th { class: "text-left pb-1 pr-3 text-slate-100 font-semibold whitespace-nowrap", "Why this bucket exists" }
                                                        th { class: "text-left pb-1 text-slate-100 font-semibold whitespace-nowrap", "Typical workload characteristics" }
                                                    }
                                                }
                                                tbody {
                                                    tr { class: "border-b border-gray-700/50 align-top",
                                                        td { class: "py-1 pr-3 whitespace-nowrap",
                                                            button { class: "text-teal-400 underline hover:text-teal-300 text-left cursor-pointer",
                                                                onclick: move |_| { pending_query.write().0 = Some("Explain the upload rate-limit bucket: what routes does it cover and why does it exist?".into()); navigator.push(Route::Home {}); },
                                                                "Upload bucket"
                                                            }
                                                        }
                                                        td { class: "py-1 pr-3 text-slate-300", "Write-heavy, ingestion, mutation" }
                                                        td { class: "py-1 pr-3 text-slate-300", "POST /upload; POST /save_vectors; POST /reindex; POST /memory/store_rag; DELETE routes" }
                                                        td { class: "py-1 pr-3 text-slate-300", "Isolates heavy ingestion tasks so they cannot impact search latency" }
                                                        td { class: "py-1 text-slate-300", "High I/O; CPU-heavy; long-running; bursty; not latency-critical" }
                                                    }
                                                    tr { class: "border-b border-gray-700/50 align-top",
                                                        td { class: "py-1 pr-3 whitespace-nowrap",
                                                            button { class: "text-teal-400 underline hover:text-teal-300 text-left cursor-pointer",
                                                                onclick: move |_| { pending_query.write().0 = Some("Explain the search rate-limit bucket: what routes does it cover and why does it exist?".into()); navigator.push(Route::Home {}); },
                                                                "Search bucket"
                                                            }
                                                        }
                                                        td { class: "py-1 pr-3 text-slate-300", "Read-only, retrieval, query" }
                                                        td { class: "py-1 pr-3 text-slate-300", "GET /search; GET /query; GET /similarity; GET /memory/query; GET /chunks" }
                                                        td { class: "py-1 pr-3 text-slate-300", "Keeps search fast and predictable by separating it from ingestion" }
                                                        td { class: "py-1 text-slate-300", "Latency-sensitive; high QPS; lightweight; often cached; horizontally scaled" }
                                                    }
                                                    tr { class: "border-b border-gray-700/50 align-top",
                                                        td { class: "py-1 pr-3 whitespace-nowrap",
                                                            button { class: "text-teal-400 underline hover:text-teal-300 text-left cursor-pointer",
                                                                onclick: move |_| { pending_query.write().0 = Some("Why separate API rate-limit buckets for search and upload? What architectural problem does this solve?".into()); navigator.push(Route::Home {}); },
                                                                "Bucket separation"
                                                            }
                                                        }
                                                        td { class: "py-1 pr-3 text-slate-300", "Architectural isolation" }
                                                        td { class: "py-1 pr-3 text-slate-300", "Shared Actix worker pool (one thread per CPU core); separation enforced by rate-limit policy, not separate threads" }
                                                        td { class: "py-1 pr-3 text-slate-300", "Prevents ingestion from blocking retrieval" }
                                                        td { class: "py-1 text-slate-300", "Ensures stable search latency under load" }
                                                    }
                                                    tr { class: "border-b border-gray-700/50 align-top",
                                                        td { class: "py-1 pr-3 whitespace-nowrap",
                                                            button { class: "text-teal-400 underline hover:text-teal-300 text-left cursor-pointer",
                                                                onclick: move |_| { pending_query.write().0 = Some("What are the characteristics of the upload workload in a RAG system? Why is it CPU and I/O heavy?".into()); navigator.push(Route::Home {}); },
                                                                "Upload workload"
                                                            }
                                                        }
                                                        td { class: "py-1 pr-3 text-slate-300", "Ingestion tasks" }
                                                        td { class: "py-1 pr-3 text-slate-300", "PDF parsing; OCR; embedding generation; vector inserts; reindexing" }
                                                        td { class: "py-1 pr-3 text-slate-300", "These tasks are heavy and can starve search threads" }
                                                        td { class: "py-1 text-slate-300", "Must be queued, throttled, or isolated" }
                                                    }
                                                    tr { class: "border-b border-gray-700/50 align-top",
                                                        td { class: "py-1 pr-3 whitespace-nowrap",
                                                            button { class: "text-teal-400 underline hover:text-teal-300 text-left cursor-pointer",
                                                                onclick: move |_| { pending_query.write().0 = Some("What are the characteristics of the search workload in a RAG system? Why is latency so critical?".into()); navigator.push(Route::Home {}); },
                                                                "Search workload"
                                                            }
                                                        }
                                                        td { class: "py-1 pr-3 text-slate-300", "Retrieval tasks" }
                                                        td { class: "py-1 pr-3 text-slate-300", "Vector search; hybrid search; memory lookup; graph traversal" }
                                                        td { class: "py-1 pr-3 text-slate-300", "These tasks are the critical path for RAG latency" }
                                                        td { class: "py-1 text-slate-300", "Must remain fast, predictable, and isolated" }
                                                    }
                                                    tr { class: "border-b border-gray-700/50 align-top",
                                                        td { class: "py-1 pr-3 whitespace-nowrap",
                                                            button { class: "text-teal-400 underline hover:text-teal-300 text-left cursor-pointer",
                                                                onclick: move |_| { pending_query.write().0 = Some("How does LRU cache eviction behave differently for search vs upload workloads in a RAG system?".into()); navigator.push(Route::Home {}); },
                                                                "Eviction impact"
                                                            }
                                                        }
                                                        td { class: "py-1 pr-3 text-slate-300", "Cache behavior" }
                                                        td { class: "py-1 pr-3 text-slate-300", "LRU caches often used in search bucket" }
                                                        td { class: "py-1 pr-3 text-slate-300", "Search bucket benefits from hot-cache behavior" }
                                                        td { class: "py-1 text-slate-300", "Upload bucket rarely benefits from caching" }
                                                    }
                                                    tr { class: "border-b border-gray-700/50 align-top",
                                                        td { class: "py-1 pr-3 whitespace-nowrap",
                                                            button { class: "text-teal-400 underline hover:text-teal-300 text-left cursor-pointer",
                                                                onclick: move |_| { pending_query.write().0 = Some("How does the rate limiter decide which bucket a request goes into? What is the routing logic?".into()); navigator.push(Route::Home {}); },
                                                                "Routing logic"
                                                            }
                                                        }
                                                        td { class: "py-1 pr-3 text-slate-300", "How endpoints are classified" }
                                                        td { class: "py-1 pr-3 text-slate-300", "All write/mutate routes → upload bucket; everything else → search bucket" }
                                                        td { class: "py-1 pr-3 text-slate-300", "Simplifies routing and scaling" }
                                                        td { class: "py-1 text-slate-300", "Prevents accidental cross-contamination of workloads" }
                                                    }
                                                    tr { class: "align-top",
                                                        td { class: "py-1 pr-3 whitespace-nowrap",
                                                            button { class: "text-teal-400 underline hover:text-teal-300 text-left cursor-pointer",
                                                                onclick: move |_| { pending_query.write().0 = Some("Why do rate-limit buckets matter operationally? What goes wrong if search and upload share the same limit?".into()); navigator.push(Route::Home {}); },
                                                                "Operational reason"
                                                            }
                                                        }
                                                        td { class: "py-1 pr-3 text-slate-300", "Why buckets matter" }
                                                        td { class: "py-1 pr-3 text-slate-300", "Search must never wait for ingestion" }
                                                        td { class: "py-1 pr-3 text-slate-300", "Guarantees low-latency retrieval" }
                                                        td { class: "py-1 text-slate-300", "Improves reliability and throughput" }
                                                    }
                                                }
                                            }
                                        }
                                        p { class: "text-slate-300",
                                            strong { "Exempt routes: " }
                                            "Some prefixes bypass the limiter entirely — "
                                            code { class: "bg-gray-600 px-1 rounded", "/monitoring" }
                                            " (Prometheus scrape) and "
                                            code { class: "bg-gray-600 px-1 rounded", "/monitor" }
                                            " (this dashboard) are both exempt. Without that exemption the dashboard's 5-second polling would consume search-bucket tokens and could rate-limit itself when QPS is set low."
                                        }
                                    }
                                }
                                if show_edit_thresholds() {
                                    div { class: "flex flex-col gap-2",
                                        div { class: "grid grid-cols-2 gap-2 text-xs",
                                            label { class: "flex flex-col gap-0.5",
                                                span { class: "text-gray-400", "Search QPS" }
                                                input {
                                                    class: "input input-xs input-bordered bg-gray-700 text-gray-200 w-full",
                                                    r#type: "number", min: "0", step: "0.1",
                                                    value: "{edit_search_qps}",
                                                    oninput: move |e| edit_search_qps.set(e.value()),
                                                }
                                            }
                                            label { class: "flex flex-col gap-0.5",
                                                span { class: "text-gray-400", "Search Burst" }
                                                input {
                                                    class: "input input-xs input-bordered bg-gray-700 text-gray-200 w-full",
                                                    r#type: "number", min: "0", step: "1",
                                                    value: "{edit_search_burst}",
                                                    oninput: move |e| edit_search_burst.set(e.value()),
                                                }
                                            }
                                            label { class: "flex flex-col gap-0.5",
                                                span { class: "text-gray-400", "Upload QPS" }
                                                input {
                                                    class: "input input-xs input-bordered bg-gray-700 text-gray-200 w-full",
                                                    r#type: "number", min: "0", step: "0.1",
                                                    value: "{edit_upload_qps}",
                                                    oninput: move |e| edit_upload_qps.set(e.value()),
                                                }
                                            }
                                            label { class: "flex flex-col gap-0.5",
                                                span { class: "text-gray-400", "Upload Burst" }
                                                input {
                                                    class: "input input-xs input-bordered bg-gray-700 text-gray-200 w-full",
                                                    r#type: "number", min: "0", step: "1",
                                                    value: "{edit_upload_burst}",
                                                    oninput: move |e| edit_upload_burst.set(e.value()),
                                                }
                                            }
                                        }
                                        if let Some(err) = thresholds_error.read().as_ref() {
                                            p { class: "text-xs text-red-400", "{err}" }
                                        }
                                        button {
                                            class: "btn btn-sm w-full",
                                            style: "background-color:#7C2A02;border-color:#7C2A02;color:white;",
                                            disabled: thresholds_saving(),
                                            onclick: move |_| {
                                                let sq = edit_search_qps.read().parse::<f64>();
                                                let sb = edit_search_burst.read().parse::<f64>();
                                                let uq = edit_upload_qps.read().parse::<f64>();
                                                let ub = edit_upload_burst.read().parse::<f64>();
                                                match (sq, sb, uq, ub) {
                                                    (Ok(sq), Ok(sb), Ok(uq), Ok(ub)) => {
                                                        thresholds_saving.set(true);
                                                        thresholds_error.set(None);
                                                        spawn(async move {
                                                            match api::update_rate_limit_thresholds(sq, sb, uq, ub).await {
                                                                Ok(()) => {
                                                                    show_edit_thresholds.set(false);
                                                                }
                                                                Err(e) => thresholds_error.set(Some(e)),
                                                            }
                                                            thresholds_saving.set(false);
                                                        });
                                                    }
                                                    _ => thresholds_error.set(Some("Invalid number".into())),
                                                }
                                            },
                                            if thresholds_saving() { "Saving…" } else { "Save" }
                                        }
                                    }
                                } else {
                                    DataTable {
                                        headers: vec!["Parameter".into(), "Value".into()],
                                        rows: vec![
                                            vec!["Trust Proxy".into(), yes_no(data.config.trust_proxy)],
                                            vec!["Search QPS".into(), format_float(data.config.search_qps).into()],
                                            vec!["Search Burst".into(), format_float(data.config.search_burst).into()],
                                            vec!["Upload QPS".into(), format_float(data.config.upload_qps).into()],
                                            vec!["Upload Burst".into(), format_float(data.config.upload_burst).into()],
                                        ],
                                    }
                                }
                            }
                            DataTable {
                                headers: vec!["Exempt Prefixes".into()],
                                rows: data.config.exempt_prefixes.iter().map(|p| vec![p.clone().into()]).collect(),
                            }
                        }

                        div { class: "mt-4",
                            RowHeader {
                                title: "Custom Rules".into(),
                                description: Some("As loaded from RATE_LIMIT_ROUTES".into()),
                            }
                            if data.config.rules.is_empty() {
                                div { class: "text-gray-500 text-sm", "No custom rules configured." }
                            } else {
                                DataTable {
                                    headers: vec!["Pattern".into(), "Match".into(), "QPS".into(), "Burst".into(), "Label".into()],
                                    rows: data.config.rules.iter().map(|rule| {
                                        let pattern = rule.get("pattern").and_then(|v| v.as_str()).unwrap_or("-");
                                        let match_kind = rule.get("match_kind").and_then(|v| v.as_str()).unwrap_or("-");
                                        let qps = rule.get("qps").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                        let burst = rule.get("burst").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                        let label = rule.get("label").and_then(|v| v.as_str()).unwrap_or("-");
                                        vec![
                                            pattern.into(),
                                            match_kind.into(),
                                            format_float(qps).into(),
                                            format_float(burst).into(),
                                            label.into(),
                                        ]
                                    }).collect(),
                                }
                            }
                        }
                    }
                } else {
                    div { class: "text-gray-400 text-sm", "No data yet." }
                }
            }
        }
    }
}

fn yes_no(flag: bool) -> String {
    if flag {
        "Yes".into()
    } else {
        "No".into()
    }
}

fn format_float(value: f64) -> String {
    format!("{:.2}", value)
}

fn build_drop_rows(data: &api::RateLimitInfoResponse, server_filter: Option<&'static str>) -> Vec<Vec<String>> {
    if let Some(server) = server_filter {
        let mut entries: Vec<_> = data.drops_by_server_route
            .iter()
            .filter(|e| e.server == server)
            .collect();
        entries.sort_by(|a, b| b.drops.cmp(&a.drops));
        entries.into_iter()
            .map(|e| vec![e.route.clone(), e.drops.to_string()])
            .collect()
    } else {
        let mut entries = data.drops_by_route.clone();
        entries.sort_by(|a, b| b.drops.cmp(&a.drops));
        entries.into_iter()
            .map(|entry| vec![entry.route, entry.drops.to_string()])
            .collect()
    }
}

fn build_drop_counts(data: &api::RateLimitInfoResponse, server_filter: Option<&'static str>) -> Vec<f64> {
    if let Some(server) = server_filter {
        let mut entries: Vec<_> = data.drops_by_server_route
            .iter()
            .filter(|e| e.server == server)
            .collect();
        entries.sort_by(|a, b| b.drops.cmp(&a.drops));
        entries.into_iter()
            .take(5)
            .map(|e| e.drops as f64)
            .collect()
    } else {
        let mut entries = data.drops_by_route.clone();
        entries.sort_by(|a, b| b.drops.cmp(&a.drops));
        entries.into_iter()
            .take(5)
            .map(|entry| entry.drops as f64)
            .collect()
    }
}
