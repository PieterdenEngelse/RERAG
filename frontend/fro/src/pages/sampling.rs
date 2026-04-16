use crate::pages::hardware::constants::{INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS};
use crate::{
    api,
    app::{PageErrors, Route},
    components::config_nav::{ConfigNav, ConfigTab},
    components::monitor::*,
};
use dioxus::prelude::*;

const TEMP_MIN: f32 = 0.0;
const TEMP_MAX: f32 = 2.0;

const PARAM_BLOCK_CLASS: &str = "flex flex-col gap-1 text-xs text-gray-200";
const STOP_BLOCK_CLASS: &str = "flex flex-col gap-1 text-xs text-gray-200";
const PARAM_COLUMN_CLASS: &str = "space-y-5 md:w-[18rem]";
const PARAM_INPUT_ROW_CLASS: &str = "flex items-end gap-2";
const PARAM_LABEL_CLASS: &str = "text-gray-400 whitespace-nowrap";
// removed local constant
// const PARAM_ICON_BUTTON_CLASS removed (using shared constant)
const PARAM_NUMBER_INPUT_CLASS: &str =
    "input input-xs input-bordered bg-gray-700 text-gray-200 !w-24";
// Wider input for comma-separated text values (e.g., stop_sequences: "\n, </s>, [END]")
const PARAM_TEXT_INPUT_CLASS: &str = "input input-xs input-bordered bg-gray-700 text-gray-200 w-72";

#[component]
fn InfoIcon() -> Element {
    rsx! {
        svg {
            class: INFO_ICON_SVG_CLASS,
            view_box: "0 0 20 20",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            circle { cx: "10", cy: "10", r: "9" }
            line { x1: "10", y1: "8", x2: "10", y2: "14" }
            circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
        }
    }
}

fn parse_f32(value: &str) -> Option<f32> {
    value.parse::<f32>().ok()
}

fn parse_usize(value: &str) -> Option<usize> {
    value.parse::<usize>().ok()
}

fn parse_i64(value: &str) -> Option<i64> {
    value.parse::<i64>().ok()
}

fn clamp_temperature(val: f32) -> f32 {
    val.clamp(TEMP_MIN, TEMP_MAX)
}

fn sanitize_llm_config(mut cfg: api::LlmConfig) -> api::LlmConfig {
    cfg.temperature = clamp_temperature(cfg.temperature);
    cfg.min_p = cfg.min_p.clamp(0.0, 1.0);
    cfg.typical_p = cfg.typical_p.clamp(0.0, 1.0);
    cfg.tfs_z = cfg.tfs_z.clamp(0.0, 1.0);
    cfg.mirostat = cfg.mirostat.clamp(0, 2);
    cfg.mirostat_eta = cfg.mirostat_eta.clamp(0.0, 1.0);
    cfg.mirostat_tau = cfg.mirostat_tau.clamp(0.0, 10.0);
    if cfg.repeat_last_n == 0 {
        cfg.repeat_last_n = 64;
    }
    cfg
}

