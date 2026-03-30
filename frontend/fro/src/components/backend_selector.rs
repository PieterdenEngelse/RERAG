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
    let mut runtime_ctx = use_context::<Signal<crate::app::RuntimeContext>>();

    // Fetch active backend once on mount — broadcast handles cross-tab updates
    {
        let mut runtime_ctx = runtime_ctx.clone();
        use_future(move || async move {
            if let Ok(health) = api::fetch_runtime_health().await {
                runtime_ctx.with_mut(|ctx| {
                    ctx.active_backend = health.active_backend;
                });
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
                    let selected_value = evt.value();
                    current_backend.set(selected_value.clone());
                    let clear_model = clear_model_on_change;
                    let mut runtime_ctx = runtime_ctx.clone();
                    spawn(async move {
                        runtime_ctx.with_mut(|ctx| ctx.switching = true);
                        // Switch runtime (returns immediately)
                        let _ = api::switch_runtime(&selected_value).await;
                        // Poll until ready (max 30 attempts, 500ms each = 15s)
                        for _ in 0..30 {
                            gloo_timers::future::TimeoutFuture::new(500).await;
                            if let Ok(health) = api::fetch_runtime_health().await {
                                if health.active_backend.as_deref() == Some(&selected_value) {
                                    runtime_ctx.with_mut(|ctx| {
                                        ctx.active_backend = health.active_backend;
                                        ctx.configured_backend = selected_value.clone();
                                    });
                                    break;
                                }
                            }
                        }
                        runtime_ctx.with_mut(|ctx| ctx.switching = false);
                        // Notify parent of backend change
                        if let Some(handler) = on_backend_changed {
                            handler.call(selected_value.clone());
                        }
                        // Then save config
                        if let Ok(mut config) = api::fetch_hardware_config().await {
                            config.config.backend_type = selected_value;
                            if clear_model {
                                config.config.model.clear();
                            }
                            let _ = api::commit_hardware_config(&config.config).await;
                        }
                    });
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
            } else if let Some(ref backend) = runtime_ctx().active_backend {
                p {
                    class: "text-xs text-gray-400 mt-1",
                    "Active: {backend}"
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
                        spawn(async move {
                            if let Ok(mut config) = api::fetch_hardware_config().await {
                                config.config.backend_type = backend_value;
                                if api::commit_hardware_config(&config.config).await.is_ok() {
                                    save_status.set("Saved".to_string());
                                    gloo_timers::future::TimeoutFuture::new(1500).await;
                                    save_status.set("Save".to_string());
                                }
                            }
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
