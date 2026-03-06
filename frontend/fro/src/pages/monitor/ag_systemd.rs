use crate::api;
use crate::app::{PageErrors, Route};
use crate::components::monitor::{Breadcrumb, BreadcrumbItem, NavTabs, Panel};
use dioxus::prelude::*;

const SYSTEMD_UNIT: &str = "ag-full-stack.service";
const DEFAULT_LIMIT: usize = 200;

#[component]
pub fn MonitorAgSystemd() -> Element {
    let mut page_errors = use_context::<Signal<PageErrors>>();
    let mut logs = use_signal(|| Option::<api::LogsResponse>::None);
    let mut is_loading = use_signal(|| false);

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

    // Fetch once on mount + auto-refresh every 5 seconds.
    use_effect(move || {
        fetch_logs();

        spawn(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(5_000).await;
                fetch_logs();
            }
        });
    });

    let note = logs().as_ref().and_then(|r| r.note.clone());
    let entries = logs()
        .as_ref()
        .map(|r| r.entries.clone())
        .unwrap_or_default();

    let mut level_filter = use_signal(|| "all".to_string());
    let filtered_entries = entries
        .iter()
        .filter(|e| {
            let current = level_filter();
            if current == "all" {
                return true;
            }
            e.level.as_deref() == Some(current.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();

    rsx! {
        div { class: "p-4 space-y-4",
            NavTabs { active: Route::MonitorAgSystemd {} }

            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("ag-systemd", None),
                ]
            }

            Panel { title: "ag-systemd (journalctl)",
                div { class: "flex items-start justify-between gap-6",
                    // Left info
                    div { class: "text-sm text-gray-300",
                        div { class: "font-mono", "Unit: {SYSTEMD_UNIT}" }
                        div { class: "text-xs text-gray-400", "Showing last {DEFAULT_LIMIT} lines" }
                    }

                    // Right controls
                    div { class: "flex flex-col items-end gap-2 whitespace-nowrap",
                        div { class: "text-sm text-gray-200", "Auto-refresh: every 5s" }

                        div { class: "flex flex-wrap items-center justify-end gap-2",
                            select {
                                class: "px-2 py-1 rounded border border-gray-600 bg-gray-900 text-white text-sm",
                                style: "appearance:auto;",
                                value: "{level_filter()}",
                                onchange: move |evt| level_filter.set(evt.value()),
                                option { value: "all", "All" }
                                option { value: "info", "Info" }
                                option { value: "warn", "Warn" }
                                option { value: "error", "Error" }
                                option { value: "debug", "Debug" }
                            }

                            button {
                                class: "px-3 py-1.5 rounded text-sm font-medium disabled:opacity-60 disabled:cursor-not-allowed",
                                style: "background:#2563eb;color:#fff;",
                                "aria-label": "refresh-systemd-logs",
                                disabled: is_loading(),
                                onclick: move |_| fetch_logs(),
                                span {
                                    if is_loading() { "Loading…" } else { "Refresh" }
                                }
                            }
                        }
                    }
                }

                if let Some(note) = note {
                    div { class: "mt-3 text-sm text-yellow-300", "{note}" }
                }

                pre {
                    class: "mt-3 whitespace-pre-wrap text-xs bg-gray-950 text-gray-200 rounded p-3 overflow-x-auto border border-gray-800",
                    if filtered_entries.is_empty() {
                        "No journal entries returned for current filter."
                    } else {
                        for entry in filtered_entries {
                            {
                                let level = entry.level.as_deref().unwrap_or("");
                                let msg = entry.message.as_deref().unwrap_or(entry.raw.as_str());
                                format!("{} {}\n", level, msg)
                            }
                        }
                    }
                }
            }
        }
    }
}
