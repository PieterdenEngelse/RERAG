use crate::{api, app::Route, components::monitor::*};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

const LOG_LIMIT: usize = 200;

#[derive(Clone, Default)]
struct LogsState {
    loading: bool,
    error: Option<String>,
    data: Option<api::LogsResponse>,
    paused: bool,
}

#[component]
pub fn MonitorLogs() -> Element {
    let state = use_signal(|| LogsState {
        loading: true,
        ..Default::default()
    });
    let mut level_filter = use_signal(|| "ALL".to_string());
    let mut search_query = use_signal(String::new);

    {
        let mut state = state;
        use_future(move || async move {
            loop {
                if state.read().paused {
                    TimeoutFuture::new(2_000).await;
                    continue;
                }

                match api::fetch_recent_logs(LOG_LIMIT).await {
                    Ok(resp) => {
                        let paused = state.read().paused;
                        state.set(LogsState {
                            loading: false,
                            error: None,
                            data: Some(resp),
                            paused,
                        })
                    }
                    Err(err) => {
                        let previous = state.read().data.clone();
                        let paused = state.read().paused;
                        state.set(LogsState {
                            loading: false,
                            error: Some(err),
                            data: previous,
                            paused,
                        })
                    }
                }
                TimeoutFuture::new(2_000).await;
            }
        });
    }

    let snapshot = state.read().clone();

    let current_filter_value = level_filter.read().clone();
    let current_query_value = search_query.read().clone();
    let query_lower = current_query_value.to_lowercase();
    let filter_for_entries = current_filter_value.clone();

    let filtered_entries: Vec<api::LogEntry> = snapshot
        .data
        .as_ref()
        .map(|data| {
            data.entries
                .iter()
                .rev()
                .filter(|entry| level_matches(entry.level.as_deref(), &filter_for_entries))
                .filter(|entry| matches_query(entry, &query_lower))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    let current_note = snapshot.data.as_ref().and_then(|d| d.note.clone());
    let current_file = snapshot.data.as_ref().and_then(|d| d.file.clone());

    let toggle_pause = {
        let mut state = state;
        move |_| {
            let current = state.read().paused;
            state.write().paused = !current;
        }
    };

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Logs", None),
                ],
            }

            NavTabs { active: Route::MonitorLogs {} }

            Panel { title: Some("Logs".to_string()), refresh: Some("2s".into()),
                div { class: "flex flex-wrap gap-3 text-xs text-gray-300 mb-3",
                    select {
                        class: "bg-gray-800 border border-gray-700 rounded px-2 py-1",
                        value: "{current_filter_value.clone()}",
                        onchange: move |evt| level_filter.set(evt.value().to_uppercase()),
                        option { value: "ALL", "All" }
                        option { value: "INFO", "Info" }
                        option { value: "WARN", "Warn" }
                        option { value: "ERROR", "Error" }
                        option { value: "DEBUG", "Debug" }
                    }
                    input {
                        class: "bg-gray-800 border border-gray-700 rounded px-2 py-1 text-white",
                        placeholder: "Search message / target",
                        value: "{current_query_value.clone()}",
                        oninput: move |evt| search_query.set(evt.value()),
                    }
                    button {
                        class: "px-3 py-1 rounded bg-teal-600 text-white",
                        onclick: toggle_pause,
                        if snapshot.paused { "Resume" } else { "Pause" }
                    }
                }

                if snapshot.loading {
                    div { class: "text-gray-400 text-sm", "Loading recent log lines…" }
                } else if let Some(err) = snapshot.error {
                    div { class: "text-red-400 text-sm", "Failed to load logs: {err}" }
                } else if snapshot.data.is_some() {
                    if let Some(note) = current_note.clone() {
                        div { class: "text-yellow-400 text-xs", "{note}" }
                    }
                    if let Some(file) = current_file.clone() {
                        div { class: "text-gray-400 text-xs mb-2", "Source: {file}" }
                    }

                    if filtered_entries.is_empty() {
                        div { class: "text-gray-500 text-sm", "No log entries match filters." }
                    } else {
                        div { class: "bg-black/50 p-3 rounded text-xs font-mono text-gray-200 space-y-1 max-h-[480px] overflow-y-auto",
                            {filtered_entries.iter().map(|entry| {
                                let ts = entry.timestamp.clone().unwrap_or_else(|| "-".into());
                                let level = entry.level.clone().unwrap_or_else(|| "INFO".into());
                                let target = entry.target.clone().unwrap_or_else(|| "app".into());
                                let message = entry.message.clone().unwrap_or_else(|| entry.raw.clone());
                                rsx! {
                                    div { class: format!("flex gap-2 {}", level_color(&level)),
                                        span { class: "text-gray-500", "{ts}" }
                                        span { class: "font-semibold", "{level}" }
                                        span { class: "text-gray-400", "{target}" }
                                        span { class: "text-gray-200", "{message}" }
                                    }
                                }
                            })}
                        }
                    }
                } else {
                    div { class: "text-gray-400 text-sm", "No data yet." }
                }
            }
        }
    }
}

fn level_matches(entry_level: Option<&str>, filter: &str) -> bool {
    if filter == "ALL" {
        return true;
    }
    entry_level
        .map(|level| level.to_uppercase() == filter)
        .unwrap_or(false)
}

fn matches_query(entry: &api::LogEntry, query_lower: &str) -> bool {
    if query_lower.is_empty() {
        return true;
    }
    let haystacks = [
        entry.message.as_deref(),
        entry.target.as_deref(),
        Some(&entry.raw),
    ];
    haystacks
        .iter()
        .flatten()
        .any(|value| value.to_lowercase().contains(query_lower))
}

fn level_color(level: &str) -> &'static str {
    match level.to_uppercase().as_str() {
        "WARN" => "text-yellow-400",
        "ERROR" => "text-red-400",
        "DEBUG" => "text-blue-400",
        _ => "text-green-400",
    }
}
