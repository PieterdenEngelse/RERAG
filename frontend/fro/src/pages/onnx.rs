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
use dioxus_router::Link;

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
        layout_model_tier: String::new(),
        layout_ml_model_id: String::new(),
        chunker_mode: String::new(),
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
    let mut show_lopdf_info = use_signal(|| false);
    let mut show_extractous_info = use_signal(|| false);
    let mut show_feature_compiled_info = use_signal(|| false);
    let mut show_enabled_info = use_signal(|| false);
    let mut show_layout_model_info = use_signal(|| false);
    let mut show_layout_enabled_toggle_info = use_signal(|| false);
    let mut layout_toggle_saving = use_signal(|| false);
    let mut layout_restart_pending = use_signal(|| false);
    let mut layout_restarting = use_signal(|| false);
    let mut layout_toggle_message = use_signal::<Option<String>>(|| None);
    // Corpus selector tile (top of page) — read-only view of per-corpus
    // Native PDF overrides, with a Link to /config/corpus to actually edit.
    let mut corpora_list = use_signal(Vec::<api::CorpusEntry>::new);
    let mut selected_corpus = use_signal(|| "default".to_string());
    let mut selected_corpus_settings = use_signal::<Option<api::CorpusSettings>>(|| None);
    // LAYOUT_ML_MODEL_ID editor — Tier 0 HF Hub spec (e.g. "cmarkea/detr-layout-detection")
    let mut model_id_draft = use_signal::<String>(String::new);
    let mut model_id_saving = use_signal(|| false);
    let mut model_id_message = use_signal::<Option<String>>(|| None);
    let mut show_model_id_info = use_signal(|| false);
    let mut show_chunker_mode_info = use_signal(|| false);
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
                    model_id_draft.set(resp.config.layout_ml_model_id.clone());
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
        spawn(async move {
            if let Ok(list) = api::fetch_corpora().await {
                corpora_list.set(list);
            }
            if let Ok(r) = api::fetch_corpus_settings(&selected_corpus()).await {
                selected_corpus_settings.set(Some(r.settings));
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

            // ONNX vs ort framing — every knob on this page is an ort/ONNX
            // Runtime setting. The .onnx file format itself has no execution
            // settings; see /docu/index/onnx for the three-layer write-up.
            {onnx_vs_ort_page_banner()}

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
                // CORPUS SELECTOR TILE — first tile on the page
                //
                // Read-only view of per-corpus Native PDF overrides.
                // The actual edit surface lives on /config/corpus; this tile
                // surfaces "which corpus and what does it currently do" plus
                // a Link to jump there.
                // ═══════════════════════════════════════════════════════════════
                Panel { title: None, refresh: None,
                    div { class: "flex flex-col gap-2",
                        div { class: "flex items-center gap-2 flex-wrap",
                            span { class: "text-base text-gray-100 font-semibold", "Corpus" }
                            {layer_badge("ag-level")}
                            select {
                                class: "select select-sm select-bordered bg-gray-700 text-gray-200 ml-2",
                                value: selected_corpus(),
                                onchange: move |evt| {
                                    let slug = evt.value();
                                    selected_corpus.set(slug.clone());
                                    selected_corpus_settings.set(None);
                                    spawn(async move {
                                        if let Ok(r) = api::fetch_corpus_settings(&slug).await {
                                            selected_corpus_settings.set(Some(r.settings));
                                        }
                                    });
                                },
                                for corpus in corpora_list.read().clone() {
                                    option {
                                        value: "{corpus.slug}",
                                        selected: corpus.slug == selected_corpus(),
                                        if corpus.doc_count > 0 {
                                            "{corpus.slug} ({corpus.doc_count} docs)"
                                        } else {
                                            "{corpus.slug}"
                                        }
                                    }
                                }
                            }
                            Link {
                                to: Route::ConfigCorpus {},
                                class: "text-blue-400 hover:text-blue-300 underline text-xs ml-2",
                                "Manage per-corpus settings →"
                            }
                        }

                        // Native PDF status for the selected corpus.
                        {
                            let global = config().layout_ml_enabled;
                            let (override_label, effective) = match selected_corpus_settings()
                                .and_then(|s| s.native_pdf_enabled)
                            {
                                Some(true) => ("on (override)", true),
                                Some(false) => ("off (override)", false),
                                None => ("inherit global", global),
                            };
                            let eff_class = if effective {
                                "text-green-400 font-semibold"
                            } else {
                                "text-gray-300"
                            };
                            rsx! {
                                div { class: "bg-gray-800 rounded px-2 py-1 text-xs flex items-center gap-3 flex-wrap",
                                    span { class: "text-gray-400", "Native PDF for this corpus:" }
                                    span { class: eff_class,
                                        if effective { "on" } else { "off" }
                                    }
                                    span { class: "text-gray-500", "·" }
                                    span { class: "text-gray-400", "{override_label}" }
                                    span { class: "text-gray-500", "·" }
                                    span { class: "text-gray-400",
                                        "global default: "
                                        span { class: if global { "text-green-400" } else { "text-gray-300" },
                                            if global { "on" } else { "off" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ═══════════════════════════════════════════════════════════════
                // LAYOUT ML STATUS TILE
                // ═══════════════════════════════════════════════════════════════
                Panel { title: None, refresh: None,
                    div { class: "flex flex-col gap-2",
                        div { class: "flex items-center gap-2 mb-1 flex-wrap",
                            span { class: "text-base text-gray-100 font-semibold", "Native PDF Extraction" }
                            {layer_badge("ag-level")}
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_layout_ml_info.set(true),
                                crate::pages::hardware::components::InfoIcon {}
                            }

                            // Match the `enable_profiling` control under
                            // Profiling & Logging — `.onnx-checkbox` with the
                            // brand-color fill when checked. Avoids the
                            // currentColor / half-alpha trap daisyUI's toggle
                            // hits in non-canonical placements. Disabled when
                            // the Cargo feature isn't compiled in.
                            {
                                let compiled = config().layout_ml_compiled;
                                let enabled_now = config().layout_ml_enabled;
                                let saving = layout_toggle_saving();
                                rsx! {
                                    // Always render full-colour. The
                                    // "Feature compiled" tile next to this
                                    // already tells the user whether the
                                    // Cargo feature is in the binary; we
                                    // don't double up by graying the
                                    // checkbox (opacity dims the brand blue
                                    // + white checkmark too). The HTML
                                    // `disabled` attribute is avoided for
                                    // the same reason — it forces browsers
                                    // back to native UA rendering and kills
                                    // `.onnx-checkbox` styling. If the
                                    // feature isn't compiled the click
                                    // still saves the override harmlessly;
                                    // it'll take effect after a rebuild.
                                    div {
                                        // Chip-style container matching the
                                        // sibling status chips (Feature
                                        // compiled / Enabled / Layout model /
                                        // Chunker mode). Same vertical padding
                                        // (`py-1`), same background, same flex
                                        // alignment — so this row's info
                                        // button lands on the same baseline /
                                        // visual lane as the chips' info
                                        // buttons.
                                        // `basis-full` forces this chip onto its
                                        // own row inside the parent flex-wrap,
                                        // so its right edge = panel right edge.
                                        // Combined with `ml-auto` on the trailing
                                        // info button, the button lands at the
                                        // same X as the Layout model chip's info
                                        // button (which has the same treatment).
                                        class: "bg-gray-800 rounded px-2 py-1 text-xs flex items-center gap-2 basis-full",
                                        title: if compiled {
                                            "Toggle LAYOUT_ML_ENABLED (restart required)"
                                        } else {
                                            "Feature not compiled — override will save but takes effect only after `cargo build --features layout_ml`"
                                        },
                                        input {
                                            r#type: "checkbox",
                                            class: PARAM_CHECKBOX_CLASS,
                                            checked: enabled_now,
                                            onchange: move |evt| {
                                                if saving {
                                                    return;
                                                }
                                                let want = evt.checked();
                                                spawn(async move {
                                                    layout_toggle_saving.set(true);
                                                    layout_toggle_message.set(None);
                                                    match api::put_runtime_setting(
                                                        "LAYOUT_ML_ENABLED",
                                                        Some(if want { "true".into() } else { "false".into() }),
                                                    ).await {
                                                        Ok(_) => {
                                                            layout_restart_pending.set(true);
                                                            layout_toggle_message.set(Some(format!(
                                                                "Saved LAYOUT_ML_ENABLED={want}. Restart required."
                                                            )));
                                                            // Optimistic local update so the readback tiles
                                                            // reflect the saved override immediately; the real
                                                            // value lands after restart.
                                                            config.with_mut(|c| c.layout_ml_enabled = want);
                                                        }
                                                        Err(e) => {
                                                            layout_toggle_message.set(Some(format!(
                                                                "Failed to save: {e}"
                                                            )));
                                                        }
                                                    }
                                                    layout_toggle_saving.set(false);
                                                });
                                            },
                                        }
                                        label { class: PARAM_LABEL_CLASS, "LAYOUT_ML_ENABLED" }
                                        button {
                                            // `ml-auto` pushes this info button
                                            // to the right edge of the chip
                                            // (which is the panel right edge
                                            // because the chip is `basis-full`).
                                            // Pairs with the Layout model chip's
                                            // info button for horizontal X
                                            // alignment between the two rows.
                                            class: format!("{PARAM_ICON_BUTTON_CLASS} ml-auto"),
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_layout_enabled_toggle_info.set(true),
                                            title: "What this toggle does",
                                            InfoIcon {}
                                        }
                                    }
                                }
                            }

                            // Caption: this is the corpus-wide default; per-corpus override lives on /config/corpus.
                            div { class: "basis-full text-xs text-gray-400 italic",
                                "This is the default for all corpora — each corpus can override on /config/corpus (no restart needed)."
                            }

                            // LAYOUT_ML_MODEL_ID input — Tier 0 HuggingFace Hub
                            // spec. Format: `owner/repo` (defaults to model.onnx
                            // inside the repo) or `owner/repo:filename.onnx`.
                            // When set, ag auto-downloads via hf-hub into
                            // ~/.cache/huggingface/hub/ on first use and
                            // reuses it on subsequent boots. Empty string =
                            // skip Tier 0, fall through to LAYOUT_DETR_MODEL_PATH.
                            div { class: "flex items-center gap-2 ml-3 flex-wrap",
                                label { class: PARAM_LABEL_CLASS, "LAYOUT_ML_MODEL_ID:" }
                                input {
                                    r#type: "text",
                                    class: "bg-gray-700 text-gray-100 text-xs rounded px-2 py-1 w-72 font-mono border border-gray-600 focus:border-blue-400 focus:outline-none",
                                    placeholder: "owner/repo[:filename] — e.g. cmarkea/detr-layout-detection",
                                    value: "{model_id_draft}",
                                    oninput: move |evt| model_id_draft.set(evt.value()),
                                }
                                button {
                                    class: "btn btn-xs bg-blue-700 hover:bg-blue-600 text-white border-none disabled:bg-gray-700 disabled:text-gray-500",
                                    disabled: model_id_saving(),
                                    onclick: move |_| {
                                        let val = model_id_draft.read().clone();
                                        spawn(async move {
                                            model_id_saving.set(true);
                                            model_id_message.set(None);
                                            let trimmed = val.trim().to_string();
                                            let payload = if trimmed.is_empty() { None } else { Some(trimmed.clone()) };
                                            match api::put_runtime_setting("LAYOUT_ML_MODEL_ID", payload).await {
                                                Ok(_) => {
                                                    layout_restart_pending.set(true);
                                                    model_id_message.set(Some(if trimmed.is_empty() {
                                                        "Cleared LAYOUT_ML_MODEL_ID. Restart required.".to_string()
                                                    } else {
                                                        format!("Saved LAYOUT_ML_MODEL_ID={trimmed}. Restart required.")
                                                    }));
                                                }
                                                Err(e) => model_id_message.set(Some(format!("Failed to save: {e}"))),
                                            }
                                            model_id_saving.set(false);
                                        });
                                    },
                                    if model_id_saving() { "Saving…" } else { "Save" }
                                }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| show_model_id_info.set(true),
                                    title: "What LAYOUT_ML_MODEL_ID does",
                                    InfoIcon {}
                                }
                                if let Some(msg) = model_id_message() {
                                    span {
                                        class: if msg.starts_with("Failed") { "text-xs text-red-400" } else { "text-xs text-green-400" },
                                        "{msg}"
                                    }
                                }
                            }

                            // Compact status chips, on the same row as the
                            // title/toggle. `flex-wrap` on the parent row
                            // lets them spill to a second line on narrow
                            // viewports.
                            div { class: "bg-gray-800 rounded px-2 py-1 text-xs flex items-center gap-2 ml-3",
                                // Read-only checkbox — reflects
                                // layout_ml_compiled, which is set at
                                // compile time via cfg!(feature =
                                // "layout_ml") and cannot change at
                                // runtime. `pointer-events-none` on the
                                // input stops the browser from briefly
                                // toggling it on click; the wrapping div
                                // catches the click and opens the
                                // explainer modal.
                                div {
                                    class: "cursor-pointer flex items-center",
                                    title: "Build-time flag — click for details",
                                    onclick: move |_| show_feature_compiled_info.set(true),
                                    input {
                                        r#type: "checkbox",
                                        class: format!("{PARAM_CHECKBOX_CLASS} pointer-events-none"),
                                        checked: config().layout_ml_compiled,
                                    }
                                }
                                span { class: "text-gray-400", "Feature compiled:" }
                                span {
                                    class: if config().layout_ml_compiled { "text-green-400 font-semibold" } else { "text-gray-300" },
                                    if config().layout_ml_compiled { "yes" } else { "no (build without layout_ml)" }
                                }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| show_feature_compiled_info.set(true),
                                    title: "What \"feature compiled\" means",
                                    InfoIcon {}
                                }
                            }
                            div { class: "bg-gray-800 rounded px-2 py-1 text-xs flex items-center gap-2",
                                span { class: "text-gray-400", "Enabled:" }
                                span {
                                    class: if config().layout_ml_enabled { "text-green-400 font-semibold" } else { "text-gray-300" },
                                    if config().layout_ml_enabled { "yes (LAYOUT_ML_ENABLED=true)" } else { "no (set LAYOUT_ML_ENABLED=true)" }
                                }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| show_enabled_info.set(true),
                                    title: "What LAYOUT_ML_ENABLED controls",
                                    InfoIcon {}
                                }
                            }
                            // `basis-full` puts this chip on its own row so
                            // its right edge = panel right edge; `ml-auto` on
                            // the info button pushes it to that right edge.
                            // Together with the LAYOUT_ML_ENABLED chip above
                            // (same treatment), the two info buttons sit at
                            // the same horizontal X coordinate.
                            div { class: "bg-gray-800 rounded px-2 py-1 text-xs flex items-center gap-2 basis-full",
                                span { class: "text-gray-400", "Layout model:" }
                                span {
                                    class: if config().layout_model_ready { "text-green-400 font-semibold" } else { "text-yellow-400" },
                                    {
                                        let tier = config().layout_model_tier.clone();
                                        if tier.is_empty() {
                                            if config().layout_model_ready { "ORT (PubLayNet)".to_string() } else { "heuristic only".to_string() }
                                        } else {
                                            tier
                                        }
                                    }
                                }
                                button {
                                    class: format!("{PARAM_ICON_BUTTON_CLASS} ml-auto"),
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| show_layout_model_info.set(true),
                                    title: "Layout model: ORT vs heuristic",
                                    InfoIcon {}
                                }
                            }
                            // Chunker mode chip — surfaces the Layer-2 chunking
                            // strategy that will run on the DocIR this pipeline
                            // produces. Info modal explains when each mode is a
                            // good fit for PDFs and links to /config/chunker.
                            // Recommended modes for PDF output: lightweight (default),
                            // semantic, sentence, pipeline. `fixed` is flagged as
                            // suboptimal — pure size-based splitting that doesn't
                            // benefit from the heading/paragraph structure the
                            // native pipeline reveals. Warning, when applicable,
                            // is rendered as a separate row below the chips so
                            // this chip stays one tight inline unit aligned with
                            // its siblings.
                            {
                                let mode_raw = config().chunker_mode.clone();
                                let mode = if mode_raw.is_empty() { "fixed".to_string() } else { mode_raw };
                                let is_recommended = matches!(
                                    mode.as_str(),
                                    "lightweight" | "semantic" | "sentence" | "pipeline"
                                );
                                // Match sibling chips exactly: same font-family
                                // (no font-mono — that has a different x-height
                                // and shifts the baseline), same color-only
                                // emphasis. The `edit` link uses leading-none
                                // so its underline doesn't pad the line-box
                                // taller than the sibling text spans, which
                                // would push the baseline down inside
                                // `flex items-center`.
                                let value_class = if is_recommended {
                                    "text-green-400 font-semibold"
                                } else {
                                    "text-yellow-400 font-semibold"
                                };
                                rsx! {
                                    div { class: "bg-gray-800 rounded px-2 py-1 text-xs flex items-center gap-2",
                                        span { class: "text-gray-400", "Chunker mode:" }
                                        span { class: value_class, "{mode}" }
                                        Link {
                                            to: Route::ConfigChunker {},
                                            class: "text-blue-400 hover:text-blue-300 underline leading-none",
                                            "edit"
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_chunker_mode_info.set(true),
                                            title: "When to pick which chunker mode for PDFs",
                                            InfoIcon {}
                                        }
                                    }
                                }
                            }
                        }

                        // Suboptimal-chunker-mode warning. Sits on its own row
                        // below the chip strip so it never disturbs chip
                        // alignment. Only renders when the mode is `fixed` (or
                        // any future non-recommended value).
                        {
                            let mode_raw = config().chunker_mode.clone();
                            let mode = if mode_raw.is_empty() { "fixed".to_string() } else { mode_raw };
                            let is_recommended = matches!(
                                mode.as_str(),
                                "lightweight" | "semantic" | "sentence" | "pipeline"
                            );
                            if !is_recommended {
                                rsx! {
                                    div { class: "text-xs text-yellow-300 mt-1",
                                        "⚠ "
                                        span { class: "font-mono", "{mode}" }
                                        " is suboptimal for native PDF output — prefer "
                                        span { class: "font-mono", "lightweight" }
                                        " or "
                                        span { class: "font-mono", "semantic" }
                                        ". "
                                        Link {
                                            to: Route::ConfigChunker {},
                                            class: "text-blue-400 hover:text-blue-300 underline",
                                            "Change on /config/chunker"
                                        }
                                    }
                                }
                            } else {
                                rsx! {}
                            }
                        }

                        // Restart-required banner — shown after the toggle is
                        // saved, drives /runtime/actions/restart-self.
                        if layout_restart_pending() {
                            div {
                                class: "rounded border border-orange-700 bg-orange-900/30 p-2 flex items-center justify-between gap-2 mb-1",
                                div { class: "text-xs text-orange-100",
                                    "Native PDF Extraction toggle is restart-required. ag will re-exec in place — no systemd or docker needed."
                                }
                                button {
                                    class: "btn btn-xs",
                                    style: "background-color:#7C2A02;color:white;border:1px solid #7C2A02;",
                                    disabled: layout_restarting(),
                                    onclick: move |_| {
                                        layout_restarting.set(true);
                                        spawn(async move {
                                            let _ = api::post_restart_self().await;
                                            let _ = api::wait_for_restart(60_000, 750).await;
                                            layout_restarting.set(false);
                                            layout_restart_pending.set(false);
                                        });
                                    },
                                    if layout_restarting() { "Restarting…" } else { "Restart now" }
                                }
                            }
                        }

                        // Toast for save success/failure.
                        if let Some(msg) = layout_toggle_message() {
                            div {
                                class: if msg.starts_with("Failed") {
                                    "text-xs text-red-400 mb-1"
                                } else {
                                    "text-xs text-green-400 mb-1"
                                },
                                "{msg}"
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
                            div { class: "flex flex-col gap-1",
                                div { class: "flex items-center gap-2",
                                    span { class: "text-base text-gray-100 font-semibold", "General" }
                                    {layer_badge("ort-session-option")}
                                    {layer_badge("ort-session-config")}
                                }
                                span { class: "text-xs text-gray-300 italic", "ONNX Runtime Parameters (50 total) - restart required to apply changes" }
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
                    div { class: "flex items-center gap-2 mb-3",
                        span { class: "text-sm text-gray-300 font-semibold", "Session Options (21)" }
                        {layer_badge("ort-session-option")}
                    }
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
                                span { class: "text-gray-300 text-xs italic", "live — no restart needed" }
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
                        div { class: "flex items-center gap-2",
                            span { class: "text-sm text-gray-300 font-semibold", "Session Config Keys (15)" }
                            {layer_badge("ort-session-config")}
                        }
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
        }

        if show_layout_ml_info() {
            {native_pdf_extraction_modal(show_layout_ml_info, show_lopdf_info, show_extractous_info)}
        }

        if show_lopdf_info() {
            {lopdf_info_modal(show_lopdf_info)}
        }

        if show_extractous_info() {
            {extractous_info_modal(show_extractous_info)}
        }

        if show_feature_compiled_info() {
            {feature_compiled_info_modal(show_feature_compiled_info)}
        }

        if show_enabled_info() {
            {enabled_info_modal(show_enabled_info)}
        }

        if show_layout_model_info() {
            {layout_model_info_modal(show_layout_model_info)}
        }

        if show_layout_enabled_toggle_info() {
            {layout_enabled_toggle_info_modal(show_layout_enabled_toggle_info)}
        }

        if show_model_id_info() {
            {layout_ml_model_id_info_modal(show_model_id_info)}
        }

        if show_chunker_mode_info() {
            {chunker_mode_for_native_pdf_modal(show_chunker_mode_info)}
        }
    }
}

/// Framing banner — top of /config/onnx. Mirrors the explainer on
/// /config/runtime and /docu/index/onnx so the reader can map each knob on
/// this page to the layer it actually configures.
fn onnx_vs_ort_page_banner() -> Element {
    rsx! {
        div { class: "border border-blue-700 bg-blue-900/20 rounded-lg p-3 text-xs space-y-1",
            div { class: "font-semibold text-blue-200 text-sm",
                "Every knob below is an ort / ONNX Runtime setting — not a .onnx file setting"
            }
            div { class: "text-gray-300 space-y-1",
                p {
                    span { class: "text-gray-400", "ONNX = file format. " }
                    "A .onnx file declares "
                    em { "what" }
                    " to compute (graph + weights). It has no threading, no optimization level, no hardware selection."
                }
                p {
                    span { class: "text-gray-400", "ONNX Runtime = the C++ engine. " }
                    "All the execution knobs on this page — graph_optimization_level, num_threads, mem_pattern, etc. — are ONNX Runtime SessionOptions, accessed via the "
                    span { class: "font-mono text-gray-100", "ort" }
                    " Rust crate (a thin typed wrapper, no extra knobs of its own)."
                }
                p { class: "text-gray-400",
                    "Each board below carries a layer tag so it's visible at the parameter level which layer owns the knob. Longer write-up at "
                    a { href: "/docu/index/onnx", class: "text-blue-400 hover:text-blue-300 underline",
                        "/docu/index/onnx"
                    }
                    "."
                }
            }
        }
    }
}

/// Small layer-source badge for board titles.
fn layer_badge(layer: &str) -> Element {
    let (label, color) = match layer {
        "ort-session-option" => ("ort · SessionOption", "bg-blue-900/40 text-blue-300 border-blue-700"),
        "ort-session-config" => ("ort · SessionConfig K/V", "bg-cyan-900/40 text-cyan-300 border-cyan-700"),
        "ag-level" => ("ag-level", "bg-amber-900/40 text-amber-300 border-amber-700"),
        "onnx-file" => ("ONNX file", "bg-purple-900/40 text-purple-300 border-purple-700"),
        other => (other, "bg-gray-700 text-gray-300 border-gray-600"),
    };
    let class = format!("px-2 py-0.5 rounded border text-[10px] uppercase tracking-wide {color}");
    rsx! {
        span { class: "{class}", "{label}" }
    }
}

/// Custom Native PDF Extraction info modal — same content as the previous
/// info_modal call, but with `lopdf` and `extractous` rendered as clickable
/// spans that open nested explainer modals.
fn native_pdf_extraction_modal(
    mut show: Signal<bool>,
    mut show_lopdf: Signal<bool>,
    mut show_extractous: Signal<bool>,
) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-900 border border-gray-700 rounded-lg p-4 w-[98vw] max-h-[92vh] flex flex-col shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-3 shrink-0",
                    h2 { class: "text-xl font-bold text-gray-100", "Native PDF Extraction" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-xs text-gray-300 leading-relaxed flex-1 min-h-0 overflow-y-auto space-y-4",
                    div { class: "pb-3 border-b border-gray-700 space-y-2",
                        h3 { class: "text-sm font-semibold text-gray-100", "Why native extraction matters" }
                        p {
                            "Without native extraction ag treats a PDF like one big text file — pdftotext concatenates every page into one undifferentiated blob, and that blob is what gets chunked, embedded, and indexed. Page numbers, table boundaries, headings, captions, code blocks: all gone before the retriever ever sees them. "
                            strong { "Native extraction reads the same PDF and produces a typed document tree" }
                            " (titles, section headers, tables, code, captions, lists, page breaks, images), which then flows into every downstream stage with its structure intact."
                        }
                        p { class: "text-gray-200", "What you actually get from that:" }
                        ul { class: "list-disc pl-5 space-y-1",
                            li {
                                strong { "Tables stay whole. " }
                                "A table is treated as one atomic unit — the chunker never slices it mid-row. A query like \"GPU utilisation 84%\" returns the full row context instead of a torn fragment that lost which column the number belonged to."
                            }
                            li {
                                strong { "Code blocks and formulas stay whole. " }
                                "Same atomic rule. A chunk never splits a function definition between an opening brace and its body."
                            }
                            li {
                                strong { "Sections become chunk boundaries. " }
                                "Headers flush the current chunk. A single chunk never straddles the end of \"Methods\" and the start of \"Results\" — search hits stay scoped to the section they came from, so the relevance signal isn't diluted by half-foreign context."
                            }
                            li {
                                strong { "Block type rides into the index. " }
                                "Every chunk carries its kind (Text, Header, Table, Caption, Code, …) as metadata. The retriever can reweight by structural role — boost section headers, demote footers — and search results expose the block type so the UI can label \"this hit came from a Table\" instead of treating every match as anonymous body text."
                            }
                            li {
                                strong { "Page numbers and bounding boxes survive. " }
                                "DocBlock keeps the source position, so \"from page 14\" links and figure-region highlights work. The plain-text path strips this — you only get \"somewhere in the file\"."
                            }
                            li {
                                strong { "Corpus health becomes visible. " }
                                "The Datastores page shows real per-corpus distribution (\"62% Text · 14% Table · 8% Header · …\") instead of a uniform 100% Text. Useful for spotting PDFs that came in mis-classified or weren't extracted natively at all."
                            }
                        }
                        p { class: "text-gray-400",
                            "Cost: ingest time goes up — pdfium renders each page and the layout model runs on each one. Query latency is unchanged (the work is already in the index). Every failure mode degrades gracefully: missing model → next tier; all tiers fail → heuristic classifier; everything fails → plain-text fallback. The pipeline never refuses to run."
                        }
                    }
                    div { class: "grid grid-cols-2 gap-4 divide-x divide-gray-700",
                    div { class: "space-y-2 pr-4",
                        h3 { class: "text-sm font-semibold text-gray-100", "How it works (the four stages)" }
                        p { "Runs entirely in-process — no Python sidecar required." }
                        p {
                            "Stage 1 uses "
                            span {
                                class: "text-blue-400 hover:text-blue-300 underline cursor-pointer font-mono",
                                onclick: move |_| show_lopdf.set(true),
                                title: "What is lopdf?",
                                "lopdf"
                            }
                            " to walk the PDF content stream and extract word tokens with bounding boxes (x0, y0, x1, y1 normalised to 0–1000). A fallback to "
                            span {
                                class: "text-blue-400 hover:text-blue-300 underline cursor-pointer font-mono",
                                onclick: move |_| show_extractous.set(true),
                                title: "What is extractous?",
                                "extractous"
                            }
                            " handles malformed PDFs where lopdf cannot parse the content stream."
                        }
                        p { "Stage 2 classifies regions using a DETR-style image model or word-feature ONNX classifier (loaded via the ort runtime). When no neural model is configured or available, a pure-Rust heuristic classifier takes over: it groups words into lines by y-proximity, then scores each line for title capitalisation, footer position, table pipe characters, and list bullet markers." }
                        p { "Stage 3 detects table structure. The ORT TableFormer model (microsoft/table-transformer-structure-recognition) is the primary path; text-mode clustering fills in until page image rendering is available." }
                        p { "Stage 4 assembles DocIR: Titles, SectionHeaders, Tables, Figures, Captions, and Lists are mapped to typed DocBlocks. Footer and noise regions are dropped." }
                        p { "Priority: Docling sidecar (if running) > NativePdfExtractor > built-in pdftotext." }
                    }
                    div { class: "space-y-2 pl-4",
                        h3 { class: "text-sm font-semibold text-gray-100", "Activation" }
                        p {
                            "To activate: set "
                            span { class: "font-mono text-gray-100", "LAYOUT_ML_ENABLED=true" }
                            ". The heuristic classifier works immediately with no download. For a neural layout model, three tiers are tried in order:"
                        }
                        ul { class: "list-disc pl-5 space-y-1",
                            li {
                                span { class: "font-mono text-gray-100", "LAYOUT_ML_MODEL_ID=owner/repo" }
                                " — Tier 0, auto-download via HuggingFace Hub. On first boot ag fetches the file into "
                                span { class: "font-mono text-gray-100", "~/.cache/huggingface/hub/" }
                                " and reuses it thereafter. Default filename is "
                                span { class: "font-mono text-gray-100", "model.onnx" }
                                "; override with "
                                span { class: "font-mono text-gray-100", "owner/repo:other-file.onnx" }
                                ". DETR-shape only (pixel_values input)."
                            }
                            li {
                                span { class: "font-mono text-gray-100", "LAYOUT_DETR_MODEL_PATH" }
                                " — Tier 1, local DETR file (used if Tier 0 isn't set or its download failed)."
                            }
                            li {
                                span { class: "font-mono text-gray-100", "LAYOUT_ORT_MODEL_PATH" }
                                " — Tier 2, word-feature ONNX classifier (used if Tier 0 and Tier 1 are unavailable)."
                            }
                            li {
                                span { class: "font-mono text-gray-100", "TABLE_FORMER_MODEL_PATH" }
                                " — TableFormer for Stage 3 table structure (huggingface.co/microsoft/table-transformer-structure-recognition)."
                            }
                        }
                        p { "Any load failure falls back to the next tier, ending at the heuristic / text-mode classifier — the pipeline never refuses to run because of a model problem." }
                        p { class: "pt-2 border-t border-gray-700 text-gray-400",
                            "Where this fits in the bigger picture: Native PDF Extraction is "
                            em { "Stage 0" }
                            " — it produces the DocIR that the "
                            a {
                                href: "/monitor/tip",
                                class: "text-blue-400 hover:text-blue-300 underline",
                                "Text Ingestion Pipeline (/monitor/tip)"
                            }
                            " then runs through Parser → Canonicalization → Typography → Chunker → Embedder. The block-type tags assigned here (Title, SectionHeader, Table, Figure, Caption, List) survive all the way into the chunk metadata and let the retriever reweight by structural role at query time."
                        }
                    }
                    }
                }
                button {
                    class: "btn btn-sm w-full mt-3 shrink-0",
                    style: "background-color:#7C2A02;",
                    onclick: move |_| show.set(false),
                    "Got it"
                }
            }
        }
    }
}

/// Info modal for the "Feature compiled" tile inside Native PDF Extraction.
fn feature_compiled_info_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-4 w-[98vw] max-h-[92vh] flex flex-col shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-3 shrink-0",
                    h2 { class: "text-lg font-semibold text-gray-100", "Feature compiled" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-xs text-gray-300 leading-relaxed flex-1 min-h-0 overflow-y-auto grid grid-cols-3 gap-4 divide-x divide-gray-700",
                    div { class: "space-y-2 pr-4",
                        h3 { class: "text-gray-100 font-semibold", "Why this checkbox doesn't toggle when you click it" }
                        p {
                            "It's a "
                            strong { "read-only status indicator" }
                            ", not a control. It shows whether the running binary includes the "
                            span { class: "font-mono text-gray-100", "layout_ml" }
                            " Cargo feature — set at compile time by "
                            span { class: "font-mono text-gray-100", "cfg!(feature = \"layout_ml\")" }
                            ". The app cannot flip this at runtime because the relevant code is either in the binary or it isn't."
                        }
                        p {
                            "Compare to the "
                            em { "Enabled" }
                            " toggle next to it: "
                            span { class: "font-mono text-gray-100", "LAYOUT_ML_ENABLED" }
                            " is a runtime env var the app reads at startup, so the app can save an override, restart itself, and read the new value. That whole loop is self-contained. Feature-compiled isn't — flipping it requires:"
                        }
                        ul { class: "list-disc pl-5 space-y-1",
                            li { "Shell access on the host machine" }
                            li { "The Rust toolchain installed" }
                            li {
                                "A "
                                span { class: "font-mono text-gray-100", "cargo build --release --features layout_ml" }
                                " (5–15 minutes the first time)"
                            }
                            li {
                                "A restart: "
                                span { class: "font-mono text-gray-100", "systemctl --user restart ag.service" }
                            }
                        }
                        p { "None of that is something a button in this UI can do for itself. A clickable toggle here would lie — appearing to enable the feature while the actual code wasn't in the binary to call." }
                    }
                    div { class: "space-y-2 px-4",
                        h3 { class: "text-gray-100 font-semibold", "What \"feature compiled\" actually gates" }
                        p {
                            "The "
                            span { class: "font-mono text-gray-100", "layout_ml" }
                            " feature is the build-time gate on the whole Native PDF Extraction pipeline. It pulls in six Cargo dependencies (all optional, all non-Windows):"
                        }
                        ul { class: "list-disc pl-5 space-y-1",
                            li {
                                span { class: "font-mono text-gray-100", "lopdf" }
                                " — parses the PDF content stream to extract per-word bounding boxes (Stage 1)."
                            }
                            li {
                                span { class: "font-mono text-gray-100", "extractous" }
                                " — fallback text extractor used when lopdf can't parse a malformed PDF."
                            }
                            li {
                                span { class: "font-mono text-gray-100", "ort" }
                                " — ONNX Runtime; loads the DETR / word-feature layout model (Stage 2) and TableFormer (Stage 3). Also pulled in by the default "
                                span { class: "font-mono text-gray-100", "onnx" }
                                " feature for embeddings."
                            }
                            li {
                                span { class: "font-mono text-gray-100", "pdfium-render" }
                                " — renders PDF pages to bitmaps so the DETR layout model can run on them. Requires the PDFium native library on the host ("
                                span { class: "font-mono text-gray-100", "PDFIUM_LIBRARY_PATH" }
                                " or a system install)."
                            }
                            li {
                                span { class: "font-mono text-gray-100", "image" }
                                " — bitmap manipulation for the rendered pages before they're handed to ort."
                            }
                            li {
                                span { class: "font-mono text-gray-100", "hf-hub" }
                                " — auto-downloads layout models from HuggingFace Hub when "
                                span { class: "font-mono text-gray-100", "LAYOUT_ML_MODEL_ID" }
                                " is set, caching under "
                                span { class: "font-mono text-gray-100", "~/.cache/huggingface/hub/" }
                                "."
                            }
                        }
                        p { class: "text-gray-400",
                            "All six are declared in "
                            span { class: "font-mono text-gray-100", "backend/Cargo.toml" }
                            " behind "
                            span { class: "font-mono text-gray-100", "[features] layout_ml = […]" }
                            " and "
                            span { class: "font-mono text-gray-100", "[target.'cfg(not(target_os = \"windows\"))'.dependencies.*]" }
                            ". Running "
                            span { class: "font-mono text-gray-100", "cargo build --release --features layout_ml" }
                            " resolves and compiles them on first build (5–15 min cold)."
                        }
                        h3 { class: "text-gray-100 font-semibold pt-1", "Why a build-time gate?" }
                        p { "Those crates add ~50 MB to the binary and noticeable compile time. Deployments that don't process PDFs (or are happy with the plain-text path) can skip them entirely. Making the choice runtime would mean shipping the cost in every build, including ones that never use it." }
                    }
                    div { class: "space-y-2 pl-4",
                        h3 { class: "text-gray-100 font-semibold", "yes" }
                        p { "The binary has the pipeline compiled in. The runtime flag (the Enabled toggle next to this) decides whether it actually executes." }
                        h3 { class: "text-gray-100 font-semibold pt-1", "no (build without layout_ml)" }
                        p {
                            "The binary was built without the feature. ag falls back to plain text extraction — no per-word bounding boxes, no layout classification. To get to "
                            em { "yes" }
                            ": rebuild with "
                            span { class: "font-mono text-gray-100", "cargo build --release --features layout_ml" }
                            " and restart ag."
                        }
                    }
                }
                button {
                    class: "btn btn-sm w-full mt-3 shrink-0",
                    style: "background-color:#7C2A02;",
                    onclick: move |_| show.set(false),
                    "Got it"
                }
            }
        }
    }
}

/// Info modal for the "Enabled" tile inside Native PDF Extraction.
fn enabled_info_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100",
                        "Enabled — "
                        span { class: "font-mono", "LAYOUT_ML_ENABLED" }
                    }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 leading-relaxed space-y-3",
                    p {
                        "Runtime switch for the Native PDF pipeline. Set "
                        span { class: "font-mono text-gray-100", "LAYOUT_ML_ENABLED=true" }
                        " in your env file (or override on the Runtime config page) to turn the pipeline on. Independent of the build-time feature flag — both must be true for the pipeline to run."
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "Why two switches?" }
                    p {
                        "The "
                        span { class: "font-mono text-gray-100", "layout_ml" }
                        " Cargo feature decides whether the code is "
                        em { "available" }
                        " in the binary. "
                        span { class: "font-mono text-gray-100", "LAYOUT_ML_ENABLED" }
                        " decides whether it "
                        em { "runs" }
                        ". You might compile it in (e.g. for a release build that ships to multiple deployments) but disable it per-host to skip the extra CPU/RAM cost on machines that don't need layout analysis."
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "What turning it on changes" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li { "Uploaded PDFs go through Stage 1–4 of the layout pipeline instead of plain-text extraction." }
                        li { "Chunks gain block-type metadata (Title, SectionHeader, Table, Figure, Caption, List) that the retriever can later weight differently." }
                        li { "Per-document indexing latency increases — more CPU, more RAM during ingestion. Steady-state query latency is unchanged." }
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "If you flip it on" }
                    p {
                        "Existing chunks keep whatever block tags they had at upload time; only newly-ingested PDFs benefit. To re-process the whole corpus, trigger a reindex."
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

/// Info modal for the inline LAYOUT_ML_ENABLED toggle in the Native PDF
/// Extraction header.
fn layout_enabled_toggle_info_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100",
                        "Native PDF Extraction toggle — "
                        span { class: "font-mono", "LAYOUT_ML_ENABLED" }
                    }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 leading-relaxed space-y-3",
                    p {
                        "Saves a runtime override for "
                        span { class: "font-mono text-gray-100", "LAYOUT_ML_ENABLED" }
                        " and surfaces a Restart-now button. Same plumbing as the Runtime config page — the override goes into "
                        span { class: "font-mono text-gray-100", "<base_dir>/overrides.json" }
                        " and is consulted at the next startup."
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "Two switches, both must be on" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            strong { "Feature compiled" }
                            " (tile to the left) — build-time gate via the "
                            span { class: "font-mono text-gray-100", "layout_ml" }
                            " Cargo feature. If this is "
                            em { "no" }
                            ", the toggle here is disabled — there's no code in the binary to enable."
                        }
                        li {
                            strong { "Enabled" }
                            " — runtime gate (this toggle). Flips "
                            span { class: "font-mono text-gray-100", "LAYOUT_ML_ENABLED" }
                            ". Both must be true for the Stage 0 pipeline to run."
                        }
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "Why restart-required" }
                    p {
                        "ag reads "
                        span { class: "font-mono text-gray-100", "LAYOUT_ML_ENABLED" }
                        " once at boot to decide whether to register "
                        span { class: "font-mono text-gray-100", "NativePdfExtractor" }
                        " into the extractor registry. The registry can't be swapped live without confusing in-flight uploads, so the change only takes effect after a self re-exec."
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "What changes when you flip it on" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li { "New PDF uploads go through Stage 0 → DocIR → Stage 1 (Parser on /monitor/tip)." }
                        li { "Chunks gain block-type tags (Title, SectionHeader, Table, Figure, Caption, List) that downstream reranking can weight." }
                        li { "Existing chunks are NOT re-tagged — they keep whatever they had at upload time. Trigger a reindex if you want the whole corpus enriched." }
                        li { "Per-document ingestion latency goes up; query latency is unaffected." }
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "Same row on /config/runtime" }
                    p {
                        "This toggle is a convenience surface. The same setting also appears as a row under "
                        em { "Pdf" }
                        " on "
                        a {
                            href: "/config/runtime",
                            class: "text-blue-400 hover:text-blue-300 underline",
                            "/config/runtime"
                        }
                        " — useful if you also want to set "
                        span { class: "font-mono text-gray-100", "LAYOUT_ML_MODEL_ID" }
                        " (the neural-classifier checkpoint) in the same place."
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

/// Info modal for the "Layout model" tile inside Native PDF Extraction.
fn layout_model_info_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-4 w-[98vw] max-h-[92vh] flex flex-col shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-3 shrink-0",
                    h2 { class: "text-lg font-semibold text-gray-100", "Layout model — ORT vs heuristic" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-xs text-gray-300 leading-relaxed flex-1 min-h-0 overflow-y-auto grid grid-cols-2 gap-4 divide-x divide-gray-700",
                    div { class: "space-y-2 pr-4",
                        p {
                            "Stage 2 of the Native PDF pipeline classifies each region as Title, SectionHeader, Body, Footer, Table, Figure, Caption, or List. Two paths exist, and this tile shows which one is currently active."
                        }
                        h3 { class: "text-gray-100 font-semibold pt-1", "ORT (PubLayNet)" }
                        p {
                            "A neural classifier (DETR-style image model or word-feature ONNX) loaded via the "
                            span { class: "font-mono text-gray-100", "ort" }
                            " runtime. Trained on PubLayNet — a ~360 k page dataset of scientific papers labelled with layout regions. Significantly more accurate on dense, multi-column documents than the heuristic, especially for figures and captions."
                        }
                        p {
                            "Three activation paths, tried in order. All appear on the "
                            a { href: "/config/runtime", class: "text-blue-400 hover:text-blue-300 underline", "Runtime config page" }
                            " under "
                            em { "Pdf" }
                            " — set, save, restart."
                        }
                        ul { class: "list-disc pl-5 space-y-1",
                            li {
                                strong { "Tier 0 — auto-download. " }
                                "Set "
                                span { class: "font-mono text-gray-100", "LAYOUT_ML_MODEL_ID=owner/repo" }
                                " and ag fetches the file via hf-hub on first boot, caching to "
                                span { class: "font-mono text-gray-100", "~/.cache/huggingface/hub/" }
                                ". No further network calls. Default filename is "
                                span { class: "font-mono text-gray-100", "model.onnx" }
                                "; use "
                                span { class: "font-mono text-gray-100", "owner/repo:filename.onnx" }
                                " if the repo names it differently. DETR-shape only — pointing this at a word-feature ONNX will fail at classify time."
                            }
                            li {
                                strong { "Tier 1 — local DETR. " }
                                "Set "
                                span { class: "font-mono text-gray-100", "LAYOUT_DETR_MODEL_PATH" }
                                " to a local DETR file. Useful when you want to pin a specific checkpoint or run offline. Falls back here if Tier 0 isn't set or its download/load failed."
                            }
                            li {
                                strong { "Tier 2 — local word-feature ONNX. " }
                                "Set "
                                span { class: "font-mono text-gray-100", "LAYOUT_ORT_MODEL_PATH" }
                                ". No auto-download path for this shape (Tier 0 is DETR-only)."
                            }
                        }
                    }
                    div { class: "space-y-2 pl-4",
                        p {
                            "Manual download example (only needed for Tier 1 / Tier 2):"
                        }
                        ul { class: "list-disc pl-5 space-y-1",
                            li {
                                span { class: "font-mono text-gray-100", "huggingface-cli download <owner/repo> --local-dir ~/models/layout" }
                            }
                            li { "Then point e.g. LAYOUT_DETR_MODEL_PATH=~/models/layout/model.onnx" }
                        }
                        h3 { class: "text-gray-100 font-semibold pt-1", "heuristic only" }
                        p {
                            "Pure-Rust fallback. Groups words into lines by y-proximity, then scores each line for title capitalisation, footer position, table pipe characters, and list bullet markers. No model download required, no GPU, deterministic. Accuracy is good for clean single-column reports; degrades on dense layouts and scientific papers."
                        }
                        h3 { class: "text-gray-100 font-semibold pt-1", "Failure mode" }
                        p {
                            "If the ORT model is configured but fails to load (missing files, bad checkpoint, version mismatch), ag logs a warning and falls back to heuristic automatically — the pipeline never refuses to run because of a layout-model problem."
                        }
                    }
                }
                button {
                    class: "btn btn-sm w-full mt-3 shrink-0",
                    style: "background-color:#7C2A02;",
                    onclick: move |_| show.set(false),
                    "Got it"
                }
            }
        }
    }
}

