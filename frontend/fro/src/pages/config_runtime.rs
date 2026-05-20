//! Runtime settings — the discoverability + override page for any deployment.
//!
//! Reads `GET /runtime/settings`, groups entries by category, renders a
//! kind-appropriate control per setting, and writes through
//! `PUT /runtime/settings/{key}`. Restart-required settings surface a banner
//! that drives `/runtime/actions/restart-self` (universal self re-exec).

use crate::{
    api::{
        self, PutSettingResponse, RuntimeRollback, RuntimeSettingEntry, SettingKind, SettingSource,
    },
    app::Route,
    components::{
        config_nav::{ConfigNav, ConfigTab},
        monitor::*,
    },
};
use dioxus::prelude::*;
use std::collections::BTreeMap;

#[derive(Clone, Default)]
struct PageState {
    loading: bool,
    error: Option<String>,
    entries: Vec<RuntimeSettingEntry>,
    rollback: Option<RuntimeRollback>,
}

#[component]
pub fn ConfigRuntime() -> Element {
    let mut state = use_signal(|| PageState {
        loading: true,
        ..Default::default()
    });
    let feedback = use_signal::<Option<String>>(|| None);
    let mut reload_tick = use_signal(|| 0u32);
    let mut restart_pending = use_signal(|| false);
    let mut restarting = use_signal(|| false);

    use_future(move || async move {
        let _ = reload_tick();
        state.write().loading = true;
        match api::fetch_runtime_settings().await {
            Ok(resp) => {
                state.set(PageState {
                    loading: false,
                    error: None,
                    entries: resp.entries,
                    rollback: resp.last_rollback,
                });
            }
            Err(e) => {
                state.write().loading = false;
                state.write().error = Some(e);
            }
        }
    });

    let snap = state.read().clone();
    let groups: BTreeMap<String, Vec<RuntimeSettingEntry>> = group_by_category(&snap.entries);
    let unregistered: Vec<RuntimeSettingEntry> = snap
        .entries
        .iter()
        .filter(|e| !e.registered)
        .cloned()
        .collect();

    let do_restart = move |_| {
        restarting.set(true);
        spawn(async move {
            let _ = api::post_restart_self().await;
            // Surface for ~10 s while ag re-execs and the page reloads.
            gloo_timers::future::TimeoutFuture::new(10_000).await;
            restarting.set(false);
            restart_pending.set(false);
            reload_tick.with_mut(|t| *t += 1);
        });
    };

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::Config {})),
                    BreadcrumbItem::new("Runtime", None),
                ],
            }
            ConfigNav { active: ConfigTab::Runtime }

            h1 { class: "text-xl font-semibold text-gray-100", "Runtime settings" }
            p { class: "text-sm text-gray-300",
                "Every setting ag knows about. Overrides save to "
                span { class: "font-mono", "<base_dir>/overrides.json" }
                " and take effect immediately for hot-reloadable keys, or after a restart for boot-bound ones."
            }

            if let Some(rb) = &snap.rollback {
                {rollback_banner(rb)}
            }

            if restart_pending() {
                {restart_banner(restarting(), do_restart)}
            }

            if restarting() {
                {restarting_overlay()}
            }

            if let Some(err) = &snap.error {
                div { class: "text-sm text-red-400", "Failed to load: {err}" }
            }
            if snap.loading {
                div { class: "text-sm text-gray-400", "Loading…" }
            }

            for (category, items) in groups {
                Panel { title: Some(format_category(&category)), refresh: None,
                    div { class: "space-y-3",
                        for entry in items {
                            SettingRow {
                                key: "{entry.key}",
                                entry: entry,
                                reload: reload_tick,
                                restart_pending: restart_pending,
                                feedback: feedback,
                            }
                        }
                    }
                }
            }

            if !unregistered.is_empty() {
                Panel {
                    title: Some("Unregistered overrides".to_string()),
                    refresh: None,
                    div { class: "text-xs text-gray-400 mb-3",
                        "Overrides for keys that ag does not currently recognise. They may have been set by an older version, or they are not yet in the known-keys registry."
                    }
                    div { class: "space-y-3",
                        for entry in unregistered {
                            SettingRow {
                                key: "{entry.key}",
                                entry: entry,
                                reload: reload_tick,
                                restart_pending: restart_pending,
                                feedback: feedback,
                            }
                        }
                    }
                }
            }

            if let Some(msg) = feedback() {
                div { class: "fixed bottom-4 right-4 bg-gray-800 border border-gray-600 rounded px-4 py-2 text-sm text-gray-200 shadow-lg",
                    "{msg}"
                }
            }
        }
    }
}

