use crate::api;
use crate::app::{PageErrors, Route};
use crate::components::monitor::{Breadcrumb, BreadcrumbItem, NavTabs, Panel};
use dioxus::prelude::*;

const SYSTEMD_UNIT: &str = "ag.service";
const DEFAULT_LIMIT: usize = 200;

#[component]
pub fn MonitorAgSystemd() -> Element {
    let mut page_errors = use_context::<Signal<PageErrors>>();
    let mut logs = use_signal(|| Option::<api::SystemdLogsResponse>::None);
    let mut is_loading = use_signal(|| false);
    let mut level_filter = use_signal(|| "all".to_string());

    let mut fetch_logs = move || {
        is_loading.set(true);
        spawn(async move {
            match api::fetch_systemd_logs(SYSTEMD_UNIT, DEFAULT_LIMIT).await {
                Ok(resp) => {
                    logs.set(Some(resp));
                    page_errors.with_mut(|e| e.clear_error("ag-systemd"));
                }
                Err(err) => {
                    page_errors.with_mut(|e| e.set_error("ag-systemd", &err));
                }
            }
            is_loading.set(false);
        });
    };

    use_effect(move || {
        fetch_logs();
        spawn(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(5_000).await;
                fetch_logs();
            }
        });
    });

    // Filter raw content lines by level keyword
    let filtered_content = {
        let filter = level_filter();
        logs().as_ref().map(|r| {
            r.content
                .lines()
                .filter(|line| {
                    if filter == "all" {
                        return true;
                    }
                    line.to_uppercase().contains(&filter.to_uppercase())
                })
                .collect::<Vec<_>>()
                .join("\n")
        }).unwrap_or_default()
    };

    let total_lines = logs().as_ref().map(|r| r.total_lines).unwrap_or(0);

    rsx! {
        div { class: "p-4 space-y-4",
            NavTabs { active: Route::MonitorAgSystemd {} }

            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Systemd", None),
                ]
            }

            div { class: "flex items-center gap-2 pl-2",
                a {
                    class: "text-xs text-gray-400 hover:text-blue-400 transition-colors",
                    style: "font-size: 0.9rem;",
                    href: "http://localhost:3001/d/dfg25if2wfxmod/ag-logs-loki",
                    target: "_blank",
                    "↗ Grafana"
                }
            }
            Panel { title: "Systemd (journalctl)",
                div { class: "flex items-start justify-between gap-6",
                    div { class: "text-sm text-gray-300",
                        div { class: "font-mono", "Unit: {SYSTEMD_UNIT}" }
                        div { class: "text-xs text-gray-400",
                            "Showing last {DEFAULT_LIMIT} lines ({total_lines} total)"
                        }
                    }

                    div { class: "flex flex-col items-end gap-2 whitespace-nowrap",
                        div { class: "text-sm text-gray-200", "Auto-refresh: every 5s" }
                        div { class: "flex flex-wrap items-center justify-end gap-2",
                            select {
                                class: "px-2 py-1 rounded border border-gray-600 bg-gray-900 text-white text-sm",
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
                                class: "px-3 py-1.5 rounded text-sm font-medium disabled:opacity-60 disabled:cursor-not-allowed",
                                style: "background:#2563eb;color:#fff;",
                                disabled: is_loading(),
                                onclick: move |_| fetch_logs(),
                                if is_loading() { "Loading…" } else { "Refresh" }
                            }
                        }
                    }
                }

                pre {
                    class: "mt-3 whitespace-pre-wrap text-xs bg-gray-950 text-gray-200 rounded p-3 overflow-x-auto border border-gray-800",
                    if filtered_content.is_empty() {
                        "No journal entries returned for current filter."
                    } else {
                        "{filtered_content}"
                    }
                }
            }
        }
    }
}