/// Nested explainer for the `extractous` crate — opens from the Native PDF
/// Extraction modal.
fn extractous_info_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100",
                        span { class: "font-mono", "extractous" }
                        " — multi-format text extractor"
                    }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 leading-relaxed space-y-3",
                    p {
                        span { class: "font-mono text-gray-100", "extractous" }
                        " is a Rust crate that pulls plain text out of many document formats — PDF, DOCX, HTML, RTF, EPUB, plus images via OCR ("
                        a {
                            href: "https://github.com/yobix-ai/extractous",
                            target: "_blank",
                            class: "text-blue-400 hover:text-blue-300 underline",
                            "github.com/yobix-ai/extractous"
                        }
                        "). Under the hood it bundles a native-compiled extractor toolkit so ag stays a single-binary deploy — no Java runtime, no Python sidecar."
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "Why ag uses it here" }
                    p {
                        "It is the "
                        em { "rescue path" }
                        " for Stage 1. When "
                        span { class: "font-mono text-gray-100", "lopdf" }
                        " cannot parse a PDF's content stream — corrupted xref tables, unusual font encodings, malformed streams — ag falls back to extractous so the document is not lost. The chunker still gets useful text to embed; only the layout-ML downstream loses its richer input."
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1",
                        "Trade-off vs "
                        span { class: "font-mono" }
                        "lopdf"
                    }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            span { class: "font-mono text-gray-100", "lopdf" }
                            " returns per-word coordinates (x0, y0, x1, y1) — enough for the layout-ML stage to detect titles, tables, footers."
                        }
                        li {
                            span { class: "font-mono text-gray-100", "extractous" }
                            " returns plain text only. The layout-ML stage downstream collapses to its heuristic classifier because there are no bounding boxes to feed it."
                        }
                        li { "Net effect: a malformed PDF still ends up indexed and retrievable; it just doesn't get the same structural tagging a well-formed PDF would." }
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "Beyond PDFs" }
                    p {
                        "extractous handles many other formats out of the box. ag could route DOCX, HTML, EPUB through it directly today, but currently the upload pipeline only invokes it as a PDF fallback. Widening that surface is on the roadmap."
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

/// Nested explainer for the `lopdf` crate — opens from the Native PDF
/// Extraction modal.
fn lopdf_info_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[85vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100",
                        span { class: "font-mono", "lopdf" }
                        " — a pure-Rust PDF library"
                    }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 leading-relaxed space-y-3",
                    p {
                        span { class: "font-mono text-gray-100", "lopdf" }
                        " is a low-level PDF reader / writer crate for Rust ("
                        a {
                            href: "https://github.com/J-F-Liu/lopdf",
                            target: "_blank",
                            class: "text-blue-400 hover:text-blue-300 underline",
                            "github.com/J-F-Liu/lopdf"
                        }
                        "). It parses the PDF file format directly — objects, streams, cross-reference tables — and exposes the structures as typed Rust values."
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "What \"low-level\" means here" }
                    p {
                        "A PDF is not a text file. It is a binary container of "
                        em { "objects" }
                        ": dictionaries, arrays, streams, references. The visible text lives inside "
                        em { "content streams" }
                        " — sequences of drawing operators like "
                        span { class: "font-mono text-gray-100", "Tj" }
                        " (show text) and "
                        span { class: "font-mono text-gray-100", "Tm" }
                        " (set text matrix). lopdf gives ag direct access to these operators so it can recover not just the text but the (x, y) position of each glyph."
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "Why ag picks it over a higher-level extractor" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            "Per-word bounding boxes. Tools like "
                            span { class: "font-mono text-gray-100", "pdftotext" }
                            " or extractous return plain strings — they discard layout information that the downstream layout-ML stage needs."
                        }
                        li { "Pure Rust. No C/C++ dependency, no Poppler, no MuPDF — ag stays a single-binary deployment." }
                        li { "Predictable failure mode. When lopdf cannot parse a malformed content stream it fails cleanly, and ag falls back to extractous for plain-text rescue." }
                    }
                    h3 { class: "text-gray-100 font-semibold pt-1", "Trade-offs" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li { "Encrypted PDFs and some rare encodings need extra work that the higher-level tools handle implicitly." }
                        li { "lopdf does not render pages to pixels. For image-based PDFs (scanned documents) ag still needs an OCR path — that lives outside this stage." }
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

/// Info modal for the LAYOUT_ML_MODEL_ID input on /config/onnx.
fn layout_ml_model_id_info_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-3xl max-h-[90vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-3",
                    h2 { class: "text-lg font-semibold text-gray-100", "LAYOUT_ML_MODEL_ID — Tier 0 auto-download" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-4",
                    // ── What it is ──
                    p {
                        "HuggingFace Hub spec for a DETR-style image-based layout model. When set, ag downloads the file via "
                        span { class: "font-mono text-gray-100", "hf-hub" }
                        " into "
                        span { class: "font-mono text-gray-100", "~/.cache/huggingface/hub/" }
                        " on first boot and reuses it on subsequent restarts — no network call once cached."
                    }
                    p {
                        "Format: "
                        span { class: "font-mono text-gray-100", "owner/repo" }
                        " (defaults to "
                        span { class: "font-mono text-gray-100", "model.onnx" }
                        " inside the repo) or "
                        span { class: "font-mono text-gray-100", "owner/repo:filename.onnx" }
                        " if the model file is named something else."
                    }
                    p {
                        "Example: "
                        span { class: "font-mono text-gray-100", "cmarkea/detr-layout-detection" }
                        " — 11-class PubLayNet model. Pair with "
                        span { class: "font-mono text-gray-100", "LAYOUT_DETR_NUM_CLASSES=11" }
                        " (the default)."
                    }

                    // ── Hard constraints ──
                    h3 { class: "text-gray-100 font-semibold pt-2", "Hard constraints (the loader will reject mismatches)" }
                    ul { class: "list-disc pl-5 space-y-1 text-gray-300",
                        li {
                            strong { class: "text-gray-100", "DETR-style architecture with "}
                            span { class: "font-mono text-gray-100", "pixel_values" }
                            strong { class: "text-gray-100", " input." }
                            " The Tier 0 loader expects an ONNX file that takes a rendered page image and outputs bounding boxes + class logits. A word-feature ONNX (Tier 2 style) or a transformer-encoder-only checkpoint compiles fine but produces garbage at classify time."
                        }
                        li {
                            strong { class: "text-gray-100", "ONNX export must exist in the repo." }
                            " Many HF layout models ship only PyTorch weights. Either pick a repo with "
                            span { class: "font-mono text-gray-100", "model.onnx" }
                            " already published, or one where the converted variant is uploaded as a separate file (then use "
                            span { class: "font-mono text-gray-100", "owner/repo:filename.onnx" }
                            ")."
                        }
                        li {
                            strong { class: "text-gray-100", "Class count must match " }
                            span { class: "font-mono text-gray-100", "LAYOUT_DETR_NUM_CLASSES" }
                            strong { class: "text-gray-100", "." }
                            " Default is 11 (matches the current Tier 1 cmarkea model). Switching to a 5-class PubLayNet or 13-class DocLayNet variant requires updating num_classes too — otherwise the argmax over class scores points at the wrong column and every region gets the wrong tag."
                        }
                    }

                    // ── Soft considerations ──
                    h3 { class: "text-gray-100 font-semibold pt-2", "Soft considerations" }
                    div { class: "overflow-x-auto",
                        table { class: "w-full text-xs border-collapse",
                            thead {
                                tr { class: "border-b border-gray-600",
                                    th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Factor" }
                                    th { class: "text-left py-1 text-gray-400 font-semibold", "Trade-off" }
                                }
                            }
                            tbody {
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-3 text-gray-200 align-top", "Training corpus" }
                                    td { class: "py-1 text-gray-300",
                                        "PubLayNet ≈ scientific papers (text-heavy, tables, figures, captions). DocLayNet ≈ broader business documents (forms, slides, financial, patents). Pick whichever matches your PDF mix."
                                    }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-3 text-gray-200 align-top", "Backbone size" }
                                    td { class: "py-1 text-gray-300",
                                        "DETR-Lite / nano variants run faster but classify worse on small regions. Full DETR / Deformable-DETR are slower but more accurate on complex layouts. Inference runs per-page during upload, so this matters at scale."
                                    }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-3 text-gray-200 align-top", "License" }
                                    td { class: "py-1 text-gray-300",
                                        "DocLayNet is CC-BY (commercial OK). Some PubLayNet derivatives inherit IBM's research license. Check the repo before deploying."
                                    }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-3 text-gray-200 align-top", "Filename convention" }
                                    td { class: "py-1 text-gray-300",
                                        "If the repo's ONNX file is "
                                        span { class: "font-mono text-gray-100", "model.onnx" }
                                        ", use "
                                        span { class: "font-mono text-gray-100", "owner/repo" }
                                        ". If named "
                                        span { class: "font-mono text-gray-100", "detr.onnx" }
                                        ", "
                                        span { class: "font-mono text-gray-100", "quantized.onnx" }
                                        ", etc., use "
                                        span { class: "font-mono text-gray-100", "owner/repo:filename.onnx" }
                                        "."
                                    }
                                }
                                tr {
                                    td { class: "py-1 pr-3 text-gray-200 align-top", "Quantization" }
                                    td { class: "py-1 text-gray-300",
                                        "Some repos ship "
                                        span { class: "font-mono text-gray-100", "model_quantized.onnx" }
                                        " alongside "
                                        span { class: "font-mono text-gray-100", "model.onnx" }
                                        " — smaller and ~2-3× faster but with measurable accuracy loss. Worth trying for high-volume ingestion."
                                    }
                                }
                            }
                        }
                    }

                    // ── Verification workflow ──
                    h3 { class: "text-gray-100 font-semibold pt-2", "Verification workflow" }
                    ol { class: "list-decimal pl-5 space-y-1 text-gray-300",
                        li {
                            "Pick a candidate — search HF Hub for layout-detection models that have ONNX in the "
                            em { "Files and versions" }
                            " tab. Filter by task = Object Detection or browse the "
                            span { class: "font-mono text-gray-100", "layout-analysis" }
                            " tag."
                        }
                        li {
                            "Set "
                            span { class: "font-mono text-gray-100", "LAYOUT_ML_MODEL_ID = owner/repo[:file]" }
                            " in the input above. Save. Restart."
                        }
                        li {
                            "Watch the journal for "
                            span { class: "font-mono text-gray-100", "Layout model loaded from HF Hub (via LAYOUT_ML_MODEL_ID)" }
                            " vs the warn fallthrough "
                            span { class: "font-mono text-gray-100", "LAYOUT_ML_MODEL_ID set but download failed" }
                            " or "
                            span { class: "font-mono text-gray-100", "HF Hub model downloaded but failed to load" }
                            ". If either warns, the Layout model chip on /config/onnx will flip back to Tier 1 / heuristic."
                        }
                        li {
                            "Upload a representative PDF, then check "
                            a { href: "/monitor/tip", class: "text-blue-400 hover:text-blue-300 underline", "/monitor/tip" }
                            " — block-type tags should distribute reasonably across Title / Body / Table / Figure rather than collapsing everything to one class."
                        }
                        li {
                            "If quality is worse than Tier 1, fall back: clear the override (Save with the input empty), restart — you're back to "
                            span { class: "font-mono text-gray-100", "LAYOUT_DETR_MODEL_PATH" }
                            "."
                        }
                    }

                    // ── Default recommendation ──
                    h3 { class: "text-gray-100 font-semibold pt-2", "Default recommendation" }
                    p {
                        "If you don't have specific requirements, "
                        strong { class: "text-gray-100", "keep this empty" }
                        " and rely on Tier 1 ("
                        span { class: "font-mono text-gray-100", "LAYOUT_DETR_MODEL_PATH" }
                        ") which is already working. Tier 0 is most useful when:"
                    }
                    ul { class: "list-disc pl-5 space-y-1 text-gray-300",
                        li { "Deploying to a fresh machine and you don't want to pre-stage the model file." }
                        li { "Comparing different layout models without managing local files manually." }
                        li { "Tracking upstream model updates — hf-hub respects revision pins; without one you get the latest commit." }
                    }

                    // ── Effect on quality of skipping Tier 0 ──
                    h3 { class: "text-gray-100 font-semibold pt-2", "Effect on quality of skipping Tier 0" }
                    p {
                        strong { class: "text-gray-100", "The tier number is operational, not quality." }
                        " Classification quality depends on "
                        em { "which model file" }
                        " is loaded, not on which tier mechanism loaded it. Skipping Tier 0 has zero quality impact "
                        em { "if" }
                        " Tier 1 points at the same model."
                    }
                    h4 { class: "text-gray-200 font-semibold text-xs pt-1", "What changes when Tier 0 → Tier 1 (same model file)" }
                    div { class: "overflow-x-auto",
                        table { class: "w-full text-xs border-collapse",
                            thead {
                                tr { class: "border-b border-gray-600",
                                    th { class: "text-left py-1 pr-3 text-gray-400 font-semibold", "Aspect" }
                                    th { class: "text-left py-1 text-gray-400 font-semibold", "Result" }
                                }
                            }
                            tbody {
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-3 text-gray-200 align-top", "Bounding-box accuracy" }
                                    td { class: "py-1 text-green-400", "Identical — same ONNX bytes, same weights" }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-3 text-gray-200 align-top", "Class assignment" }
                                    td { class: "py-1 text-green-400", "Identical — same softmax over same logits" }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-3 text-gray-200 align-top", "Per-page latency" }
                                    td { class: "py-1 text-green-400", "Identical — same compute graph" }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-3 text-gray-200 align-top", "First-boot delay" }
                                    td { class: "py-1 text-gray-300", "Faster — no HF Hub download (Tier 0 incurs a one-time ~100 MB pull)" }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-3 text-gray-200 align-top", "Offline behaviour" }
                                    td { class: "py-1 text-gray-300", "Works — Tier 0 needs a populated cache or network on first boot" }
                                }
                                tr {
                                    td { class: "py-1 pr-3 text-gray-200 align-top", "Upstream updates" }
                                    td { class: "py-1 text-gray-300", "Frozen at the staged file (Tier 0 pulls latest unless revision-pinned)" }
                                }
                            }
                        }
                    }
                    h4 { class: "text-gray-200 font-semibold text-xs pt-2", "Where quality actually changes" }
                    ul { class: "list-disc pl-5 space-y-1 text-gray-300",
                        li {
                            "Tier 1 also misses → "
                            strong { class: "text-yellow-400", "Tier 2" }
                            " ("
                            span { class: "font-mono text-gray-100", "LAYOUT_ORT_MODEL_PATH" }
                            ", word-feature ONNX). "
                            em { "Notable drop." }
                            " Word-feature ORT classifies from text + geometry only — it cannot see page pixels. Tables and figures get misclassified more often, especially in sparse documents."
                        }
                        li {
                            "Tier 2 also misses → "
                            strong { class: "text-yellow-400", "heuristic" }
                            " (pure-Rust rules). "
                            em { "Big drop on complex layouts." }
                            " Font-size + position rules work on single-column prose, struggle on multi-column papers, mixed text/figures, dense tables."
                        }
                        li {
                            strong { class: "text-yellow-400", "You point Tier 0 at a different DETR model than Tier 1. " }
                            "Variable — depends on the model. A model trained on the wrong corpus (PubLayNet for business docs, DocLayNet for papers) underperforms. A mismatched "
                            span { class: "font-mono text-gray-100", "LAYOUT_DETR_NUM_CLASSES" }
                            " returns gibberish."
                        }
                    }
                    p { class: "text-gray-400",
                        "Quality concerns kick in only if Tier 1 itself fails to load. The "
                        em { "Layout model:" }
                        " chip on this page tells you which tier is currently active — as long as it shows "
                        span { class: "font-mono text-gray-100", "DETR (local: …)" }
                        " or "
                        span { class: "font-mono text-gray-100", "DETR (HF Hub: …)" }
                        ", classification quality is identical between the two."
                    }

                    // ── Fallthrough behaviour ──
                    h3 { class: "text-gray-100 font-semibold pt-2", "Fallthrough behaviour" }
                    p { class: "text-gray-400",
                        "Tier 0 expects a DETR-style ONNX with a "
                        span { class: "font-mono text-gray-100", "pixel_values" }
                        " input. Pointing this at a word-feature checkpoint will fail at classify time. On download or load failure, ag warns and falls through to Tier 1 ("
                        span { class: "font-mono text-gray-100", "LAYOUT_DETR_MODEL_PATH" }
                        ") and Tier 2 ("
                        span { class: "font-mono text-gray-100", "LAYOUT_ORT_MODEL_PATH" }
                        ")."
                    }
                    p { class: "text-gray-400",
                        "Save is restart-required — the override is written immediately, but the model loads at boot."
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

/// Info modal: which chunker mode to pick for Native PDF Extraction output.
/// Mirrors guidance on /config/chunker but framed specifically around the
/// block-tagged DocIR that the native pipeline produces.
fn chunker_mode_for_native_pdf_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-3xl max-h-[90vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-3",
                    h2 { class: "text-lg font-semibold text-gray-100", "Chunker mode — picking one for Native PDF output" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-3",
                    p {
                        "The Native PDF pipeline produces a "
                        span { class: "font-mono text-gray-100", "DocIR" }
                        " with block-type tags (Title, SectionHeader, Body, Table, Figure, Caption, List, …). The chunker walks those blocks via "
                        span { class: "font-mono text-gray-100", "chunk_ir" }
                        " — atomic blocks (Table, Code, Formula) become single chunks regardless of mode, and section headers always flush the pending accumulation."
                    }
                    p {
                        "The chunker mode only controls "
                        em { "how body-text accumulations between those boundaries are further sliced when they exceed the size limit." }
                        " That's the lever the modes below adjust."
                    }
                    h3 { class: "text-gray-100 font-semibold pt-2", "When to pick which" }
                    ul { class: "list-disc pl-5 space-y-2",
                        li {
                            strong { class: "text-gray-100", "Lightweight — recommended default. " }
                            "Most PDFs have headings. The in-text heading detector (lines starting "
                            span { class: "font-mono text-gray-100", "#" }
                            ", ALL-CAPS, "
                            span { class: "font-mono text-gray-100", ":" }
                            "-suffixed) flushes on natural breaks inside long sections, on top of the DocIR header flushes the IR walker already provides. No embeddings — cheap."
                        }
                        li {
                            strong { class: "text-gray-100", "Semantic — for long narrative PDFs. " }
                            "Research papers, books, manuals, dossiers. Topic-shift detection inside long body sections produces more coherent chunks where headings are sparse. One embedding call per segment — slower indexing, better retrieval."
                        }
                        li {
                            strong { class: "text-gray-100", "Sentence — narrative-heavy without strong headings. " }
                            "Essays, articles, conversational prose. Sentence-first split with overlap improves recall when the document is one long flow of text."
                        }
                        li {
                            strong { class: "text-gray-100", "Fixed — only for very table/list-heavy PDFs. " }
                            "Since tables and code blocks are already atomic regardless of mode, Fixed is a reasonable choice when body text is short and you want minimal compute. Splits body text purely by size with sentence-boundary snap."
                        }
                        li {
                            strong { class: "text-gray-100", "Pipeline — highest quality on long, complex PDFs. " }
                            "Composes Lightweight pre-split with Semantic refinement (or another combination). Most expensive but best retrieval quality on mixed-format documents (prose + tables + figures)."
                        }
                    }
                    p { class: "text-gray-400 pt-1",
                        "Switching the chunker mode is hot-reloaded — no restart required. The change takes effect on the next upload or reindex."
                    }
                    p {
                        "Set it on the "
                        Link {
                            to: Route::ConfigChunker {},
                            class: "text-blue-400 hover:text-blue-300 underline",
                            "Chunker configuration page"
                        }
                        " — either as the global default or per-corpus."
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
