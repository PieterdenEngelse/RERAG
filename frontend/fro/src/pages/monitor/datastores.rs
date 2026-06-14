//! Monitor — Datastores page.
//!
//! ag keeps two datastores, and both speak the Redis protocol:
//!   - the L3 cache  — optional, ephemeral search-result cache
//!   - FalkorDB      — the persistent knowledge-graph store (a Redis module)
//!
//! Because both answer the same `INFO` command, they share one health panel;
//! each section then adds what is specific to its role.

use crate::{
    api,
    app::{PageErrors, Route},
    components::monitor::*,
    pages::hardware::constants::{
        INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
    },
};
use dioxus::prelude::*;
use dioxus_router::Link;
use gloo_timers::future::TimeoutFuture;

#[derive(Clone, Default)]
struct DatastoresState {
    loading: bool,
    error: Option<String>,
    data: Option<api::DatastoresResponse>,
}

#[component]
pub fn MonitorDatastores() -> Element {
    let state = use_signal(|| DatastoresState {
        loading: true,
        ..Default::default()
    });
    let mut show_info = use_signal(|| false);
    let mut show_l3_optional = use_signal(|| false);
    let mut show_info_cmd = use_signal(|| false);
    let mut show_sentinel = use_signal(|| false);
    let mut show_wire_protocol = use_signal(|| false);
    let mut field_help = use_context_provider(|| Signal::new(None::<&'static str>));
    let show_container_info = use_signal(|| false);
    let also_container = use_signal(|| false);
    let submitting = use_signal(|| false);
    let show_restarting = use_signal(|| false);
    let mut can_manage_compose = use_signal(|| false);

    use_future(move || async move {
        if let Ok(caps) = api::fetch_capabilities().await {
            can_manage_compose.set(caps.can_manage_compose);
        }
    });

    {
        let mut state = state;
        let mut page_errors = use_context::<Signal<PageErrors>>();
        use_future(move || async move {
            loop {
                match api::fetch_datastores().await {
                    Ok(resp) => {
                        state.set(DatastoresState {
                            loading: false,
                            error: None,
                            data: Some(resp),
                        });
                        page_errors.with_mut(|e| e.clear_error("datastores"));
                    }
                    Err(err) => {
                        let previous = state.read().data.clone();
                        state.set(DatastoresState {
                            loading: false,
                            error: Some(err.clone()),
                            data: previous,
                        });
                        page_errors.with_mut(|e| e.set_error("datastores", &err));
                    }
                }
                TimeoutFuture::new(10_000).await;
            }
        });
    }

    let snapshot = state.read().clone();

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorTip {})),
                    BreadcrumbItem::new("Datastores", None),
                ],
            }
            NavTabs { active: Route::MonitorDatastores {} }

            div { class: "flex items-center gap-2",
                h1 { class: "text-xl font-semibold text-gray-100", "Datastores" }
                button {
                    class: PARAM_ICON_BUTTON_CLASS,
                    style: PARAM_ICON_BUTTON_STYLE,
                    onclick: move |_| show_info.set(!show_info()),
                    title: "Show info",
                    svg {
                        class: INFO_ICON_SVG_CLASS,
                        view_box: "0 0 20 20",
                        fill: "none",
                        stroke: "currentColor",
                        circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                        line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                    }
                }
            }
            p { class: "text-sm text-gray-300",
                "ag keeps two Redis-protocol datastores: an "
                span {
                    class: "text-blue-400 hover:text-blue-300 underline cursor-pointer",
                    onclick: move |_| show_l3_optional.set(true),
                    title: "Why you might turn off L3",
                    "optional"
                }
                " L3 cache and FalkorDB, the knowledge-graph store. Both answer the same "
                span {
                    class: "text-blue-400 hover:text-blue-300 underline cursor-pointer",
                    onclick: move |_| show_info_cmd.set(true),
                    title: "What the INFO command is",
                    "INFO"
                }
                " command, so they share one health view."
            }

            if show_info() {
                {info_modal(show_info)}
            }
            if show_l3_optional() {
                {l3_optional_modal(show_l3_optional)}
            }
            if show_info_cmd() {
                {info_command_modal(show_info_cmd, show_wire_protocol)}
            }
            if show_wire_protocol() {
                {wire_protocol_modal(show_wire_protocol)}
            }
            if field_help().is_some() {
                {field_reference_modal(field_help, show_sentinel)}
            }
            if show_sentinel() {
                {sentinel_modal(show_sentinel)}
            }
            if show_container_info() {
                {stop_container_modal(show_container_info)}
            }

            if let Some(err) = &snapshot.error {
                div { class: "text-sm text-red-400", "Failed to load: {err}" }
            }

            if show_restarting() {
                {restarting_overlay()}
            }

            if let Some(data) = &snapshot.data {
                div { class: "flex flex-row gap-3 items-start w-full",
                    div { class: "flex-1 min-w-0",
                        {aligned_panel(
                            "L3 Cache — Redis",
                            "Optional · ephemeral · search-result cache",
                            "10s",
                            cache_section(&data.cache, also_container, submitting, show_restarting, show_container_info, can_manage_compose()),
                        )}
                    }
                    div { class: "shrink-0",
                        {field_button_column()}
                    }
                    div { class: "flex-1 min-w-0",
                        {aligned_panel(
                            "FalkorDB — Knowledge-graph store",
                            "Persistent · Redis module · falkordb.service",
                            "10s",
                            falkor_section(&data.falkordb),
                        )}
                    }
                }
                div { class: "text-sm text-gray-400",
                    "Tune these stores on the "
                    Link {
                        to: Route::ConfigFalkorDb {},
                        class: "text-blue-400 hover:text-blue-300",
                        "FalkorDB"
                    }
                    " and "
                    Link {
                        to: Route::ConfigRedis {},
                        class: "text-blue-400 hover:text-blue-300",
                        "Redis parameters"
                    }
                    " config pages."
                }
            } else if snapshot.loading {
                div { class: "text-sm text-gray-400", "Loading…" }
            }
        }
    }
}

