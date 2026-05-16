use crate::api::{self, BackendType};
use crate::pages::hardware::constants::INFO_ICON_SVG_CLASS;
use dioxus::prelude::*;

/// A reusable backend selector dropdown component.
/// When the backend changes, it saves to the hardware config and optionally clears the model.
#[component]
pub fn BackendSelector(
    current_backend: Signal<String>,
    #[props(default = true)] clear_model_on_change: bool,
    #[props(default = false)] show_save_button: bool,
    #[props(default = false)] show_info_button: bool,
    #[props(default)] on_backend_changed: Option<EventHandler<String>>,
) -> Element {
    let backend_options = BackendType::all();
    let mut save_status = use_signal(|| "Save".to_string());
    let mut show_backend_info = use_signal(|| false);
    // Use shared runtime context
    let runtime_ctx = use_context::<Signal<crate::app::RuntimeContext>>();

    // Poll the active backend so the board self-heals when a runtime
    // starts or stops while the page is open (first fetch is immediate).
    {
        let mut runtime_ctx = runtime_ctx.clone();
        use_future(move || async move {
            loop {
                if let Ok(health) = api::fetch_runtime_health().await {
                    runtime_ctx.with_mut(|ctx| {
                        ctx.active_backend = health.active_backend;
                    });
                }
                gloo_timers::future::TimeoutFuture::new(5000).await;
            }
        });
    }

    rsx! {
        div {
            class: "flex flex-col items-center gap-2",
            select {
                class: "select select-sm select-bordered bg-gray-700 text-gray-200",
                value: current_backend(),
                onchange: move |evt| {
                    let selected = evt.value();
                    current_backend.set(selected.clone());
                    // Selection only updates the dropdown and reflects the pending
                    // choice in the runtime board; the Save button applies it.
                    let mut runtime_ctx = runtime_ctx.clone();
                    runtime_ctx.with_mut(|ctx| ctx.configured_backend = selected);
                },
                for option in backend_options.iter() {
                    option {
                        value: option.to_api_string(),
                        selected: current_backend() == option.to_api_string(),
                        "{option.label()}"
                    }
                }
            }
            // Show active backend or switching state
            if runtime_ctx().switching {
                p {
                    class: "text-xs text-yellow-400 mt-1",
                    "Starting..."
                }
            } else {
                {
                    let ctx = runtime_ctx();
                    let configured = &ctx.configured_backend;
                    let active = ctx.active_backend.as_deref().unwrap_or("");
                    let is_cloud = matches!(configured.as_str(), "openai" | "anthropic" | "openrouter");
                    // Cloud backends have no local process to health-check — skip the discrepancy warning
                    if !is_cloud && !configured.is_empty() && !active.is_empty() && configured != active {
                        rsx! {
                            p {
                                class: "text-xs text-yellow-400 mt-1",
                                "Configured: {configured} | Running: {active}"
                            }
                        }
                    } else if !active.is_empty() || (is_cloud && !configured.is_empty()) {
                        let display = if is_cloud && active.is_empty() { configured.as_str() } else { active };
                        rsx! {
                            p {
                                class: "text-xs text-gray-400 mt-1",
                                "Active: {display}"
                            }
                        }
                    } else if !configured.is_empty() {
                        rsx! {
                            p {
                                class: "text-xs text-gray-400 mt-1",
                                "Configured: {configured}"
                            }
                        }
                    } else {
                        rsx! {}
                    }
                }
            }
            if show_info_button {
                button {
                    class: "shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80 pointer-events-auto",
                    style: "width: 1.5rem; height: 1.5rem; min-width: 1.5rem; min-height: 1.5rem; background-color: #1D6B9A; border: 1px solid #1D6B9A;",
                    onclick: move |_| show_backend_info.set(true),
                    title: "Info about backend selection",
                    svg {
                        class: INFO_ICON_SVG_CLASS,
                        view_box: "0 0 20 20",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.5",
                        circle { cx: "10", cy: "10", r: "9" }
                        line { x1: "10", y1: "8", x2: "10", y2: "14" }
                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                    }
                }
            }
            if show_save_button {
                button {
                    class: "shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80 pointer-events-auto text-white text-sm font-medium",
                    style: "background-color: #1D6B9A; border: 1px solid #1D6B9A; padding: 0.25rem 1rem;",
                    onclick: move |_| {
                        let backend_value = current_backend();
                        let clear_model = clear_model_on_change;
                        let mut runtime_ctx = runtime_ctx.clone();
                        spawn(async move {
                            save_status.set("Saving...".to_string());
                            // Local runtimes have a service to start; cloud backends do not.
                            let is_local = matches!(
                                backend_value.as_str(),
                                "ollama" | "llama_cpp"
                            );
                            if is_local {
                                runtime_ctx.with_mut(|ctx| ctx.switching = true);
                                let _ = api::switch_runtime(&backend_value).await;
                                // Poll until the runtime reports ready (max 30 × 500ms = 15s)
                                for _ in 0..30 {
                                    gloo_timers::future::TimeoutFuture::new(500).await;
                                    if let Ok(health) = api::fetch_runtime_health().await {
                                        if health.active_backend.as_deref()
                                            == Some(&backend_value)
                                        {
                                            break;
                                        }
                                    }
                                }
                                // Refresh the board with whatever actually came up.
                                if let Ok(health) = api::fetch_runtime_health().await {
                                    runtime_ctx
                                        .with_mut(|ctx| ctx.active_backend = health.active_backend);
                                }
                                runtime_ctx.with_mut(|ctx| ctx.switching = false);
                            }
                            // Notify parent of backend change
                            if let Some(handler) = on_backend_changed {
                                handler.call(backend_value.clone());
                            }
                            // Persist the choice
                            let mut saved = false;
                            if let Ok(mut config) = api::fetch_hardware_config().await {
                                let backend_changed =
                                    config.config.backend_type != backend_value;
                                config.config.backend_type = backend_value.clone();
                                if clear_model && backend_changed {
                                    config.config.model.clear();
                                }
                                if api::commit_hardware_config(&config.config).await.is_ok() {
                                    runtime_ctx.with_mut(|ctx| {
                                        ctx.configured_backend = backend_value.clone();
                                    });
                                    saved = true;
                                }
                            }
                            if saved {
                                save_status.set("Saved".to_string());
                                gloo_timers::future::TimeoutFuture::new(1500).await;
                            }
                            save_status.set("Save".to_string());
                        });
                    },
                    "{save_status}"
                }
            }
        }

        // Backend info modal
        if show_backend_info() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_backend_info.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-md max-h-[95vh] overflow-y-auto shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-lg font-semibold text-gray-100", "Inference backend" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_backend_info.set(false),
                            "×"
                        }
                    }
                    div { class: "text-sm text-gray-300 space-y-3",
                        p { "Select the runtime that executes prompts (local llama.cpp, vLLM, OpenAI, etc.)." }
                        p { "Switching backend clears the model name so you can pick a compatible artifact." }
                    }
                }
            }
        }
    }
}
