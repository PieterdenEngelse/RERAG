use crate::{api, app::Route, components::monitor::*};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::JsFuture;
use web_sys::console;

const CACHE_INFO_COMMAND: &str = "curl http://127.0.0.1:3010/monitor/cache/info";

#[derive(Clone, Default)]
struct CacheState {
    loading: bool,
    error: Option<String>,
    data: Option<api::CacheInfoResponse>,
}

#[component]
pub fn MonitorCache() -> Element {
    let state = use_signal(|| CacheState {
        loading: true,
        ..Default::default()
    });
    let mut deep_dive_more_info = use_signal(|| false);

    {
        let mut state = state.clone();
        use_future(move || async move {
            loop {
                match api::fetch_cache_info().await {
                    Ok(resp) => state.set(CacheState {
                        loading: false,
                        error: None,
                        data: Some(resp),
                    }),
                    Err(err) => {
                        let previous = state.read().data.clone();
                        state.set(CacheState {
                            loading: false,
                            error: Some(err),
                            data: previous,
                        });
                    }
                }
                TimeoutFuture::new(10_000).await;
            }
        });
    }

    let snapshot = state.read().clone();
    let mut troubleshooting_open = use_signal(|| false);

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Cache", None),
                ],
            }

            NavTabs { active: Route::MonitorCache {} }

            RowHeader {
                title: "Cache Layers".into(),
                description: Some("Live statistics for L1/L2 caches and Redis".into()),
            }

            Panel { title: Some("Summary".into()), refresh: Some("10s".into()),
                if snapshot.loading {
                    div { class: "text-gray-400 text-sm", "Loading cache stats…" }
                } else if let Some(err) = snapshot.error {
                    div { class: "text-red-400 text-sm", "Failed to load stats: {err}" }
                } else if let Some(data) = snapshot.data.clone() {
                    div { class: "text-gray-300 text-sm leading-relaxed mb-2",
                        span { class: "font-semibold text-gray-200", "How to read this:" }
                        " L1 hit rate shows how often we answer straight from memory, L2 reflects the disk-backed cache, and Redis TTL tells you how long remote entries live. Sustained drops in hit rate or a very low TTL usually translate to extra load on Tantivy and the embedder."
                    }
                    details { class: "bg-slate-800/70 rounded border border-slate-700 p-3 text-xs text-slate-200 mb-3", open: troubleshooting_open(),
                        summary { class: "flex items-center gap-2 cursor-pointer text-teal-300 font-semibold focus:outline-none focus-visible:ring-2 focus-visible:ring-teal-500 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-900 rounded",
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
                        div { class: "text-[11px] text-slate-400 mt-2",
                            "Use this to validate cache behaviour when traffic changes or whenever hit rate drifts below expected targets."
                        }
                        div { class: "mt-3 grid grid-cols-1 md:grid-cols-2 gap-4 text-xs text-slate-100 leading-relaxed", role: "list",
                            div { role: "listitem",
                                p { class: "font-semibold text-slate-50", "L1 / L2 hygiene" }
                                ul { class: "list-disc ml-5 space-y-1",
                                    li { "Watch the 5-minute rate of cache_hits_total vs cache_misses_total; <80% hit rate usually means polluted entries." }
                                    li { "Flush the tier that regressed first (L1 before L2) and confirm recovery before touching Redis." }
                                    li { "Compare L2 entries against expected corpus size so silent evictions don’t masquerade as fresh misses." }
                                }
                            }
                            div { role: "listitem",
                                p { class: "font-semibold text-slate-50", "Load correlation" }
                                ul { class: "list-disc ml-5 space-y-1",
                                    li { "Overlay Request Rate/Volume charts with cache_misses_total to confirm misses match legitimate traffic spikes." }
                                    li { "If misses climb while req/s is flat, investigate stale embeddings or uneven shard routing before scaling out." }
                                    li { "Run a warm-up batch after deploys to repopulate hot keys so users don’t pay the miss penalty." }
                                }
                            }
                            div { class: "border-t border-slate-700 pt-3 md:pt-0 md:border-t-0", role: "listitem",
                                p { class: "font-semibold text-slate-50", "Redis + logs sanity pass" }
                                ul { class: "list-disc ml-5 space-y-1",
                                    li { "Confirm Redis TTL plus connection health; TTL dipping to single digits indicates eviction pressure or clock drift." }
                                    li { "Tail ag.service logs for cache errors, slow fetches, or embedder backpressure while the summary refreshes." }
                                    li { "Escalate if Redis stays Connected=false for >30s or hit rates fail to recover after a flush cycle." }
                                }
                            }
                            div { class: "border-t border-slate-700 pt-3 md:pt-0 md:border-t-0", role: "listitem",
                                p { class: "font-semibold text-slate-50", "Deep dive" }
                                ul { class: "list-disc ml-5 space-y-1",
                                    li {
                                        "Pair this panel with the Requests checklist and capture  "
                                        code { {CACHE_INFO_COMMAND} }
                                        "  output plus Tempo/Grafana traces while caches are misbehaving."
                                        span { " " }
                                        button {
                                            class: "text-[11px] text-slate-300 underline hover:text-teal-300",
                                            onclick: move |_| deep_dive_more_info.with_mut(|value| *value = !*value),
                                            "More info"
                                        }
                                        if deep_dive_more_info() {
                                            div { class: "mt-3 space-y-3 text-slate-100 border border-slate-700 rounded p-3 bg-slate-900/40",
                                                div { class: "flex justify-end",
                                                    button {
                                                        class: "text-slate-400 hover:text-red-400 text-2xl leading-none",
                                                        onclick: move |_| deep_dive_more_info.with_mut(|value| *value = false),
                                                        "×"
                                                    }
                                                }
                                                div {
                                                    p { class: "font-semibold", "1. Open both panels side-by-side" }
                                                    ul { class: "list-disc ml-5 text-[11px] text-slate-200 space-y-1",
                                                        li { "In the frontend, keep the Cache tab open with the checklist expanded." }
                                                        li { "In another browser tab or window, open the Requests tab and expand its troubleshooting checklist." }
                                                    }
                                                }
                                                div {
                                                    p { class: "font-semibold", "2. Capture the cache snapshot" }
                                                    ul { class: "list-disc ml-5 text-[11px] text-slate-200 space-y-1",
                                                        li {
                                                            "In a terminal on the same host that’s running ag, run:"
                                                            div { class: "bg-slate-900/60 border border-slate-700 px-2 py-1 mt-1 text-[10px] flex items-center justify-between gap-3",
                                                                code { class: "whitespace-nowrap overflow-x-auto", {CACHE_INFO_COMMAND} }
                                                                button {
                                                                    class: "text-[10px] px-2 py-1 rounded bg-slate-800 border border-slate-600 text-slate-200 hover:text-white hover:border-slate-400",
                                                                    onclick: move |_| {
                                                                        copy_command_to_clipboard(CACHE_INFO_COMMAND);
                                                                    },
                                                                    "Copy"
                                                                }
                                                            }
                                                        }
                                                        li { "Save the output (e.g., redirect to a file) so you can reference exact hit/miss counts, TTL, and Redis status." }
                                                    }
                                                }
                                                div {
                                                    p { class: "font-semibold", "3. Collect traces/metrics at the same moment" }
                                                    ul { class: "list-disc ml-5 text-[11px] text-slate-200 space-y-1",
                                                        li { "While the cache info is captured, open Tempo/Grafana dashboards and note the relevant traces—especially around the timeframe when misses spike." }
                                                        li { "Screenshot or export the trace view and any Prometheus panels (Request Rate/Volume, cache_misses_total, latency histograms, etc.) so you can correlate later." }
                                                    }
                                                }
                                                div {
                                                    p { class: "font-semibold", "4. Compare notes" }
                                                    ul { class: "list-disc ml-5 text-[11px] text-slate-200 space-y-1",
                                                        li { "Use the Cache checklist to interpret what you see in the cache/info output." }
                                                        li { "Use the Requests checklist to diagnose whether the load spike aligns with those cache misses." }
                                                    }
                                                }
                                                div {
                                                    p { class: "font-semibold", "5. Document findings" }
                                                    ul { class: "list-disc ml-5 text-[11px] text-slate-200 space-y-1",
                                                        li { "Record the curl output, screenshots, and any trace links in your troubleshooting notes or ticket so it’s easy to share with teammates." }
                                                    }
                                                }
                                                p { class: "text-[11px] text-slate-300",
                                                    "Once you’ve gathered both the cache snapshot and the trace/metric context, you’ll have the paired evidence the checklist is nudging you toward."
                                                }
                                                div { class: "flex justify-end",
                                                    button {
                                                        class: "text-[11px] text-slate-300 underline hover:text-red-400",
                                                        onclick: move |_| deep_dive_more_info.with_mut(|value| *value = false),
                                                        "Close ×"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                        StatCard {
                            title: "L1 Hit Rate".into(),
                            value: format!("{:.1}", data.l1.hit_rate * 100.0).into(),
                            unit: Some("%".into()),
                        }
                        StatCard {
                            title: "L2 Hit Rate".into(),
                            value: format!("{:.1}", data.l2.hit_rate * 100.0).into(),
                            unit: Some("%".into()),
                        }
                        StatCard {
                            title: "Redis TTL".into(),
                            value: format!("{}", data.redis.ttl_seconds).into(),
                            unit: Some("s".into()),
                        }
                    }

                    Panel { title: Some("Hit Ratio".into()), refresh: Some("10s".into()),
                        ChartPlaceholder {
                            values: vec![
                                data.l1.hit_rate * 100.0,
                                data.l2.hit_rate * 100.0,
                                if data.redis.enabled && data.redis.connected { 100.0 } else { 0.0 },
                            ],
                            label: "L1/L2 hit % & Redis".to_string(),
                            unit: "%".to_string(),
                        }
                    }

                    div { class: "grid grid-cols-1 md:grid-cols-2 gap-4 mt-4",
                        Panel { title: Some("L1 Details".into()), refresh: None,
                            DataTable {
                                headers: vec!["Metric".into(), "Value".into()],
                                rows: vec![
                                    vec!["Enabled".into(), yes_no(data.l1.enabled)],
                                    vec!["Hits".into(), data.l1.hits.to_string().into()],
                                    vec!["Misses".into(), data.l1.misses.to_string().into()],
                                    vec!["Total Searches".into(), data.l1.total_searches.to_string().into()],
                                ],
                            }
                        }
                        Panel { title: Some("L2 Details".into()), refresh: None,
                            DataTable {
                                headers: vec!["Metric".into(), "Value".into()],
                                rows: vec![
                                    vec!["Enabled".into(), yes_no(data.l2.enabled)],
                                    vec!["L1 Hits".into(), data.l2.l1_hits.to_string().into()],
                                    vec!["L1 Misses".into(), data.l2.l1_misses.to_string().into()],
                                    vec!["L2 Hits".into(), data.l2.l2_hits.to_string().into()],
                                    vec!["L2 Misses".into(), data.l2.l2_misses.to_string().into()],
                                    vec!["Entries".into(), data.l2.total_items.to_string().into()],
                                ],
                            }
                        }
                    }

                    Panel { title: Some("Redis Connection".into()), refresh: Some("10s".into()),
                        DataTable {
                            headers: vec!["Field".into(), "Value".into()],
                            rows: vec![
                                vec!["Enabled".into(), yes_no(data.redis.enabled)],
                                vec!["Connected".into(), yes_no(data.redis.connected)],
                                vec!["TTL".into(), format!("{}s", data.redis.ttl_seconds).into()],
                                vec!["L1 Hits".into(), data.counters.hits_total.to_string().into()],
                                vec!["L1 Misses".into(), data.counters.misses_total.to_string().into()],
                            ],
                        }
                    }
                } else {
                    div { class: "text-gray-400 text-sm", "No cache stats available" }
                }
            }
        }
    }
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

fn yes_no(flag: bool) -> String {
    if flag {
        "Yes".into()
    } else {
        "No".into()
    }
}