/// L3 cache section: connection state, optional health panel, and the toggle controls.
fn cache_section(
    c: &api::CacheDatastore,
    mut also_container: Signal<bool>,
    mut submitting: Signal<bool>,
    mut show_restarting: Signal<bool>,
    mut show_container_info: Signal<bool>,
    can_manage_compose: bool,
) -> Element {
    let enabled_now = c.enabled;
    let new_enabled = !enabled_now;
    let action_label = if enabled_now {
        "Disable L3 cache"
    } else {
        "Enable L3 cache"
    };
    let checkbox_label = if enabled_now {
        "Also stop the redis container"
    } else {
        "Also start the redis container if it isn't running"
    };

    let on_click = move |_| {
        let stop_container = *also_container.read();
        spawn(async move {
            submitting.set(true);
            let res = api::post_l3_toggle(&api::L3ToggleRequest {
                enabled: new_enabled,
                stop_container,
            })
            .await;
            match res {
                Ok(_) => {
                    show_restarting.set(true);
                    // The L3 cache is swapped in-process; give the auto-refetch
                    // loop a moment to pick up the new health state.
                    TimeoutFuture::new(2_500).await;
                    show_restarting.set(false);
                    submitting.set(false);
                }
                Err(_) => {
                    submitting.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "space-y-3",
            div { class: "flex items-center gap-x-2 text-xs h-5 overflow-hidden whitespace-nowrap",
                if !c.enabled {
                    span { class: "px-2 py-0.5 rounded bg-gray-700 text-gray-300 font-semibold",
                        "Disabled"
                    }
                } else if c.health.reachable {
                    span { class: "px-2 py-0.5 rounded bg-green-900/40 text-green-400 font-semibold",
                        "Connected"
                    }
                } else {
                    span { class: "px-2 py-0.5 rounded bg-red-900/40 text-red-400 font-semibold",
                        "Disconnected"
                    }
                }
                span { class: "text-gray-300", "·" }
                span { class: "text-gray-400",
                    "URL: "
                    span { class: "font-mono text-gray-200", "{c.url}" }
                }
                span { class: "text-gray-300", "·" }
                span { class: "text-gray-400",
                    "TTL: "
                    span { class: "font-mono text-gray-200", "{c.ttl_seconds}s" }
                }
            }
            if c.enabled {
                {health_panel(&c.health)}
            } else {
                div { class: "text-xs text-gray-400",
                    "L3 cache disabled — flip the toggle below to enable it. Takes effect immediately, no restart."
                }
            }

            div { class: "border-t border-gray-700 pt-3 mt-2 space-y-2",
                if can_manage_compose {
                    div { class: "flex items-center gap-2",
                        label { class: "flex items-center gap-2 text-xs text-gray-300 cursor-pointer select-none",
                            input {
                                r#type: "checkbox",
                                class: "cursor-pointer",
                                checked: also_container(),
                                oninput: move |evt| {
                                    also_container.set(evt.value() == "true" || evt.value() == "on");
                                },
                            }
                            "{checkbox_label}"
                        }
                        button {
                            class: PARAM_ICON_BUTTON_CLASS,
                            style: PARAM_ICON_BUTTON_STYLE,
                            onclick: move |_| show_container_info.set(true),
                            title: "When (not) to tick this",
                            svg {
                                class: INFO_ICON_SVG_CLASS,
                                view_box: "0 0 20 20",
                                fill: "none",
                                stroke: "currentColor",
                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                            }
                        }
                    }
                }
                div { class: "flex items-center gap-3",
                    button {
                        class: "btn btn-sm",
                        style: "background-color:#7C2A02;color:white;border:1px solid #7C2A02;",
                        disabled: submitting(),
                        onclick: on_click,
                        if submitting() { "Submitting…" } else { "{action_label}" }
                    }
                    span { class: "text-[10px] text-gray-400",
                        "Saves REDIS_ENABLED to overrides.json and hot-swaps the cache in place. No restart."
                    }
                }
            }
        }
    }
}

/// Brief overlay shown while the L3 cache is being swapped in-process.
fn restarting_overlay() -> Element {
    rsx! {
        div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60 pointer-events-auto",
            div { class: "bg-gray-800 border border-gray-600 rounded-lg p-6 max-w-md text-center shadow-xl",
                div { class: "text-base font-semibold text-gray-100 mb-2",
                    "Applying L3 change…"
                }
                p { class: "text-sm text-gray-300",
                    "The setting has been saved. The cache is being swapped in place; this panel will refresh momentarily."
                }
            }
        }
    }
}

/// FalkorDB section: service + connection state, health panel, graph table.
fn falkor_section(f: &api::FalkorDatastore) -> Element {
    let svc_color = match f.service_state.as_str() {
        "active" => "text-green-400",
        "unknown" => "text-gray-400",
        _ => "text-red-400",
    };
    rsx! {
        div { class: "space-y-3",
            div { class: "flex items-center gap-x-2 text-xs h-5 overflow-hidden whitespace-nowrap",
                if f.health.reachable {
                    span { class: "px-2 py-0.5 rounded bg-green-900/40 text-green-400 font-semibold",
                        "Connected"
                    }
                } else {
                    span { class: "px-2 py-0.5 rounded bg-red-900/40 text-red-400 font-semibold",
                        "Disconnected"
                    }
                }
                span { class: "text-gray-300", "·" }
                span { class: "text-gray-400",
                    "falkordb.service: "
                    span { class: "font-mono {svc_color}", "{f.service_state}" }
                }
                span { class: "text-gray-300", "·" }
                span { class: "text-gray-400",
                    "URL: "
                    span { class: "font-mono text-gray-200", "{f.url}" }
                }
            }
            {health_panel(&f.health)}
            if !f.graphs.is_empty() {
                div {
                    div { class: "text-xs font-semibold text-gray-300 mb-1", "Graphs" }
                    table { class: "w-full text-xs",
                        thead {
                            tr { class: "text-gray-400 text-left",
                                th { class: "py-1 pr-4", "Name" }
                                th { class: "py-1 pr-4", "Nodes" }
                                th { class: "py-1", "Edges" }
                            }
                        }
                        tbody {
                            for g in f.graphs.iter() {
                                tr { class: "border-t border-gray-700",
                                    td { class: "py-1 pr-4 font-mono text-gray-200", "{g.name}" }
                                    td { class: "py-1 pr-4 text-gray-100", "{g.nodes}" }
                                    td { class: "py-1 text-gray-100", "{g.edges}" }
                                }
                            }
                        }
                    }
                }
            }
            p { class: "text-[10px] text-gray-400",
                "Graph contents and visualisation live on the "
                Link {
                    to: Route::MonitorKnowledgeGraph {},
                    class: "text-blue-400 hover:text-blue-300",
                    "Knowledge Graph"
                }
                " page."
            }
        }
    }
}

/// The shared Redis-protocol health grid (`INFO` + `DBSIZE`).
fn health_panel(h: &api::RedisServerHealth) -> Element {
    if !h.reachable {
        let err = h
            .error
            .clone()
            .unwrap_or_else(|| "server unreachable".to_string());
        return rsx! {
            div { class: "text-xs text-red-400", "Not reachable — {err}" }
        };
    }

    let hits = h.keyspace_hits;
    let misses = h.keyspace_misses;
    let hit_rate = if hits + misses > 0 {
        format!("{:.1}%", hits as f64 / (hits + misses) as f64 * 100.0)
    } else {
        "—".to_string()
    };
    let maxmem = if h.maxmemory_bytes == 0 {
        "unlimited".to_string()
    } else {
        fmt_bytes(h.maxmemory_bytes)
    };

    rsx! {
        div { class: "grid grid-cols-1",
            {stat("Version", h.redis_version.clone(), "Version")}
            {stat("Mode", h.redis_mode.clone(), "Mode")}
            {stat("Uptime", fmt_uptime(h.uptime_seconds), "Uptime")}
            {stat("Connected clients", h.connected_clients.to_string(), "Connected clients")}
            {stat("Memory used", h.used_memory_human.clone(), "Memory used")}
            {stat("Memory limit", maxmem, "Memory limit")}
            {stat("Eviction policy", h.maxmemory_policy.clone(), "Eviction policy")}
            {stat("Keys (DBSIZE)", h.db_keys.to_string(), "Keys (DBSIZE)")}
            {stat("Keyspace hit rate", hit_rate, "Keyspace hit rate")}
            {stat("Hits / misses", format!("{hits} / {misses}"), "Hits / misses")}
            {stat("Evicted keys", h.evicted_keys.to_string(), "Evicted keys")}
            {stat("Ops / sec", h.instantaneous_ops_per_sec.to_string(), "Ops / sec")}
            {stat("Commands processed", h.total_commands_processed.to_string(), "Commands processed")}
            {stat("Persistence (AOF)", if h.aof_enabled { "on".to_string() } else { "off".to_string() }, "Persistence (AOF)")}
            {stat("Unsaved changes", h.rdb_changes_since_last_save.to_string(), "Unsaved changes")}
        }
    }
}

/// One label/value row inside the health grid. Height locked to `h-8` (32px)
/// so the middle-column info buttons stay aligned with these rows.
fn stat(label: &str, value: String, _key: &'static str) -> Element {
    rsx! {
        div { class: "flex items-center justify-between gap-2 h-8 border-b border-gray-700 min-w-0",
            span { class: "text-gray-400 shrink-0", "{label}" }
            span { class: "text-gray-100 font-mono truncate", title: "{value}", "{value}" }
        }
    }
}

/// Page-level info modal — written in the app's end-user voice.
fn info_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100", "About Datastores" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-3 leading-relaxed",
                    p {
                        "ag relies on two datastores, and both speak the Redis protocol — which is why this page shows them side by side with the same health panel."
                    }
                    p {
                        strong { "L3 Cache. " }
                        "An optional in-memory cache for search results. It is ephemeral: if it goes away, ag simply recomputes results and keeps working. Controlled by REDIS_ENABLED, REDIS_URL and REDIS_TTL."
                    }
                    p {
                        strong { "FalkorDB. " }
                        "The persistent knowledge-graph store. It is a Redis module, so it answers INFO just like the cache, but it also holds graphs. Losing it loses graph data, so it runs as the falkordb.service system service."
                    }
                    p {
                        strong { "FalkorDB does not store vectors. " }
                        "Two of the URLs on this page start with "
                        code { class: "text-gray-200", "redis://" }
                        " but only the L3 cache is actually a Redis cache — FalkorDB just speaks the same wire protocol. The knowledge graph holds entities, relations, and an "
                        code { class: "text-gray-200", "embedding_id" }
                        " pointer on each chunk. The vectors themselves live elsewhere: document embeddings are kept by Tantivy on disk (with optional HNSW/PQ indexes), and agent-memory embeddings live in process memory and are persisted as a single binary file when ag shuts down."
                    }
                    p {
                        strong { "Reading the panel. " }
                        "Memory used versus limit shows headroom; the eviction policy decides what happens once the limit is hit. Keyspace hit rate is how often lookups find a value. Persistence (AOF) and unsaved changes show how much data an abrupt stop would lose."
                    }
                }
                button {
                    class: "btn btn-sm w-full mt-4",
                    style: "background-color:#7C2A02;",
                    onclick: move |_| show.set(false),
                    "Got it"
                }
            }
        }
    }
}

