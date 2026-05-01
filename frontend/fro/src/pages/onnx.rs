//! ONNX Runtime Configuration Page

use crate::pages::hardware::constants::{PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE};
use crate::{
    api,
    app::{PageErrors, Route},
    components::config_nav::{ConfigNav, ConfigTab},
    components::monitor::*,
    pages::hardware::components::{info_modal, InfoIcon},
    pages::onnx_help::OnnxHelpTopic,
};
use dioxus::prelude::*;

/// Returns ONNX config with default values matching the backend defaults
fn onnx_defaults() -> api::OnnxConfigInfo {
    api::OnnxConfigInfo {
        model_path: "models/embedding_model.onnx".to_string(),
        max_length: 512,
        embedding_dim: 384,
        num_threads: 4,
        inter_op_num_threads: 1,
        optimization_level: "all".to_string(),
        execution_mode: "sequential".to_string(),
        enable_mem_pattern: true,
        enable_cpu_mem_arena: true,
        deterministic_compute: false,
        optimized_model_path: None,
        enable_profiling: false,
        profiling_output_path: None,
        log_id: None,
        log_level: "info".to_string(),
        log_verbosity: 0,
        use_env_allocators: false,
        denormal_as_zero: false,
        enable_quant_qdq: true,
        enable_double_qdq_remover: true,
        enable_qdq_cleanup: false,
        approximate_gelu: false,
        enable_aot_inlining: true,
        disabled_optimizers: Vec::new(),
        use_device_allocator_for_initializers: false,
        allow_inter_op_spinning: true,
        allow_intra_op_spinning: true,
        use_prepacking: true,
        independent_thread_pool: false,
        no_env_execution_providers: false,
        embedding_batch_size: 32,
        layout_ml_compiled: false,
        layout_ml_enabled: false,
        layout_model_ready: false,
    }
}

// Styling constants matching hardware page
const PARAM_BLOCK_CLASS: &str = "flex flex-col gap-1 text-xs text-gray-200";
const PARAM_COLUMN_CLASS: &str = "param-column-spacing";
const PARAM_LABEL_CLASS: &str = "text-gray-400 whitespace-nowrap";
const PARAM_NUMBER_INPUT_CLASS: &str =
    "input input-xs input-bordered bg-gray-700 text-gray-200 !w-24";
const PARAM_SELECT_CLASS: &str = "select select-xs select-bordered bg-gray-700 text-gray-200 w-32";
const PARAM_CHECKBOX_CLASS: &str = "checkbox checkbox-xs onnx-checkbox";
// removed local constant
// const PARAM_ICON_BUTTON_CLASS removed (using shared constant)

