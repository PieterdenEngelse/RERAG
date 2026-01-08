mod components;
mod constants;
mod help_content;
mod helpers;
pub mod state;

use components::{info_modal, InfoIcon};
use constants::*;
use help_content::HelpTopic;
use helpers::{format_gpu_label, format_model_label};

use crate::{
    api::{self, BackendType},
    app::Route,
    components::config_nav::{ConfigNav, ConfigTab},
    components::monitor::*,
};
use dioxus::prelude::*;

#[component]
pub fn ConfigHardware() -> Element {
    let mut hardware_config = use_signal(api::HardwareConfig::default);
    let status = use_signal(|| Option::<String>::None);
    let error = use_signal(|| Option::<String>::None);
    let loading = use_signal(|| false);
    let saving = use_signal(|| false);

    let physical_cores = use_signal(|| Option::<usize>::None);
    let gpus: Signal<Option<Vec<api::GpuInfo>>> = use_signal(|| None);
    let system_info: Signal<Option<api::SystemInfo>> = use_signal(|| None);
    let models: Signal<Vec<api::ModelInfo>> = use_signal(Vec::new);
    let models_loading = use_signal(|| false);
    let model_error = use_signal(|| Option::<String>::None);
    let last_model_backend = use_signal(|| String::new());

    let api_key_status = use_signal(|| Option::<String>::None);
    let api_key_error = use_signal(|| Option::<String>::None);
    let api_keys_loaded = use_signal(|| false);
    let has_openai_key = use_signal(|| false);
    let has_anthropic_key = use_signal(|| false);
    let openai_masked = use_signal(String::new);
    let anthropic_masked = use_signal(String::new);
    let openai_from_env = use_signal(|| false);
    let anthropic_from_env = use_signal(|| false);
    let openai_input = use_signal(String::new);
    let anthropic_input = use_signal(String::new);
    let saving_keys = use_signal(|| false);
    let mut show_api_key_values = use_signal(|| false);

    let show_backend_info = use_signal(|| false);
    let show_model_info = use_signal(|| false);
    let show_num_thread_info = use_signal(|| false);

    let anthropic_llm_config = use_signal(api::LlmConfig::default);
    let anthropic_llm_loading = use_signal(|| false);
    let anthropic_llm_error = use_signal(|| Option::<String>::None);

    // Sampling parameters (editable LlmConfig)
    let mut sampling_config = use_signal(api::LlmConfig::default);
    let sampling_loading = use_signal(|| false);
    let sampling_error = use_signal(|| Option::<String>::None);
    let show_temperature_info = use_signal(|| false);
    let show_top_k_info = use_signal(|| false);
    let show_top_p_info = use_signal(|| false);
    let show_min_p_info = use_signal(|| false);
    let show_typical_p_info = use_signal(|| false);
    let show_tfs_z_info = use_signal(|| false);
    let show_seed_info = use_signal(|| false);
    // Repetition control info signals
    let show_repeat_last_n_info = use_signal(|| false);
    let show_repeat_penalty_info = use_signal(|| false);
    let show_presence_penalty_info = use_signal(|| false);
    let show_frequency_penalty_info = use_signal(|| false);
    let show_penalize_newline_info = use_signal(|| false);
    // Mirostat info signals
    let show_mirostat_info = use_signal(|| false);
    let show_mirostat_tau_info = use_signal(|| false);
    let show_mirostat_eta_info = use_signal(|| false);
    // Output control info signals
    let show_num_predict_info = use_signal(|| false);
    let show_num_keep_info = use_signal(|| false);
    let show_stop_sequences_info = use_signal(|| false);
    let show_num_gpu_info = use_signal(|| false);
    let show_gpu_layers_info = use_signal(|| false);
    let show_main_gpu_info = use_signal(|| false);
    let show_rope_base_info = use_signal(|| false);
    let show_rope_scale_info = use_signal(|| false);
    let show_low_vram_info = use_signal(|| false);
    let show_f16_kv_info = use_signal(|| false);
    let show_num_batch_info = use_signal(|| false);
    let show_num_ctx_info = use_signal(|| false);
    let show_numa_info = use_signal(|| false);
    let show_mmap_info = use_signal(|| false);
    let show_mlock_info = use_signal(|| false);
    let show_logits_all_info = use_signal(|| false);
    let show_vocab_only_info = use_signal(|| false);
    let show_reload_info = use_signal(|| false);
    let show_rope_tuning_info = use_signal(|| false);

    {
        let mut hardware_config = hardware_config.clone();
        let mut status = status.clone();
        let mut error = error.clone();
        let mut loading = loading.clone();
        let mut models = models.clone();
        let mut models_loading = models_loading.clone();
        let mut model_error = model_error.clone();
        let mut last_model_backend = last_model_backend.clone();
        use_future(move || async move {
            loading.set(true);
            error.set(None);
            match api::fetch_hardware_config().await {
                Ok(resp) => {
                    last_model_backend.set(resp.config.backend_type.clone());
                    models_loading.set(true);
                    match api::fetch_models(&resp.config.backend_type).await {
                        Ok(list) => {
                            models.set(list);
                            model_error.set(None);
                        }
                        Err(e) => {
                            models.set(Vec::new());
                            model_error.set(Some(e));
                        }
                    }
                    models_loading.set(false);
                    status.set(Some(resp.message));
                    hardware_config.set(resp.config);
                }
                Err(err_msg) => {
                    error.set(Some(format!("Failed to load hardware config: {}", err_msg)));
                }
            }
            loading.set(false);
        });
    }

    {
        let mut physical_cores = physical_cores.clone();
        use_future(move || async move {
            if let Ok(cores) = api::fetch_physical_cores().await {
                physical_cores.set(Some(cores));
            }
        });
    }

    {
        let mut gpus = gpus.clone();
        let mut system_info = system_info.clone();
        use_future(move || async move {
            if let Ok(info) = api::fetch_gpus().await {
                gpus.set(Some(info));
            }
            if let Ok(info) = api::fetch_system_info().await {
                system_info.set(Some(info));
            }
        });
    }

    {
        let mut models = models.clone();
        let mut models_loading = models_loading.clone();
        let mut model_error = model_error.clone();
        let hardware_config = hardware_config.clone();
        let mut last_model_backend = last_model_backend.clone();
        use_effect(move || {
            let backend = hardware_config().backend_type.clone();
            // Use peek() to read without subscribing, avoiding read-write in same scope
            if backend == last_model_backend.peek().clone() {
                return;
            }
            last_model_backend.set(backend.clone());
            models_loading.set(true);
            model_error.set(None);
            spawn(async move {
                match api::fetch_models(&backend).await {
                    Ok(list) => {
                        models.set(list);
                        model_error.set(None);
                    }
                    Err(e) => {
                        models.set(Vec::new());
                        model_error.set(Some(e));
                    }
                }
                models_loading.set(false);
            });
        });
    }

    {
        let mut anthropic_llm_config = anthropic_llm_config.clone();
        let mut anthropic_llm_loading = anthropic_llm_loading.clone();
        let mut anthropic_llm_error = anthropic_llm_error.clone();
        use_future(move || async move {
            anthropic_llm_loading.set(true);
            anthropic_llm_error.set(None);
            match api::fetch_llm_config().await {
                Ok(resp) => {
                    anthropic_llm_config.set(resp.config);
                }
                Err(err) => {
                    anthropic_llm_error.set(Some(format!("Failed to load LLM config: {}", err)));
                }
            }
            anthropic_llm_loading.set(false);
        });
    }

    // Load sampling config
    {
        let mut sampling_config = sampling_config.clone();
        let mut sampling_loading = sampling_loading.clone();
        let mut sampling_error = sampling_error.clone();
        use_future(move || async move {
            sampling_loading.set(true);
            sampling_error.set(None);
            match api::fetch_llm_config().await {
                Ok(resp) => {
                    sampling_config.set(resp.config);
                }
                Err(err) => {
                    sampling_error.set(Some(format!("Failed to load sampling config: {}", err)));
                }
            }
            sampling_loading.set(false);
        });
    }

    {
        let mut api_key_status = api_key_status.clone();
        let mut api_key_error = api_key_error.clone();
        let mut api_keys_loaded = api_keys_loaded.clone();
        let mut has_openai_key = has_openai_key.clone();
        let mut has_anthropic_key = has_anthropic_key.clone();
        let mut openai_masked = openai_masked.clone();
        let mut anthropic_masked = anthropic_masked.clone();
        let mut openai_from_env = openai_from_env.clone();
        let mut anthropic_from_env = anthropic_from_env.clone();
        use_future(move || async move {
            match api::fetch_api_keys().await {
                Ok(resp) => {
                    has_openai_key.set(resp.has_openai_key);
                    has_anthropic_key.set(resp.has_anthropic_key);
                    openai_masked.set(resp.openai_key_masked);
                    anthropic_masked.set(resp.anthropic_key_masked);
                    openai_from_env.set(resp.openai_from_env);
                    anthropic_from_env.set(resp.anthropic_from_env);
                    api_key_status.set(Some(resp.message));
                    api_keys_loaded.set(true);
                }
                Err(err) => {
                    api_key_error.set(Some(format!("Failed to load API keys: {}", err)));
                }
            }
        });
    }

    let on_save = {
        let hardware_config = hardware_config.clone();
        let status = status.clone();
        let mut error = error.clone();
        let mut saving = saving.clone();
        move |_| {
            saving.set(true);
            error.set(None);
            let payload = hardware_config();
            let mut hardware_config = hardware_config.clone();
            let mut status = status.clone();
            let mut error = error.clone();
            let mut saving = saving.clone();
            spawn(async move {
                match api::commit_hardware_config(&payload).await {
                    Ok(resp) => {
                        status.set(Some(resp.message));
                        hardware_config.set(resp.config);
                    }
                    Err(err_msg) => {
                        error.set(Some(format!("Failed to save hardware config: {}", err_msg)));
                    }
                }
                saving.set(false);
            });
        }
    };

    let on_save_keys = {
        let openai_input = openai_input.clone();
        let anthropic_input = anthropic_input.clone();
        let mut saving_keys = saving_keys.clone();
        let api_key_status = api_key_status.clone();
        let mut api_key_error = api_key_error.clone();
        let has_openai_key = has_openai_key.clone();
        let has_anthropic_key = has_anthropic_key.clone();
        let openai_masked = openai_masked.clone();
        let anthropic_masked = anthropic_masked.clone();
        move |_| {
            if saving_keys() {
                return;
            }
            saving_keys.set(true);
            api_key_error.set(None);
            let payload = api::ApiKeysRequest {
                openai_api_key: openai_input(),
                anthropic_api_key: anthropic_input(),
            };
            let mut saving_keys = saving_keys.clone();
            let mut api_key_status = api_key_status.clone();
            let mut api_key_error = api_key_error.clone();
            let mut has_openai_key = has_openai_key.clone();
            let mut has_anthropic_key = has_anthropic_key.clone();
            let mut openai_masked = openai_masked.clone();
            let mut anthropic_masked = anthropic_masked.clone();
            let mut openai_input = openai_input.clone();
            let mut anthropic_input = anthropic_input.clone();
            spawn(async move {
                match api::save_api_keys(&payload).await {
                    Ok(resp) => {
                        api_key_status.set(Some(resp.message));
                        has_openai_key.set(resp.has_openai_key);
                        has_anthropic_key.set(resp.has_anthropic_key);
                        openai_masked.set(resp.openai_key_masked);
                        anthropic_masked.set(resp.anthropic_key_masked);
                        openai_input.set(String::new());
                        anthropic_input.set(String::new());
                    }
                    Err(err_msg) => {
                        api_key_error.set(Some(format!("Failed to save API keys: {}", err_msg)));
                    }
                }
                saving_keys.set(false);
            });
        }
    };

    let hardware_values = hardware_config();
    let backend_enum = hardware_values.get_backend_type();
    let backend_type_raw = hardware_values.backend_type.clone();
    let is_anthropic_backend = backend_type_raw.eq_ignore_ascii_case("anthropic");
    let supports_threads = backend_enum.supports_thread_config();
    let supports_gpu = backend_enum.supports_gpu_config();
    let supports_gpu_layers = backend_enum.supports_gpu_layers();
    let supports_rope = backend_enum.supports_rope_config();
    let supports_memory = backend_enum.supports_memory_options();
    let is_cloud_backend = backend_enum.is_cloud_backend();
    let is_ollama_backend = matches!(backend_enum, api::BackendType::Ollama);

    let physical_cores_text = physical_cores()
        .map(|cores| format!("Physical cores: {}", cores))
        .unwrap_or_else(|| "Physical cores: --".into());

    let backend_options = BackendType::all();

    let model_hint = if models_loading() {
        Some(("Loading models…".to_string(), "text-blue-300".to_string()))
    } else if let Some(err) = model_error() {
        Some((format!("Model fetch failed: {}", err), "text-red-300".to_string()))
    } else if models().is_empty() {
        Some(("Enter model name manually or ensure backend provides a list.".to_string(), "text-yellow-300".to_string()))
    } else {
        None
    };

    let refresh_models = {
        let hardware_config = hardware_config.clone();
        let mut models = models.clone();
        let mut models_loading = models_loading.clone();
        let mut model_error = model_error.clone();
        move |_| {
            if models_loading() {
                return;
            }
            let backend = hardware_config().backend_type.clone();
            models_loading.set(true);
            model_error.set(None);
            spawn(async move {
                match api::fetch_models(&backend).await {
                    Ok(list) => {
                        models.set(list);
                        model_error.set(None);
                    }
                    Err(err) => {
                        models.set(Vec::new());
                        model_error.set(Some(err));
                    }
                }
                models_loading.set(false);
            });
        }
    };

    let available_models = models();
    let active_model_name = hardware_config().model.clone();
    let active_model_in_list = available_models.iter().any(|m| m.name == active_model_name);
    let gpus_value = gpus();
    let system_info_value = system_info();
    let anthropic_llm = anthropic_llm_config();

    let mut backend_info_signal = show_backend_info.clone();
    let mut model_info_signal = show_model_info.clone();
    let mut num_thread_info_signal = show_num_thread_info.clone();
    let mut num_batch_info_signal = show_num_batch_info.clone();
    let mut num_gpu_info_signal = show_num_gpu_info.clone();
    let mut main_gpu_info_signal = show_main_gpu_info.clone();
    let mut gpu_layers_info_signal = show_gpu_layers_info.clone();
    let mut low_vram_info_signal = show_low_vram_info.clone();
    let mut f16_kv_info_signal = show_f16_kv_info.clone();
    let mut rope_base_info_signal = show_rope_base_info.clone();
    let mut rope_scale_info_signal = show_rope_scale_info.clone();
    let mut num_ctx_info_signal = show_num_ctx_info.clone();
    let mut numa_info_signal = show_numa_info.clone();
    let mut mmap_info_signal = show_mmap_info.clone();
    let mut mlock_info_signal = show_mlock_info.clone();
    let mut logits_all_info_signal = show_logits_all_info.clone();
    let mut vocab_only_info_signal = show_vocab_only_info.clone();
    let mut reload_info_signal = show_reload_info.clone();
    let mut rope_tuning_info_signal = show_rope_tuning_info.clone();
    let mut temperature_info_signal = show_temperature_info.clone();
    let mut top_k_info_signal = show_top_k_info.clone();
    let mut top_p_info_signal = show_top_p_info.clone();
    let mut min_p_info_signal = show_min_p_info.clone();
    let mut typical_p_info_signal = show_typical_p_info.clone();
    let mut tfs_z_info_signal = show_tfs_z_info.clone();
    let mut seed_info_signal = show_seed_info.clone();
    let mut repeat_last_n_info_signal = show_repeat_last_n_info.clone();
    let mut repeat_penalty_info_signal = show_repeat_penalty_info.clone();
    let mut presence_penalty_info_signal = show_presence_penalty_info.clone();
    let mut frequency_penalty_info_signal = show_frequency_penalty_info.clone();
    let mut penalize_newline_info_signal = show_penalize_newline_info.clone();
    let mut mirostat_info_signal = show_mirostat_info.clone();
    let mut mirostat_tau_info_signal = show_mirostat_tau_info.clone();
    let mut mirostat_eta_info_signal = show_mirostat_eta_info.clone();
    let mut num_predict_info_signal = show_num_predict_info.clone();
    let mut num_keep_info_signal = show_num_keep_info.clone();
    let mut stop_sequences_info_signal = show_stop_sequences_info.clone();
    let mut openai_input_signal = openai_input.clone();
    let mut anthropic_input_signal = anthropic_input.clone();

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::Config {})),
                    BreadcrumbItem::new("Hardware & performance", Some(Route::ConfigHardware {})),
                ],
            }

            ConfigNav { active: ConfigTab::Hardware }

            Panel { title: None, refresh: None,
                div { class: "flex flex-row gap-4",
                    // Left side: all content
                    div { class: "flex-1 flex flex-col gap-2",
                        span { class: "text-base text-gray-100 font-semibold", "Change backend and model" }
                        if loading() {
                            div { class: "text-xs text-blue-300", "Loading hardware config…" }
                        } else if let Some(err) = error() {
                            div { class: "text-xs text-red-400", "{err}" }
                        } else if let Some(msg) = status() {
                            div { class: "text-xs text-gray-400", "{msg}" }
                        }
                        div { class: "flex flex-col md:flex-row md:items-start gap-4",
                    div { class: "flex flex-col gap-1 text-xs text-gray-200 md:w-64",
                        div { class: "flex items-center gap-2",
                            label { class: "text-gray-300 font-medium", "Backend" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| backend_info_signal.set(true),
                                title: "Backend help",
                                InfoIcon {}
                            }
                        }
                        div { class: "flex items-end gap-2",
                            select {
                                class: "select select-sm select-bordered bg-gray-700 text-gray-200",
                                value: backend_type_raw.clone(),
                                onchange: move |evt| {
                                    let selected_value = evt.value();
                                    hardware_config.with_mut(|cfg| {
                                        cfg.backend_type = selected_value.clone();
                                        cfg.model.clear();
                                    });
                                },
                                for option in backend_options.iter() {
                                    option {
                                        value: option.to_api_string(),
                                        selected: backend_type_raw == option.to_api_string(),
                                        "{option.label()}"
                                    }
                                }
                            }
                        }
                    }
                    div { class: "flex flex-col gap-1 text-xs text-gray-200",
                        span { class: "font-semibold text-gray-300", "Active backend" }
                        span { class: "text-sm text-gray-100", "{backend_enum.label()}" }
                        span { class: "text-xs text-gray-400", "Active model: {hardware_values.model}" }
                        span { class: "text-xs text-gray-400", "Supports: threads {supports_threads}, GPU {supports_gpu}, GPU layers {supports_gpu_layers}, RoPE {supports_rope}" }
                    }
                    div { class: PARAM_BLOCK_CLASS,
                        div { class: "flex items-center gap-2",
                            label { class: "text-gray-300 font-medium", "Model" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| model_info_signal.set(true),
                                title: "Model selection guidance",
                                InfoIcon {}
                            }
                        }
                        div { class: "flex flex-col gap-1",
                            span { class: "text-[0.7rem] text-gray-400", "Active model" }
                            input {
                                r#type: "text",
                                class: PARAM_TEXT_INPUT_CLASS,
                                value: if active_model_name.is_empty() { "Not set".to_string() } else { active_model_name.clone() },
                                readonly: true,
                            }
                        }
                        div { class: "flex items-center gap-2",
                            select {
                                class: "select select-sm select-bordered bg-gray-700 text-gray-200",
                                value: if active_model_in_list { hardware_values.model.clone() } else { active_model_name.clone() },
                                onchange: move |evt| {
                                    let value = evt.value();
                                    hardware_config.with_mut(|cfg| cfg.model = value);
                                },
                                option {
                                    value: active_model_name.clone(),
                                    disabled: true,
                                    selected: !active_model_in_list,
                                    "Active model: {active_model_name}"
                                }
                                for model in available_models.iter() {
                                    option {
                                        value: model.name.clone(),
                                        selected: hardware_values.model == model.name,
                                        "{format_model_label(model)}"
                                    }
                                }
                            }
                            button {
                                class: "btn btn-xs",
                                onclick: refresh_models,
                                disabled: models_loading(),
                                if models_loading() { "Working…" } else { "Reload" }
                            }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| reload_info_signal.set(true),
                                title: "Reload help",
                                InfoIcon {}
                            }
                        }
                        if let Some((message, class_name)) = model_hint {
                            span { class: class_name, "{message}" }
                        }
                        if available_models.is_empty() {
                            span { class: "text-[0.7rem] text-yellow-300", "No discovery results yet" }
                        }
                        div { class: "flex items-center gap-3 text-[0.7rem] text-gray-400",
                            span {
                                if models_loading() {
                                    "Refreshing list…"
                                } else {
                                    "{available_models.len()} model entries"
                                }
                            }
                        }
                    }
                    div { class: PARAM_BLOCK_CLASS,
                        span { class: "text-gray-300 font-medium", "Model discovery status" }
                        if let Some(err) = model_error() {
                            div { class: "text-xs text-red-400", "{err}" }
                        } else if available_models.is_empty() {
                            div { class: "text-xs text-yellow-300", "No catalog entries returned. You can still type the model name manually." }
                        } else {
                            div { class: "text-xs text-green-300", "Model list loaded." }
                        }
                    }
                        }
                    }
                    // Right side: save button, vertically centered
                    div { class: "flex items-center",
                        button {
                            class: "btn btn-md px-8",
                            style: "background-color: #1D6B9A; border-color: #1D6B9A; color: white; box-shadow: none;",
                            onclick: on_save.clone(),
                            disabled: saving() || loading(),
                            if saving() { "Saving…" } else { "Save" }
                        }
                    }
                }
            }

            Panel { title: Some("System overview".into()), refresh: None,
                div { class: "grid grid-cols-1 md:grid-cols-3 gap-4 text-xs text-gray-200",
                    div { class: "rounded border border-gray-700 bg-gray-800 p-4 flex flex-col gap-2",
                        span { class: "text-sm text-gray-300 font-semibold", "CPU" }
                        span { "{physical_cores_text}" }
                        if let Some(info) = system_info_value.clone() {
                            span { "Logical cores: {info.logical_cores}" }
                            span { "OS: {info.os} ({info.os_family})" }
                            span { "Arch: {info.arch}" }
                        } else {
                            span { "Collecting system info…" }
                        }
                    }
                    div { class: "rounded border border-gray-700 bg-gray-800 p-4 flex flex-col gap-2",
                        span { class: "text-sm text-gray-300 font-semibold", "GPU inventory" }
                        if let Some(list) = gpus_value.clone() {
                            if list.is_empty() {
                                span { class: "text-gray-400", "No GPUs reported by backend" }
                            } else {
                                for gpu in list.iter() {
                                    div { class: "text-gray-200", "{format_gpu_label(gpu)}" }
                                }
                            }
                        } else {
                            span { class: "text-gray-400", "Detecting GPU hardware…" }
                        }
                    }
                    div { class: "rounded border border-gray-700 bg-gray-800 p-4 flex flex-col gap-2",
                        span { class: "text-sm text-gray-300 font-semibold", "Backend summary" }
                        span { "Backend: {backend_enum.label()}" }
                        span { "Cloud backend: {is_cloud_backend}" }
                        span { "GPU config support: {supports_gpu}" }
                        div { class: "flex items-center gap-2",
                            span { "RoPE tuning: {supports_rope}" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| rope_tuning_info_signal.set(true),
                                title: "RoPE tuning help",
                                InfoIcon {}
                            }
                        }
                    }
                }
            }

            Panel { title: None, refresh: None,
                div { class: "flex flex-wrap gap-4 items-stretch",
                div { class: "rounded border border-gray-600 p-4 w-fit",
                    span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Model Loading" }
                    div { class: "flex flex-wrap gap-10 justify-start",
                        // Column 1: Threading
                    div { class: PARAM_COLUMN_CLASS,
                        span { class: "text-gray-300 font-semibold", "Threading" }
                        if supports_threads {
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "num_thread" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        min: "1",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{}", hardware_values.num_thread),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<usize>() {
                                                hardware_config.with_mut(|cfg| cfg.num_thread = value.max(1));
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| num_thread_info_signal.set(true),
                                        title: "Thread help",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "num_ctx" }
                            div { class: "flex items-end gap-2",
                                input {
                                    r#type: "number",
                                    min: "256",
                                    step: "128",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: format!("{}", hardware_values.num_ctx),
                                    onchange: move |evt| {
                                        if let Ok(value) = evt.value().parse::<usize>() {
                                            hardware_config.with_mut(|cfg| cfg.num_ctx = value.max(256));
                                        }
                                    }
                                }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| num_ctx_info_signal.set(true),
                                    title: "Context size help",
                                    InfoIcon {}
                                }
                            }
                        }
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "num_batch" }
                            div { class: "flex items-end gap-2",
                                input {
                                    r#type: "number",
                                    min: "1",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: format!("{}", hardware_values.num_batch),
                                    onchange: move |evt| {
                                        if let Ok(value) = evt.value().parse::<usize>() {
                                            hardware_config.with_mut(|cfg| cfg.num_batch = value.max(1));
                                        }
                                    }
                                }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| num_batch_info_signal.set(true),
                                    title: "Batch size help",
                                    InfoIcon {}
                                }
                            }
                        }
                    }

                    // Column 2: Memory
                    div { class: PARAM_COLUMN_CLASS,
                        span { class: "text-gray-300 font-semibold", "Memory" }
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "NUMA" }
                            div { class: "flex items-end gap-2",
                                input {
                                    r#type: "checkbox",
                                    class: "toggle toggle-sm !border !border-white",
                                    style: {
                                        format!(
                                            "border: 1px solid white; background-color: {};",
                                            if hardware_values.numa { "" } else { "#d1d5db" }
                                        )
                                    },
                                    checked: hardware_values.numa,
                                    onchange: move |_| {
                                        hardware_config.with_mut(|cfg| cfg.numa = !cfg.numa);
                                    },
                                }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| numa_info_signal.set(true),
                                    title: "NUMA help",
                                    InfoIcon {}
                                }
                            }
                        }
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "use_mmap" }
                            div { class: "flex items-end gap-2",
                                input {
                                    r#type: "checkbox",
                                    class: "toggle toggle-sm !border !border-white",
                                    style: {
                                        format!(
                                            "border: 1px solid white; background-color: {};",
                                            if hardware_values.use_mmap { "" } else { "#d1d5db" }
                                        )
                                    },
                                    checked: hardware_values.use_mmap,
                                    onchange: move |_| {
                                        hardware_config.with_mut(|cfg| cfg.use_mmap = !cfg.use_mmap);
                                    },
                                }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| mmap_info_signal.set(true),
                                    title: "mmap help",
                                    InfoIcon {}
                                }
                            }
                        }
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "use_mlock" }
                            div { class: "flex items-end gap-2",
                                input {
                                    r#type: "checkbox",
                                    class: "toggle toggle-sm !border !border-white",
                                    style: {
                                        format!(
                                            "border: 1px solid white; background-color: {};",
                                            if hardware_values.use_mlock { "" } else { "#d1d5db" }
                                        )
                                    },
                                    checked: hardware_values.use_mlock,
                                    onchange: move |_| {
                                        hardware_config.with_mut(|cfg| cfg.use_mlock = !cfg.use_mlock);
                                    },
                                }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| mlock_info_signal.set(true),
                                    title: "mlock help",
                                    InfoIcon {}
                                }
                            }
                        }
                    }

                    // Column 3: GPU
                    div { class: PARAM_COLUMN_CLASS,
                        span { class: "text-gray-300 font-semibold", "GPU" }
                        if supports_gpu {
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "num_gpu" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        min: "0",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{}", hardware_values.num_gpu),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<usize>() {
                                                hardware_config.with_mut(|cfg| cfg.num_gpu = value);
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| num_gpu_info_signal.set(true),
                                        title: "num_gpu help",
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "main_gpu" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        min: "0",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{}", hardware_values.main_gpu),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<usize>() {
                                                hardware_config.with_mut(|cfg| cfg.main_gpu = value);
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| main_gpu_info_signal.set(true),
                                        title: "main_gpu help",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                        if supports_gpu_layers {
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "gpu_layers" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        min: "0",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{}", hardware_values.gpu_layers),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<usize>() {
                                                hardware_config.with_mut(|cfg| cfg.gpu_layers = value);
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| gpu_layers_info_signal.set(true),
                                        title: "gpu_layers help",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                        // Advanced options
                        span { class: "text-gray-300 font-semibold mt-2", "Advanced" }
                        if supports_memory {
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center gap-2",
                                    label { class: "{PARAM_LABEL_CLASS} inline-block w-16", "low_vram" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| low_vram_info_signal.set(true),
                                        title: "low_vram help",
                                        InfoIcon {}
                                    }
                                }
                                input {
                                    r#type: "checkbox",
                                    class: "toggle toggle-sm !border !border-white",
                                    style: {
                                        format!(
                                            "border: 1px solid white; background-color: {};",
                                            if hardware_values.low_vram { "" } else { "#d1d5db" }
                                        )
                                    },
                                    checked: hardware_values.low_vram,
                                    onchange: move |_| {
                                        hardware_config.with_mut(|cfg| cfg.low_vram = !cfg.low_vram);
                                    },
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center gap-2",
                                    label { class: "{PARAM_LABEL_CLASS} inline-block w-16", "f16_kv" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| f16_kv_info_signal.set(true),
                                        title: "f16_kv help",
                                        InfoIcon {}
                                    }
                                }
                                input {
                                    r#type: "checkbox",
                                    class: "toggle toggle-sm !border !border-white",
                                    style: {
                                        format!(
                                            "border: 1px solid white; background-color: {};",
                                            if hardware_values.f16_kv { "" } else { "#d1d5db" }
                                        )
                                    },
                                    checked: hardware_values.f16_kv,
                                    onchange: move |_| {
                                        hardware_config.with_mut(|cfg| cfg.f16_kv = !cfg.f16_kv);
                                    },
                                }
                            }
                        }
                        div { class: PARAM_BLOCK_CLASS,
                            div { class: "flex items-center gap-2",
                                label { class: "{PARAM_LABEL_CLASS} inline-block w-16", "logits_all" }
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| logits_all_info_signal.set(true),
                                    title: "logits_all help",
                                    InfoIcon {}
                                }
                            }
                            input {
                                r#type: "checkbox",
                                class: "toggle toggle-sm !border !border-white",
                                style: {
                                    format!(
                                        "border: 1px solid white; background-color: {};",
                                        if hardware_values.logits_all { "" } else { "#d1d5db" }
                                    )
                                },
                                checked: hardware_values.logits_all,
                                onchange: move |_| {
                                    hardware_config.with_mut(|cfg| cfg.logits_all = !cfg.logits_all);
                                },
                            }
                        }
                    }
                }
                    if supports_rope {
                        div { class: "mt-4 flex flex-wrap gap-10 justify-start",
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "RoPE" }
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center gap-2",
                                    label { class: PARAM_LABEL_CLASS_TIGHT, "rope_frequency_base" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| rope_base_info_signal.set(true),
                                        title: "RoPE base help",
                                        InfoIcon {}
                                    }
                                }
                                input {
                                    r#type: "number",
                                    min: "1",
                                    step: "100",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: format!("{:.0}", hardware_values.rope_frequency_base),
                                    onchange: move |evt| {
                                        if let Ok(value) = evt.value().parse::<f32>() {
                                            hardware_config.with_mut(|cfg| cfg.rope_frequency_base = value.max(1.0));
                                        }
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center gap-2",
                                    label { class: PARAM_LABEL_CLASS_TIGHT, "rope_frequency_scale" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| rope_scale_info_signal.set(true),
                                        title: "RoPE scale help",
                                        InfoIcon {}
                                    }
                                }
                                input {
                                    r#type: "number",
                                    step: "0.1",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: format!("{:.2}", hardware_values.rope_frequency_scale),
                                    onchange: move |evt| {
                                        if let Ok(value) = evt.value().parse::<f32>() {
                                            hardware_config.with_mut(|cfg| cfg.rope_frequency_scale = value.max(0.1));
                                        }
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center gap-2",
                                    label { class: PARAM_LABEL_CLASS, "vocab_only" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| vocab_only_info_signal.set(true),
                                        title: "vocab_only help",
                                        InfoIcon {}
                                    }
                                }
                                input {
                                    r#type: "checkbox",
                                    class: "toggle toggle-sm !border !border-white",
                                    style: {
                                        format!(
                                            "border: 1px solid white; background-color: {};",
                                            if hardware_values.vocab_only { "" } else { "#d1d5db" }
                                        )
                                    },
                                    checked: hardware_values.vocab_only,
                                    onchange: move |_| {
                                        hardware_config.with_mut(|cfg| cfg.vocab_only = !cfg.vocab_only);
                                    },
                                }
                            }
                        }
                    }
                    }
                }

                // Sampling parameters group (Ollama only)
                if is_ollama_backend {
                div { class: "rounded border border-gray-600 p-4 w-fit",
                    span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Sampling" }
                    if sampling_loading() {
                        div { class: "text-xs text-gray-400", "Loading sampling config…" }
                    } else if let Some(err) = sampling_error() {
                        div { class: "text-xs text-red-400", "{err}" }
                    } else {
                        div { class: "flex flex-wrap gap-10 justify-start",
                            // Column 1: Basic sampling
                            div { class: PARAM_COLUMN_CLASS,
                                span { class: "text-gray-300 font-semibold", "Basic" }
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "temperature" }
                                    div { class: "flex items-end gap-2",
                                        input {
                                            r#type: "number",
                                            step: "0.1",
                                            min: "0",
                                            max: "2",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: format!("{:.1}", sampling_config().temperature),
                                            onchange: move |evt| {
                                                if let Ok(value) = evt.value().parse::<f32>() {
                                                    sampling_config.with_mut(|cfg| cfg.temperature = value.clamp(0.0, 2.0));
                                                }
                                            }
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| temperature_info_signal.set(true),
                                            title: "Temperature help",
                                            InfoIcon {}
                                        }
                                    }
                                }
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "top_k" }
                                    div { class: "flex items-end gap-2",
                                        input {
                                            r#type: "number",
                                            step: "1",
                                            min: "1",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: format!("{}", sampling_config().top_k),
                                            onchange: move |evt| {
                                                if let Ok(value) = evt.value().parse::<usize>() {
                                                    sampling_config.with_mut(|cfg| cfg.top_k = value.max(1));
                                                }
                                            }
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| top_k_info_signal.set(true),
                                            title: "Top-K help",
                                            InfoIcon {}
                                        }
                                    }
                                }
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "top_p" }
                                    div { class: "flex items-end gap-2",
                                        input {
                                            r#type: "number",
                                            step: "0.05",
                                            min: "0",
                                            max: "1",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: format!("{:.2}", sampling_config().top_p),
                                            onchange: move |evt| {
                                                if let Ok(value) = evt.value().parse::<f32>() {
                                                    sampling_config.with_mut(|cfg| cfg.top_p = value.clamp(0.0, 1.0));
                                                }
                                            }
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| top_p_info_signal.set(true),
                                            title: "Top-P help",
                                            InfoIcon {}
                                        }
                                    }
                                }
                            }
                            // Column 2: Advanced sampling
                            div { class: PARAM_COLUMN_CLASS,
                                span { class: "text-gray-300 font-semibold", "Advanced" }
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "min_p" }
                                    div { class: "flex items-end gap-2",
                                        input {
                                            r#type: "number",
                                            step: "0.01",
                                            min: "0",
                                            max: "1",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: format!("{:.2}", sampling_config().min_p),
                                            onchange: move |evt| {
                                                if let Ok(value) = evt.value().parse::<f32>() {
                                                    sampling_config.with_mut(|cfg| cfg.min_p = value.clamp(0.0, 1.0));
                                                }
                                            }
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| min_p_info_signal.set(true),
                                            title: "Min-P help",
                                            InfoIcon {}
                                        }
                                    }
                                }
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "typical_p" }
                                    div { class: "flex items-end gap-2",
                                        input {
                                            r#type: "number",
                                            step: "0.01",
                                            min: "0",
                                            max: "1",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: format!("{:.2}", sampling_config().typical_p),
                                            onchange: move |evt| {
                                                if let Ok(value) = evt.value().parse::<f32>() {
                                                    sampling_config.with_mut(|cfg| cfg.typical_p = value.clamp(0.0, 1.0));
                                                }
                                            }
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| typical_p_info_signal.set(true),
                                            title: "Typical-P help",
                                            InfoIcon {}
                                        }
                                    }
                                }
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "tfs_z" }
                                    div { class: "flex items-end gap-2",
                                        input {
                                            r#type: "number",
                                            step: "0.01",
                                            min: "0",
                                            max: "1",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: format!("{:.2}", sampling_config().tfs_z),
                                            onchange: move |evt| {
                                                if let Ok(value) = evt.value().parse::<f32>() {
                                                    sampling_config.with_mut(|cfg| cfg.tfs_z = value.clamp(0.0, 1.0));
                                                }
                                            }
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| tfs_z_info_signal.set(true),
                                            title: "TFS help",
                                            InfoIcon {}
                                        }
                                    }
                                }
                            }
                            // Column 3: Optional
                            div { class: PARAM_COLUMN_CLASS,
                                span { class: "text-gray-300 font-semibold", "Optional" }
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "seed" }
                                    div { class: "flex items-end gap-2",
                                        input {
                                            r#type: "number",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            placeholder: "random",
                                            value: sampling_config().seed.map(|s| s.to_string()).unwrap_or_default(),
                                            onchange: move |evt| {
                                                let val = evt.value();
                                                if val.trim().is_empty() {
                                                    sampling_config.with_mut(|cfg| cfg.seed = None);
                                                } else if let Ok(seed) = val.parse::<i64>() {
                                                    sampling_config.with_mut(|cfg| cfg.seed = Some(seed));
                                                }
                                            }
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| seed_info_signal.set(true),
                                            title: "Seed help",
                                            InfoIcon {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Mirostat (adaptive sampling) group (Ollama only)
                div { class: "rounded border border-gray-600 p-4 w-fit",
                    span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Mirostat" }
                    div { class: "flex flex-wrap gap-10 justify-start",
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Adaptive Sampling" }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "mirostat" }
                                div { class: "flex items-end gap-2",
                                    select {
                                        class: "select select-xs select-bordered bg-gray-700 text-gray-200",
                                        value: format!("{}", sampling_config().mirostat),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<i32>() {
                                                sampling_config.with_mut(|cfg| cfg.mirostat = value.clamp(0, 2));
                                            }
                                        },
                                        option { value: "0", "0 - Disabled" }
                                        option { value: "1", "1 - Mirostat v1" }
                                        option { value: "2", "2 - Mirostat v2" }
                                    }
                                    div { class: "flex-1" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| mirostat_info_signal.set(true),
                                        title: "Mirostat help",
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "mirostat_tau" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        step: "0.1",
                                        min: "0",
                                        max: "10",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{:.1}", sampling_config().mirostat_tau),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<f32>() {
                                                sampling_config.with_mut(|cfg| cfg.mirostat_tau = value.clamp(0.0, 10.0));
                                            }
                                        }
                                    }
                                    div { class: "flex-1" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| mirostat_tau_info_signal.set(true),
                                        title: "Mirostat tau help",
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "mirostat_eta" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        step: "0.01",
                                        min: "0",
                                        max: "1",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{:.2}", sampling_config().mirostat_eta),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<f32>() {
                                                sampling_config.with_mut(|cfg| cfg.mirostat_eta = value.clamp(0.0, 1.0));
                                            }
                                        }
                                    }
                                    div { class: "flex-1" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| mirostat_eta_info_signal.set(true),
                                        title: "Mirostat eta help",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                    }
                }

                // Repetition Control group (Ollama only)
                div { class: "rounded border border-gray-600 p-4 w-fit",
                    span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Repetition Control" }
                    div { class: "flex flex-wrap gap-10 justify-start",
                        // Column 1: Lookback & Penalty
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Penalties" }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "repeat_last_n" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        step: "1",
                                        min: "-1",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{}", sampling_config().repeat_last_n),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<i64>() {
                                                sampling_config.with_mut(|cfg| cfg.repeat_last_n = value.max(-1) as usize);
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| repeat_last_n_info_signal.set(true),
                                        title: "Repeat last N help",
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "repeat_penalty" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        step: "0.1",
                                        min: "0",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{:.1}", sampling_config().repeat_penalty),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<f32>() {
                                                sampling_config.with_mut(|cfg| cfg.repeat_penalty = value.max(0.0));
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| repeat_penalty_info_signal.set(true),
                                        title: "Repeat penalty help",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                        // Column 2: Presence & Frequency
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Token Penalties" }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "presence_penalty" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        step: "0.1",
                                        min: "0",
                                        max: "2",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{:.1}", sampling_config().presence_penalty),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<f32>() {
                                                sampling_config.with_mut(|cfg| cfg.presence_penalty = value.clamp(0.0, 2.0));
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| presence_penalty_info_signal.set(true),
                                        title: "Presence penalty help",
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "frequency_penalty" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        step: "0.1",
                                        min: "0",
                                        max: "2",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{:.1}", sampling_config().frequency_penalty),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<f32>() {
                                                sampling_config.with_mut(|cfg| cfg.frequency_penalty = value.clamp(0.0, 2.0));
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| frequency_penalty_info_signal.set(true),
                                        title: "Frequency penalty help",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                        // Column 3: Options
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Options" }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "penalize_newline" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "checkbox",
                                        class: "toggle toggle-sm !border !border-white",
                                        style: {
                                            format!(
                                                "border: 1px solid white; background-color: {};",
                                                if sampling_config().penalize_newline { "" } else { "#d1d5db" }
                                            )
                                        },
                                        checked: sampling_config().penalize_newline,
                                        onchange: move |_| {
                                            sampling_config.with_mut(|cfg| cfg.penalize_newline = !cfg.penalize_newline);
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| penalize_newline_info_signal.set(true),
                                        title: "Penalize newline help",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                    }
                }

                // Output Control group (Ollama only)
                div { class: "rounded border border-gray-600 p-4 w-fit",
                    span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Output Control" }
                    div { class: "flex flex-wrap gap-10 justify-start",
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Generation" }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "num_predict" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        step: "1",
                                        min: "-2",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{}", sampling_config().num_predict),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<i64>() {
                                                sampling_config.with_mut(|cfg| cfg.num_predict = value.max(-2));
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| num_predict_info_signal.set(true),
                                        title: "Max tokens help",
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "num_keep" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "number",
                                        step: "1",
                                        min: "-1",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: format!("{}", sampling_config().num_keep),
                                        onchange: move |evt| {
                                            if let Ok(value) = evt.value().parse::<i64>() {
                                                sampling_config.with_mut(|cfg| cfg.num_keep = value.max(-1));
                                            }
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| num_keep_info_signal.set(true),
                                        title: "Tokens to keep help",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                        div { class: PARAM_COLUMN_CLASS,
                            span { class: "text-gray-300 font-semibold", "Stop Conditions" }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "stop" }
                                div { class: "flex items-end gap-2",
                                    input {
                                        r#type: "text",
                                        class: "input input-xs input-bordered bg-gray-700 text-gray-200 w-48",
                                        placeholder: "e.g. </s>, ###",
                                        value: sampling_config().stop_sequences.join(", "),
                                        onchange: move |evt| {
                                            let value = evt.value();
                                            let sequences: Vec<String> = value
                                                .split(',')
                                                .map(|s| s.trim().to_string())
                                                .filter(|s| !s.is_empty())
                                                .collect();
                                            sampling_config.with_mut(|cfg| cfg.stop_sequences = sequences);
                                        }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| stop_sequences_info_signal.set(true),
                                        title: "Stop sequences help",
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

            if is_cloud_backend {
                Panel { title: Some("Cloud API keys".into()), refresh: None,
                    div { class: "flex items-center justify-between gap-4",
                        span { class: "text-base text-gray-200 font-semibold", "Stored credentials" }
                        button {
                            class: "btn btn-xs",
                            style: "background-color: #1D6B9A; border-color: #1D6B9A; color: white; box-shadow: none;",
                            onclick: on_save_keys.clone(),
                            disabled: saving_keys(),
                            if saving_keys() { "Saving…" } else { "Save keys" }
                        }
                    }
                    if !api_keys_loaded() {
                        div { class: "text-xs text-gray-400", "Fetching key status…" }
                    }
                    if let Some(err) = api_key_error() {
                        div { class: "text-xs text-red-400", "{err}" }
                    } else if let Some(msg) = api_key_status() {
                        div { class: "text-xs text-gray-400", "{msg}" }
                    }
                    div { class: "flex flex-col md:flex-row gap-6",
                        div { class: PARAM_BLOCK_CLASS,
                            span { class: "text-sm text-gray-300 font-semibold", "OpenAI" }
                            span { class: "text-[0.7rem] text-gray-400", if openai_from_env() { "Loaded from environment" } else { "Persisted in database" } }
                            span { class: "text-[0.7rem] text-gray-400", { if has_openai_key() { format!("Current: {}", if show_api_key_values() { openai_masked() } else { "••••••".into() }) } else { "No key stored".into() } } }
                            input {
                                r#type: "password",
                                class: PARAM_TEXT_INPUT_CLASS,
                                placeholder: "sk-...",
                                value: openai_input_signal(),
                                oninput: move |evt| openai_input_signal.set(evt.value()),
                            }
                        }
                        div { class: PARAM_BLOCK_CLASS,
                            span { class: "text-sm text-gray-300 font-semibold", "Anthropic" }
                            span { class: "text-[0.7rem] text-gray-400", if anthropic_from_env() { "Loaded from environment" } else { "Persisted in database" } }
                            span { class: "text-[0.7rem] text-gray-400", { if has_anthropic_key() { format!("Current: {}", if show_api_key_values() { anthropic_masked() } else { "••••••".into() }) } else { "No key stored".into() } } }
                            input {
                                r#type: "password",
                                class: PARAM_TEXT_INPUT_CLASS,
                                placeholder: "anthropic-key",
                                value: anthropic_input_signal(),
                                oninput: move |evt| anthropic_input_signal.set(evt.value()),
                            }
                        }
                    }
                    button {
                        class: "btn btn-ghost btn-xs w-fit",
                        onclick: move |_| show_api_key_values.set(!show_api_key_values()),
                        if show_api_key_values() { "Hide stored values" } else { "Show stored values" }
                    }
                }
            }

            if is_anthropic_backend {
                Panel { title: Some("Anthropic helper".into()), refresh: None,
                    if anthropic_llm_loading() {
                        div { class: "text-xs text-gray-400", "Loading remote config…" }
                    } else if let Some(err) = anthropic_llm_error() {
                        div { class: "text-xs text-red-400", "{err}" }
                    } else {
                        div { class: "text-xs text-gray-200 space-y-2",
                            span { class: "text-sm text-gray-300 font-semibold", "Recommended sampling" }
                            span { "Max tokens: {anthropic_llm.max_tokens}" }
                            span { "Temperature: {anthropic_llm.temperature}" }
                            span { "Top-p: {anthropic_llm.top_p}" }
                            span { "Repeat penalty: {anthropic_llm.repeat_penalty}" }
                            span { "Repeat last n: {anthropic_llm.repeat_last_n}" }
                        }
                    }
                }
            }
        }

        // Help modals - using HelpTopic for centralized content management
        if show_backend_info() {
            { info_modal(HelpTopic::Backend.title(), show_backend_info, HelpTopic::Backend.paragraphs()) }
        }
        if show_model_info() {
            { info_modal(HelpTopic::Model.title(), show_model_info, HelpTopic::Model.paragraphs()) }
        }
        if show_num_thread_info() {
            { info_modal(HelpTopic::NumThread.title(), show_num_thread_info, HelpTopic::NumThread.paragraphs()) }
        }
        if show_num_gpu_info() {
            { info_modal(HelpTopic::NumGpu.title(), show_num_gpu_info, HelpTopic::NumGpu.paragraphs()) }
        }
        if show_gpu_layers_info() {
            { info_modal(HelpTopic::GpuLayers.title(), show_gpu_layers_info, HelpTopic::GpuLayers.paragraphs()) }
        }
        if show_main_gpu_info() {
            { info_modal(HelpTopic::MainGpu.title(), show_main_gpu_info, HelpTopic::MainGpu.paragraphs()) }
        }
        if show_rope_base_info() {
            { info_modal(HelpTopic::RopeBase.title(), show_rope_base_info, HelpTopic::RopeBase.paragraphs()) }
        }
        if show_rope_scale_info() {
            { info_modal(HelpTopic::RopeScale.title(), show_rope_scale_info, HelpTopic::RopeScale.paragraphs()) }
        }
        if show_low_vram_info() {
            { info_modal(HelpTopic::LowVram.title(), show_low_vram_info, HelpTopic::LowVram.paragraphs()) }
        }
        if show_f16_kv_info() {
            { info_modal(HelpTopic::F16Kv.title(), show_f16_kv_info, HelpTopic::F16Kv.paragraphs()) }
        }
        if show_num_batch_info() {
            { info_modal(HelpTopic::NumBatch.title(), show_num_batch_info, HelpTopic::NumBatch.paragraphs()) }
        }
        if show_num_ctx_info() {
            { info_modal(HelpTopic::NumCtx.title(), show_num_ctx_info, HelpTopic::NumCtx.paragraphs()) }
        }
        if show_numa_info() {
            { info_modal(HelpTopic::Numa.title(), show_numa_info, HelpTopic::Numa.paragraphs()) }
        }
        if show_mmap_info() {
            { info_modal(HelpTopic::Mmap.title(), show_mmap_info, HelpTopic::Mmap.paragraphs()) }
        }
        if show_mlock_info() {
            { info_modal(HelpTopic::Mlock.title(), show_mlock_info, HelpTopic::Mlock.paragraphs()) }
        }
        if show_logits_all_info() {
            { info_modal(HelpTopic::LogitsAll.title(), show_logits_all_info, HelpTopic::LogitsAll.paragraphs()) }
        }
        if show_vocab_only_info() {
            { info_modal(HelpTopic::VocabOnly.title(), show_vocab_only_info, HelpTopic::VocabOnly.paragraphs()) }
        }
        if show_reload_info() {
            { info_modal(HelpTopic::Reload.title(), show_reload_info, HelpTopic::Reload.paragraphs()) }
        }
        if show_rope_tuning_info() {
            { info_modal(HelpTopic::RopeTuning.title(), show_rope_tuning_info, HelpTopic::RopeTuning.paragraphs()) }
        }
        // Sampling parameter help modals
        if show_temperature_info() {
            { info_modal(HelpTopic::Temperature.title(), show_temperature_info, HelpTopic::Temperature.paragraphs()) }
        }
        if show_top_k_info() {
            { info_modal(HelpTopic::TopK.title(), show_top_k_info, HelpTopic::TopK.paragraphs()) }
        }
        if show_top_p_info() {
            { info_modal(HelpTopic::TopP.title(), show_top_p_info, HelpTopic::TopP.paragraphs()) }
        }
        if show_min_p_info() {
            { info_modal(HelpTopic::MinP.title(), show_min_p_info, HelpTopic::MinP.paragraphs()) }
        }
        if show_typical_p_info() {
            { info_modal(HelpTopic::TypicalP.title(), show_typical_p_info, HelpTopic::TypicalP.paragraphs()) }
        }
        if show_tfs_z_info() {
            { info_modal(HelpTopic::TfsZ.title(), show_tfs_z_info, HelpTopic::TfsZ.paragraphs()) }
        }
        if show_seed_info() {
            { info_modal(HelpTopic::Seed.title(), show_seed_info, HelpTopic::Seed.paragraphs()) }
        }
        // Repetition control help modals
        if show_repeat_last_n_info() {
            { info_modal(HelpTopic::RepeatLastN.title(), show_repeat_last_n_info, HelpTopic::RepeatLastN.paragraphs()) }
        }
        if show_repeat_penalty_info() {
            { info_modal(HelpTopic::RepeatPenalty.title(), show_repeat_penalty_info, HelpTopic::RepeatPenalty.paragraphs()) }
        }
        if show_presence_penalty_info() {
            { info_modal(HelpTopic::PresencePenalty.title(), show_presence_penalty_info, HelpTopic::PresencePenalty.paragraphs()) }
        }
        if show_frequency_penalty_info() {
            { info_modal(HelpTopic::FrequencyPenalty.title(), show_frequency_penalty_info, HelpTopic::FrequencyPenalty.paragraphs()) }
        }
        if show_penalize_newline_info() {
            { info_modal(HelpTopic::PenalizeNewline.title(), show_penalize_newline_info, HelpTopic::PenalizeNewline.paragraphs()) }
        }
        // Mirostat help modals
        if show_mirostat_info() {
            { info_modal(HelpTopic::Mirostat.title(), show_mirostat_info, HelpTopic::Mirostat.paragraphs()) }
        }
        if show_mirostat_tau_info() {
            { info_modal(HelpTopic::MirostatTau.title(), show_mirostat_tau_info, HelpTopic::MirostatTau.paragraphs()) }
        }
        if show_mirostat_eta_info() {
            { info_modal(HelpTopic::MirostatEta.title(), show_mirostat_eta_info, HelpTopic::MirostatEta.paragraphs()) }
        }
        // Output control help modals
        if show_num_predict_info() {
            { info_modal(HelpTopic::NumPredict.title(), show_num_predict_info, HelpTopic::NumPredict.paragraphs()) }
        }
        if show_num_keep_info() {
            { info_modal(HelpTopic::NumKeep.title(), show_num_keep_info, HelpTopic::NumKeep.paragraphs()) }
        }
        if show_stop_sequences_info() {
            { info_modal(HelpTopic::StopSequences.title(), show_stop_sequences_info, HelpTopic::StopSequences.paragraphs()) }
        }
    }
}