/// Why the L3 cache is called "optional" — concrete reasons to turn it off.
fn l3_optional_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100",
                        "Why the L3 cache is optional"
                    }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-3 leading-relaxed",
                    p {
                        "ag runs fine without the L3 cache. It is shipped on by default (REDIS_ENABLED=true in the example env), but here are concrete reasons to set REDIS_ENABLED=false."
                    }
                    ol { class: "list-decimal pl-5 space-y-2",
                        li {
                            strong { "You are tuning retrieval. " }
                            "L3 survives restarts, so results computed against an old embedder or chunker stay cached until each key's TTL expires. The in-process tiers vanish on restart and stop showing you stale answers; L3 does not."
                        }
                        li {
                            strong { "You re-index more often than REDIS_TTL (default 3600s). " }
                            "Cached results outlive the documents they were computed from. Either shorten the TTL or turn L3 off."
                        }
                        li {
                            strong { "One less container. " }
                            "The redis container is small, but it adds a process, a port, a healthcheck, and another security feed to track. On a single-box deployment the \"shared across instances\" benefit is zero — the only remaining win is surviving restarts."
                        }
                        li {
                            strong { "Low cache hit rate. " }
                            "If your queries are mostly unique, L3 makes a localhost round-trip just to miss. The in-process tiers already absorb the queries that actually repeat, at much lower cost. Check the hit-rate counter on this page before deciding."
                        }
                        li {
                            strong { "You want clean restarts. " }
                            "Restarting ag clears the in-process tiers but not L3 — confusing when you want everything fresh. Disabling makes \"restart to clear caches\" actually true."
                        }
                        li {
                            strong { "Data hygiene. " }
                            "L3 writes serialised search results to its disk volume; the in-process tiers never persist. If your queries or documents are sensitive, that is a footprint to consider."
                        }
                        li {
                            strong { "Memory pressure. " }
                            "Cached results can pile up (hundreds of bytes to a few KB each, no memory limit by default). If RAM is tight and the hit rate is low, drop L3."
                        }
                    }
                    p {
                        strong { "Not a reason: " }
                        "to save memory in ag itself. L3 runs in its own container — disabling it does not shrink ag."
                    }
                }
                button {
                    class: "btn btn-sm w-full mt-4",
                    style: "background-color:#7C2A02;",
                    onclick: move |_| show.set(false),
                    "Got it"
                }
            }
        }
    }
}

