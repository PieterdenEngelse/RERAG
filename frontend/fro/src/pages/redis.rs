//! Redis / FalkorDB server-parameter tuning page.
//!
//! FalkorDB is a Redis module, so the instance has two parameter layers:
//!   - Redis server params  — read/written via `CONFIG GET/SET`.
//!   - FalkorDB module params — via `GRAPH.CONFIG GET/SET`.
//!
//! Editable rows are runtime-settable and applied live; restart/load-time-only
//! params are shown read-only. Live changes do NOT survive a FalkorDB restart —
//! see docs/redis.md.

use crate::pages::hardware::constants::{
    INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
};
use crate::{
    api,
    components::config_nav::{ConfigNav, ConfigTab},
};
use dioxus::prelude::*;
use std::collections::HashMap;

const PARAM_BLOCK_CLASS: &str = "flex flex-col gap-1 text-xs text-gray-200";
const PARAM_LABEL_CLASS: &str = "text-gray-400 whitespace-nowrap font-mono";
const CARD_CLASS: &str = "rounded border border-gray-600 p-4 w-full";
const CARD_TITLE_CLASS: &str = "text-sm text-gray-300 font-semibold";
const ACTION_BTN_STYLE: &str = "background-color: #1D6B9A; border-color: #1D6B9A; color: white;";
const REDIS_INPUT_CLASS: &str = "input input-xs input-bordered bg-gray-700 text-gray-200 w-40";