fn info_modal(title: &str, toggle: Signal<bool>, paragraphs: Vec<&str>) -> Element {
    let mut toggle = toggle;
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| toggle.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 max-w-lg max-h-[80vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100", "{title}" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| toggle.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-3",
                    for paragraph in paragraphs {
                        p { "{paragraph}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn ConfigSampling() -> Element {
    let mut llm_config = use_signal(api::LlmConfig::default);
    let llm_loading = use_signal(|| true);
    let mut llm_error = use_signal(|| Option::<String>::None);
    let llm_status = use_signal(|| Option::<String>::None);
    let mut llm_saving = use_signal(|| false);
    let mut show_temp_info = use_signal(|| false);
    let mut show_repeat_penalty_info = use_signal(|| false);
    let mut show_topk_info = use_signal(|| false);
    let mut show_max_tokens_info = use_signal(|| false);
    let mut show_topp_info = use_signal(|| false);
    let mut show_seed_info = use_signal(|| false);
    let mut show_frequency_penalty_info = use_signal(|| false);
    let mut show_presence_penalty_info = use_signal(|| false);
    let mut show_stop_sequences_info = use_signal(|| false);
    let mut show_min_p_info = use_signal(|| false);
    let mut show_typical_p_info = use_signal(|| false);
    let mut show_tfs_z_info = use_signal(|| false);
    let mut show_mirostat_info = use_signal(|| false);
    let mut show_mirostat_eta_info = use_signal(|| false);
    let mut show_mirostat_tau_info = use_signal(|| false);
    let mut show_repeat_last_n_info = use_signal(|| false);
    let backend_type = use_signal(|| Option::<api::BackendType>::None);
    let backend_error = use_signal(|| Option::<String>::None);

    {
        let mut llm_config = llm_config.clone();
        let mut llm_loading = llm_loading.clone();
        let mut llm_error = llm_error.clone();
        let mut llm_status = llm_status.clone();
        let mut page_errors = use_context::<Signal<PageErrors>>();
        use_future(move || async move {
            llm_loading.set(true);
            llm_error.set(None);
            page_errors.with_mut(|e| e.clear_error("sampling"));
            match api::fetch_llm_config().await {
                Ok(resp) => {
                    llm_config.set(sanitize_llm_config(resp.config));
                    llm_status.set(Some(resp.message));
                    page_errors.with_mut(|e| e.clear_error("sampling"));
                }
                Err(err) => {
                    let e = format!("Failed to load LLM config: {}", err);
                    llm_error.set(Some(e.clone()));
                    page_errors.with_mut(|errs| errs.set_error("sampling", &e));
                    let _ = api::log_frontend_error("sampling", &e).await;
                }
            }
            llm_loading.set(false);
        });
    }

    {
        let mut backend_type = backend_type.clone();
        let mut backend_error = backend_error.clone();
        use_future(move || async move {
            match api::fetch_hardware_config().await {
                Ok(resp) => {
                    backend_type.set(Some(resp.config.get_backend_type()));
                    backend_error.set(None);
                }
                Err(err) => {
                    backend_error.set(Some(format!("Failed to load backend info: {}", err)));
                }
            }
        });
    }

    let on_llm_save = move |_| {
        llm_saving.set(true);
        llm_error.set(None);
        let payload = sanitize_llm_config(llm_config());
        let mut llm_config = llm_config.clone();
        let mut llm_status = llm_status.clone();
        let mut llm_error = llm_error.clone();
        let mut llm_saving = llm_saving.clone();
        spawn(async move {
            match api::commit_llm_config(&payload).await {
                Ok(resp) => {
                    llm_status.set(Some(resp.message));
                    llm_config.set(sanitize_llm_config(resp.config));
                }
                Err(err) => {
                    llm_error.set(Some(format!("Failed to save LLM config: {}", err)));
                }
            }
            llm_saving.set(false);
        });
    };

    let backend_type_value = backend_type();
    let backend_error_value = backend_error();
    let supports_extended_sampling = backend_type_value
        .map(|bt| matches!(bt, api::BackendType::Ollama))
        .unwrap_or(true);

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::Config {})),
                    BreadcrumbItem::new("Sampling", Some(Route::ConfigSampling {})),
                ],
            }

            ConfigNav { active: ConfigTab::Sampling }

            Panel { title: Some("LLM Parameters".into()), refresh: None,
                div { class: "flex items-center justify-between",
                    span { class: "text-base text-gray-200 font-semibold", "Parameters" }
                    button {
                        class: "btn btn-primary btn-xs",
                        onclick: on_llm_save.clone(),
                        disabled: llm_saving() || llm_loading(),
                        if llm_saving() { "Saving…" } else { "Save" }
                    }
                }
                if let Some(err) = llm_error() {
                    div { class: "text-xs text-red-400", "{err}" }
                } else if let Some(status) = llm_status() {
                    div { class: "text-xs text-gray-400", "{status}" }
                }
                if let Some(bt) = backend_type_value {
                    div { class: "text-xs text-gray-400", "Active backend: {bt.label()}" }
                    if !supports_extended_sampling {
                        div { class: "text-[0.65rem] text-amber-300", "Advanced sampling controls hidden because {bt.label()} does not support these Ollama-specific parameters." }
                    }
                } else if let Some(err) = backend_error_value {
                    div { class: "text-xs text-red-400", "{err}" }
                }
                div {
                    class: "flex flex-col gap-6 md:flex-row md:flex-wrap md:gap-x-[1cm] md:gap-y-6",
                    div { class: PARAM_COLUMN_CLASS,
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "temperature" }
                            div { class: PARAM_INPUT_ROW_CLASS,
                                input {
                                    r#type: "number",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: llm_config().temperature.to_string(),
                                    step: "0.1",
                                    min: "0",
                                    max: "2",
                                    disabled: llm_loading(),
                                    oninput: move |evt| {
                                        if let Some(val) = parse_f32(&evt.value()) {
                                            llm_config.with_mut(|cfg| cfg.temperature = clamp_temperature(val));
                                        }
                                    }
                                }
                                div {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    onclick: move |_| show_temp_info.set(true),
                                    InfoIcon {}
                                }
                            }
                        }
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "repeat_penalty" }
                            div { class: PARAM_INPUT_ROW_CLASS,
                                input {
                                    r#type: "number",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: llm_config().repeat_penalty.to_string(),
                                    step: "0.1",
                                    min: "0",
                                    disabled: llm_loading(),
                                    oninput: move |evt| {
                                        if let Some(val) = parse_f32(&evt.value()) {
                                            llm_config.with_mut(|cfg| cfg.repeat_penalty = val);
                                        }
                                    }
                                }
                                div {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    onclick: move |_| show_repeat_penalty_info.set(true),
                                    InfoIcon {}
                                }
                            }
                        }
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "max_tokens" }
                            div { class: PARAM_INPUT_ROW_CLASS,
                                input {
                                    r#type: "number",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: llm_config().max_tokens.to_string(),
                                    step: "128",
                                    min: "1",
                                    disabled: llm_loading(),
                                    oninput: move |evt| {
                                        if let Some(val) = parse_usize(&evt.value()) {
                                            llm_config.with_mut(|cfg| cfg.max_tokens = val);
                                        }
                                    }
                                }
                                div {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    onclick: move |_| show_max_tokens_info.set(true),
                                    InfoIcon {}
                                }
                            }
                        }
                    }
                    div { class: PARAM_COLUMN_CLASS, style: "margin-left:-10mm;",
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "top_k" }
                            div { class: PARAM_INPUT_ROW_CLASS,
                                input {
                                    r#type: "number",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: llm_config().top_k.to_string(),
                                    step: "1",
                                    min: "1",
                                    disabled: llm_loading(),
                                    oninput: move |evt| {
                                        if let Some(val) = parse_usize(&evt.value()) {
                                            llm_config.with_mut(|cfg| cfg.top_k = val);
                                        }
                                    }
                                }
                                div {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    onclick: move |_| show_topk_info.set(true),
                                    InfoIcon {}
                                }
                            }
                        }
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "frequency_penalty" }
                            div { class: PARAM_INPUT_ROW_CLASS,
                                input {
                                    r#type: "number",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: llm_config().frequency_penalty.to_string(),
                                    step: "0.1",
                                    min: "0",
                                    max: "2",
                                    disabled: llm_loading(),
                                    oninput: move |evt| {
                                        if let Some(val) = parse_f32(&evt.value()) {
                                            llm_config.with_mut(|cfg| cfg.frequency_penalty = val.clamp(0.0, 2.0));
                                        }
                                    }
                                }
                                div {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    onclick: move |_| show_frequency_penalty_info.set(true),
                                    InfoIcon {}
                                }
                            }
                        }
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "seed" }
                            div { class: PARAM_INPUT_ROW_CLASS,
                                input {
                                    r#type: "number",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: llm_config().seed.map(|seed| seed.to_string()).unwrap_or_default(),
                                    placeholder: "None",
                                    disabled: llm_loading(),
                                    oninput: move |evt| {
                                        let value = evt.value();
                                        if value.trim().is_empty() {
                                            llm_config.with_mut(|cfg| cfg.seed = None);
                                        } else if let Some(val) = parse_i64(&value) {
                                            llm_config.with_mut(|cfg| cfg.seed = Some(val));
                                        }
                                    }
                                }
                                div {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    onclick: move |_| show_seed_info.set(true),
                                    InfoIcon {}
                                }
                            }
                        }
                    }
                    div { class: PARAM_COLUMN_CLASS, style: "margin-left:-10mm;",
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "top_p" }
                            div { class: PARAM_INPUT_ROW_CLASS,
                                input {
                                    r#type: "number",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: llm_config().top_p.to_string(),
                                    step: "0.05",
                                    min: "0",
                                    max: "1",
                                    disabled: llm_loading(),
                                    oninput: move |evt| {
                                        if let Some(val) = parse_f32(&evt.value()) {
                                            llm_config.with_mut(|cfg| cfg.top_p = val);
                                        }
                                    }
                                }
                                div {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    onclick: move |_| show_topp_info.set(true),
                                    InfoIcon {}
                                }
                            }
                        }
                        div { class: PARAM_BLOCK_CLASS,
                            label { class: PARAM_LABEL_CLASS, "presence_penalty" }
                            div { class: PARAM_INPUT_ROW_CLASS,
                                input {
                                    r#type: "number",
                                    class: PARAM_NUMBER_INPUT_CLASS,
                                    value: llm_config().presence_penalty.to_string(),
                                    step: "0.1",
                                    min: "0",
                                    max: "2",
                                    disabled: llm_loading(),
                                    oninput: move |evt| {
                                        if let Some(val) = parse_f32(&evt.value()) {
                                            llm_config.with_mut(|cfg| cfg.presence_penalty = val.clamp(0.0, 2.0));
                                        }
                                    }
                                }
                                div {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    onclick: move |_| show_presence_penalty_info.set(true),
                                    InfoIcon {}
                                }
                            }
                        }
                    }
                    if supports_extended_sampling {
                        div { class: PARAM_COLUMN_CLASS, style: "margin-left:-10mm;",
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "min_p" }
                                div { class: PARAM_INPUT_ROW_CLASS,
                                    input {
                                        r#type: "number",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: llm_config().min_p.to_string(),
                                        step: "0.01",
                                        min: "0",
                                        max: "1",
                                        disabled: llm_loading(),
                                        oninput: move |evt| {
                                            if let Some(val) = parse_f32(&evt.value()) {
                                                llm_config.with_mut(|cfg| cfg.min_p = val);
                                            }
                                        }
                                    }
                                    div {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        onclick: move |_| show_min_p_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "typical_p" }
                                div { class: PARAM_INPUT_ROW_CLASS,
                                    input {
                                        r#type: "number",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: llm_config().typical_p.to_string(),
                                        step: "0.01",
                                        min: "0",
                                        max: "1",
                                        disabled: llm_loading(),
                                        oninput: move |evt| {
                                            if let Some(val) = parse_f32(&evt.value()) {
                                                llm_config.with_mut(|cfg| cfg.typical_p = val);
                                            }
                                        }
                                    }
                                    div {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        onclick: move |_| show_typical_p_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "tfs_z" }
                                div { class: PARAM_INPUT_ROW_CLASS,
                                    input {
                                        r#type: "number",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: llm_config().tfs_z.to_string(),
                                        step: "0.01",
                                        min: "0",
                                        max: "1",
                                        disabled: llm_loading(),
                                        oninput: move |evt| {
                                            if let Some(val) = parse_f32(&evt.value()) {
                                                llm_config.with_mut(|cfg| cfg.tfs_z = val);
                                            }
                                        }
                                    }
                                    div {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        onclick: move |_| show_tfs_z_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "repeat_last_n" }
                                div { class: PARAM_INPUT_ROW_CLASS,
                                    input {
                                        r#type: "number",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: llm_config().repeat_last_n.to_string(),
                                        step: "1",
                                        min: "1",
                                        disabled: llm_loading(),
                                        oninput: move |evt| {
                                            if let Some(val) = parse_usize(&evt.value()) {
                                                llm_config.with_mut(|cfg| cfg.repeat_last_n = val.max(1));
                                            }
                                        }
                                    }
                                    div {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        onclick: move |_| show_repeat_last_n_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                    }
                    if supports_extended_sampling {
                        div { class: PARAM_COLUMN_CLASS, style: "margin-left:-10mm;",
                            div { class: STOP_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "stop_sequences" }
                                div { class: PARAM_INPUT_ROW_CLASS,
                                    input {
                                        r#type: "text",
                                        class: PARAM_TEXT_INPUT_CLASS,
                                        value: llm_config().stop_sequences.join(", "),
                                        placeholder: "e.g. END, ###, \n\n",
                                        disabled: llm_loading(),
                                        oninput: move |evt| {
                                            let value = evt.value();
                                            let sequences: Vec<String> = value
                                                .split(',')
                                                .map(|s| s.trim().to_string())
                                                .filter(|s| !s.is_empty())
                                                .collect();
                                            llm_config.with_mut(|cfg| cfg.stop_sequences = sequences);
                                        }
                                    }
                                    div {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        onclick: move |_| show_stop_sequences_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "mirostat" }
                                div { class: PARAM_INPUT_ROW_CLASS,
                                    select {
                                        class: "select select-xs select-bordered bg-gray-700 text-gray-200",
                                        value: llm_config().mirostat.to_string(),
                                        disabled: llm_loading(),
                                        onchange: move |evt| {
                                            if let Ok(val) = evt.value().parse::<i32>() {
                                                llm_config.with_mut(|cfg| cfg.mirostat = val);
                                            }
                                        },
                                        option { value: "0", "0 • uit" }
                                        option { value: "1", "1 • adaptief (v1)" }
                                        option { value: "2", "2 • adaptief (v2)" }
                                    }
                                    div {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        onclick: move |_| show_mirostat_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "mirostat_eta" }
                                div { class: PARAM_INPUT_ROW_CLASS,
                                    input {
                                        r#type: "number",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: llm_config().mirostat_eta.to_string(),
                                        step: "0.01",
                                        min: "0",
                                        max: "1",
                                        disabled: llm_loading(),
                                        oninput: move |evt| {
                                            if let Some(val) = parse_f32(&evt.value()) {
                                                llm_config.with_mut(|cfg| cfg.mirostat_eta = val);
                                            }
                                        }
                                    }
                                    div {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        onclick: move |_| show_mirostat_eta_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                            }
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "mirostat_tau" }
                                div { class: PARAM_INPUT_ROW_CLASS,
                                    input {
                                        r#type: "number",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: llm_config().mirostat_tau.to_string(),
                                        step: "0.1",
                                        min: "0",
                                        max: "10",
                                        disabled: llm_loading(),
                                        oninput: move |evt| {
                                            if let Some(val) = parse_f32(&evt.value()) {
                                                llm_config.with_mut(|cfg| cfg.mirostat_tau = val);
                                            }
                                        }
                                    }
                                    div {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        onclick: move |_| show_mirostat_tau_info.set(true),
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if show_temp_info() {
                { info_modal("Temperature", show_temp_info.clone(), vec![
                    "Temperature in an LLM typically ranges from 0.0 to 2.0. Even though it is stored as a float32, which technically allows around seven significant digits, in practice one decimal place is usually enough. When finer control is needed, two decimals such as 0.75 or 0.85 are sometimes used, but going beyond that offers almost no practical benefit.",
                    "Temperature influences how the model selects the next token from the probability distribution it produces. A token is the smallest textual unit a language model processes. Depending on the tokenizer, a token may be a whole word, part of a word, punctuation, or even whitespace.",
                    "Each token is mapped to a dense embedding vector containing a real-valued number for every dimension. These values encode semantic information in a continuous vector space, and the distance between vectors reflects how similar their meanings are across all dimensions.",
                    "After a token is embedded, it passes through all transformer layers. The final hidden state is then transformed by a weight matrix plus a bias term. This linear transformation produces a vector of logits, which are raw, unnormalized scores representing how likely each token is before normalization. The softmax function converts these logits into probabilities, each strictly between 0 and 1, and the entire set always sums to 1.",
                    "Once the probabilities are available, the model must choose the next token. This is done through a sampling strategy. The most common strategies are temperature sampling, top-k sampling, top-p (nucleus) sampling, and greedy decoding. Greedy decoding simply selects the token with the highest probability and can be thought of as the conceptual equivalent of using a temperature close to zero. A higher temperature makes the model more exploratory and more willing to choose lower-probability tokens, which increases diversity in the generated text.",
                    "Strategy order:",
                    "1. Model outputs logits",
                    "2. Apply temperature (reshapes distribution)",
                    "3. Apply top-k (hard cutoff)",
                    "4. Apply top-p (adaptive cutoff)",
                    "5. Renormalize",
                    "6. Sample next token",
                ]) }
            }
            if show_repeat_penalty_info() {
                { info_modal("Repeat penalty", show_repeat_penalty_info.clone(), vec![
                    "The repeat_penalty parameter discourages the model from repeating the same words or phrases too often by lowering the probability of tokens that have already appeared.",
                    "The more often a token has appeared, the stronger the penalty becomes. It does not forbid repetition but it nudges the model to pick new words unless repeating something is genuinely the best choice.",
                ]) }
            }
            if show_topk_info() {
                { info_modal("Top K", show_topk_info.clone(), vec![
                    "Top‑k works by letting the model choose the next token only from the k most likely options. The model first produces a probability score for every possible token, then sorts them from most to least likely, keeps only the top k of them, sets all others to zero, renormalizes the remaining probabilities, and finally samples one token from that reduced set.",
                    "It's a hard cutoff: if a token isn't in the top k, it simply cannot be chosen. For more background, see the temperature explanation on this page.",
                ]) }
            }
            if show_max_tokens_info() {
                { info_modal("Max tokens", show_max_tokens_info.clone(), vec![
                    "Max_tokens simply limits how many tokens the model may emit before it must stop. It does not affect creativity or randomness; it is only a length cap.",
                    "If the model reaches that limit it stops, even if the answer is not finished. A higher limit lets the model continue until it naturally decides to stop.",
                ]) }
            }
            if show_topp_info() {
                { info_modal("Top P", show_topp_info.clone(), vec![
                    "Top‑p is a way of sampling that constantly reshapes itself around whatever the model believes is most likely at that moment. After the model produces a probability for every possible next token, those tokens are sorted from most to least likely. Then the algorithm begins adding their probabilities together, one by one, until the running total reaches the threshold p that you chose. The moment the cumulative sum crosses that value, the process stops, and the tokens included so far become the entire pool the model is allowed to choose from. Everything outside that pool is discarded by setting its probability to zero, and the remaining probabilities are renormalized so they add up to one again.",
                    "What makes top‑p interesting is that it adapts to the shape of the distribution. When the model is \"confident\" — meaning one or a few tokens have very high probability and the rest are far behind — the cumulative sum reaches p quickly, so the allowed pool is small. When the model is \"uncertain\" — meaning many tokens have similar probabilities and no single option dominates — the cumulative sum rises slowly, so the pool grows larger. The threshold p doesn't come from the model at all — it's a number you choose, and the algorithm just keeps adding probabilities until it reaches that target. For more info see temperature info on this page.",
                ]) }
            }
            if show_seed_info() {
                { info_modal("Seed", show_seed_info.clone(), vec![
                    "Use case: If you don't like the output, you can try a different seed to explore a different variation of the same prompt. Each seed gives you a different sampling path, so you get a different version of the answer without changing temperature or other settings.",
                    "A seed is just a number that selects one specific random sequence. Changing the seed changes the sampling path. Keeping the seed the same reproduces the exact same output every time, as long as the prompt and sampling settings stay the same.",
                    "You don't need to try many seeds. Even a few (3–10) will give you noticeably different variations. The full seed range (0 to 4,294,967,295) exists for technical reasons, not because you should explore all of it.",
                    "The size of the seed number doesn't matter. A low seed is not safer, and a high seed is not more creative. All seeds are equally random. The seed only defines which random sequence is used, not how random the model is. The actual randomness level is controlled by temperature, top‑p, and similar settings.",
                    "If you find an output you like, keep the same seed to reproduce it exactly. If you don't like the output, change the seed to get a different variation. That's the entire strategy — simple, predictable, and effective.",
                    "When the seed is none, the model simply does not lock the random number generator to a fixed starting point. That means the model uses a fresh, unpredictable random sequence every time you generate, so each run can produce a different output even if the prompt and settings are identical.",
                ]) }
            }
            if show_frequency_penalty_info() {
                { info_modal("Frequency penalty", show_frequency_penalty_info.clone(), vec![
                    "Frequency penalty reduces the likelihood of tokens proportionally to how often they have already appeared in the generated text. The more frequently a token appears, the stronger the penalty becomes.",
                    "This is different from repeat_penalty, which applies a fixed penalty regardless of how many times a token appeared. Frequency penalty scales with occurrence count, making it more aggressive against heavily repeated words.",
                    "A value of 0 means no penalty. Higher values (up to 2.0) increasingly discourage repetition. Use this when you want to reduce word repetition while still allowing occasional reuse of common words.",
                ]) }
            }
            if show_presence_penalty_info() {
                { info_modal("Presence penalty", show_presence_penalty_info.clone(), vec![
                    "Presence penalty applies a flat penalty to any token that has already appeared in the text, regardless of how many times it appeared. A token that appeared once gets the same penalty as one that appeared ten times.",
                    "This encourages the model to explore new topics and vocabulary rather than staying focused on the same concepts. It's useful for creative writing or brainstorming where you want diverse ideas.",
                    "A value of 0 means no penalty. Higher values (up to 2.0) push the model to use new words. Unlike frequency_penalty, this doesn't scale with repetition count — it just asks: has this token appeared before?",
                ]) }
            }
            if show_stop_sequences_info() {
                { info_modal("Stop sequences", show_stop_sequences_info.clone(), vec![
                    "Stop sequences are strings that tell the model to stop generating as soon as one of them is encountered. When the model produces any of these sequences, generation immediately halts.",
                    "This is useful for controlling output structure. For example, if you're generating a list, you might use a stop sequence like \"11.\" to limit the list to 10 items. For dialogue, you might stop at a specific character's name.",
                    "Enter multiple stop sequences separated by commas. Common examples include: </s>, <|endoftext|>, <|im_end|>, <|end|>, ###, or custom markers like [DONE].",
                    "Note: The appropriate stop sequences depend on the model used. Different models use different end-of-sequence tokens (e.g., Llama uses </s>, GPT models use <|endoftext|>, Phi-3 uses <|end|>).",
                    "⚠️ Warning: Be careful with role markers like <|user|> or <|assistant|> as stop sequences. They can cause premature stopping if the model legitimately outputs them (e.g., in dialogues). Stick to true end tokens like <|end|> or </s> for safest results. Ollama usually configures the right defaults in the Modelfile.",
                ]) }
            }
            if show_min_p_info() {
                { info_modal("min_p", show_min_p_info.clone(), vec![
                    "min_p is an alternative nucleus cutoff: tokens below this minimum probability are ignored before sampling continues.",
                    "Educational link: comparing it with top-p teaches how probability mass can be trimmed from either the top or the bottom of the distribution.",
                ]) }
            }
            if show_typical_p_info() {
                { info_modal("typical_p", show_typical_p_info.clone(), vec![
                    "Typical sampling keeps tokens whose cumulative surprise (entropy) is below this threshold.",
                    "Educational value: illustrates entropy-driven sampling and why some models prefer \"typical\" tokens over purely likely ones.",
                ]) }
            }
            if show_tfs_z_info() {
                { info_modal("tfs_z", show_tfs_z_info.clone(), vec![
                    "Tail Free Sampling removes the long tail of the distribution until the remaining mass reaches z.",
                    "Educational link: demonstrates how trimming heavy tails affects diversity versus coherence.",
                ]) }
            }
            if show_mirostat_info() {
                { info_modal("Mirostat", show_mirostat_info.clone(), vec![
                    "Mirostat is an adaptive sampling algorithm that tries to keep the surprise (entropy) near a target value.",
                    "Set 0 to disable, 1 for the v1 algorithm, or 2 for v2.",
                    "Educational value: shows how adaptive controllers keep sampling stable over long generations.",
                ]) }
            }
            if show_mirostat_eta_info() {
                { info_modal("mirostat_eta", show_mirostat_eta_info.clone(), vec![
                    "Eta is the learning rate for Mirostat: higher values react faster but can overshoot.",
                    "Educational value: connects eta to standard optimization / control theory concepts.",
                ]) }
            }
            if show_mirostat_tau_info() {
                { info_modal("mirostat_tau", show_mirostat_tau_info.clone(), vec![
                    "Tau is the target entropy that Mirostat tries to maintain.",
                    "Educational value: demonstrates how steering entropy changes model behaviour (creative vs focused).",
                ]) }
            }
            if show_repeat_last_n_info() {
                { info_modal("repeat_last_n", show_repeat_last_n_info.clone(), vec![
                    "repeat_last_n decides how many of the last tokens are considered when applying repetition penalties.",
                    "Educational value: frames the penalty as a moving \"memory span\" that controls how far back the model looks when discouraging repeats.",
                ]) }
            }
        }
    }
}