/// What the Redis wire protocol (RESP) is — opened from the INFO command modal.
fn wire_protocol_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100",
                        "The Redis wire protocol"
                    }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-3 leading-relaxed",
                    p {
                        "A wire protocol is the agreed format two programs use to talk over a network connection: how a request is framed in bytes, and how the reply comes back. Redis's is called "
                        span { class: "font-mono text-gray-100", "RESP" }
                        " (REdis Serialization Protocol)."
                    }
                    p {
                        "It's deliberately simple and text-based. A command goes out as an array of arguments, and the reply is tagged by its first byte so the client knows what it's reading:"
                    }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            span { class: "font-mono text-gray-100", "+" }
                            " a simple string (e.g. "
                            span { class: "font-mono text-gray-100", "+OK" }
                            ")"
                        }
                        li {
                            span { class: "font-mono text-gray-100", "-" }
                            " an error"
                        }
                        li {
                            span { class: "font-mono text-gray-100", ":" }
                            " an integer"
                        }
                        li {
                            span { class: "font-mono text-gray-100", "$" }
                            " a bulk string (arbitrary bytes, length-prefixed) — how the "
                            span { class: "font-mono text-gray-100", "INFO" }
                            " text block comes back"
                        }
                        li {
                            span { class: "font-mono text-gray-100", "*" }
                            " an array of the above"
                        }
                    }
                    p {
                        "Because the format is small and stable, any client library or store that implements it can interoperate. That's the key point for this page: the L3 cache is a Redis server and FalkorDB is a Redis module, but both speak "
                        span { class: "font-mono text-gray-100", "RESP" }
                        ", so ag can talk to them with one client and read their health the same way."
                    }
                }
                button {
                    class: "btn btn-sm w-full mt-4",
                    style: "background-color:#7C2A02;",
                    onclick: move |_| show.set(false),
                    "Got it"
                }
            }
        }
    }
}

