use crate::{
    api,
    app::{PageErrors, Route},
    components::monitor::*,
};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

#[derive(Clone, Default)]
struct RateLimitState {
    loading: bool,
    error: Option<String>,
    data: Option<api::RateLimitInfoResponse>,
    toggling: bool,
}

#[component]
pub fn MonitorRateLimits() -> Element {
    let state = use_signal(|| RateLimitState {
        loading: true,
        ..Default::default()
    });
    let limiter_info_open = use_signal(|| false);

    {
        let mut state = state.clone();
        let mut page_errors = use_context::<Signal<PageErrors>>();
        use_future(move || async move {
            loop {
                match api::fetch_rate_limit_info().await {
                    Ok(resp) => {
                        state.set(RateLimitState {
                            loading: false,
                            error: None,
                            data: Some(resp),
                            toggling: false,
                        });
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

    let snapshot = state.read().clone();
    let drop_rows = snapshot.data.as_ref().map(|d| build_drop_rows(d));
    let drop_counts = snapshot.data.as_ref().map(|d| build_drop_counts(d));

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

fn build_drop_rows(data: &api::RateLimitInfoResponse) -> Vec<Vec<String>> {
    let mut entries = data.drops_by_route.clone();
    entries.sort_by(|a, b| b.drops.cmp(&a.drops));
    entries
        .into_iter()
        .map(|entry| vec![entry.route, entry.drops.to_string()])
        .collect()
}

fn build_drop_counts(data: &api::RateLimitInfoResponse) -> Vec<f64> {
    let mut entries = data.drops_by_route.clone();
    entries.sort_by(|a, b| b.drops.cmp(&a.drops));
    entries
        .into_iter()
        .take(5)
        .map(|entry| entry.drops as f64)
        .collect()
}
