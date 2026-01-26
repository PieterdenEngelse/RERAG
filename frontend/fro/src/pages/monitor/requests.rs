use crate::{
    api,
    app::{PageErrors, Route},
    components::monitor::*,
};
use chrono::TimeZone;
use dioxus::prelude::*;
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use urlencoding::encode;
use wasm_bindgen_futures::JsFuture;
use web_sys::console;

const REQUEST_METRICS_COMMAND: &str =
    "curl http://127.0.0.1:3010/monitoring/metrics | grep http_requests_total";
const JOURNALCTL_COMMAND: &str = "journalctl -u ag.service -n 200 -f";
const TAIL_LOGS_COMMAND: &str = "tail -f logs/ag.log";

#[derive(Clone)]
struct RequestsState {
    loading: bool,
    error: Option<String>,
    request_rate_rps: f64,
    latency_p95_ms: f64,
    error_rate_percent: f64,
    latency_breakdown: api::LatencyBreakdown,
    status_breakdown: api::StatusBreakdown,
    points: Vec<api::RequestChartPoint>,
    busy: bool,
}

impl Default for RequestsState {
    fn default() -> Self {
        Self {
            loading: true,
            error: None,
            request_rate_rps: 0.0,
            latency_p95_ms: 0.0,
            error_rate_percent: 0.0,
            latency_breakdown: api::LatencyBreakdown::default(),
            status_breakdown: api::StatusBreakdown::default(),
            points: Vec::new(),
            busy: false,
        }
    }
}

