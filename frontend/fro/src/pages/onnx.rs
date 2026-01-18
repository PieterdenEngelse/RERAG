//! ONNX Runtime Configuration Page

use crate::{
    api,
    app::Route,
    components::config_nav::{ConfigNav, ConfigTab},
    components::monitor::*,
    pages::hardware::components::InfoIcon,
};
use dioxus::prelude::*;

// Styling constants matching hardware page
const PARAM_BLOCK_CLASS: &str = "flex flex-col gap-1 text-xs text-gray-200";
const PARAM_COLUMN_CLASS: &str = "param-column-spacing";
const PARAM_LABEL_CLASS: &str = "text-gray-400 whitespace-nowrap";
const PARAM_NUMBER_INPUT_CLASS: &str = "input input-xs input-bordered bg-gray-700 text-gray-200 !w-24";
const PARAM_SELECT_CLASS: &str = "select select-xs select-bordered bg-gray-700 text-gray-200 w-32";
const PARAM_CHECKBOX_CLASS: &str = "checkbox checkbox-xs border-4 border-white";
const PARAM_ICON_BUTTON_CLASS: &str =
    "w-6 h-6 min-w-6 min-h-6 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80";
const PARAM_ICON_BUTTON_STYLE: &str = "background-color: #1D6B9A; border: 1px solid #1D6B9A;";

