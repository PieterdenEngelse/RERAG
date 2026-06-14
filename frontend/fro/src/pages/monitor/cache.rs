use crate::{
    api,
    app::{PageErrors, Route},
    components::monitor::*,
    pages::hardware::constants::{
        PARAM_ICON_BUTTON_STYLE, QUICK_ACTION_INFO_BUTTON_CLASS, QUICK_ACTION_INFO_ICON_CLASS,
    },
};
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

    // Clear Cache action state
    let mut cache_loading = use_signal(|| false);
    let mut cache_result = use_signal(|| Option::<String>::None);
    let mut show_cache_info = use_signal(|| false);

    let on_clear_cache = move |_| {
        cache_loading.set(true);
        cache_result.set(None);
        spawn(async move {
            match api::clear_cache().await {
                Ok(_) => cache_result.set(Some("\u{2713} Cache cleared".to_string())),
                Err(e) => cache_result.set(Some(format!("\u{2717} {}", e))),
            }
            cache_loading.set(false);
        });
    };

    {
        let mut state = state;
        let mut page_errors = use_context::<Signal<PageErrors>>();
        use_future(move || async move {
            loop {
                match api::fetch_cache_info().await {
                    Ok(resp) => {
                        state.set(CacheState {
                            loading: false,
                            error: None,
                            data: Some(resp),
                        });
                        page_errors.with_mut(|e| e.clear_error("cache"));
                    }
                    Err(err) => {
                        let previous = state.read().data.clone();
                        state.set(CacheState {
                            loading: false,
                            error: Some(err.clone()),
                            data: previous,
                        });
                        page_errors.with_mut(|errs| errs.set_error("cache", &err));
                        let _ = api::log_frontend_error("cache", &err).await;
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
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorTip {})),
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
                        div { class: "mt-3 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 text-xs text-slate-100 leading-relaxed", role: "list",
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

                    // Stats + Hit Ratio chart — 4-col so the chart sits beside the rates
                    div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4",
                        StatCard {
                            title: "L1 Hit Rate".into(),
                            value: format!("{:.1}", data.l1.hit_rate * 100.0).into(),
                            unit: Some("%".into()),
                            info_tooltip: Some("L1 Cache — In-Process LRU\n\nL1 is the fastest cache tier in ag. It lives entirely inside the backend process as a fixed-capacity Least-Recently-Used (LRU) map, so every hit is answered in nanoseconds with zero I/O — no disk, no network, no Tantivy, no embedder.\n\nHow it works\nEvery search query is hashed into a cache key. On arrival the key is looked up in the LRU map. If found (hit), the stored result is returned immediately and the entry is promoted to the \"most recently used\" slot. If not found (miss), the query continues down the stack to L2, then L3 (Redis), and finally to Tantivy + the embedding model. Once the full result is computed it is inserted into L1 (and L2) so the next identical query can be answered instantly.\n\nCapacity & eviction\nThe LRU has a fixed maximum number of entries. When it is full and a new entry must be inserted, the least-recently-used entry is evicted. A sudden hit-rate drop often means the working set has grown beyond the LRU capacity — hot keys are being evicted before they can be reused.\n\nWhat this number tells you\nHit rate = hits ÷ (hits + misses). 90 %+ is healthy for a stable query mix. Values below 70 % are a warning sign: users are paying the full Tantivy + embedding cost on most requests, which adds latency and CPU load. Compare the Hits and Misses counters in L1 Details below to see the absolute scale — a 70 % rate on 10 queries is harmless; on 10 000 it is expensive.\n\nCommon causes of a low L1 hit rate\n• Query diversity is high — every user asks something different, so nothing stays hot long enough to be reused.\n• LRU capacity is too small relative to the active working set.\n• A recent deploy flushed the cache and it has not yet warmed up.\n• Embeddings changed (model swap or config change), invalidating all stored keys.\n\nHow to improve it\nIncrease LRU capacity in the backend config, pre-warm the cache after deploys by replaying representative queries, or use a query-normalisation step to collapse near-duplicate queries into the same key.".into()),
                        }
                        StatCard {
                            title: "L2 Hit Rate".into(),
                            value: format!("{:.1}", data.l2.hit_rate * 100.0).into(),
                            unit: Some("%".into()),
                            info_tooltip: Some("L2 Cache — Concurrent DashMap\n\nL2 is the second cache tier. It is an in-process concurrent hash map (DashMap) that is larger and slightly slower than L1 but still entirely in RAM with no network hop.\n\nHow it relates to L1\nL1 and L2 are checked in order. A request only reaches L2 after an L1 miss. When L2 answers a query it also backfills L1 with the result, so the same query will be answered by L1 on the next call. This layered design means the effective combined hit rate is:\n  combined = 1 − ((1 − L1 rate) × (1 − L2 rate))\n\nThe L2 hit rate shown here is computed only over the queries that L1 already missed, so a healthy system can show a moderate L2 rate even when L1 is above 90 %.\n\nDashMap vs LRU\nUnlike L1, L2 does not have a strict entry-count cap enforced by LRU eviction — it grows until memory pressure or an explicit flush. The trade-off is that DashMap can hold a much larger working set without churning hot entries, but it requires more RAM and its entries can go stale if not invalidated after a re-index.\n\nWhat the counters show\nThe L2 Details panel below breaks out L1 hits, L1 misses, L2 hits, L2 misses, and total stored entries. Watch L2 Misses relative to L1 Misses: if L1 misses are high and L2 misses are also high, both tiers are cold and every query is hitting Tantivy.\n\nCommon causes of a low L2 hit rate\n• L2 is disabled (check Enabled field in L2 Details).\n• The corpus was recently re-indexed, invalidating stored embeddings.\n• Query vectors are highly variable (semantic search with diverse prompts).\n• Memory pressure caused a manual flush or process restart.\n\nHow to improve it\nEnsure L2 is enabled. After a re-index, trigger a warm-up pass. If Entries is near zero shortly after startup, the cache has not yet been populated — this is normal and will resolve as traffic flows through the system.".into()),
                        }
                        StatCard {
                            title: "Redis TTL".into(),
                            value: format!("{}", data.redis.ttl_seconds).into(),
                            unit: Some("s".into()),
                            info_tooltip: Some("Redis — L3 Remote Cache & TTL\n\nRedis is the third and outermost cache tier (L3). Unlike L1 and L2, which live inside the backend process and are lost on restart, Redis is an external in-memory store that survives process restarts and can be shared across multiple backend instances.\n\nWhat TTL means\nEvery key written to Redis is given a Time-To-Live (TTL). After that many seconds the key expires and Redis deletes it automatically. The next request for that key will be a cache miss — ag will recompute the result from Tantivy + the embedding model and write a fresh entry back to Redis.\n\nWhy TTL matters\n• Too short (e.g. < 60 s): entries expire before they can be reused, Redis provides almost no benefit, and Tantivy/embedder load stays high.\n• Too long (e.g. > 24 h): stale results accumulate. After a corpus re-index or an embedding model change, users may receive outdated answers until their key expires.\n• Single-digit TTL values are a red flag — they usually indicate eviction pressure (Redis maxmemory policy kicking in), clock drift between the backend and Redis, or a misconfigured TTL env variable.\n\nThe value shown here is the configured default TTL — the actual remaining lifetime of any individual key depends on when it was written. Use redis-cli TTL <key> to inspect a specific entry.\n\nConnection health\nThe Redis Connection panel below shows whether Redis is reachable. If Connected = No, ag falls back to L1/L2 only — all writes and reads to L3 are silently skipped. The system remains functional but loses the persistence and sharing benefits of Redis.\n\nKey env variables\n  REDIS_ENABLED=true       — toggles L3 entirely\n  REDIS_URL                — connection string (default redis://127.0.0.1:6379)\n  CACHE_TTL_SECONDS        — sets the default TTL written here\n\nHow to tune TTL\nStart with a TTL that comfortably exceeds your average re-index interval. If you re-index every hour, a TTL of 3 600 s (1 h) ensures Redis entries never outlive the index. After a manual re-index, flush the Redis keyspace (redis-cli FLUSHDB) rather than waiting for natural expiry to avoid serving stale answers.".into()),
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
                    }

                    // L1 Details + L2 Details + Redis Connection — 3-col
                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-4 mt-4",
                        Panel { title: Some("L1 Details".into()), refresh: None,
                            DataTable {
                                headers: vec!["Metric".into(), "Value".into()],
                                rows: vec![
                                    vec!["Enabled".into(), yes_no(data.l1.enabled)],
                                    vec!["Hits".into(), data.l1.hits.to_string()],
                                    vec!["Misses".into(), data.l1.misses.to_string()],
                                    vec!["Total Searches".into(), data.l1.total_searches.to_string()],
                                ],
                            }
                        }
                        Panel { title: Some("L2 Details".into()), refresh: None,
                            DataTable {
                                headers: vec!["Metric".into(), "Value".into()],
                                rows: vec![
                                    vec!["Enabled".into(), yes_no(data.l2.enabled)],
                                    vec!["L1 Hits".into(), data.l2.l1_hits.to_string()],
                                    vec!["L1 Misses".into(), data.l2.l1_misses.to_string()],
                                    vec!["L2 Hits".into(), data.l2.l2_hits.to_string()],
                                    vec!["L2 Misses".into(), data.l2.l2_misses.to_string()],
                                    vec!["Entries".into(), data.l2.total_items.to_string()],
                                ],
                            }
                        }
                        Panel { title: Some("Redis Connection".into()), refresh: Some("10s".into()),
                            DataTable {
                                headers: vec!["Field".into(), "Value".into()],
                                rows: vec![
                                    vec!["Enabled".into(), yes_no(data.redis.enabled)],
                                    vec!["Connected".into(), yes_no(data.redis.connected)],
                                    vec!["TTL".into(), format!("{}s", data.redis.ttl_seconds)],
                                    vec!["L1 Hits".into(), data.counters.hits_total.to_string()],
                                    vec!["L1 Misses".into(), data.counters.misses_total.to_string()],
                                ],
                            }
                        }
                    }
                } else {
                    div { class: "text-gray-400 text-sm", "No cache stats available" }
                }
            }

            div { class: "lg:max-w-md",
            Panel { title: Some("Actions".into()), refresh: None,
                div { class: "flex items-center gap-3 flex-wrap",
                    button {
                        class: "px-4 py-2 rounded bg-gray-700 text-gray-200 hover:bg-gray-600 transition-colors",
                        disabled: cache_loading(),
                        onclick: on_clear_cache,
                        if cache_loading() { "Clearing\u{2026}" } else { "Clear Cache" }
                    }
                    button {
                        class: QUICK_ACTION_INFO_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        onclick: move |_| show_cache_info.set(true),
                        title: "What this action does",
                        ActionInfoIcon {}
                    }
                    if let Some(result) = cache_result() {
                        span {
                            class: if result.starts_with('\u{2713}') { "text-green-400 text-sm" } else { "text-red-400 text-sm" },
                            "{result}"
                        }
                    }
                }
            }
            }

            if show_cache_info() {
                ClearCacheInfoModal {
                    on_close: move |_| show_cache_info.set(false),
                }
            }
        }
    }
}

#[component]
fn ActionInfoIcon() -> Element {
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

#[component]
fn ClearCacheInfoModal(on_close: EventHandler<()>) -> Element {
    let content = "Clears all search result caches:\n\n\u{2022} L1 Cache: In-memory cache (fastest, lost on restart)\n\u{2022} L2 Cache: Disk-based cache (persists across restarts)\n\nNote: Redis (L3) cache is not cleared by this action.\n\nUse this when:\n\u{2022} Search results seem stale after document updates\n\u{2022} Testing cache performance\n\u{2022} Debugging cache-related issues\n\nCache will rebuild automatically as new searches are performed.";
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| on_close.call(()),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[90vw] max-w-lg max-h-[90vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-3",
                    h2 { class: "text-base font-semibold text-gray-100", "Clear Cache" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| on_close.call(()),
                        "\u{00d7}"
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