#[component]
pub fn MonitorRequests() -> Element {
    let state = use_signal(RequestsState::default);

    {
        let mut state = state.clone();
        let mut page_errors = use_context::<Signal<PageErrors>>();
        use_future(move || async move {
            loop {
                match api::fetch_requests_snapshot().await {
                    Ok(snapshot) => {
                        let busy = state.read().busy;
                        state.set(RequestsState {
                            loading: false,
                            error: None,
                            request_rate_rps: snapshot.request_rate_rps,
                            latency_p95_ms: snapshot.latency_p95_ms,
                            error_rate_percent: snapshot.error_rate_percent,
                            latency_breakdown: snapshot.latency_breakdown,
                            status_breakdown: snapshot.status_breakdown,
                            points: snapshot.points,
                            busy,
                        });
                        page_errors.with_mut(|e| e.clear_error("requests"));
                    }
                    Err(e) => {
                        let previous = state.read().clone();
                        state.set(RequestsState {
                            loading: false,
                            error: Some(e.clone()),
                            ..previous
                        });
                        page_errors.with_mut(|errs| errs.set_error("requests", &e));
                        let _ = api::log_frontend_error("requests", &e).await;
                    }
                }

                TimeoutFuture::new(5_000).await;
            }
        });
    }

    let snapshot = state.read().clone();
    let request_counts = build_request_counts(&snapshot.points);
    let mut troubleshooting_open = use_signal(|| false);

    let trigger_sample_traffic = {
        let state = state.clone();
        move |_| {
            if state.read().busy {
                return;
            }
            let mut state = state.clone();
            state.write().busy = true;
            spawn(async move {
                if let Err(err) = run_sample_traffic().await {
                    web_sys::console::error_1(&format!("Sample traffic failed: {}", err).into());
                }
                state.write().busy = false;
            });
        }
    };

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Requests", None),
                ],
            }

            NavTabs { active: Route::MonitorRequests {} }

            Panel { title: Some("Summary".into()), refresh: Some("5s".into()),
                if snapshot.loading {
                    div { class: "text-gray-400 text-sm", "Loading latest stats..." }
                } else if let Some(err) = &snapshot.error {
                    div { class: "text-red-400 text-sm", "Failed to load stats: {err}" }
                } else {
                    div { class: "text-gray-300 text-sm leading-relaxed mb-1",
                        span { class: "font-semibold text-gray-200", "How to read this:" }
                        " Request rate shows how many HTTP operations hit the backend each second, latency p95 is the slowest 5% of requests, and error rate counts any 4xx/5xx responses. If request rate spikes while latency or errors climb, investigate upstream traffic or degraded dependencies."
                    }
                    details { class: "bg-slate-800/70 rounded border border-slate-700 p-3 text-xs text-slate-200 mb-3", open: troubleshooting_open(),
                        summary { class: "flex items-center gap-2 cursor-pointer text-teal-300 font-semibold",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                evt.prevent_default();
                                troubleshooting_open.set(!troubleshooting_open());
                            },
                            span { "Troubleshooting checklist" }
                            if troubleshooting_open() {
                                span { class: "ml-auto text-slate-400 text-xl leading-none", "×" }
                            }
                        }
                        div { class: "mt-2 grid grid-cols-1 md:grid-cols-2 gap-4 text-slate-100 leading-relaxed",
                            div { class: "space-y-3",
                                div {
                                    p { class: "font-semibold text-slate-50", "1. Confirm backend load" }
                                    ul { class: "list-disc ml-5 space-y-1",
                                        li { "Use the Requests tab’s chart plus the Overview tab to make sure the spike isn’t just a single blip." }
                                        li {
                                            "Shell command: "
                                            div { class: "bg-slate-900/60 border border-slate-700 px-2 py-1 mt-1 text-[10px] flex items-center justify-between gap-3",
                                                code { class: "whitespace-nowrap overflow-x-auto", {REQUEST_METRICS_COMMAND} }
                                                button {
                                                    class: "text-[10px] px-2 py-1 rounded bg-slate-800 border border-slate-600 text-slate-200 hover:text-white hover:border-slate-400",
                                                    onclick: move |_| copy_command_to_clipboard(REQUEST_METRICS_COMMAND),
                                                    "Copy"
                                                }
                                            }
                                            " then inspect labels (method/status/route) for unusual traffic."
                                        }
                                    }
                                }
                                div {
                                    p { class: "font-semibold text-slate-50", "2. Trace the source of traffic" }
                                    ul { class: "list-disc ml-5 space-y-1",
                                        li { "Review ingress or load-balancer logs (Nginx/Envoy/etc.) to see which clients or services are driving the surge." }
                                        li { "Pause any synthetic traffic (e.g., the \"Generate sample traffic\" helper) to rule out internal noise." }
                                    }
                                }
                                div {
                                    p { class: "font-semibold text-slate-50", "3. Inspect downstream dependencies" }
                                    ul { class: "list-disc ml-5 space-y-1",
                                        li { "Jump to the Cache, Index, or Rate Limit tabs for symptoms like low hit rate or rate-limit drops." }
                                        li {
                                            "Check external services (Redis, vector DB, embedding providers). For Redis specifically, run "
                                            code { "redis-cli INFO" }
                                            " or review its dashboard."
                                        }
                                    }
                                }
                            }
                            div { class: "space-y-3",
                                div {
                                    p { class: "font-semibold text-slate-50", "4. Peek at logs for rollback hints" }
                                    ul { class: "list-disc ml-5 space-y-1",
                                        li { "Filter the Logs tab around the spike timeframe for WARN/ERROR rows." }
                                        li {
                                            "On the host, tail directly with "
                                            span { class: "inline-flex items-center gap-2 bg-slate-900/60 border border-slate-700 px-2 py-1 text-[10px] rounded",
                                                code { class: "whitespace-nowrap", {JOURNALCTL_COMMAND} }
                                                button {
                                                    class: "text-[10px] px-2 py-1 rounded bg-slate-800 border border-slate-600 text-slate-200 hover:text-white hover:border-slate-400",
                                                    onclick: move |_| copy_command_to_clipboard(JOURNALCTL_COMMAND),
                                                    "Copy"
                                                }
                                            }
                                            span { class: "mx-1 text-slate-400", "or" }
                                            span { class: "inline-flex items-center gap-2 bg-slate-900/60 border border-slate-700 px-2 py-1 text-[10px] rounded",
                                                code { class: "whitespace-nowrap", {TAIL_LOGS_COMMAND} }
                                                button {
                                                    class: "text-[10px] px-2 py-1 rounded bg-slate-800 border border-slate-600 text-slate-200 hover:text-white hover:border-slate-400",
                                                    onclick: move |_| copy_command_to_clipboard(TAIL_LOGS_COMMAND),
                                                    "Copy"
                                                }
                                            }
                                            " to spot timeouts or dependency failures."
                                        }
                                    }
                                }
                                div {
                                    p { class: "font-semibold text-slate-50", "5. Use tracing / metrics dashboards" }
                                    ul { class: "list-disc ml-5 space-y-1",
                                        li { "If OpenTelemetry is enabled, inspect Tempo/Grafana traces to see which spans (DB, embedding, cache) accumulate latency." }
                                        li {
                                            "Plot histograms like "
                                            code { "http_request_duration_ms_bucket" }
                                            " in Grafana/Prometheus, along with DB pool metrics, to find bottlenecks."
                                        }
                                    }
                                }
                                div {
                                    p { class: "font-semibold text-slate-50", "6. Engage upstream teams" }
                                    ul { class: "list-disc ml-5 space-y-1",
                                        li { "If another internal service owns the surge, coordinate with them and suggest rate limiting/backoff if it’s misbehaving." }
                                        li { "Document the root cause—client load, dependency slowness, or network failures—so you can right-size caching or throttling afterward." }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "grid grid-cols-1 gap-4 md:grid-cols-3",
                        StatCard {
                            title: "Request Rate".into(),
                            value: format!("{:.2}", snapshot.request_rate_rps).into(),
                            unit: Some("req/s".into()),
                        }
                        StatCard {
                            title: "Latency p95".into(),
                            value: format!("{:.1}", snapshot.latency_p95_ms).into(),
                            unit: Some("ms".into()),
                        }
                        StatCard {
                            title: "Error Rate".into(),
                            value: format!("{:.2}", snapshot.error_rate_percent).into(),
                            unit: Some("%".into()),
                        }
                    }

                    div { class: "grid grid-cols-1 md:grid-cols-2 gap-4 mt-4",
                        Panel { title: Some("Latency Breakdown".into()), refresh: None,
                            DataTable {
                                headers: vec!["Percentile".into(), "Latency".into()],
                                rows: vec![
                                    vec!["p50".into(), format!("{:.1} ms", snapshot.latency_breakdown.p50_ms)],
                                    vec!["p95".into(), format!("{:.1} ms", snapshot.latency_breakdown.p95_ms)],
                                    vec!["p99".into(), format!("{:.1} ms", snapshot.latency_breakdown.p99_ms)],
                                ],
                            }
                        }
                        Panel { title: Some("Status Breakdown".into()), refresh: None,
                            DataTable {
                                headers: vec!["Status".into(), "Percentage".into()],
                                rows: vec![
                                    vec!["2xx".into(), format!("{:.2}%", snapshot.status_breakdown.success_rate)],
                                    vec!["4xx".into(), format!("{:.2}%", snapshot.status_breakdown.client_error_rate)],
                                    vec!["5xx".into(), format!("{:.2}%", snapshot.status_breakdown.server_error_rate)],
                                ],
                            }
                        }
                    }

                    div { class: "flex flex-wrap gap-2 text-xs mt-4",
                        button {
                            class: "px-3 py-1 rounded bg-teal-600 text-white disabled:opacity-40",
                            disabled: snapshot.busy,
                            onclick: trigger_sample_traffic.clone(),
                            if snapshot.busy {
                                "Running sample traffic…"
                            } else {
                                "Generate sample traffic"
                            }
                        }
                    }
                }
            }

            Panel { title: Some("Request Volume".into()), refresh: Some("5s".into()),
                if request_counts.is_empty() {
                    div { class: "text-gray-500 text-sm", "No recent samples yet." }
                } else {
                    ChartPlaceholder {
                        values: request_counts.clone(),
                        label: "Requests per second".to_string(),
                        unit: " req".to_string(),
                    }
                }
            }

            Panel { title: Some("Raw Samples".into()), refresh: Some("5s".into()),
                DataTable {
                    headers: vec!["Timestamp".into(), "Latency".into()],
                    rows: snapshot.points.iter().rev().take(5).map(|p| vec![
                        format_timestamp(p.ts),
                        format!("{:.1} ms", p.latency_ms),
                    ]).collect(),
                }
            }
        }
    }
}