#[component]
pub fn ConfigOnnx() -> Element {
    let mut config = use_signal(api::OnnxConfigInfo::default);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| Option::<String>::None);
    let mut saving = use_signal(|| false);
    let mut save_message = use_signal(|| Option::<String>::None);

    // Load config on mount
    use_effect(move || {
        spawn(async move {
            loading.set(true);
            error.set(None);
            match api::fetch_onnx_config().await {
                Ok(resp) => {
                    config.set(resp.config);
                    loading.set(false);
                }
                Err(e) => {
                    error.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    // Save handler
    let save_config = move |_| {
        let current = config.read().clone();
        spawn(async move {
            saving.set(true);
            save_message.set(None);
            
            let request = api::OnnxConfigRequest {
                num_threads: Some(current.num_threads),
                inter_op_num_threads: Some(current.inter_op_num_threads),
                optimization_level: Some(current.optimization_level.clone()),
                execution_mode: Some(current.execution_mode.clone()),
                enable_mem_pattern: Some(current.enable_mem_pattern),
                enable_cpu_mem_arena: Some(current.enable_cpu_mem_arena),
                deterministic_compute: Some(current.deterministic_compute),
                optimized_model_path: Some(current.optimized_model_path.clone()),
                enable_profiling: Some(current.enable_profiling),
                profiling_output_path: Some(current.profiling_output_path.clone()),
                log_id: Some(current.log_id.clone()),
                log_level: Some(current.log_level.clone()),
                log_verbosity: Some(current.log_verbosity),
                use_env_allocators: Some(current.use_env_allocators),
                denormal_as_zero: Some(current.denormal_as_zero),
                enable_quant_qdq: Some(current.enable_quant_qdq),
                enable_double_qdq_remover: Some(current.enable_double_qdq_remover),
                enable_qdq_cleanup: Some(current.enable_qdq_cleanup),
                approximate_gelu: Some(current.approximate_gelu),
                enable_aot_inlining: Some(current.enable_aot_inlining),
                disabled_optimizers: Some(current.disabled_optimizers.clone()),
                use_device_allocator_for_initializers: Some(current.use_device_allocator_for_initializers),
                allow_inter_op_spinning: Some(current.allow_inter_op_spinning),
                allow_intra_op_spinning: Some(current.allow_intra_op_spinning),
                use_prepacking: Some(current.use_prepacking),
                independent_thread_pool: Some(current.independent_thread_pool),
                no_env_execution_providers: Some(current.no_env_execution_providers),
                ..Default::default()
            };
            
            match api::update_onnx_config(request).await {
                Ok(resp) => {
                    config.set(resp.config);
                    save_message.set(Some(resp.message));
                }
                Err(e) => {
                    save_message.set(Some(format!("Error: {}", e)));
                }
            }
            saving.set(false);
        });
    };

    rsx! {
        div { class: "space-y-5",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::Config {})),
                    BreadcrumbItem::new("ONNX", Some(Route::ConfigOnnx {})),
                ],
            }

            ConfigNav { active: ConfigTab::Onnx }

            if loading() {
                Panel { title: None, refresh: None,
                    div { class: "text-xs text-blue-300", "Loading ONNX config…" }
                }
            } else if let Some(err) = error() {
                Panel { title: None, refresh: None,
                    div { class: "text-xs text-red-400", "Error: {err}" }
                }
            } else {
                // Status message
                if let Some(msg) = save_message() {
                    Panel { title: None, refresh: None,
                        div {
                            class: if msg.starts_with("Error") { "text-xs text-red-400" } else { "text-xs text-green-400" },
                            "{msg}"
                        }
                    }

                }

                // ═══════════════════════════════════════════════════════════════
                // GENERAL TILE - All ONNX Runtime Parameters (50 total)
                // ═══════════════════════════════════════════════════════════════
                Panel { title: None, refresh: None,
                    div { class: "flex flex-col gap-2",
                        span { class: "text-base text-gray-100 font-semibold", "General" }
                        span { class: "text-xs text-gray-500 italic mb-2", "ONNX Runtime Parameters (50 total) - restart required to apply changes" }
                        
                        div { class: "flex flex-wrap gap-4 items-stretch",

                // ═══════════════════════════════════════════════════════════════
                // BOARD 1: Session Options (21 parameters)
                // ═══════════════════════════════════════════════════════════════
                div { class: "rounded border border-gray-600 p-4 w-full",
                    span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Session Options (21)" }
                    div { class: "flex flex-wrap gap-28 justify-start",
                        
                        // Column 1: Optimization & Execution
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Optimization" }
                            
                            // graph_optimization_level
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "graph_optimization_level" }
                                div { class: "flex items-center justify-between w-full",
                                    select {
                                        class: PARAM_SELECT_CLASS,
                                        value: "{config().optimization_level}",
                                        onchange: move |e| {
                                            config.write().optimization_level = e.value();
                                        },
                                        option { value: "disable", "0 - Off" }
                                        option { value: "basic", "1 - Basic" }
                                        option { value: "extended", "2 - Extended" }
                                        option { value: "all", "3 - All" }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Graph optimization level",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // execution_mode
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "execution_mode" }
                                div { class: "flex items-center justify-between w-full",
                                    select {
                                        class: PARAM_SELECT_CLASS,
                                        value: "{config().execution_mode}",
                                        onchange: move |e| {
                                            config.write().execution_mode = e.value();
                                        },
                                        option { value: "sequential", "Sequential" }
                                        option { value: "parallel", "Parallel" }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Execution mode",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // execution_order
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "execution_order" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "Default" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Execution order",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // use_deterministic_compute
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().deterministic_compute,
                                        onchange: move |e| {
                                            config.write().deterministic_compute = e.checked();
                                        }
                                    }
                                    label { class: PARAM_LABEL_CLASS, "use_deterministic_compute" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Use deterministic compute",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // optimized_model_filepath
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "optimized_model_filepath" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "text",
                                        class: "input input-xs input-bordered bg-gray-700 text-gray-200 w-48",
                                        value: "{config().optimized_model_path.clone().unwrap_or_default()}",
                                        placeholder: "Leave empty to disable",
                                        oninput: move |e| {
                                            let value = e.value();
                                            config.write().optimized_model_path = if value.trim().is_empty() {
                                                None
                                            } else {
                                                Some(value)
                                            };
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Optimized model filepath",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                        
                        // Column 2: Threading
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Threading" }
                            
                            // intra_op_num_threads
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "intra_op_num_threads" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "number",
                                        min: "0",
                                        max: "64",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: "{config().num_threads}",
                                        oninput: move |e| {
                                            if let Ok(v) = e.value().parse::<usize>() {
                                                config.write().num_threads = v;
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Intra-op num threads",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // inter_op_num_threads
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "inter_op_num_threads" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "number",
                                        min: "0",
                                        max: "64",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: "{config().inter_op_num_threads}",
                                        oninput: move |e| {
                                            if let Ok(v) = e.value().parse::<usize>() {
                                                config.write().inter_op_num_threads = v;
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Inter-op num threads",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            
                            // custom_create_thread_fn
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "custom_create_thread_fn" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "None" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Custom create thread function",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // custom_join_thread_fn
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "custom_join_thread_fn" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "None" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Custom join thread function",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                        
                        // Column 3: Memory
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Memory" }
                            
                            // enable_mem_pattern
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center w-full gap-3",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().enable_mem_pattern,
                                        onchange: move |e| {
                                            config.write().enable_mem_pattern = e.checked();
                                        }
                                    }
                                    label { class: "{PARAM_LABEL_CLASS} flex-1", "enable_mem_pattern" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Enable memory pattern",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // enable_cpu_mem_arena
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center w-full gap-3",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().enable_cpu_mem_arena,
                                        onchange: move |e| {
                                            config.write().enable_cpu_mem_arena = e.checked();
                                        }
                                    }
                                    label { class: "{PARAM_LABEL_CLASS} flex-1", "enable_cpu_mem_arena" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Enable CPU memory arena",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // use_device_allocator_for_initializers
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center w-full gap-3",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().use_device_allocator_for_initializers,
                                        onchange: move |e| {
                                            config.write().use_device_allocator_for_initializers = e.checked();
                                        }
                                    }
                                    label { class: "{PARAM_LABEL_CLASS} flex-1", "use_device_allocator_for_initializers" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Use device allocator for initializers",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // free_dimension_overrides
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "free_dimension_overrides" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "None" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Free dimension overrides",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // session_config_entries
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session_config_entries" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "None" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Session config entries",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                        
                        // Column 4: Profiling & Logging
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Profiling & Logging" }
                            
                            // enable_profiling
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().enable_profiling,
                                        onchange: move |e| {
                                            config.write().enable_profiling = e.checked();
                                        }
                                    }
                                    label { class: PARAM_LABEL_CLASS, "enable_profiling" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Enable profiling",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // profile_file_prefix
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "profile_file_prefix" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "text",
                                        class: "input input-xs input-bordered bg-gray-700 text-gray-200 w-48 font-mono",
                                        value: "{config().profiling_output_path.clone().unwrap_or_default()}",
                                        placeholder: "onnxruntime_profile.json",
                                        oninput: move |e| {
                                            let value = e.value();
                                            config.write().profiling_output_path = if value.trim().is_empty() {
                                                None
                                            } else {
                                                Some(value)
                                            };
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Profile file prefix",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // log_id
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "log_id" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "text",
                                        class: "input input-xs input-bordered bg-gray-700 text-gray-200 w-40",
                                        value: "{config().log_id.clone().unwrap_or_default()}",
                                        placeholder: "Optional",
                                        oninput: move |e| {
                                            let value = e.value();
                                            config.write().log_id = if value.trim().is_empty() {
                                                None
                                            } else {
                                                Some(value)
                                            };
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Log ID",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // log_severity_level
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "log_severity_level" }
                                div { class: "flex items-center justify-between w-full",
                                    select {
                                        class: PARAM_SELECT_CLASS,
                                        value: "{config().log_level}",
                                        onchange: move |e| {
                                            config.write().log_level = e.value();
                                        },
                                        option { value: "verbose", "Verbose" }
                                        option { value: "info", "Info" }
                                        option { value: "warning", "Warning" }
                                        option { value: "error", "Error" }
                                        option { value: "fatal", "Fatal" }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Log severity level",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            // log_verbosity_level
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "log_verbosity_level" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "number",
                                        min: "0",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: "{config().log_verbosity}",
                                        oninput: move |e| {
                                            if let Ok(v) = e.value().parse::<i32>() {
                                                config.write().log_verbosity = v.max(0);
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Log verbosity level",
                                        InfoIcon {}
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

                // ═══════════════════════════════════════════════════════════════
                // SESSION CONFIG KEYS TILE
                // ═══════════════════════════════════════════════════════════════
                Panel { title: None, refresh: None,
                div { class: "rounded border border-gray-600 p-4 w-full",
                    div { class: "flex items-center justify-between mb-3",
                        span { class: "text-sm text-gray-300 font-semibold", "Session Config Keys (15)" }
                    }
                    div { class: "flex flex-wrap gap-28 justify-start",
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Model" }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session.save_model_format" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "ONNX" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Save model format",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session.use_ort_model_bytes_directly" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "0" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Use ORT model bytes directly",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session.use_ort_model_bytes_for_initializers" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "0" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Use ORT model bytes for initializers",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session.disable_prepacking" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: !config().use_prepacking,
                                        onchange: move |e| {
                                            config.write().use_prepacking = !e.checked();
                                        }
                                    }
                                    span { class: "text-gray-400 text-xs", if config().use_prepacking { "Prepacking enabled" } else { "Prepacking disabled" } }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Disable prepacking",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session.use_env_allocators" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().use_env_allocators,
                                        onchange: move |e| {
                                            config.write().use_env_allocators = e.checked();
                                        }
                                    }
                                    span { class: "text-gray-400 text-xs", if config().use_env_allocators { "Enabled" } else { "Disabled" } }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Use environment allocators",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                        
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Threading" }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session.intra_op.allow_spinning" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().allow_intra_op_spinning,
                                        onchange: move |e| {
                                            config.write().allow_intra_op_spinning = e.checked();
                                        }
                                    }
                                    span { class: "text-gray-400 text-xs", if config().allow_intra_op_spinning { "Enabled" } else { "Disabled" } }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Allow intra-op spinning",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session.inter_op.allow_spinning" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().allow_inter_op_spinning,
                                        onchange: move |e| {
                                            config.write().allow_inter_op_spinning = e.checked();
                                        }
                                    }
                                    span { class: "text-gray-400 text-xs", if config().allow_inter_op_spinning { "Enabled" } else { "Disabled" } }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Allow inter-op spinning",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session.intra_op.spin_control" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "0" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Intra-op spin control",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session.dynamic_block_base" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "0" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Dynamic block base",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session.set_denormal_as_zero" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().denormal_as_zero,
                                        onchange: move |e| {
                                            config.write().denormal_as_zero = e.checked();
                                        }
                                    }
                                    span { class: "text-gray-400 text-xs", if config().denormal_as_zero { "Enabled" } else { "Disabled" } }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Set denormal as zero",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                        
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Optimization" }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "session.graph_optimizations_loop_level" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "0" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Graph optimizations loop level",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "optimization.disable_specified_optimizers" }
                                div { class: "flex items-center justify-between w-full",
                                    textarea {
                                        class: "textarea textarea-xs textarea-bordered bg-gray-700 text-gray-200 w-48",
                                        rows: 2,
                                        placeholder: "comma separated",
                                        value: "{config().disabled_optimizers.join(\", \")}",
                                        oninput: move |e| {
                                            let value = e.value();
                                            let list = value
                                                .split(',')
                                                .filter_map(|s| {
                                                    let trimmed = s.trim();
                                                    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
                                                })
                                                .collect::<Vec<_>>();
                                            config.write().disabled_optimizers = list;
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Disable specified optimizers",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "optimization.enable_gelu_approximation" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().approximate_gelu,
                                        onchange: move |e| {
                                            config.write().approximate_gelu = e.checked();
                                        }
                                    }
                                    span { class: "text-gray-400 text-xs", if config().approximate_gelu { "Enabled" } else { "Disabled" } }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Enable GELU approximation",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "optimization.enable_bias_gelu_fusion" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "1" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Enable bias GELU fusion",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "optimization.enable_conv_bn_fusion" }
                                div { class: "flex items-center justify-between w-full",
                                    span { class: "text-gray-400 text-xs", "1" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Enable conv BN fusion",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "optimization.enable_quant_qdq" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().enable_quant_qdq,
                                        onchange: move |e| {
                                            config.write().enable_quant_qdq = e.checked();
                                        }
                                    }
                                    span { class: "text-gray-400 text-xs", if config().enable_quant_qdq { "Enabled" } else { "Disabled" } }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Enable quant QDQ",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "optimization.enable_double_qdq_remover" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().enable_double_qdq_remover,
                                        onchange: move |e| {
                                            config.write().enable_double_qdq_remover = e.checked();
                                        }
                                    }
                                    span { class: "text-gray-400 text-xs", if config().enable_double_qdq_remover { "Enabled" } else { "Disabled" } }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Enable double QDQ remover",
                                        InfoIcon {}
                                    }
                                }
                            }
                            
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "optimization.enable_qdq_cleanup" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: config().enable_qdq_cleanup,
                                        onchange: move |e| {
                                            config.write().enable_qdq_cleanup = e.checked();
                                        }
                                    }
                                    span { class: "text-gray-400 text-xs", if config().enable_qdq_cleanup { "Enabled" } else { "Disabled" } }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        title: "Enable QDQ cleanup",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }

                    div { class: "mt-6 grid grid-cols-1 lg:grid-cols-2 gap-4",
                        div { class: "rounded border border-dashed border-gray-600 p-4",
                            span { class: "text-xs text-gray-400 uppercase tracking-wide", "Run Options (5)" }
                            div { class: "flex flex-wrap gap-5 mt-3",
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold", "Execution" }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "run_tag" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "None" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Run tag",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                                
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold", "Logging" }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "log_severity_level" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "inherit" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Log severity level",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "log_verbosity_level" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "inherit" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Log verbosity level",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "log_tag" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "None" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Log tag",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        div { class: "rounded border border-dashed border-gray-600 p-4",
                            span { class: "text-xs text-gray-400 uppercase tracking-wide", "CPU Execution Provider (9)" }
                            div { class: "flex flex-wrap gap-5 mt-3",
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold", "Threading" }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "intra_op_num_threads" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "0 (auto)" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Intra-op num threads",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "inter_op_num_threads" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "0 (auto)" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Inter-op num threads",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    
                                }
                                
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold", "Arena" }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "use_arena" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "true" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Use arena",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "arena_extend_strategy" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "kNextPowerOfTwo" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Arena extend strategy",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "initial_chunk_size_bytes" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "0" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Initial chunk size bytes",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "max_chunk_size_bytes" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "0" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Max chunk size bytes",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "initial_growth_chunk_size_bytes" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "0" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Initial growth chunk size bytes",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "max_dead_bytes_per_chunk" }
                                        div { class: "flex items-center justify-between w-full",
                                            span { class: "text-gray-400 text-xs", "0" }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                title: "Max dead bytes per chunk",
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                }

                // ═══════════════════════════════════════════════════════════════
                // Model Info tile (read-only)
                // ═══════════════════════════════════════════════════════════════
                Panel { title: None, refresh: None,
                    div { class: "flex flex-col gap-2",
                        span { class: "text-base text-gray-100 font-semibold", "Model Info" }
                        
                        div { class: "flex flex-wrap gap-4 items-stretch",
                            div { class: "rounded border border-gray-600 p-4 w-fit",
                                span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Embedding Model" }
                                div { class: "flex flex-wrap gap-5 justify-start",
                                    div { class: PARAM_COLUMN_CLASS,
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "model_path" }
                                            div { class: "flex items-center justify-between w-full",
                                                span { class: "text-gray-200 text-xs font-mono", "{config().model_path}" }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    title: "Model path",
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "embedding_dim" }
                                            div { class: "flex items-center justify-between w-full",
                                                span { class: "text-gray-200 text-xs", "{config().embedding_dim}" }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    title: "Embedding dimension",
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "max_length" }
                                            div { class: "flex items-center justify-between w-full",
                                                span { class: "text-gray-200 text-xs", "{config().max_length}" }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    title: "Max length",
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ═══════════════════════════════════════════════════════════════
                // Save button
                // ═══════════════════════════════════════════════════════════════
                Panel { title: None, refresh: None,
                    div { class: "flex items-center gap-4",
                        button {
                            class: "btn btn-sm btn-primary",
                            disabled: saving(),
                            onclick: save_config,
                            if saving() { "Saving..." } else { "Save Configuration" }
                        }
                        span { class: "text-xs text-gray-400", "Changes require backend restart to take effect" }
                    }
                }
            }
        }
    }
}