/// What the Redis-protocol `INFO` command is and why both stores answer it.
fn info_command_modal(mut show: Signal<bool>, mut show_wire_protocol: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100",
                        "The INFO command"
                    }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-3 leading-relaxed",
                    p {
                        "Both stores speak the Redis "
                        span {
                            class: "text-blue-400 hover:text-blue-300 underline cursor-pointer",
                            onclick: move |_| show_wire_protocol.set(true),
                            title: "What the wire protocol is",
                            "wire protocol"
                        }
                        ", and "
                        span { class: "font-mono text-gray-100", "INFO" }
                        " is a built-in command in that protocol. Ask a server "
                        span { class: "font-mono text-gray-100", "INFO" }
                        " and it replies with one block of text reporting its version, uptime, memory use, connection count, keyspace hit/miss counters, eviction stats, persistence state, and more. ag reads that reply every 10 seconds and turns it into the health grid on this page."
                    }
                    p {
                        "The reply is grouped into sections — every field on this page comes from one of them:"
                    }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            span { class: "font-mono text-gray-100", "server" }
                            " — Version, Mode, Uptime"
                        }
                        li {
                            span { class: "font-mono text-gray-100", "clients" }
                            " — Connected clients"
                        }
                        li {
                            span { class: "font-mono text-gray-100", "memory" }
                            " — Memory used, Memory limit, Eviction policy"
                        }
                        li {
                            span { class: "font-mono text-gray-100", "stats" }
                            " — Keyspace hit rate, Hits / misses, Evicted keys, Ops / sec, Commands processed"
                        }
                        li {
                            span { class: "font-mono text-gray-100", "persistence" }
                            " — Persistence (AOF), Unsaved changes"
                        }
                    }
                    p {
                        "One field is the exception: "
                        strong { "Keys (DBSIZE)" }
                        " comes from a separate "
                        span { class: "font-mono text-gray-100", "DBSIZE" }
                        " command, which just counts the keys currently stored."
                    }
                    p {
                        "Why both datastores answer it: FalkorDB is a Redis module loaded into a Redis server, so it inherits the whole Redis command set — "
                        span { class: "font-mono text-gray-100", "INFO" }
                        " included. The L3 cache is a plain Redis server. To a protocol client they look like the same kind of store, which is exactly why ag can show one shared health view for both."
                    }
                }
                button {
                    class: "btn btn-sm w-full mt-4",
                    style: "background-color:#7C2A02;",
                    onclick: move |_| show.set(false),
                    "Got it"
                }
            }
        }
    }
}