#[component]
pub fn ConfigOnnx() -> Element {
    let mut config = use_signal(api::OnnxConfigInfo::default);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| Option::<String>::None);
    let mut saving = use_signal(|| false);
    let mut save_message = use_signal(|| Option::<String>::None);
    let mut show_defaults_info = use_signal(|| false);

    // Info modal signals for each parameter
    let mut show_graph_opt_info = use_signal(|| false);
    let mut show_exec_mode_info = use_signal(|| false);
    let mut show_num_threads_info = use_signal(|| false);
    let mut show_inter_op_threads_info = use_signal(|| false);
    let mut show_mem_pattern_info = use_signal(|| false);
    let mut show_cpu_mem_arena_info = use_signal(|| false);
    let mut show_deterministic_info = use_signal(|| false);
    let mut show_opt_model_path_info = use_signal(|| false);
    let mut show_profiling_info = use_signal(|| false);
    let mut _show_profiling_path_info = use_signal(|| false);
    let mut _show_log_id_info = use_signal(|| false);
    let mut show_log_level_info = use_signal(|| false);
    let mut _show_log_verbosity_info = use_signal(|| false);
    let mut _show_env_allocators_info = use_signal(|| false);
    let mut _show_denormal_info = use_signal(|| false);
    let mut _show_device_alloc_info = use_signal(|| false);
    let mut _show_inter_spin_info = use_signal(|| false);
    let mut _show_intra_spin_info = use_signal(|| false);
    let mut _show_prepacking_info = use_signal(|| false);
    let mut _show_indep_pool_info = use_signal(|| false);
    let mut _show_no_env_ep_info = use_signal(|| false);
    let mut show_quant_qdq_info = use_signal(|| false);
    let mut show_double_qdq_info = use_signal(|| false);
    let mut _show_qdq_cleanup_info = use_signal(|| false);
    let mut _show_approx_gelu_info = use_signal(|| false);
    let mut _show_aot_inlining_info = use_signal(|| false);
    let mut _show_disabled_opt_info = use_signal(|| false);
    let mut _show_model_path_info = use_signal(|| false);
    let mut _show_embed_dim_info = use_signal(|| false);
    let mut _show_max_length_info = use_signal(|| false);
    let mut show_embed_batch_size_info = use_signal(|| false);
    let mut show_layout_ml_info = use_signal(|| false);
    // Session Options (read-only / advanced)
    let mut show_exec_order_info = use_signal(|| false);
    let mut show_create_thread_info = use_signal(|| false);
    let mut show_join_thread_info = use_signal(|| false);
    let mut show_free_dim_info = use_signal(|| false);
    let mut show_session_config_info = use_signal(|| false);
    // Session Config Keys
    let mut show_save_model_fmt_info = use_signal(|| false);
    let mut show_ort_bytes_direct_info = use_signal(|| false);
    let mut show_ort_bytes_init_info = use_signal(|| false);
    let mut show_intra_spin_ctrl_info = use_signal(|| false);
    let mut show_dyn_block_info = use_signal(|| false);
    let mut show_graph_opt_loop_info = use_signal(|| false);
    let mut show_bias_gelu_info = use_signal(|| false);
    let mut show_conv_bn_info = use_signal(|| false);
    // Run Options
    let mut show_run_tag_info = use_signal(|| false);
    let mut show_run_log_sev_info = use_signal(|| false);
    let mut show_run_log_verb_info = use_signal(|| false);
    let mut show_log_tag_info = use_signal(|| false);
    // CPU Execution Provider
    let mut show_ep_intra_threads_info = use_signal(|| false);
    let mut show_ep_inter_threads_info = use_signal(|| false);
    let mut show_use_arena_info = use_signal(|| false);
    let mut show_arena_extend_info = use_signal(|| false);
    let mut show_init_chunk_info = use_signal(|| false);
    let mut show_max_chunk_info = use_signal(|| false);
    let mut show_growth_chunk_info = use_signal(|| false);
    let mut show_dead_bytes_info = use_signal(|| false);

    // Get global page errors context
    let mut page_errors = use_context::<Signal<PageErrors>>();

    // Load config on mount
    use_effect(move || {
        spawn(async move {
            loading.set(true);
            error.set(None);
            page_errors.with_mut(|e| e.clear_error("onnx"));
            match api::fetch_onnx_config().await {
                Ok(resp) => {
                    config.set(resp.config);
                    loading.set(false);
                    page_errors.with_mut(|e| e.clear_error("onnx"));
                }
                Err(e) => {
                    error.set(Some(e.clone()));
                    loading.set(false);
                    page_errors.with_mut(|errs| errs.set_error("onnx", &e));
                    let _ = api::log_frontend_error("onnx", &e).await;
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
                use_device_allocator_for_initializers: Some(
                    current.use_device_allocator_for_initializers,
                ),
                allow_inter_op_spinning: Some(current.allow_inter_op_spinning),
                allow_intra_op_spinning: Some(current.allow_intra_op_spinning),
                use_prepacking: Some(current.use_prepacking),
                independent_thread_pool: Some(current.independent_thread_pool),
                no_env_execution_providers: Some(current.no_env_execution_providers),
                embedding_batch_size: Some(current.embedding_batch_size),
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
                // LAYOUT ML STATUS TILE
                // ═══════════════════════════════════════════════════════════════
                Panel { title: None, refresh: None,
                    div { class: "flex flex-col gap-2",
                        div { class: "flex items-center gap-2 mb-1",
                            span { class: "text-base text-gray-100 font-semibold", "Native PDF Extraction" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_layout_ml_info.set(true),
                                crate::pages::hardware::components::InfoIcon {}
                            }
                        }
                        div { class: "grid grid-cols-3 gap-2 text-xs",
                            div { class: "bg-gray-800 rounded p-2",
                                div { class: "text-gray-400 mb-1", "Feature compiled" }
                                div {
                                    class: if config().layout_ml_compiled { "text-green-400 font-semibold" } else { "text-gray-500" },
                                    if config().layout_ml_compiled { "yes" } else { "no (build without layout_ml)" }
                                }
                            }
                            div { class: "bg-gray-800 rounded p-2",
                                div { class: "text-gray-400 mb-1", "Enabled" }
                                div {
                                    class: if config().layout_ml_enabled { "text-green-400 font-semibold" } else { "text-gray-500" },
                                    if config().layout_ml_enabled { "yes (LAYOUT_ML_ENABLED=true)" } else { "no (set LAYOUT_ML_ENABLED=true)" }
                                }
                            }
                            div { class: "bg-gray-800 rounded p-2",
                                div { class: "text-gray-400 mb-1", "Layout model" }
                                div {
                                    class: if config().layout_model_ready { "text-green-400 font-semibold" } else { "text-yellow-400" },
                                    if config().layout_model_ready { "ORT (PubLayNet)" } else { "heuristic only" }
                                }
                            }
                        }
                    }
                }

                // ═══════════════════════════════════════════════════════════════
                // GENERAL TILE - All ONNX Runtime Parameters (50 total)
                // ═══════════════════════════════════════════════════════════════
                Panel { title: None, refresh: None,
                    div { class: "flex flex-col gap-2",
                        // Header row with title and save button
                        div { class: "flex items-center justify-between",
                            div { class: "flex flex-col",
                                span { class: "text-base text-gray-100 font-semibold", "General" }
                                span { class: "text-xs text-gray-500 italic", "ONNX Runtime Parameters (50 total) - restart required to apply changes" }
                            }
                            div { class: "flex items-center gap-2",
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| show_defaults_info.set(true),
                                    title: "View default values",
                                    InfoIcon {}
                                }
                                button {
                                    class: "btn btn-sm btn-outline text-gray-300 border-gray-500 hover:bg-gray-700 hover:border-gray-500",
                                    onclick: move |_| {
                                        config.set(onnx_defaults());
                                    },
                                    "Reset to Defaults"
                                }
                                button {
                                    class: "btn btn-sm text-white",
                                    style: "background-color: #1D6B9A; border-color: #1D6B9A;",
                                    disabled: saving(),
                                    onclick: save_config,
                                    if saving() { "Saving..." } else { "Save Configuration" }
                                }
                            }
                        }

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
                                        onclick: move |_| show_graph_opt_info.set(true),
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
                                        onclick: move |_| show_exec_mode_info.set(true),
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
                                        onclick: move |_| show_exec_order_info.set(true),
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
                                        onclick: move |_| show_deterministic_info.set(true),
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
                                        onclick: move |_| show_opt_model_path_info.set(true),
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
                                        onclick: move |_| show_num_threads_info.set(true),
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
                                        onclick: move |_| show_inter_op_threads_info.set(true),
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
                                        onclick: move |_| show_create_thread_info.set(true),
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
                                        onclick: move |_| show_join_thread_info.set(true),
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
                                        onclick: move |_| show_mem_pattern_info.set(true),
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
                                        onclick: move |_| show_cpu_mem_arena_info.set(true),
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
                                        onclick: move |_| _show_device_alloc_info.set(true),
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
                                        onclick: move |_| show_free_dim_info.set(true),
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
                                        onclick: move |_| show_session_config_info.set(true),
                                        title: "Session config entries",
                                        InfoIcon {}
                                    }
                                }
                            }

                            // embedding_batch_size
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "embedding_batch_size" }
                                div { class: "flex items-center justify-between w-full",
                                    input {
                                        r#type: "number",
                                        min: "1",
                                        max: "512",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: "{config().embedding_batch_size}",
                                        oninput: move |e| {
                                            if let Ok(v) = e.value().parse::<usize>() {
                                                config.write().embedding_batch_size = v.max(1);
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_embed_batch_size_info.set(true),
                                        title: "Embedding batch size",
                                        InfoIcon {}
                                    }
                                }
                                span { class: "text-gray-500 text-xs italic", "live — no restart needed" }
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
                                        onclick: move |_| show_profiling_info.set(true),
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
                                        onclick: move |_| _show_profiling_path_info.set(true),
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
                                        onclick: move |_| _show_log_id_info.set(true),
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
                                        onclick: move |_| show_log_level_info.set(true),
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
                                        onclick: move |_| _show_log_verbosity_info.set(true),
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
                                        onclick: move |_| show_save_model_fmt_info.set(true),
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
                                        onclick: move |_| show_ort_bytes_direct_info.set(true),
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
                                        onclick: move |_| show_ort_bytes_init_info.set(true),
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
                                        onclick: move |_| _show_prepacking_info.set(true),
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
                                        onclick: move |_| _show_env_allocators_info.set(true),
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
                                        onclick: move |_| _show_intra_spin_info.set(true),
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
                                        onclick: move |_| _show_inter_spin_info.set(true),
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
                                        onclick: move |_| show_intra_spin_ctrl_info.set(true),
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
                                        onclick: move |_| show_dyn_block_info.set(true),
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
                                        onclick: move |_| _show_denormal_info.set(true),
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
                                        onclick: move |_| show_graph_opt_loop_info.set(true),
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
                                        onclick: move |_| _show_disabled_opt_info.set(true),
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
                                        onclick: move |_| _show_approx_gelu_info.set(true),
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
                                        onclick: move |_| show_bias_gelu_info.set(true),
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
                                        onclick: move |_| show_conv_bn_info.set(true),
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
                                        onclick: move |_| show_quant_qdq_info.set(true),
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
                                        onclick: move |_| show_double_qdq_info.set(true),
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
                                        onclick: move |_| _show_qdq_cleanup_info.set(true),
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
                                                onclick: move |_| show_run_tag_info.set(true),
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
                                                onclick: move |_| show_run_log_sev_info.set(true),
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
                                                onclick: move |_| show_run_log_verb_info.set(true),
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
                                                onclick: move |_| show_log_tag_info.set(true),
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
                                                onclick: move |_| show_ep_intra_threads_info.set(true),
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
                                                onclick: move |_| show_ep_inter_threads_info.set(true),
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
                                                onclick: move |_| show_use_arena_info.set(true),
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
                                                onclick: move |_| show_arena_extend_info.set(true),
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
                                                onclick: move |_| show_init_chunk_info.set(true),
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
                                                onclick: move |_| show_max_chunk_info.set(true),
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
                                                onclick: move |_| show_growth_chunk_info.set(true),
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
                                                onclick: move |_| show_dead_bytes_info.set(true),
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

            }
        }

        // Default values info modal
        if show_defaults_info() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_defaults_info.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-lg font-semibold text-gray-100", "ONNX Default Values" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_defaults_info.set(false),
                            "×"
                        }
                    }
                    div { class: "text-sm text-gray-300 space-y-1 font-mono",
                        div { class: "grid grid-cols-2 gap-x-4 gap-y-1",
                            span { class: "text-gray-400", "num_threads:" }
                            span { "4" }
                            span { class: "text-gray-400", "inter_op_num_threads:" }
                            span { "1" }
                            span { class: "text-gray-400", "optimization_level:" }
                            span { "all (3)" }
                            span { class: "text-gray-400", "execution_mode:" }
                            span { "sequential" }
                            span { class: "text-gray-400", "enable_mem_pattern:" }
                            span { class: "text-green-400", "true" }
                            span { class: "text-gray-400", "enable_cpu_mem_arena:" }
                            span { class: "text-green-400", "true" }
                            span { class: "text-gray-400", "deterministic_compute:" }
                            span { class: "text-red-400", "false" }
                            span { class: "text-gray-400", "enable_profiling:" }
                            span { class: "text-red-400", "false" }
                            span { class: "text-gray-400", "log_level:" }
                            span { "info" }
                            span { class: "text-gray-400", "log_verbosity:" }
                            span { "0" }
                            span { class: "text-gray-400", "use_env_allocators:" }
                            span { class: "text-red-400", "false" }
                            span { class: "text-gray-400", "denormal_as_zero:" }
                            span { class: "text-red-400", "false" }
                            span { class: "text-gray-400", "enable_quant_qdq:" }
                            span { class: "text-green-400", "true" }
                            span { class: "text-gray-400", "enable_double_qdq_remover:" }
                            span { class: "text-green-400", "true" }
                            span { class: "text-gray-400", "enable_qdq_cleanup:" }
                            span { class: "text-red-400", "false" }
                            span { class: "text-gray-400", "approximate_gelu:" }
                            span { class: "text-red-400", "false" }
                            span { class: "text-gray-400", "enable_aot_inlining:" }
                            span { class: "text-green-400", "true" }
                            span { class: "text-gray-400", "use_device_allocator_for_initializers:" }
                            span { class: "text-red-400", "false" }
                            span { class: "text-gray-400", "allow_inter_op_spinning:" }
                            span { class: "text-green-400", "true" }
                            span { class: "text-gray-400", "allow_intra_op_spinning:" }
                            span { class: "text-green-400", "true" }
                            span { class: "text-gray-400", "use_prepacking:" }
                            span { class: "text-green-400", "true" }
                            span { class: "text-gray-400", "independent_thread_pool:" }
                            span { class: "text-red-400", "false" }
                            span { class: "text-gray-400", "no_env_execution_providers:" }
                            span { class: "text-red-400", "false" }
                        }
                    }
                    button {
                        class: "btn btn-sm text-white mt-4 w-full",
                        style: "background-color: #1D6B9A; border-color: #1D6B9A;",
                        onclick: move |_| show_defaults_info.set(false),
                        "Close"
                    }
                }
            }
        }

        // Info modals for ONNX parameters
        if show_graph_opt_info() {
            { info_modal(OnnxHelpTopic::GraphOptimizationLevel.title(), show_graph_opt_info, OnnxHelpTopic::GraphOptimizationLevel.paragraphs()) }
        }
        if show_exec_mode_info() {
            { info_modal(OnnxHelpTopic::ExecutionMode.title(), show_exec_mode_info, OnnxHelpTopic::ExecutionMode.paragraphs()) }
        }
        if show_num_threads_info() {
            { info_modal(OnnxHelpTopic::NumThreads.title(), show_num_threads_info, OnnxHelpTopic::NumThreads.paragraphs()) }
        }
        if show_inter_op_threads_info() {
            { info_modal(OnnxHelpTopic::InterOpNumThreads.title(), show_inter_op_threads_info, OnnxHelpTopic::InterOpNumThreads.paragraphs()) }
        }
        if show_mem_pattern_info() {
            { info_modal(OnnxHelpTopic::EnableMemPattern.title(), show_mem_pattern_info, OnnxHelpTopic::EnableMemPattern.paragraphs()) }
        }
        if show_cpu_mem_arena_info() {
            { info_modal(OnnxHelpTopic::EnableCpuMemArena.title(), show_cpu_mem_arena_info, OnnxHelpTopic::EnableCpuMemArena.paragraphs()) }
        }
        if show_deterministic_info() {
            { info_modal(OnnxHelpTopic::DeterministicCompute.title(), show_deterministic_info, OnnxHelpTopic::DeterministicCompute.paragraphs()) }
        }
        if show_opt_model_path_info() {
            { info_modal(OnnxHelpTopic::OptimizedModelFilepath.title(), show_opt_model_path_info, OnnxHelpTopic::OptimizedModelFilepath.paragraphs()) }
        }
        if show_profiling_info() {
            { info_modal(OnnxHelpTopic::EnableProfiling.title(), show_profiling_info, OnnxHelpTopic::EnableProfiling.paragraphs()) }
        }
        if show_log_level_info() {
            { info_modal(OnnxHelpTopic::LogLevel.title(), show_log_level_info, OnnxHelpTopic::LogLevel.paragraphs()) }
        }
        if show_quant_qdq_info() {
            { info_modal(OnnxHelpTopic::EnableQuantQdq.title(), show_quant_qdq_info, OnnxHelpTopic::EnableQuantQdq.paragraphs()) }
        }
        if show_double_qdq_info() {
            { info_modal(OnnxHelpTopic::EnableDoubleQdqRemover.title(), show_double_qdq_info, OnnxHelpTopic::EnableDoubleQdqRemover.paragraphs()) }
        }
        if _show_profiling_path_info() {
            { info_modal(OnnxHelpTopic::ProfilingOutputPath.title(), _show_profiling_path_info, OnnxHelpTopic::ProfilingOutputPath.paragraphs()) }
        }
        if _show_log_id_info() {
            { info_modal(OnnxHelpTopic::LogId.title(), _show_log_id_info, OnnxHelpTopic::LogId.paragraphs()) }
        }
        if _show_log_verbosity_info() {
            { info_modal(OnnxHelpTopic::LogVerbosity.title(), _show_log_verbosity_info, OnnxHelpTopic::LogVerbosity.paragraphs()) }
        }
        if _show_env_allocators_info() {
            { info_modal(OnnxHelpTopic::UseEnvAllocators.title(), _show_env_allocators_info, OnnxHelpTopic::UseEnvAllocators.paragraphs()) }
        }
        if _show_denormal_info() {
            { info_modal(OnnxHelpTopic::DenormalAsZero.title(), _show_denormal_info, OnnxHelpTopic::DenormalAsZero.paragraphs()) }
        }
        if _show_device_alloc_info() {
            { info_modal(OnnxHelpTopic::UseDeviceAllocatorForInitializers.title(), _show_device_alloc_info, OnnxHelpTopic::UseDeviceAllocatorForInitializers.paragraphs()) }
        }
        if _show_inter_spin_info() {
            { info_modal(OnnxHelpTopic::AllowInterOpSpinning.title(), _show_inter_spin_info, OnnxHelpTopic::AllowInterOpSpinning.paragraphs()) }
        }
        if _show_intra_spin_info() {
            { info_modal(OnnxHelpTopic::AllowIntraOpSpinning.title(), _show_intra_spin_info, OnnxHelpTopic::AllowIntraOpSpinning.paragraphs()) }
        }
        if _show_prepacking_info() {
            { info_modal(OnnxHelpTopic::UsePrepacking.title(), _show_prepacking_info, OnnxHelpTopic::UsePrepacking.paragraphs()) }
        }
        if _show_qdq_cleanup_info() {
            { info_modal(OnnxHelpTopic::EnableQdqCleanup.title(), _show_qdq_cleanup_info, OnnxHelpTopic::EnableQdqCleanup.paragraphs()) }
        }
        if _show_approx_gelu_info() {
            { info_modal(OnnxHelpTopic::ApproximateGelu.title(), _show_approx_gelu_info, OnnxHelpTopic::ApproximateGelu.paragraphs()) }
        }
        if _show_disabled_opt_info() {
            { info_modal(OnnxHelpTopic::DisabledOptimizers.title(), _show_disabled_opt_info, OnnxHelpTopic::DisabledOptimizers.paragraphs()) }
        }
        if _show_model_path_info() {
            { info_modal(OnnxHelpTopic::ModelPath.title(), _show_model_path_info, OnnxHelpTopic::ModelPath.paragraphs()) }
        }
        if _show_embed_dim_info() {
            { info_modal(OnnxHelpTopic::EmbeddingDim.title(), _show_embed_dim_info, OnnxHelpTopic::EmbeddingDim.paragraphs()) }
        }
        if _show_max_length_info() {
            { info_modal(OnnxHelpTopic::MaxLength.title(), _show_max_length_info, OnnxHelpTopic::MaxLength.paragraphs()) }
        }
        // Pre-declared signals for params not yet shown in UI
        if _show_indep_pool_info() {
            { info_modal(OnnxHelpTopic::IndependentThreadPool.title(), _show_indep_pool_info, OnnxHelpTopic::IndependentThreadPool.paragraphs()) }
        }
        if _show_no_env_ep_info() {
            { info_modal(OnnxHelpTopic::NoEnvExecutionProviders.title(), _show_no_env_ep_info, OnnxHelpTopic::NoEnvExecutionProviders.paragraphs()) }
        }
        if _show_aot_inlining_info() {
            { info_modal(OnnxHelpTopic::EnableAotInlining.title(), _show_aot_inlining_info, OnnxHelpTopic::EnableAotInlining.paragraphs()) }
        }
        // Session Options (read-only / advanced)
        if show_exec_order_info() {
            { info_modal(OnnxHelpTopic::ExecutionOrder.title(), show_exec_order_info, OnnxHelpTopic::ExecutionOrder.paragraphs()) }
        }
        if show_create_thread_info() {
            { info_modal(OnnxHelpTopic::CustomCreateThreadFn.title(), show_create_thread_info, OnnxHelpTopic::CustomCreateThreadFn.paragraphs()) }
        }
        if show_join_thread_info() {
            { info_modal(OnnxHelpTopic::CustomJoinThreadFn.title(), show_join_thread_info, OnnxHelpTopic::CustomJoinThreadFn.paragraphs()) }
        }
        if show_free_dim_info() {
            { info_modal(OnnxHelpTopic::FreeDimensionOverrides.title(), show_free_dim_info, OnnxHelpTopic::FreeDimensionOverrides.paragraphs()) }
        }
        if show_session_config_info() {
            { info_modal(OnnxHelpTopic::SessionConfigEntries.title(), show_session_config_info, OnnxHelpTopic::SessionConfigEntries.paragraphs()) }
        }
        // Session Config Keys
        if show_save_model_fmt_info() {
            { info_modal(OnnxHelpTopic::SaveModelFormat.title(), show_save_model_fmt_info, OnnxHelpTopic::SaveModelFormat.paragraphs()) }
        }
        if show_ort_bytes_direct_info() {
            { info_modal(OnnxHelpTopic::UseOrtModelBytesDirectly.title(), show_ort_bytes_direct_info, OnnxHelpTopic::UseOrtModelBytesDirectly.paragraphs()) }
        }
        if show_ort_bytes_init_info() {
            { info_modal(OnnxHelpTopic::UseOrtModelBytesForInitializers.title(), show_ort_bytes_init_info, OnnxHelpTopic::UseOrtModelBytesForInitializers.paragraphs()) }
        }
        if show_intra_spin_ctrl_info() {
            { info_modal(OnnxHelpTopic::IntraOpSpinControl.title(), show_intra_spin_ctrl_info, OnnxHelpTopic::IntraOpSpinControl.paragraphs()) }
        }
        if show_dyn_block_info() {
            { info_modal(OnnxHelpTopic::DynamicBlockBase.title(), show_dyn_block_info, OnnxHelpTopic::DynamicBlockBase.paragraphs()) }
        }
        if show_graph_opt_loop_info() {
            { info_modal(OnnxHelpTopic::GraphOptimizationsLoopLevel.title(), show_graph_opt_loop_info, OnnxHelpTopic::GraphOptimizationsLoopLevel.paragraphs()) }
        }
        if show_bias_gelu_info() {
            { info_modal(OnnxHelpTopic::EnableBiasGeluFusion.title(), show_bias_gelu_info, OnnxHelpTopic::EnableBiasGeluFusion.paragraphs()) }
        }
        if show_conv_bn_info() {
            { info_modal(OnnxHelpTopic::EnableConvBnFusion.title(), show_conv_bn_info, OnnxHelpTopic::EnableConvBnFusion.paragraphs()) }
        }
        // Run Options
        if show_run_tag_info() {
            { info_modal(OnnxHelpTopic::RunTag.title(), show_run_tag_info, OnnxHelpTopic::RunTag.paragraphs()) }
        }
        if show_run_log_sev_info() {
            { info_modal(OnnxHelpTopic::RunLogSeverityLevel.title(), show_run_log_sev_info, OnnxHelpTopic::RunLogSeverityLevel.paragraphs()) }
        }
        if show_run_log_verb_info() {
            { info_modal(OnnxHelpTopic::RunLogVerbosityLevel.title(), show_run_log_verb_info, OnnxHelpTopic::RunLogVerbosityLevel.paragraphs()) }
        }
        if show_log_tag_info() {
            { info_modal(OnnxHelpTopic::LogTag.title(), show_log_tag_info, OnnxHelpTopic::LogTag.paragraphs()) }
        }
        // CPU Execution Provider
        if show_ep_intra_threads_info() {
            { info_modal(OnnxHelpTopic::EpIntraOpNumThreads.title(), show_ep_intra_threads_info, OnnxHelpTopic::EpIntraOpNumThreads.paragraphs()) }
        }
        if show_ep_inter_threads_info() {
            { info_modal(OnnxHelpTopic::EpInterOpNumThreads.title(), show_ep_inter_threads_info, OnnxHelpTopic::EpInterOpNumThreads.paragraphs()) }
        }
        if show_use_arena_info() {
            { info_modal(OnnxHelpTopic::UseArena.title(), show_use_arena_info, OnnxHelpTopic::UseArena.paragraphs()) }
        }
        if show_arena_extend_info() {
            { info_modal(OnnxHelpTopic::ArenaExtendStrategy.title(), show_arena_extend_info, OnnxHelpTopic::ArenaExtendStrategy.paragraphs()) }
        }
        if show_init_chunk_info() {
            { info_modal(OnnxHelpTopic::InitialChunkSizeBytes.title(), show_init_chunk_info, OnnxHelpTopic::InitialChunkSizeBytes.paragraphs()) }
        }
        if show_max_chunk_info() {
            { info_modal(OnnxHelpTopic::MaxChunkSizeBytes.title(), show_max_chunk_info, OnnxHelpTopic::MaxChunkSizeBytes.paragraphs()) }
        }
        if show_growth_chunk_info() {
            { info_modal(OnnxHelpTopic::InitialGrowthChunkSizeBytes.title(), show_growth_chunk_info, OnnxHelpTopic::InitialGrowthChunkSizeBytes.paragraphs()) }
        }
        if show_dead_bytes_info() {
            { info_modal(OnnxHelpTopic::MaxDeadBytesPerChunk.title(), show_dead_bytes_info, OnnxHelpTopic::MaxDeadBytesPerChunk.paragraphs()) }
        }
        if show_embed_batch_size_info() {
            { info_modal("Embedding Batch Size", show_embed_batch_size_info, vec![
                "Controls how many document chunks are sent to the ONNX model in a single inference pass.",
                "ONNX attention is O(batch × heads × seq²) in memory. For a model with 12 attention heads and 512-token sequences, a batch of 500 chunks needs hundreds of GB of intermediate tensors — crashing the process.",
                "Lower values (8–16) protect against OOM when indexing large or image-heavy PDFs. Higher values (32–128) give better GPU/CPU utilisation once you have enough RAM.",
                "This setting takes effect immediately — no restart required. The default is 32, which is safe for most laptops and desktop machines.",
            ]) }

        if show_layout_ml_info() {
            { info_modal("Native PDF Extraction", show_layout_ml_info, vec![
                "The native PDF extraction pipeline runs entirely in-process — no Python sidecar required.",
                "Stage 1 uses lopdf to walk the PDF content stream and extract word tokens with bounding boxes (x0, y0, x1, y1 normalised to 0–1000). A fallback to extractous handles malformed PDFs where lopdf cannot parse the content stream.",
                "Stage 2 classifies regions using LayoutXLM (via candle). When the LayoutXLM model is not downloaded, a pure-Rust heuristic classifier takes over: it groups words into lines by y-proximity, then scores each line for title capitalisation, footer position, table pipe characters, and list bullet markers.",
                "Stage 3 detects table structure. The ORT TableFormer model (microsoft/table-transformer-structure-recognition) is the primary path; text-mode clustering fills in until page image rendering is available.",
                "Stage 4 assembles DocIR: Titles, SectionHeaders, Tables, Figures, Captions, and Lists are mapped to typed DocBlocks. Footer and noise regions are dropped.",
                "To activate: set LAYOUT_ML_ENABLED=true. The heuristic classifier works immediately with no download. To use the candle LayoutXLM path: find a fine-tuned checkpoint on huggingface.co (search 'layoutxlm document layout') that has config.json, tokenizer.json, and model.safetensors with a 13-class document-layout classifier head. Set LAYOUT_ML_MODEL_ID=owner/repo-name — the server downloads the weights automatically on first startup and caches them to ~/.cache/huggingface/hub/. Any load failure falls back to heuristic automatically.",
                "Priority: Docling sidecar (if running) > NativePdfExtractor > built-in pdftotext.",
            ]) }
        }
        }
    }
}