/// Info "i" icon — matches the canonical info-button spec.
#[component]
fn InfoIcon() -> Element {
    rsx! {
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

/// Seed the edits map with the current value of every editable parameter.
fn seed_edits(r: &api::RedisConfigResponse) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for p in &r.params {
        m.insert(format!("{}:{}", p.section, p.key), p.value.clone());
    }
    m
}

/// Parse a Redis memory value (`0`, `268435456`, `256mb`, `1gb`, …) into bytes.
fn parse_redis_memory(s: &str) -> Option<u64> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }
    let (num, mult): (&str, u64) = if let Some(n) = s.strip_suffix("kb") {
        (n, 1024)
    } else if let Some(n) = s.strip_suffix("mb") {
        (n, 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("gb") {
        (n, 1024 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix('k') {
        (n, 1000)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 1_000_000)
    } else if let Some(n) = s.strip_suffix('g') {
        (n, 1_000_000_000)
    } else if let Some(n) = s.strip_suffix('b') {
        (n, 1)
    } else {
        return None;
    };
    num.trim()
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(mult))
}

/// Human-readable byte count (binary units).
fn fmt_bytes(n: u64) -> String {
    const K: u64 = 1024;
    const M: u64 = 1024 * 1024;
    const G: u64 = 1024 * 1024 * 1024;
    if n >= G {
        format!("{:.1} GiB", n as f64 / G as f64)
    } else if n >= M {
        format!("{:.0} MiB", n as f64 / M as f64)
    } else if n >= K {
        format!("{:.0} KiB", n as f64 / K as f64)
    } else {
        format!("{n} B")
    }
}

/// Fetch live config and refresh the page state.
async fn load_redis(
    mut resp: Signal<Option<api::RedisConfigResponse>>,
    mut edits: Signal<HashMap<String, String>>,
    mut loading: Signal<bool>,
    mut load_error: Signal<Option<String>>,
) {
    match api::fetch_redis_config().await {
        Ok(r) => {
            edits.set(seed_edits(&r));
            resp.set(Some(r));
            load_error.set(None);
        }
        Err(e) => load_error.set(Some(e)),
    }
    loading.set(false);
}

/// One parameter row: label (+ "restart" tag) + editable input + info button.
/// For `maxmemory`, shows a warning when the edited value exceeds MemoryMax.
fn param_row(
    param: &api::RedisParam,
    mut edits: Signal<HashMap<String, String>>,
    mut field_help: Signal<Option<(String, String)>>,
    memory_max: Option<u64>,
) -> Element {
    let mapkey = format!("{}:{}", param.section, param.key);
    let current = edits()
        .get(&mapkey)
        .cloned()
        .unwrap_or_else(|| param.value.clone());
    let key = param.key.clone();
    let help = param.help.clone();
    let is_restart = param.mode == "restart";
    // maxmemory may not exceed the systemd MemoryMax cgroup cap.
    let mem_warning: Option<String> = if param.section == "redis" && param.key == "maxmemory" {
        match (parse_redis_memory(&current), memory_max) {
            (Some(want), Some(cap)) if want > 0 && want > cap => Some(format!(
                "Exceeds MemoryMax ({}) — the systemd cgroup cap. Raise MemoryMax in \
                 falkordb.service first (daemon-reload + restart), or FalkorDB will be \
                 OOM-killed instead of evicting. Apply will refuse this value.",
                fmt_bytes(cap)
            )),
            _ => None,
        }
    } else {
        None
    };
    rsx! {
        div { class: PARAM_BLOCK_CLASS,
            label { class: PARAM_LABEL_CLASS, "{param.key}" }
            div { class: "flex items-end gap-2",
                input {
                    r#type: "text",
                    class: REDIS_INPUT_CLASS,
                    value: "{current}",
                    oninput: move |evt| {
                        edits.with_mut(|m| {
                            m.insert(mapkey.clone(), evt.value());
                        });
                    },
                }
                button {
                    class: PARAM_ICON_BUTTON_CLASS,
                    style: PARAM_ICON_BUTTON_STYLE,
                    onclick: move |_| field_help.set(Some((key.clone(), help.clone()))),
                    InfoIcon {}
                }
                if is_restart {
                    span {
                        class: "text-[10px] uppercase tracking-wide text-amber-300 border border-amber-700/60 rounded px-1",
                        title: "Applying this restarts FalkorDB",
                        "restart"
                    }
                }
            }
            if let Some(w) = mem_warning {
                div { class: "text-xs text-red-400 mt-1 max-w-[15rem]", "{w}" }
            }
        }
    }
}

#[component]
pub fn ConfigRedis() -> Element {
    let resp = use_signal(|| Option::<api::RedisConfigResponse>::None);
    let loading = use_signal(|| true);
    let load_error = use_signal(|| Option::<String>::None);
    let edits = use_signal(HashMap::<String, String>::new);

    let mut applying = use_signal(|| false);
    let mut apply_msg = use_signal(|| Option::<String>::None);
    let mut apply_err = use_signal(|| Option::<String>::None);

    let mut field_help = use_signal(|| Option::<(String, String)>::None);
    let mut show_help = use_signal(|| false);

    // ── Load on mount ────────────────────────────────────────────────
    use_effect(move || {
        spawn(load_redis(resp, edits, loading, load_error));
    });

    let reload = move |_| {
        spawn(load_redis(resp, edits, loading, load_error));
    };

    let on_apply = move |_| {
        let Some(r) = resp() else {
            return;
        };
        let cur = edits();
        let mut changes = Vec::new();
        for p in &r.params {
            let mk = format!("{}:{}", p.section, p.key);
            if let Some(v) = cur.get(&mk) {
                if v.trim() != p.value.trim() {
                    changes.push(api::RedisChange {
                        section: p.section.clone(),
                        key: p.key.clone(),
                        value: v.trim().to_string(),
                    });
                }
            }
        }
        if changes.is_empty() {
            apply_msg.set(Some("No changes to apply.".to_string()));
            apply_err.set(None);
            return;
        }
        applying.set(true);
        apply_msg.set(Some(format!("Applying {} change(s)…", changes.len())));
        apply_err.set(None);
        spawn(async move {
            match api::apply_redis_config(changes).await {
                Ok(res) => {
                    let failed: Vec<String> = res
                        .results
                        .iter()
                        .filter(|r| !r.ok)
                        .map(|r| format!("{} ({})", r.key, r.error.clone().unwrap_or_default()))
                        .collect();
                    if failed.is_empty() {
                        apply_msg.set(Some(res.message));
                        apply_err.set(None);
                    } else {
                        apply_msg.set(None);
                        apply_err.set(Some(format!("Some changes failed — {}", failed.join("; "))));
                    }
                    load_redis(resp, edits, loading, load_error).await;
                }
                Err(e) => {
                    apply_msg.set(None);
                    apply_err.set(Some(format!("Apply failed: {}", e)));
                }
            }
            applying.set(false);
        });
    };

    rsx! {
        div { class: "p-6 space-y-6 w-full",
            ConfigNav { active: ConfigTab::Redis }

            if loading() {
                div { class: "flex items-center justify-center py-8",
                    span { class: "loading loading-spinner loading-lg text-primary" }
                }
            } else if let Some(err) = load_error() {
                div { class: "alert alert-error", span { "{err}" } }
            } else if let Some(r) = resp() {
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 shadow space-y-4",
                    // Header
                    div { class: "flex items-start justify-between flex-wrap gap-3",
                        div { class: "flex items-center gap-3 flex-wrap",
                            h3 { class: "text-sm font-semibold text-gray-200", "Redis / FalkorDB Server" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_help.set(true),
                                InfoIcon {}
                            }
                            span { class: "text-xs font-semibold text-gray-400", "Status:" }
                            span {
                                class: if r.connected { "text-xs font-semibold text-green-400" } else { "text-xs font-semibold text-red-400" },
                                if r.connected { "Connected" } else { "Unreachable" }
                            }
                            if r.connected && !r.redis_version.is_empty() {
                                span { class: "text-xs text-gray-400", "Redis {r.redis_version}" }
                            }
                            if r.connected && !r.used_memory_human.is_empty() {
                                span { class: "text-xs text-gray-400", "mem {r.used_memory_human}" }
                            }
                            if let Some(cap) = r.memory_max_bytes {
                                span { class: "text-xs text-gray-400",
                                    {format!("MemoryMax {}", fmt_bytes(cap))}
                                }
                            }
                        }
                        div { class: "flex items-center gap-2",
                            if r.connected {
                                button {
                                    class: "btn btn-sm",
                                    style: ACTION_BTN_STYLE,
                                    onclick: on_apply,
                                    disabled: applying(),
                                    if applying() { "Applying…" } else { "Apply" }
                                }
                            }
                            button {
                                class: "btn btn-sm btn-outline",
                                onclick: reload,
                                "Reload"
                            }
                        }
                    }

                    if !r.connected {
                        div { class: "alert alert-error",
                            span { "{r.message}" }
                        }
                    } else {
                        // Apply feedback
                        if let Some(m) = apply_msg() {
                            div { class: "text-xs text-green-400", "{m}" }
                        }
                        if let Some(e) = apply_err() {
                            div { class: "text-xs text-red-400", "{e}" }
                        }

                        // Non-persistence warning
                        div { class: "rounded border border-amber-700/60 bg-amber-950/40 text-amber-200 text-xs p-2",
                            "Two-mode Apply — runtime parameters are set live via CONFIG SET / "
                            "GRAPH.CONFIG SET (immediate, not persisted across a manual restart). Rows "
                            "tagged "
                            span { class: "uppercase font-semibold", "restart" }
                            " are written into the falkordb.service unit and applied by restarting "
                            "FalkorDB (persisted) — the unit is backed up first and rolled back if "
                            "FalkorDB fails to come up."
                        }

                        // Redis server params
                        div { class: CARD_CLASS,
                            div { class: "mb-3",
                                span { class: CARD_TITLE_CLASS, "Redis Server" }
                                span { class: "text-xs text-gray-400 ml-2 font-mono", "CONFIG GET/SET" }
                            }
                            div { class: "grid grid-cols-2 lg:grid-cols-3 gap-x-8 gap-y-3",
                                for param in r.params.iter().filter(|p| p.section == "redis") {
                                    {param_row(param, edits, field_help, r.memory_max_bytes)}
                                }
                            }
                        }

                        // FalkorDB module params
                        div { class: CARD_CLASS,
                            div { class: "mb-3",
                                span { class: CARD_TITLE_CLASS, "FalkorDB Module" }
                                span { class: "text-xs text-gray-400 ml-2 font-mono", "GRAPH.CONFIG GET/SET" }
                            }
                            div { class: "grid grid-cols-2 lg:grid-cols-3 gap-x-8 gap-y-3",
                                for param in r.params.iter().filter(|p| p.section == "falkordb") {
                                    {param_row(param, edits, field_help, r.memory_max_bytes)}
                                }
                            }
                        }
                    }
                }

                // Per-field info modal
                if let Some((key, help)) = field_help() {
                    div {
                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                        onclick: move |_| field_help.set(None),
                        div {
                            class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[90vw] max-w-lg max-h-[90vh] overflow-y-auto shadow-xl",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-center justify-between mb-3",
                                h2 { class: "text-base font-semibold text-gray-100 font-mono", "{key}" }
                                button {
                                    class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                    onclick: move |_| field_help.set(None),
                                    "×"
                                }
                            }
                            p { class: "text-sm text-gray-300", "{help}" }
                        }
                    }
                }

                // Page help modal
                if show_help() {
                    div {
                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                        onclick: move |_| show_help.set(false),
                        div {
                            class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[90vw] max-w-2xl max-h-[90vh] overflow-y-auto shadow-xl",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-center justify-between mb-3",
                                h2 { class: "text-base font-semibold text-gray-100", "Redis / FalkorDB Server Parameters" }
                                button {
                                    class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                    onclick: move |_| show_help.set(false),
                                    "×"
                                }
                            }
                            div { class: "text-sm text-gray-300 space-y-3",
                                p { "FalkorDB is a Redis module, so its instance is tuned at two layers — both shown here:" }
                                ul { class: "list-disc list-inside space-y-1 text-gray-400",
                                    li { "Redis Server — the Redis process itself (memory ceiling, persistence, networking), read and written via CONFIG GET/SET." }
                                    li { "FalkorDB Module — the GraphBLAS query engine (query timeouts, memory caps, result limits), via GRAPH.CONFIG GET/SET." }
                                }
                                p { "Every row is editable. Apply routes each change by mode: runtime parameters via CONFIG SET / GRAPH.CONFIG SET (live, not persisted across a manual restart); 'restart' parameters by rewriting the falkordb.service unit's ExecStart, then daemon-reload + restart (persisted)." }
                                p { class: "text-amber-200",
                                    "Before a restart-mode change the unit is backed up; if FalkorDB fails to come up it is rolled back automatically. A restart-mode Apply restarts FalkorDB — use Reconnect on the FalkorDB page afterward if graph features are needed."
                                }
                                p { class: "text-gray-400",
                                    "Connection settings (URI, password, pool) live on the FalkorDB config page; this page tunes the server those settings connect to."
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