#[component]
fn SettingRow(
    entry: RuntimeSettingEntry,
    reload: Signal<u32>,
    restart_pending: Signal<bool>,
    feedback: Signal<Option<String>>,
) -> Element {
    let initial = entry
        .override_value
        .clone()
        .or(entry.effective.clone())
        .unwrap_or_default();
    let input = use_signal(|| initial.clone());
    let mut row_error = use_signal::<Option<String>>(|| None);
    let mut saving = use_signal(|| false);

    let dirty = *input.read() != initial;
    let has_override = entry.override_value.is_some();
    let key_for_save = entry.key.clone();
    let key_for_clear = entry.key.clone();
    let restart_required = entry.restart_required;

    let save = {
        let mut reload = reload;
        let mut restart_pending = restart_pending;
        let mut feedback = feedback;
        move |value: String| {
            let key = key_for_save.clone();
            spawn(async move {
                saving.set(true);
                row_error.set(None);
                match api::put_runtime_setting(&key, Some(value)).await {
                    Ok(PutSettingResponse {
                        restart_required: rr,
                        ..
                    }) => {
                        feedback.set(Some(format!("Saved {key}")));
                        if rr {
                            restart_pending.set(true);
                        }
                        reload.with_mut(|t| *t += 1);
                    }
                    Err(e) => {
                        row_error.set(Some(e.clone()));
                        feedback.set(Some(format!("Failed to save {key}: {e}")));
                    }
                }
                saving.set(false);
            });
        }
    };

    let clear = {
        let mut reload = reload;
        let mut feedback = feedback;
        move |_| {
            let key = key_for_clear.clone();
            spawn(async move {
                saving.set(true);
                row_error.set(None);
                match api::delete_runtime_setting(&key).await {
                    Ok(()) => {
                        feedback.set(Some(format!("Cleared override for {key}")));
                        if restart_required {
                            restart_pending.set(true);
                        }
                        reload.with_mut(|t| *t += 1);
                    }
                    Err(e) => {
                        row_error.set(Some(e.clone()));
                        feedback.set(Some(format!("Failed to clear {key}: {e}")));
                    }
                }
                saving.set(false);
            });
        }
    };

    let source_label = match entry.source {
        SettingSource::Override => "override",
        SettingSource::Env => "env",
        SettingSource::Default => "default",
        SettingSource::Unset => "unset",
    };
    let source_color = match entry.source {
        SettingSource::Override => "bg-amber-900/40 text-amber-300",
        SettingSource::Env => "bg-blue-900/40 text-blue-300",
        SettingSource::Default => "bg-gray-700 text-gray-300",
        SettingSource::Unset => "bg-gray-800 text-gray-400",
    };

    rsx! {
        div { class: "border border-gray-700 bg-gray-800/40 rounded p-3 space-y-2",
            div { class: "flex flex-wrap items-center gap-2",
                span { class: "font-mono text-sm text-gray-100", "{entry.key}" }
                span { class: "px-2 py-0.5 rounded text-[10px] uppercase tracking-wide {source_color}",
                    "{source_label}"
                }
                if restart_required {
                    span { class: "px-2 py-0.5 rounded text-[10px] uppercase tracking-wide bg-orange-900/40 text-orange-300",
                        "restart"
                    }
                }
                if !entry.registered {
                    span { class: "px-2 py-0.5 rounded text-[10px] uppercase tracking-wide bg-purple-900/40 text-purple-300",
                        "unregistered"
                    }
                }
                if let Some(cat) = &entry.category {
                    span { class: "text-[10px] text-gray-400", "{cat}" }
                }
            }
            if let Some(desc) = &entry.description {
                p { class: "text-xs text-gray-300", "{desc}" }
            }

            div { class: "flex flex-wrap items-center gap-2",
                {render_control(entry.kind.clone(), input)}
                button {
                    class: "btn btn-sm",
                    style: "background-color:#7C2A02;color:white;border:1px solid #7C2A02;",
                    disabled: saving() || !dirty,
                    onclick: move |_| {
                        let v = input.read().clone();
                        save(v);
                    },
                    if saving() { "Saving…" } else { "Save" }
                }
                if has_override {
                    button {
                        class: "btn btn-sm btn-ghost",
                        disabled: saving(),
                        onclick: clear,
                        "Clear override"
                    }
                }
                div { class: "text-[10px] text-gray-400 ml-auto",
                    "env: "
                    span { class: "font-mono text-gray-300",
                        "{entry.env_value.clone().unwrap_or_else(|| \"—\".to_string())}"
                    }
                }
            }

            if let Some(err) = row_error() {
                div { class: "text-xs text-red-400", "Error: {err}" }
            }
        }
    }
}