/// When (and when not) to tick the "Also start/stop the redis container" box.
fn stop_container_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100",
                        "Also start / stop the redis container"
                    }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-3 leading-relaxed",
                    p {
                        "By default the toggle only writes "
                        span { class: "font-mono", "REDIS_ENABLED" }
                        " to "
                        span { class: "font-mono", "ag.env" }
                        " and restarts ag — it does not touch the redis container at all. Ticking this box also runs "
                        span { class: "font-mono", "docker compose up -d redis" }
                        " (when enabling) or "
                        span { class: "font-mono", "docker compose stop redis" }
                        " (when disabling). Two separate decisions, one click."
                    }

                    p {
                        strong { "Enabling L3 — tick this if:" }
                    }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            "The redis container is currently stopped. Without ticking, ag will come back up and silently retry the connection forever."
                        }
                        li {
                            "You only manage the redis container through ag — \"one click, everything ready\" is the simplest path."
                        }
                    }
                    p {
                        strong { "Enabling L3 — leave it unticked if:" }
                    }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            "The container is already running (the L3 panel shows "
                            span { class: "text-green-400 font-semibold", "Connected" }
                            " above). The extra "
                            span { class: "font-mono", "up -d" }
                            " is a no-op but still triggers a compose round-trip."
                        }
                        li {
                            "Another tool or systemd unit on this box manages the redis container and you don't want ag racing it."
                        }
                    }

                    p {
                        strong { "Disabling L3 — tick this if:" }
                    }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            "You're done with L3 for a while and want to reclaim the container's RAM/CPU (small, but non-zero)."
                        }
                        li {
                            "You're cleaning up before a reboot or before sharing the machine."
                        }
                    }
                    p {
                        strong { "Disabling L3 — leave it unticked if:" }
                    }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            "You expect to flip L3 back on soon. Keep the container warm and avoid AOF stop/start churn."
                        }
                        li {
                            "Another tool on this box uses the same redis instance (rare on a single-box dev setup, but possible)."
                        }
                        li {
                            "You're disabling L3 just to A/B-compare retrieval with and without it — the container being up costs nothing while ag isn't talking to it."
                        }
                    }

                    p {
                        strong { "Is it safe? " }
                        "Yes. Stop is graceful ("
                        span { class: "font-mono", "docker compose stop" }
                        " sends SIGTERM), so AOF flushes before exit and data on the "
                        span { class: "font-mono", "redis-data" }
                        " volume persists. Restarting later picks up where it left off."
                    }
                }
                button {
                    class: "btn btn-sm w-full mt-4",
                    style: "background-color:#7C2A02;",
                    onclick: move |_| show.set(false),
                    "Got it"
                }
            }
        }
    }
}

/// Panel-equivalent with a fixed-height header so the two boards line up
/// regardless of how long the title or subtitle is. Used in place of the shared
/// `Panel` component on this page because the side-by-side layout demands
/// pixel-equal headers — wrapping titles would push one board's field rows
/// below the other's and break the middle-column button alignment.
fn aligned_panel(title: &str, subtitle: &str, refresh: &str, children: Element) -> Element {
    rsx! {
        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 shadow",
            div { class: "h-6 mb-3 flex items-center justify-between gap-3 overflow-hidden",
                div { class: "flex items-center gap-3 min-w-0",
                    h3 { class: "text-sm font-semibold text-gray-200 truncate", "{title}" }
                    span { class: "text-[10px] text-gray-400 truncate", "{subtitle}" }
                }
                span { class: "text-xs text-white shrink-0", "{refresh}" }
            }
            div { class: "text-gray-100 text-xs space-y-2", {children} }
        }
    }
}

