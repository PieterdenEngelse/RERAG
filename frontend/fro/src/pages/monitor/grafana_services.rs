use crate::app::Route;
use crate::components::monitor::{Breadcrumb, BreadcrumbItem, NavTabs, Panel};
use crate::pages::hardware::constants::{
    INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
};
use dioxus::prelude::*;

const DEFAULT_LIMIT: usize = 200;

// (unit, description, scope)
// scope: "system" | "user"
const SERVICES: &[(&str, &str, &str)] = &[
    (
        "prometheus-node-exporter.service",
        "Host system metrics",
        "system",
    ),
    ("vector.service", "Log shipper → Loki", "user"),
    ("alertmanager.service", "Alert routing", "system"),
];

async fn fetch_logs(unit: &str, scope: &str, limit: usize) -> Result<(String, usize), String> {
    let url = format!(
        "http://127.0.0.1:3010/monitoring/systemd/logs?unit={}&limit={}&scope={}",
        unit, limit, scope
    );
    let resp = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
    let content = json["content"].as_str().unwrap_or("").to_string();
    let total = json["total_lines"].as_u64().unwrap_or(0) as usize;
    Ok((content, total))
}

#[component]
pub fn MonitorGrafanaServices() -> Element {
    let mut show_info = use_signal(|| false);
    let mut expanded: Signal<Option<usize>> = use_signal(|| None);
    let mut log_content = use_signal(String::new);
    let mut log_total = use_signal(|| 0usize);
    let mut log_loading = use_signal(|| false);
    let mut log_error: Signal<Option<String>> = use_signal(|| None);
    let mut level_filter = use_signal(|| "all".to_string());

    let expanded_for_future = expanded;
    use_future(move || async move {
        loop {
            if let Some(idx) = expanded_for_future() {
                let (unit, _, scope) = SERVICES[idx];
                log_loading.set(true);
                log_error.set(None);
                match fetch_logs(unit, scope, DEFAULT_LIMIT).await {
                    Ok((content, total)) => {
                        log_content.set(content);
                        log_total.set(total);
                    }
                    Err(e) => {
                        log_error.set(Some(e));
                    }
                }
                log_loading.set(false);
            }
            gloo_timers::future::TimeoutFuture::new(5000).await;
        }
    });

    let filtered_content = {
        let raw = log_content();
        let filter = level_filter();
        if filter == "all" {
            raw
        } else {
            raw.lines()
                .filter(|l| l.to_uppercase().contains(&filter.to_uppercase()))
                .collect::<Vec<_>>()
                .join("\n")
        }
    };

    rsx! {
        div { class: "p-4 space-y-4",
            NavTabs { active: Route::MonitorGrafanaServices {} }

            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Systemd", Some(Route::MonitorAgSystemd {})),
                    BreadcrumbItem::new("Grafana Services", None),
                ]
            }

            Panel { title: None,
                div { class: "flex items-center gap-2 mb-3",
                    h3 { class: "text-sm font-semibold text-gray-200", "Grafana — Required Services" }
                    button {
                        class: PARAM_ICON_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        title: "What are these services?",
                        onclick: move |_| show_info.set(!show_info()),
                        svg {
                            class: INFO_ICON_SVG_CLASS,
                            view_box: "0 0 20 20",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.5",
                            circle { cx: "10", cy: "10", r: "9" }
                            line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                            circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                        }
                    }
                }

                if show_info() {
                    div { class: "bg-gray-700 rounded p-3 text-xs text-gray-300 mb-3 space-y-3",
                        div { class: "flex items-center justify-between mb-1",
                            p { class: "font-semibold text-gray-100", "Why these services run native, not in Docker" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-lg font-bold leading-none",
                                onclick: move |_| show_info.set(false),
                                "×"
                            }
                        }
                        p { "Grafana itself runs as a Docker container. To function fully it needs data from multiple sources, each running differently." }
                        p { class: "font-semibold", style: "color: #00BCD4;", "Native binaries (managed by systemd)" }
                        p { "These run directly on the host and are the data sources Grafana depends on." }
                        p { class: "font-semibold text-gray-200", "prometheus-node-exporter" }
                        p { "Exposes host system metrics — CPU, memory, disk, network — by reading directly from /proc and /sys. Running it in a container requires mounting large parts of the host filesystem and results in less accurate metrics. The standard approach everywhere is to run it as a native binary." }
                        p { class: "font-semibold text-gray-200", "vector" }
                        p { "Ships logs from the systemd journal to Loki. It reads from /run/log/journal directly. Containerizing it requires privileged access and journal socket mounts. Running it as a user service with direct journal access is simpler and more reliable." }
                        p { class: "font-semibold text-gray-200", "alertmanager" }
                        p { "Handles alert routing from Prometheus. Runs as a native binary on the host." }
                        p { class: "font-semibold pt-1", style: "color: #00BCD4;", "Docker containers (managed by docker compose)" }
                        p { "ag-grafana, ag-prometheus, ag-loki, ag-tempo, and ag-otel all run as Docker containers. They are self-contained network services that communicate over HTTP and gRPC. Docker manages their startup order and inter-dependencies." }
                        p { class: "font-semibold text-gray-100 pt-1", "The rule of thumb" }
                        p { "If something needs deep host access — filesystem, journal, kernel interfaces — run it native with systemd. If it is a self-contained service that communicates over the network, run it in Docker." }
                        p { class: "font-semibold text-gray-100 pt-1", "Boot Order" }
                        div { class: "font-mono space-y-0.5",
                            div { "1. docker.service" }
                            div { "2. ag.service" }
                            div { "3. prometheus-node-exporter.service" }
                            div { "4. vector.service  (user — starts on login)" }
                            div { "5. docker compose up -d  →  all containers" }
                        }
                        button {
                            class: "w-full mt-2 py-1.5 rounded text-xs font-medium text-white",
                            style: "background-color: #2563eb;",
                            onclick: move |_| show_info.set(false),
                            "Got it"
                        }
                    }
                }

                div { class: "space-y-2 text-sm",
                    for (idx, (unit, description, scope)) in SERVICES.iter().enumerate() {
                        div { class: "rounded border border-gray-700",
                            div {
                                class: "grid grid-cols-3 gap-2 bg-gray-800 rounded-t px-2 py-2 items-center cursor-pointer hover:bg-gray-600",
                                style: "border-left: 2px solid #00BCD4;",
                                onclick: {
                                    let unit = unit.to_string();
                                    let scope = scope.to_string();
                                    move |_| {
                                        if expanded() == Some(idx) {
                                            expanded.set(None);
                                        } else {
                                            expanded.set(Some(idx));
                                            log_content.set(String::new());
                                            log_total.set(0);
                                            log_error.set(None);
                                            level_filter.set("all".to_string());
                                            let unit = unit.clone();
                                            let scope = scope.clone();
                                            spawn(async move {
                                                log_loading.set(true);
                                                match fetch_logs(&unit, &scope, DEFAULT_LIMIT).await {
                                                    Ok((c, t)) => { log_content.set(c); log_total.set(t); }
                                                    Err(e) => { log_error.set(Some(e)); }
                                                }
                                                log_loading.set(false);
                                            });
                                        }
                                    }
                                },
                                div { class: "flex items-center gap-3",
                                    span { class: "font-mono text-gray-200 text-xs", style: "min-width: 18rem;", "{unit}" }
                                    if expanded() != Some(idx) {
                                        span { class: "text-xs cursor-pointer", style: "color:#00BCD4;", "Click to view logs ▼" }
                                    }
                                }
                                span { class: "text-gray-400 text-xs", "{description}" }
                                div { class: "flex items-center justify-between",
                                    span { class: "font-mono text-xs text-gray-300",
                                        if *scope == "user" { "(user service)" } else { "(system service)" }
                                    }
                                    span { class: "text-gray-300 text-xs ml-2",
                                        if expanded() == Some(idx) { "▲" } else { "▼" }
                                    }
                                }
                            }

                            if expanded() == Some(idx) {
                                div { class: "bg-gray-900 rounded-b px-3 py-3",
                                    div { class: "flex items-start justify-between gap-6 mb-2",
                                        div { class: "text-xs text-gray-300",
                                            div { class: "font-mono", "Unit: {unit}" }
                                            div { class: "text-gray-400",
                                                "Showing last {DEFAULT_LIMIT} lines ({log_total()} total)"
                                            }
                                        }
                                        div { class: "flex flex-col items-end gap-2 whitespace-nowrap",
                                            div { class: "text-xs text-gray-200", "Auto-refresh: every 5s" }
                                            div { class: "flex flex-wrap items-center justify-end gap-2",
                                                select {
                                                    class: "px-2 py-1 rounded border border-gray-600 bg-gray-900 text-white text-xs",
                                                    style: "appearance:auto;",
                                                    value: "{level_filter()}",
                                                    onchange: move |evt| level_filter.set(evt.value()),
                                                    option { value: "all", "All" }
                                                    option { value: "INFO", "Info" }
                                                    option { value: "WARN", "Warn" }
                                                    option { value: "ERROR", "Error" }
                                                    option { value: "DEBUG", "Debug" }
                                                }
                                                button {
                                                    class: "px-3 py-1.5 rounded text-xs font-medium disabled:opacity-60 disabled:cursor-not-allowed",
                                                    style: "background:#2563eb;color:#fff;",
                                                    disabled: log_loading(),
                                                    onclick: {
                                                        let unit = unit.to_string();
                                                        let scope = scope.to_string();
                                                        move |_| {
                                                            let unit = unit.clone();
                                                            let scope = scope.clone();
                                                            spawn(async move {
                                                                log_loading.set(true);
                                                                log_error.set(None);
                                                                match fetch_logs(&unit, &scope, DEFAULT_LIMIT).await {
                                                                    Ok((c, t)) => { log_content.set(c); log_total.set(t); }
                                                                    Err(e) => { log_error.set(Some(e)); }
                                                                }
                                                                log_loading.set(false);
                                                            });
                                                        }
                                                    },
                                                    if log_loading() { "Loading…" } else { "Refresh" }
                                                }
                                                button {
                                                    class: "px-3 py-1.5 rounded text-xs font-medium",
                                                    style: "background:#374151;color:#fff;",
                                                    onclick: move |_| expanded.set(None),
                                                    "Close"
                                                }
                                            }
                                        }
                                    }

                                    if log_loading() && filtered_content.is_empty() {
                                        div { class: "flex items-center gap-2 py-4 text-xs text-gray-400",
                                            div { class: "animate-spin rounded-full h-4 w-4 border-b-2 border-blue-400" }
                                            "Loading journal…"
                                        }
                                    } else if let Some(err) = log_error() {
                                        div { class: "bg-red-900/30 border border-red-700 rounded p-3 text-xs text-red-300",
                                            "Error: {err}"
                                        }
                                    } else {
                                        pre {
                                            class: "whitespace-pre-wrap text-xs bg-gray-950 text-gray-200 rounded p-3 overflow-x-auto border border-gray-800 max-h-96",
                                            if filtered_content.is_empty() {
                                                "-- No entries --"
                                            } else {
                                                "{filtered_content}"
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