fn build_request_counts(points: &[api::RequestChartPoint]) -> Vec<f64> {
    if points.is_empty() {
        return Vec::new();
    }

    let mut counts = Vec::new();
    let mut current_ts = points[0].ts;
    let mut current_count = 0_u32;

    for point in points {
        if point.ts == current_ts {
            current_count += 1;
        } else {
            counts.push(current_count as f64);
            current_ts = point.ts;
            current_count = 1;
        }
    }
    counts.push(current_count as f64);

    const MAX_BUCKETS: usize = 30;
    if counts.len() > MAX_BUCKETS {
        counts[counts.len() - MAX_BUCKETS..].to_vec()
    } else {
        counts
    }
}

async fn run_sample_traffic() -> Result<(), String> {
    for i in 0..3 {
        let query = format!("monitor-test-{}", i);
        let url = format!(
            "{}/search?q={}",
            api::resolve_api_base_url(),
            encode(&query)
        );
        Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
    }
    Ok(())
}

fn copy_command_to_clipboard(command: &str) {
    if let Some(window) = web_sys::window() {
        let navigator = window.navigator();
        let clipboard = navigator.clipboard();
        let cmd = command.to_string();
        spawn(async move {
            let promise = clipboard.write_text(&cmd);
            if let Err(err) = JsFuture::from(promise).await {
                console::warn_1(&err);
            }
        });
    } else {
        console::warn_1(&"window unavailable for clipboard copy".into());
    }
}

fn format_timestamp(ts: i64) -> String {
    chrono::Utc
        .timestamp_opt(ts, 0)
        .single()
        .unwrap_or_else(|| chrono::Utc.timestamp_opt(0, 0).unwrap())
        .naive_utc()
        .to_string()
}
