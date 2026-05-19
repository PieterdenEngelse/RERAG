use crate::api;
use crate::app::Route;
use crate::components::monitor::{Breadcrumb, BreadcrumbItem, NavTabs, Panel};
use dioxus::prelude::*;

#[component]
pub fn MonitorOnnxStatus() -> Element {
    let mut config = use_signal(|| Option::<api::EmbeddingConfigResponse>::None);
    let mut onnx_config = use_signal(|| Option::<serde_json::Value>::None);
    let mut is_loading = use_signal(|| false);
    let mut error: Signal<Option<String>> = use_signal(|| None);

    let mut fetch_data = move || {
        is_loading.set(true);
        error.set(None);
        spawn(async move {
            // Fetch embedding config
            match api::fetch_embedding_config().await {
                Ok(resp) => config.set(Some(resp)),
                Err(e) => error.set(Some(e)),
            }
            // Fetch ONNX runtime config
            let url = format!("{}/config/onnx", api::resolve_api_base_url());
            if let Ok(resp) = gloo_net::http::Request::get(&url).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    onnx_config.set(Some(json));
                }
            }
            is_loading.set(false);
        });
    };

    use_effect(move || {
        fetch_data();
    });

    let cfg = config();
    let ocfg = onnx_config();

    rsx! {
        div { class: "p-4 space-y-4",
            NavTabs { active: Route::MonitorOnnxStatus {} }

            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("ONNX", None),
                ]
            }

            Panel { title: "Embedding Model Status",
                if is_loading() && cfg.is_none() {
                    div { class: "flex items-center gap-2 py-4 text-xs text-gray-400",
                        div { class: "animate-spin rounded-full h-4 w-4 border-b-2 border-blue-400" }
                        "Loading…"
                    }
                } else if let Some(ref err) = error() {
                    div { class: "bg-red-900/30 border border-red-700 rounded p-3 text-xs text-red-300",
                        "Error: {err}"
                    }
                } else if let Some(ref c) = cfg {
                    div { class: "space-y-2 text-sm",
                        div { class: "grid grid-cols-2 gap-x-6 gap-y-1 text-xs",
                            span { class: "text-gray-400", "Provider" }
                            span { class: "text-gray-200 font-mono", "{c.provider}" }
                            span { class: "text-gray-400", "Model Path" }
                            span { class: "text-gray-200 font-mono", "{c.onnx.model_path}" }
                            span { class: "text-gray-400", "Model Exists" }
                            span { class: if c.onnx.model_exists { "text-green-400" } else { "text-red-400" },
                                if c.onnx.model_exists { "Yes" } else { "No" }
                            }
                            span { class: "text-gray-400", "Ready" }
                            span { class: if c.onnx.ready { "text-green-400" } else { "text-red-400" },
                                if c.onnx.ready { "Yes" } else { "No" }
                            }
                        }
                    }
                }
            }

            if let Some(ref oc) = ocfg {
                if let Some(cfg_obj) = oc.get("config") {
                    Panel { title: "ONNX Runtime Configuration",
                        div { class: "grid grid-cols-2 gap-x-6 gap-y-1 text-xs",
                            span { class: "text-gray-400", "Max Length" }
                            span { class: "text-gray-200 font-mono",
                                "{cfg_obj.get(\"max_length\").and_then(|v| v.as_u64()).unwrap_or(0)}"
                            }
                            span { class: "text-gray-400", "Embedding Dim" }
                            span { class: "text-gray-200 font-mono",
                                "{cfg_obj.get(\"embedding_dim\").and_then(|v| v.as_u64()).unwrap_or(0)}"
                            }
                            span { class: "text-gray-400", "Threads" }
                            span { class: "text-gray-200 font-mono",
                                "{cfg_obj.get(\"num_threads\").and_then(|v| v.as_u64()).unwrap_or(0)}"
                            }
                            span { class: "text-gray-400", "Inter-Op Threads" }
                            span { class: "text-gray-200 font-mono",
                                "{cfg_obj.get(\"inter_op_num_threads\").and_then(|v| v.as_u64()).unwrap_or(0)}"
                            }
                            span { class: "text-gray-400", "Optimization Level" }
                            span { class: "text-gray-200 font-mono",
                                "{cfg_obj.get(\"optimization_level\").and_then(|v| v.as_str()).unwrap_or(\"-\")}"
                            }
                            span { class: "text-gray-400", "Execution Mode" }
                            span { class: "text-gray-200 font-mono",
                                "{cfg_obj.get(\"execution_mode\").and_then(|v| v.as_str()).unwrap_or(\"-\")}"
                            }
                            span { class: "text-gray-400", "Memory Pattern" }
                            span { class: "text-gray-200 font-mono",
                                if cfg_obj.get("enable_mem_pattern").and_then(|v| v.as_bool()).unwrap_or(false) { "Enabled" } else { "Disabled" }
                            }
                            span { class: "text-gray-400", "CPU Mem Arena" }
                            span { class: "text-gray-200 font-mono",
                                if cfg_obj.get("enable_cpu_mem_arena").and_then(|v| v.as_bool()).unwrap_or(false) { "Enabled" } else { "Disabled" }
                            }
                            span { class: "text-gray-400", "Deterministic" }
                            span { class: "text-gray-200 font-mono",
                                if cfg_obj.get("deterministic_compute").and_then(|v| v.as_bool()).unwrap_or(false) { "Yes" } else { "No" }
                            }
                            span { class: "text-gray-400", "Profiling" }
                            span { class: "text-gray-200 font-mono",
                                if cfg_obj.get("enable_profiling").and_then(|v| v.as_bool()).unwrap_or(false) { "Enabled" } else { "Disabled" }
                            }
                        }
                    }
                }
            }

            Panel { title: "Actions",
                div { class: "flex gap-2",
                    button {
                        class: "px-3 py-1.5 rounded text-xs font-medium disabled:opacity-60",
                        style: "background:#2563eb;color:#fff;",
                        disabled: is_loading(),
                        onclick: move |_| fetch_data(),
                        if is_loading() { "Loading…" } else { "Refresh" }
                    }
                }
            }
        }
    }
}