/// Vertical column of standard rust-color info buttons, one per health field,
/// rendered between the two boards. Each button has the same row height as a
/// `stat()` row so it visually aligns with the corresponding field across both panels.
fn field_button_column() -> Element {
    const FIELDS: [&str; 15] = [
        "Version",
        "Mode",
        "Uptime",
        "Connected clients",
        "Memory used",
        "Memory limit",
        "Eviction policy",
        "Keys (DBSIZE)",
        "Keyspace hit rate",
        "Hits / misses",
        "Evicted keys",
        "Ops / sec",
        "Commands processed",
        "Persistence (AOF)",
        "Unsaved changes",
    ];
    let mut field_help = use_context::<Signal<Option<&'static str>>>();
    rsx! {
        // Faithful clone of `aligned_panel` + a panel section, rendered transparent
        // with invisible content, so the button grid starts at the exact same Y as
        // each panel's `health_panel` grid. Every wrapper here mirrors the real
        // markup byte-for-byte (header, content wrapper, the connection-status row,
        // the `h-8 border-b` rows) — alignment falls out of structural identity
        // rather than any hand-computed pixel offset.
        div { class: "border border-transparent rounded-lg p-4",
            div { class: "h-6 mb-3 flex items-center justify-between gap-3 overflow-hidden" }
            div { class: "text-gray-100 text-xs space-y-2",
                div { class: "space-y-3",
                    div { class: "invisible flex items-center gap-x-2 text-xs h-5 overflow-hidden whitespace-nowrap",
                        span { class: "px-2 py-0.5 rounded font-semibold", "Connected" }
                    }
                    div { class: "grid grid-cols-1",
                        for key in FIELDS.iter() {
                            div { class: "flex items-center justify-center h-8 border-b border-transparent",
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| field_help.set(Some(*key)),
                                    title: "{key}",
                                    svg {
                                        class: INFO_ICON_SVG_CLASS,
                                        view_box: "0 0 20 20",
                                        fill: "none",
                                        stroke: "currentColor",
                                        circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                                        line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
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

/// Plain-text explanation for a single field key, shown by the per-field modal.
fn field_description(key: &str) -> &'static str {
    match key {
        "Version" => "Redis server version reported by INFO server. Tells you what feature set and bug-fix level the store is running.",
        "Mode" => "Standalone, cluster, or sentinel. ag uses standalone for both stores; cluster or sentinel only show up if you've deliberately deployed those.",
        "Uptime" => "Time since the server process started. A short uptime after a recent change is normal; an unexpected reset means something restarted the container.",
        "Connected clients" => "Open TCP connections. ag itself accounts for a handful — the search server, the upload server, hot-reload subscribers. Spikes can indicate a runaway caller.",
        "Memory used" => "Current resident memory of the store process. Watch this against the limit to gauge pressure.",
        "Memory limit" => "Configured maxmemory, or 'unlimited' if unset. Without a limit the store can grow until the host runs out of RAM. ag sets no limit, so this reads 'unlimited' — which also means the eviction policy never takes effect, since there's no ceiling for it to react to. For the L3 cache a limit + LRU eviction is the safe default.",
        "Eviction policy" => "What happens when memory hits the limit. 'noeviction' refuses new writes; 'allkeys-lru' drops the least-recently-used key; 'volatile-ttl' drops the soonest-to-expire key. ag runs both stores on Redis's default — noeviction — with no memory limit set, so neither evicts on its own. For FalkorDB that's the safe choice: the graph never loses data silently. For the L3 cache it means a full store would refuse writes rather than make room; giving it a memory limit plus an LRU policy is the usual change if you'd rather it discard old cached results than reject new ones. Note the policy is moot until a limit exists: with memory unlimited (ag's default), nothing ever triggers eviction no matter which policy is set.",
        "Keys (DBSIZE)" => "Total keys in the default DB. For FalkorDB this counts graph nodes, edges, and metadata keys; for L3 it counts cached search results.",
        "Keyspace hit rate" => "Percentage of GETs that found a value: hits / (hits + misses). A low rate on L3 means the cache isn't earning its keep — your queries are mostly unique.",
        "Hits / misses" => "Raw counters underneath the hit rate. Useful when the percentage rounds to 0% or 100% and you want absolute numbers.",
        "Evicted keys" => "Lifetime count of keys removed by the eviction policy. A growing number means you're hitting the memory limit regularly — consider raising maxmemory or shortening TTL.",
        "Ops / sec" => "How busy the store is right now: a snapshot rate over roughly the last 1.6 seconds, not a running total. It idles at 0 whenever nothing is hitting the store — and these stores only see traffic in bursts (L3 during a search, FalkorDB during ingestion or a graph query), so 0 is the normal resting value, not a fault. Even this page's 10-second health check is too few commands to register as 1/sec. To watch it move you'd have to refresh mid-burst; for cumulative proof the store is being used at all, look at Commands processed instead.",
        "Commands processed" => "Lifetime total commands the server has handled. Useful as a sanity check: is this store actually being talked to?",
        "Persistence (AOF)" => "Whether the append-only-file is enabled. AOF writes every change to disk so an abrupt stop loses little data. AOF off means you only have periodic snapshots — restart-time data loss can be larger.",
        "Unsaved changes" => "Writes since the last RDB snapshot (rdb_changes_since_last_save). High values mean a crash right now would lose that many writes, unless AOF is on.",
        _ => "No description available for this field.",
    }
}

/// Per-field info modal — content is selected by the active field key in `field_help`.
fn field_reference_modal(
    mut field_help: Signal<Option<&'static str>>,
    mut show_sentinel: Signal<bool>,
) -> Element {
    let key = field_help().unwrap_or("");
    let description = field_description(key);
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| field_help.set(None),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-md shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-3",
                    h2 { class: "text-lg font-semibold text-gray-100", "{key}" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| field_help.set(None),
                        "×"
                    }
                }
                if key == "Mode" {
                    p { class: "text-sm text-gray-300 leading-relaxed",
                        "Standalone, cluster, or "
                        span {
                            class: "text-blue-400 hover:text-blue-300 underline cursor-pointer",
                            onclick: move |_| show_sentinel.set(true),
                            title: "What sentinel mode is",
                            "sentinel"
                        }
                        ". ag uses standalone for both stores; cluster or sentinel only show up if you've deliberately deployed those."
                    }
                } else {
                    p { class: "text-sm text-gray-300 leading-relaxed", "{description}" }
                }
                button {
                    class: "btn btn-sm w-full mt-4",
                    style: "background-color:#7C2A02;",
                    onclick: move |_| field_help.set(None),
                    "Got it"
                }
            }
        }
    }
}

/// What Redis Sentinel is — opened from the "sentinel" link in the Mode modal.
fn sentinel_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100",
                        "Sentinel mode"
                    }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-3 leading-relaxed",
                    p {
                        "Sentinel is Redis's built-in high-availability setup. Instead of one server holding your data, you run a primary plus one or more replicas, and a small set of Sentinel processes that watch over them. The Sentinels do three jobs:"
                    }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            strong { "Monitor. " }
                            "Each Sentinel keeps pinging the primary and its replicas to confirm they're alive."
                        }
                        li {
                            strong { "Fail over. " }
                            "If enough Sentinels agree the primary is down (a quorum), they promote a replica to be the new primary and point the others at it — no human needed."
                        }
                        li {
                            strong { "Hand out the address. " }
                            "Clients ask a Sentinel \"who is the primary right now?\" rather than hardcoding it, so they follow the primary across a failover automatically."
                        }
                    }
                    p {
                        "You typically run an odd number of Sentinels (often three) so they can vote and avoid a split-brain where two servers each think they're in charge."
                    }
                    p {
                        strong { "Sentinel vs. cluster: " }
                        "Sentinel is about staying available (automatic failover for a single logical primary); cluster is about scaling (the keyspace sharded across many primaries). They solve different problems."
                    }
                    p {
                        "ag doesn't use either — it runs both stores as plain standalone servers on one box. This mode value only reads "
                        span { class: "font-mono text-gray-100", "sentinel" }
                        " if you've deliberately deployed that topology, so on a normal ag install you'll always see "
                        span { class: "font-mono text-gray-100", "standalone" }
                        "."
                    }
                }
                button {
                    class: "btn btn-sm w-full mt-4",
                    style: "background-color:#7C2A02;",
                    onclick: move |_| show.set(false),
                    "Got it"
                }
            }
        }
    }
}

/// Human-readable byte size.
fn fmt_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} {}", UNITS[0])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

/// Human-readable uptime from a seconds count.
fn fmt_uptime(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3_600;
    let m = (secs % 3_600) / 60;
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}