fn render_control(kind: Option<SettingKind>, mut input: Signal<String>) -> Element {
    match kind {
        Some(SettingKind::Bool) => rsx! {
            label { class: "inline-flex items-center gap-2 text-sm text-gray-200 cursor-pointer select-none",
                input {
                    r#type: "checkbox",
                    class: "cursor-pointer",
                    checked: matches_truthy(&input.read()),
                    oninput: move |evt| {
                        let on = evt.value() == "true" || evt.value() == "on";
                        input.set(if on { "true".into() } else { "false".into() });
                    },
                }
                span { "{input.read()}" }
            }
        },
        Some(SettingKind::Enum(values)) => rsx! {
            select {
                class: "select select-sm select-bordered bg-gray-700 text-gray-200",
                onchange: move |evt| input.set(evt.value()),
                for v in values {
                    option { value: "{v}", selected: *input.read() == *v, "{v}" }
                }
            }
        },
        Some(SettingKind::U64) | Some(SettingKind::F64) => rsx! {
            input {
                r#type: "number",
                class: "input input-sm input-bordered bg-gray-700 text-gray-200 font-mono w-32",
                value: "{input.read()}",
                oninput: move |evt| input.set(evt.value()),
            }
        },
        _ => rsx! {
            input {
                r#type: "text",
                class: "input input-sm input-bordered bg-gray-700 text-gray-200 font-mono flex-1 min-w-40",
                value: "{input.read()}",
                oninput: move |evt| input.set(evt.value()),
            }
        },
    }
}

fn matches_truthy(v: &str) -> bool {
    let v = v.trim().to_lowercase();
    matches!(v.as_str(), "true" | "1" | "yes" | "on")
}

fn rollback_banner(rb: &RuntimeRollback) -> Element {
    let path = rb.last_bad_file.clone();
    let at = rb.rolled_back_at.clone();
    rsx! {
        div { class: "border border-amber-700 bg-amber-900/30 rounded-lg p-4 text-sm space-y-1",
            div { class: "font-semibold text-amber-200",
                "Previous boot did not reach healthy"
            }
            p { class: "text-amber-100",
                "ag rolled back the runtime overrides so this boot could start. The bad file is preserved — open it and re-apply only the keys you want."
            }
            p { class: "text-xs text-amber-200",
                "Rolled back: "
                span { class: "font-mono", "{at}" }
                " · file: "
                span { class: "font-mono", "{path}" }
            }
        }
    }
}

fn restart_banner(in_progress: bool, on_restart: impl FnMut(MouseEvent) + 'static) -> Element {
    rsx! {
        div { class: "border border-orange-700 bg-orange-900/30 rounded-lg p-4 flex items-center justify-between gap-3",
            div { class: "text-sm text-orange-100",
                "A setting that requires a restart was changed. Click to apply via self re-exec — works on any deployment."
            }
            button {
                class: "btn btn-sm",
                style: "background-color:#7C2A02;color:white;border:1px solid #7C2A02;",
                disabled: in_progress,
                onclick: on_restart,
                if in_progress { "Restarting…" } else { "Restart now" }
            }
        }
    }
}

fn restarting_overlay() -> Element {
    rsx! {
        div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            div { class: "bg-gray-800 border border-gray-600 rounded-lg p-6 max-w-md text-center shadow-xl space-y-2",
                div { class: "text-base font-semibold text-gray-100",
                    "Restarting via self re-exec…"
                }
                p { class: "text-sm text-gray-300",
                    "The new process replaces the current one in place. This page will refresh once ag is back up."
                }
            }
        }
    }
}

fn group_by_category(
    entries: &[RuntimeSettingEntry],
) -> BTreeMap<String, Vec<RuntimeSettingEntry>> {
    let mut groups: BTreeMap<String, Vec<RuntimeSettingEntry>> = BTreeMap::new();
    for e in entries {
        if !e.registered {
            continue;
        }
        let cat = e.category.clone().unwrap_or_else(|| "other".to_string());
        groups.entry(cat).or_default().push(e.clone());
    }
    groups
}

fn format_category(cat: &str) -> String {
    let mut chars = cat.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
